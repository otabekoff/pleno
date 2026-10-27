//! `std:net`: TCP sockets on the event loop.
//!
//! `Net.listen(host, port)` returns a `Listener` and `Net.connect(host, port)`
//! a promise for a `Socket`. Every operation that waits on the network
//! (`connect`, `accept`, `read`, `readLine`, `write`) returns a promise, so a
//! program awaits it and other tasks keep running in the meantime: the sockets
//! are non-blocking and registered with the VM's event loop only while an
//! operation waits on them. A socket is registered with nothing while idle,
//! so an open socket nobody waits on does not keep the program running.
//!
//! The socket lives in a `NativeResource`. The `Socket` and `Listener` values
//! are maps whose methods are bound to that resource, so the resource closes
//! when `close()` is called, when nothing refers to it any more (the garbage
//! collector clears it), or when its VM is dropped.

use indexmap::IndexMap;
use mio::Interest;
use pleno_value::gc::GcRc;
use pleno_value::{
    BoundNativeMethodState, MapState, NativeFunction, NativeResource, RuntimeError, VMProvider,
    Value,
};
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::rc::Rc;

/// What `read()` returns at most when no size is given.
const DEFAULT_READ_BYTES: usize = 64 * 1024;
/// The longest line `readLine()` accepts, so a peer that never sends a
/// newline cannot exhaust memory.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

const SOCKET_RESOURCE: &str = "net_socket";
const LISTENER_RESOURCE: &str = "net_listener";

// --- State -----------------------------------------------------------------

enum ReadKind {
    Bytes(usize),
    Line,
}

struct PendingRead {
    op: u64,
    kind: ReadKind,
    promise: u64,
    timer: Option<u64>,
}

struct PendingWrite {
    op: u64,
    data: Vec<u8>,
    written: usize,
    promise: u64,
    timer: Option<u64>,
}

/// A `connect` in progress: the addresses still to try, in order.
struct Connecting {
    op: u64,
    target: String,
    remaining: VecDeque<SocketAddr>,
    last_error: Option<std::io::Error>,
    promise: u64,
    timer: Option<u64>,
    timeout_ms: Option<u64>,
}

/// Where the socket is registered with the event loop, if anywhere.
struct Registration {
    token: mio::Token,
    interest: Interest,
}

struct SocketState {
    stream: Option<mio::net::TcpStream>,
    registration: Option<Registration>,
    connecting: Option<Connecting>,
    local: String,
    remote: String,
    buffer: Vec<u8>,
    eof: bool,
    reads: VecDeque<PendingRead>,
    writes: VecDeque<PendingWrite>,
    timeout_ms: Option<u64>,
    next_op: u64,
}

struct PendingAccept {
    op: u64,
    promise: u64,
    timer: Option<u64>,
}

struct ListenerState {
    listener: Option<mio::net::TcpListener>,
    registration: Option<Registration>,
    accepts: VecDeque<PendingAccept>,
    address: String,
    timeout_ms: Option<u64>,
    next_op: u64,
}

/// A promise to settle once the state is no longer borrowed.
enum Settle {
    Resolve(u64, Value),
    /// A promise and the message of the `Error` to reject it with.
    Reject(u64, String),
    /// An accepted connection: resolve with a new socket for this stream.
    Accepted(u64, mio::net::TcpStream),
}

// --- Helpers ---------------------------------------------------------------

fn string(text: impl Into<String>) -> Value {
    Value::String(Rc::new(text.into()))
}

fn check_network(vm: &dyn VMProvider, what: &str) -> Result<(), RuntimeError> {
    if vm.is_network_allowed() {
        return Ok(());
    }
    Err(RuntimeError::new(format!(
        "SecurityError: network access denied: {what}"
    )))
}

fn resource_arg<'a>(
    args: &'a [Value],
    name: &str,
    method: &str,
) -> Result<&'a GcRc<NativeResource>, RuntimeError> {
    match args.first() {
        Some(Value::Resource(resource)) if resource.borrow().name == name => Ok(resource),
        _ => Err(RuntimeError::new(format!(
            "{method}() must be called on a {}",
            if name == SOCKET_RESOURCE {
                "Socket"
            } else {
                "Listener"
            }
        ))),
    }
}

/// Runs `f` on the socket state behind `resource`; `None` once the resource
/// has been released (its VM is going away).
fn with_socket<R>(
    resource: &GcRc<NativeResource>,
    f: impl FnOnce(&mut SocketState) -> R,
) -> Option<R> {
    let resource = resource.borrow();
    let mut inner = resource.inner.borrow_mut();
    let state = inner.as_mut()?.downcast_mut::<SocketState>()?;
    Some(f(state))
}

fn with_listener<R>(
    resource: &GcRc<NativeResource>,
    f: impl FnOnce(&mut ListenerState) -> R,
) -> Option<R> {
    let resource = resource.borrow();
    let mut inner = resource.inner.borrow_mut();
    let state = inner.as_mut()?.downcast_mut::<ListenerState>()?;
    Some(f(state))
}

fn bind_method(
    vm: &mut dyn VMProvider,
    receiver: &Value,
    function: NativeFunction,
) -> Result<Value, RuntimeError> {
    Ok(Value::BoundNativeMethod(vm.get_gc().allocate(
        BoundNativeMethodState {
            receiver: receiver.clone(),
            function,
        },
    )?))
}

fn settle(vm: &mut dyn VMProvider, settlements: Vec<Settle>) -> Result<(), RuntimeError> {
    for settlement in settlements {
        match settlement {
            Settle::Resolve(promise, value) => vm.resolve_promise(promise, value),
            Settle::Reject(promise, message) => {
                let reason = vm.error_value(&message);
                vm.reject_promise(promise, reason);
            }
            Settle::Accepted(promise, stream) => match new_socket(vm, stream) {
                Ok(socket) => vm.resolve_promise(promise, socket),
                Err(error) => {
                    let reason = vm.error_value(&error.message);
                    vm.reject_promise(promise, reason);
                }
            },
        }
    }
    Ok(())
}

/// The timeout in milliseconds that `setTimeout(ms)` was given.
fn timeout_arg(value: Option<&Value>) -> Result<Option<u64>, RuntimeError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(ms)) if *ms > 0 => Ok(Some(*ms as u64)),
        Some(Value::Float(ms)) if *ms > 0.0 && ms.is_finite() => Ok(Some(ms.ceil() as u64)),
        Some(other) => Err(RuntimeError::new(format!(
            "a timeout is a number of milliseconds greater than 0, or null for none; got {}",
            describe(other)
        ))),
    }
}

fn describe(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{s}\""),
        other => format!("{} ({})", other, other.type_name()),
    }
}

fn host_port(args: &[Value], function: &str) -> Result<(String, u16), RuntimeError> {
    let usage = format!(
        "{function}(host, port) expects a host String and a port Int, as in {function}(\"127.0.0.1\", 8080)"
    );
    let host = match args.first() {
        Some(Value::String(host)) => host.to_string(),
        _ => return Err(RuntimeError::new(usage)),
    };
    let port = match args.get(1) {
        Some(Value::Int(port)) if (0..=65535).contains(port) => *port as u16,
        Some(Value::Int(port)) => {
            return Err(RuntimeError::new(format!(
                "{function}(): port {port} is out of range (0 to 65535)"
            )));
        }
        _ => return Err(RuntimeError::new(usage)),
    };
    Ok((host, port))
}

/// `host:port`, with brackets around an IPv6 host.
fn display_target(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// The addresses of `host`, IPv4 first: `localhost` then means 127.0.0.1 for
/// both `listen` and `connect`, so they meet, and a client does not first
/// wait for `::1` to refuse (which takes about two seconds on Windows).
fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, RuntimeError> {
    let mut addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| RuntimeError::new(format!("cannot resolve host \"{host}\": {e}")))?
        .collect();
    addrs.sort_by_key(SocketAddr::is_ipv6);
    if addrs.is_empty() {
        return Err(RuntimeError::new(format!(
            "cannot resolve host \"{host}\": no addresses found"
        )));
    }
    Ok(addrs)
}

/// A clear message for a failed connection attempt to `target`.
fn connect_error(target: &str, error: &std::io::Error) -> String {
    match error.kind() {
        ErrorKind::ConnectionRefused => format!("connection refused: {target}"),
        ErrorKind::TimedOut => format!("TimeoutError: connection to {target} timed out"),
        _ => format!("cannot connect to {target}: {error}"),
    }
}

fn io_error(action: &str, peer: &str, error: &std::io::Error) -> String {
    match error.kind() {
        ErrorKind::ConnectionReset => format!("connection reset by peer: {peer}"),
        ErrorKind::ConnectionAborted => format!("connection aborted: {peer}"),
        ErrorKind::BrokenPipe => format!("connection closed by peer: {peer}"),
        _ => format!("{action} failed on {peer}: {error}"),
    }
}

// --- Event loop registration ------------------------------------------------

/// Registers `source` for `wanted` (or deregisters it for `None`), with
/// `callback` to run when it is ready. A changed interest re-registers it.
fn sync_registration(
    vm: &mut dyn VMProvider,
    source: &mut dyn mio::event::Source,
    registration: &mut Option<Registration>,
    wanted: Option<Interest>,
    callback: impl FnOnce(&mut dyn VMProvider) -> Result<Value, RuntimeError>,
) -> Result<(), RuntimeError> {
    if let Some(current) = registration.as_ref() {
        if Some(current.interest) == wanted {
            return Ok(());
        }
        let token = current.token;
        *registration = None;
        vm.unregister_io(source, token)?;
    }
    if let Some(interest) = wanted {
        let callback = callback(vm)?;
        let token = vm.register_io(source, interest, callback, true)?;
        *registration = Some(Registration { token, interest });
    }
    Ok(())
}

/// Deregisters `source` if it is registered; errors are ignored, as this runs
/// while closing.
fn drop_registration(
    vm: &mut dyn VMProvider,
    source: &mut dyn mio::event::Source,
    registration: &mut Option<Registration>,
) {
    if let Some(current) = registration.take() {
        let _ = vm.unregister_io(source, current.token);
    }
}

fn socket_interest(state: &SocketState) -> Option<Interest> {
    let read = !state.reads.is_empty();
    let write = !state.writes.is_empty() || state.connecting.is_some();
    match (read, write) {
        (true, true) => Some(Interest::READABLE | Interest::WRITABLE),
        (true, false) => Some(Interest::READABLE),
        (false, true) => Some(Interest::WRITABLE),
        (false, false) => None,
    }
}

fn update_socket_registration(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
) -> Result<(), RuntimeError> {
    let receiver = Value::Resource(resource.clone());
    let result = with_socket(resource, |state| {
        let wanted = socket_interest(state);
        let Some(stream) = state.stream.as_mut() else {
            return Ok(());
        };
        sync_registration(vm, stream, &mut state.registration, wanted, |vm| {
            bind_method(vm, &receiver, socket_on_ready)
        })
    });
    result.unwrap_or(Ok(()))
}

fn update_listener_registration(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
) -> Result<(), RuntimeError> {
    let receiver = Value::Resource(resource.clone());
    let result = with_listener(resource, |state| {
        let wanted = (!state.accepts.is_empty()).then_some(Interest::READABLE);
        let Some(listener) = state.listener.as_mut() else {
            return Ok(());
        };
        sync_registration(vm, listener, &mut state.registration, wanted, |vm| {
            bind_method(vm, &receiver, listener_on_ready)
        })
    });
    result.unwrap_or(Ok(()))
}

/// Starts the timer that rejects operation `op` after `timeout_ms`.
fn start_timer(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
    timeout_ms: Option<u64>,
    op: u64,
    on_timeout: NativeFunction,
) -> Result<Option<u64>, RuntimeError> {
    let Some(ms) = timeout_ms else {
        return Ok(None);
    };
    let callback = bind_method(vm, &Value::Resource(resource.clone()), on_timeout)?;
    Ok(Some(vm.add_timer(
        ms,
        callback,
        vec![Value::Int(op as i64)],
    )))
}

fn cancel(vm: &mut dyn VMProvider, timers: impl IntoIterator<Item = Option<u64>>) {
    for timer in timers.into_iter().flatten() {
        vm.cancel_timer(timer);
    }
}

// --- Socket objects --------------------------------------------------------

fn new_socket_state(stream: Option<mio::net::TcpStream>) -> SocketState {
    SocketState {
        stream,
        registration: None,
        connecting: None,
        local: String::new(),
        remote: String::new(),
        buffer: Vec::new(),
        eof: false,
        reads: VecDeque::new(),
        writes: VecDeque::new(),
        timeout_ms: None,
        next_op: 1,
    }
}

/// The `Socket` object for `resource`, a connected socket.
fn socket_object(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
    local: &str,
    remote: &str,
) -> Result<Value, RuntimeError> {
    let receiver = Value::Resource(resource.clone());
    let mut entries = IndexMap::new();
    entries.insert("localAddress".to_string(), string(local));
    entries.insert("remoteAddress".to_string(), string(remote));
    for (name, function) in [
        ("read", socket_read as NativeFunction),
        ("readLine", socket_read_line),
        ("write", socket_write),
        ("setTimeout", socket_set_timeout),
        ("close", socket_close),
    ] {
        entries.insert(name.to_string(), bind_method(vm, &receiver, function)?);
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))?))
}

/// Wraps an accepted connection in a `Socket`.
fn new_socket(vm: &mut dyn VMProvider, stream: mio::net::TcpStream) -> Result<Value, RuntimeError> {
    let local = stream
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let remote = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    stream.set_nodelay(true).ok();
    let mut state = new_socket_state(Some(stream));
    state.local = local.clone();
    state.remote = remote.clone();
    let resource = vm.get_gc().allocate(NativeResource::new(
        SOCKET_RESOURCE.to_string(),
        Box::new(state),
    ))?;
    socket_object(vm, &resource, &local, &remote)
}

// --- Net.connect -----------------------------------------------------------

/// `Net.connect(host, port, timeout: ms?)`: a promise for a connected `Socket`.
pub fn net_connect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (host, port) = host_port(args, "connect")?;
    let target = display_target(&host, port);
    check_network(vm, &format!("connect to {target}"))?;
    let named_timeout = vm.get_named_args().get("timeout").cloned();
    let timeout_ms = timeout_arg(named_timeout.as_ref().or(args.get(2)))?;
    let addrs = resolve(&host, port)?;

    let (promise_id, promise) = vm.create_promise()?;
    let mut state = new_socket_state(None);
    state.connecting = Some(Connecting {
        op: 0,
        target,
        remaining: addrs.into(),
        last_error: None,
        promise: promise_id,
        timer: None,
        timeout_ms,
    });
    let resource = vm.get_gc().allocate(NativeResource::new(
        SOCKET_RESOURCE.to_string(),
        Box::new(state),
    ))?;
    let timer = start_timer(vm, &resource, timeout_ms, 0, socket_on_timeout)?;
    with_socket(&resource, |state| {
        if let Some(connecting) = state.connecting.as_mut() {
            connecting.timer = timer;
        }
    });
    advance_connect(vm, &resource, false)?;
    Ok(promise)
}

/// Starts connecting to the next address once the current attempt failed
/// (or none was started), and settles the `connect` promise when it is done.
/// `ready` says the event loop reported the attempt in progress as ready:
/// only then is its outcome known (on Windows, a connection still in progress
/// already reports its peer address).
fn advance_connect(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
    ready: bool,
) -> Result<(), RuntimeError> {
    enum Outcome {
        Waiting,
        Connected(u64, Option<u64>, String, String),
        Failed(u64, Option<u64>, String),
    }
    let mut ready = ready;
    let outcome = with_socket(resource, |state| {
        loop {
            let Some(connecting) = state.connecting.as_mut() else {
                return Outcome::Waiting;
            };
            if let Some(stream) = state.stream.as_mut() {
                if !ready {
                    return Outcome::Waiting;
                }
                // An attempt is in progress: has it finished?
                let failure = match stream.take_error() {
                    Ok(Some(error)) | Err(error) => Some(error),
                    Ok(None) => match stream.peer_addr() {
                        Ok(remote) => {
                            let local = stream
                                .local_addr()
                                .map(|a| a.to_string())
                                .unwrap_or_default();
                            stream.set_nodelay(true).ok();
                            let remote = remote.to_string();
                            state.local = local.clone();
                            state.remote = remote.clone();
                            let connecting = state.connecting.take().expect("connecting");
                            return Outcome::Connected(
                                connecting.promise,
                                connecting.timer,
                                local,
                                remote,
                            );
                        }
                        Err(error)
                            if error.kind() == ErrorKind::NotConnected
                                || error.kind() == ErrorKind::WouldBlock =>
                        {
                            return Outcome::Waiting;
                        }
                        Err(error) => Some(error),
                    },
                };
                if let Some(error) = failure {
                    connecting.last_error = Some(error);
                    // Give up on this address; dropping the stream closes it.
                    if let Some(mut stream) = state.stream.take() {
                        drop_registration(vm, &mut stream, &mut state.registration);
                    }
                }
                continue;
            }
            let Some(addr) = connecting.remaining.pop_front() else {
                let connecting = state.connecting.take().expect("connecting");
                let message = match &connecting.last_error {
                    Some(error) => connect_error(&connecting.target, error),
                    None => format!("cannot connect to {}", connecting.target),
                };
                return Outcome::Failed(connecting.promise, connecting.timer, message);
            };
            match mio::net::TcpStream::connect(addr) {
                Ok(stream) => {
                    state.stream = Some(stream);
                    ready = false;
                }
                Err(error) => connecting.last_error = Some(error),
            }
        }
    });
    match outcome {
        None | Some(Outcome::Waiting) => update_socket_registration(vm, resource),
        Some(Outcome::Connected(promise, timer, local, remote)) => {
            cancel(vm, [timer]);
            update_socket_registration(vm, resource)?;
            let socket = socket_object(vm, resource, &local, &remote)?;
            vm.resolve_promise(promise, socket);
            Ok(())
        }
        Some(Outcome::Failed(promise, timer, message)) => {
            cancel(vm, [timer]);
            settle(vm, vec![Settle::Reject(promise, message)])
        }
    }
}

// --- Socket readiness, reads and writes ------------------------------------

/// Takes the text a pending read can return from `buffer`, or `None` if it
/// must wait for more data. At the end of the stream, an empty buffer gives
/// `Some(None)`: the read returns `null`.
fn take_read(
    buffer: &mut Vec<u8>,
    eof: bool,
    kind: &ReadKind,
) -> Result<Option<Option<String>>, String> {
    match kind {
        ReadKind::Line => {
            if let Some(end) = buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=end).collect();
                let mut text = String::from_utf8_lossy(&line[..end]).into_owned();
                if text.ends_with('\r') {
                    text.pop();
                }
                return Ok(Some(Some(text)));
            }
            if buffer.len() > MAX_LINE_BYTES {
                return Err(format!(
                    "readLine(): the line is longer than {MAX_LINE_BYTES} bytes"
                ));
            }
            if eof {
                if buffer.is_empty() {
                    return Ok(Some(None));
                }
                let rest = std::mem::take(buffer);
                return Ok(Some(Some(String::from_utf8_lossy(&rest).into_owned())));
            }
            Ok(None)
        }
        ReadKind::Bytes(max) => {
            if buffer.is_empty() {
                return Ok(if eof { Some(None) } else { None });
            }
            let limit = (*max).min(buffer.len());
            let take = match std::str::from_utf8(&buffer[..limit]) {
                Ok(_) => limit,
                Err(error) if error.valid_up_to() > 0 => error.valid_up_to(),
                Err(error) => match error.error_len() {
                    // Invalid bytes at the start: return them as U+FFFD.
                    Some(len) => len,
                    // A character cut off at `limit`: return it whole if it
                    // has arrived, otherwise wait for the rest.
                    None => match std::str::from_utf8(&buffer[..buffer.len().min(4)]) {
                        Ok(text) => text.chars().next().map_or(0, char::len_utf8),
                        Err(e) if e.valid_up_to() > 0 => e.valid_up_to(),
                        Err(_) if eof => buffer.len(),
                        Err(_) => return Ok(None),
                    },
                },
            };
            let bytes: Vec<u8> = buffer.drain(..take).collect();
            Ok(Some(Some(String::from_utf8_lossy(&bytes).into_owned())))
        }
    }
}

/// Serves the pending reads and writes as far as the socket allows.
fn pump_socket(
    state: &mut SocketState,
    settlements: &mut Vec<Settle>,
    timers: &mut Vec<Option<u64>>,
) {
    let remote = state.remote.clone();
    // Reads, in the order they were made.
    while let Some(front) = state.reads.front() {
        match take_read(&mut state.buffer, state.eof, &front.kind) {
            Ok(Some(text)) => {
                let read = state.reads.pop_front().expect("read");
                timers.push(read.timer);
                settlements.push(Settle::Resolve(
                    read.promise,
                    text.map(string).unwrap_or(Value::Null),
                ));
                continue;
            }
            Err(message) => {
                let read = state.reads.pop_front().expect("read");
                timers.push(read.timer);
                settlements.push(Settle::Reject(read.promise, message));
                continue;
            }
            Ok(None) => {}
        }
        let Some(stream) = state.stream.as_mut() else {
            break;
        };
        let mut chunk = vec![0; DEFAULT_READ_BYTES];
        match stream.read(&mut chunk) {
            Ok(0) => state.eof = true,
            Ok(n) => state.buffer.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => {
                let message = io_error("read", &remote, &error);
                for read in state.reads.drain(..) {
                    timers.push(read.timer);
                    settlements.push(Settle::Reject(read.promise, message.clone()));
                }
                break;
            }
        }
    }
    // Writes, in order.
    while let Some(front) = state.writes.front_mut() {
        let Some(stream) = state.stream.as_mut() else {
            break;
        };
        if front.written == front.data.len() {
            let write = state.writes.pop_front().expect("write");
            timers.push(write.timer);
            settlements.push(Settle::Resolve(write.promise, Value::Null));
            continue;
        }
        match stream.write(&front.data[front.written..]) {
            Ok(n) => front.written += n,
            Err(error) if error.kind() == ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => {
                let message = io_error("write", &remote, &error);
                for write in state.writes.drain(..) {
                    timers.push(write.timer);
                    settlements.push(Settle::Reject(write.promise, message.clone()));
                }
                break;
            }
        }
    }
}

/// Runs the socket's pending operations, then settles what finished.
fn run_socket(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
) -> Result<(), RuntimeError> {
    let mut settlements = Vec::new();
    let mut timers = Vec::new();
    with_socket(resource, |state| {
        if state.connecting.is_none() {
            pump_socket(state, &mut settlements, &mut timers);
        }
    });
    cancel(vm, timers);
    update_socket_registration(vm, resource)?;
    settle(vm, settlements)
}

/// Event loop callback: the socket is readable or writable.
fn socket_on_ready(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let Some(Value::Resource(resource)) = args.first() else {
        return Ok(Value::Null);
    };
    let connecting = with_socket(resource, |state| state.connecting.is_some()).unwrap_or(false);
    let result = if connecting {
        advance_connect(vm, resource, true)
    } else {
        run_socket(vm, resource)
    };
    // An error here has no caller to go to; the operations it concerns were
    // rejected already.
    let _ = result;
    Ok(Value::Null)
}

/// Timer callback: operation `args[1]` took longer than its timeout.
fn socket_on_timeout(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (Some(Value::Resource(resource)), Some(Value::Int(op))) = (args.first(), args.get(1))
    else {
        return Ok(Value::Null);
    };
    let op = *op as u64;
    let settlement = with_socket(resource, |state| {
        let ms = state.timeout_ms.unwrap_or(0);
        if let Some(connecting) = &state.connecting
            && connecting.op == op
        {
            let connecting = state.connecting.take().expect("connecting");
            if let Some(mut stream) = state.stream.take() {
                drop_registration(vm, &mut stream, &mut state.registration);
            }
            return Some(Settle::Reject(
                connecting.promise,
                format!(
                    "TimeoutError: connection to {} timed out after {} ms",
                    connecting.target,
                    connecting.timeout_ms.unwrap_or(0)
                ),
            ));
        }
        if let Some(index) = state.reads.iter().position(|read| read.op == op) {
            let read = state.reads.remove(index).expect("read");
            let name = match read.kind {
                ReadKind::Line => "readLine()",
                ReadKind::Bytes(_) => "read()",
            };
            return Some(Settle::Reject(
                read.promise,
                format!("TimeoutError: {name} timed out after {ms} ms"),
            ));
        }
        if let Some(index) = state.writes.iter().position(|write| write.op == op) {
            let write = state.writes.remove(index).expect("write");
            return Some(Settle::Reject(
                write.promise,
                format!("TimeoutError: write() timed out after {ms} ms"),
            ));
        }
        None
    })
    .flatten();
    if let Some(settlement) = settlement {
        let _ = update_socket_registration(vm, resource);
        settle(vm, vec![settlement])?;
    }
    Ok(Value::Null)
}

/// Queues a read of `kind` and returns its promise.
fn start_read(
    vm: &mut dyn VMProvider,
    args: &[Value],
    kind: ReadKind,
    method: &str,
) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, SOCKET_RESOURCE, method)?.clone();
    check_network(vm, &format!("{method}()"))?;
    if !with_socket(&resource, |state| state.stream.is_some()).unwrap_or(false) {
        return Err(RuntimeError::new(format!(
            "cannot {method}(): the socket is closed"
        )));
    }
    let (promise_id, promise) = vm.create_promise()?;
    let started = with_socket(&resource, |state| {
        if state.stream.is_none() {
            return Err(format!("cannot {method}(): the socket is closed"));
        }
        let op = state.next_op;
        state.next_op += 1;
        state.reads.push_back(PendingRead {
            op,
            kind,
            promise: promise_id,
            timer: None,
        });
        Ok((op, state.timeout_ms))
    })
    .unwrap_or_else(|| Err(format!("cannot {method}(): the socket is closed")));
    let (op, timeout_ms) = started.map_err(RuntimeError::new)?;
    let timer = start_timer(vm, &resource, timeout_ms, op, socket_on_timeout)?;
    with_socket(&resource, |state| {
        if let Some(read) = state.reads.iter_mut().find(|read| read.op == op) {
            read.timer = timer;
        }
    });
    run_socket(vm, &resource)?;
    Ok(promise)
}

/// `socket.read(maxBytes = 65536)`: a promise for the next data received, at
/// most `maxBytes` bytes as a String, or `null` once the peer has closed.
fn socket_read(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let max = match args.get(1) {
        None | Some(Value::Null) => DEFAULT_READ_BYTES,
        Some(Value::Int(n)) if *n > 0 => *n as usize,
        Some(other) => {
            return Err(RuntimeError::new(format!(
                "read(maxBytes) expects an Int greater than 0; got {}",
                describe(other)
            )));
        }
    };
    start_read(vm, args, ReadKind::Bytes(max), "read")
}

/// `socket.readLine()`: a promise for the next line, without its `\n` (or
/// `\r\n`), or `null` once the peer has closed and no data is left.
fn socket_read_line(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    start_read(vm, args, ReadKind::Line, "readLine")
}

/// `socket.write(data)`: queues `data` (a String, sent as UTF-8) and returns
/// a promise that resolves once all of it has been handed to the system.
fn socket_write(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, SOCKET_RESOURCE, "write")?.clone();
    let data = match args.get(1) {
        Some(Value::String(text)) => text.as_bytes().to_vec(),
        Some(other) => {
            return Err(RuntimeError::new(format!(
                "write(data) expects a String; got {}",
                describe(other)
            )));
        }
        None => return Err(RuntimeError::new("write(data) expects a String")),
    };
    check_network(vm, "write()")?;
    if !with_socket(&resource, |state| state.stream.is_some()).unwrap_or(false) {
        return Err(RuntimeError::new("cannot write(): the socket is closed"));
    }
    let (promise_id, promise) = vm.create_promise()?;
    let started = with_socket(&resource, |state| {
        state.stream.as_ref()?;
        let op = state.next_op;
        state.next_op += 1;
        state.writes.push_back(PendingWrite {
            op,
            data,
            written: 0,
            promise: promise_id,
            timer: None,
        });
        Some((op, state.timeout_ms))
    })
    .flatten();
    let Some((op, timeout_ms)) = started else {
        return Err(RuntimeError::new("cannot write(): the socket is closed"));
    };
    let timer = start_timer(vm, &resource, timeout_ms, op, socket_on_timeout)?;
    with_socket(&resource, |state| {
        if let Some(write) = state.writes.iter_mut().find(|write| write.op == op) {
            write.timer = timer;
        }
    });
    run_socket(vm, &resource)?;
    Ok(promise)
}

/// `socket.setTimeout(ms)`: how long each later `read`, `readLine` and
/// `write` may wait before it fails with a `TimeoutError`; `null` for no limit.
fn socket_set_timeout(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, SOCKET_RESOURCE, "setTimeout")?;
    let timeout = timeout_arg(args.get(1))?;
    with_socket(resource, |state| state.timeout_ms = timeout);
    Ok(Value::Null)
}

/// `socket.close()`: closes the connection. Reads and writes still waiting
/// fail. Closing a closed socket does nothing.
fn socket_close(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, SOCKET_RESOURCE, "close")?.clone();
    let mut settlements = Vec::new();
    let mut timers = Vec::new();
    with_socket(&resource, |state| {
        if let Some(mut stream) = state.stream.take() {
            drop_registration(vm, &mut stream, &mut state.registration);
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        if let Some(connecting) = state.connecting.take() {
            timers.push(connecting.timer);
            settlements.push(Settle::Reject(
                connecting.promise,
                format!(
                    "the socket was closed while connecting to {}",
                    connecting.target
                ),
            ));
        }
        for read in state.reads.drain(..) {
            timers.push(read.timer);
            settlements.push(Settle::Reject(
                read.promise,
                "the socket was closed while a read was waiting".to_string(),
            ));
        }
        for write in state.writes.drain(..) {
            timers.push(write.timer);
            settlements.push(Settle::Reject(
                write.promise,
                "the socket was closed before all data was sent".to_string(),
            ));
        }
        state.buffer.clear();
    });
    cancel(vm, timers);
    settle(vm, settlements)?;
    Ok(Value::Null)
}

// --- Net.listen ------------------------------------------------------------

/// `Net.listen(host, port)`: a `Listener` bound to the address. Port 0 picks
/// a free port; `listener.port` tells which.
pub fn net_listen(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (host, port) = host_port(args, "listen")?;
    let target = display_target(&host, port);
    check_network(vm, &format!("listen on {target}"))?;
    let mut last_error = None;
    let mut bound = None;
    for addr in resolve(&host, port)? {
        match mio::net::TcpListener::bind(addr) {
            Ok(listener) => {
                bound = Some(listener);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let Some(listener) = bound else {
        let message = match last_error {
            Some(error) if error.kind() == ErrorKind::AddrInUse => {
                format!("cannot listen on {target}: the address is already in use")
            }
            Some(error) if error.kind() == ErrorKind::AddrNotAvailable => {
                format!("cannot listen on {target}: the address is not available on this machine")
            }
            Some(error) => format!("cannot listen on {target}: {error}"),
            None => format!("cannot listen on {target}"),
        };
        return Err(RuntimeError::new(message));
    };
    let local = listener
        .local_addr()
        .map_err(|e| RuntimeError::new(format!("cannot listen on {target}: {e}")))?;
    let address = local.to_string();
    let state = ListenerState {
        listener: Some(listener),
        registration: None,
        accepts: VecDeque::new(),
        address: address.clone(),
        timeout_ms: None,
        next_op: 1,
    };
    let resource = Value::Resource(vm.get_gc().allocate(NativeResource::new(
        LISTENER_RESOURCE.to_string(),
        Box::new(state),
    ))?);
    let mut entries = IndexMap::new();
    entries.insert("address".to_string(), string(address));
    entries.insert("host".to_string(), string(local.ip().to_string()));
    entries.insert("port".to_string(), Value::Int(local.port() as i64));
    for (name, function) in [
        ("accept", listener_accept as NativeFunction),
        ("setTimeout", listener_set_timeout),
        ("close", listener_close),
    ] {
        entries.insert(name.to_string(), bind_method(vm, &resource, function)?);
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))?))
}

fn run_listener(
    vm: &mut dyn VMProvider,
    resource: &GcRc<NativeResource>,
) -> Result<(), RuntimeError> {
    let mut settlements = Vec::new();
    let mut timers = Vec::new();
    with_listener(resource, |state| {
        while !state.accepts.is_empty() {
            let Some(listener) = state.listener.as_mut() else {
                break;
            };
            match listener.accept() {
                Ok((stream, _)) => {
                    let accept = state.accepts.pop_front().expect("accept");
                    timers.push(accept.timer);
                    settlements.push(Settle::Accepted(accept.promise, stream));
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::Interrupted
                            | ErrorKind::ConnectionAborted
                            | ErrorKind::ConnectionReset
                    ) => {}
                Err(error) => {
                    let accept = state.accepts.pop_front().expect("accept");
                    timers.push(accept.timer);
                    settlements.push(Settle::Reject(
                        accept.promise,
                        format!("accept() failed on {}: {error}", state.address),
                    ));
                }
            }
        }
    });
    cancel(vm, timers);
    update_listener_registration(vm, resource)?;
    settle(vm, settlements)
}

fn listener_on_ready(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if let Some(Value::Resource(resource)) = args.first() {
        let _ = run_listener(vm, resource);
    }
    Ok(Value::Null)
}

fn listener_on_timeout(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (Some(Value::Resource(resource)), Some(Value::Int(op))) = (args.first(), args.get(1))
    else {
        return Ok(Value::Null);
    };
    let op = *op as u64;
    let settlement = with_listener(resource, |state| {
        let index = state.accepts.iter().position(|accept| accept.op == op)?;
        let accept = state.accepts.remove(index)?;
        Some(Settle::Reject(
            accept.promise,
            format!(
                "TimeoutError: accept() timed out after {} ms",
                state.timeout_ms.unwrap_or(0)
            ),
        ))
    })
    .flatten();
    if let Some(settlement) = settlement {
        let _ = update_listener_registration(vm, resource);
        settle(vm, vec![settlement])?;
    }
    Ok(Value::Null)
}

/// `listener.accept()`: a promise for the next incoming connection's `Socket`.
fn listener_accept(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, LISTENER_RESOURCE, "accept")?.clone();
    check_network(vm, "accept()")?;
    if !with_listener(&resource, |state| state.listener.is_some()).unwrap_or(false) {
        return Err(RuntimeError::new("cannot accept(): the listener is closed"));
    }
    let (promise_id, promise) = vm.create_promise()?;
    let started = with_listener(&resource, |state| {
        state.listener.as_ref()?;
        let op = state.next_op;
        state.next_op += 1;
        state.accepts.push_back(PendingAccept {
            op,
            promise: promise_id,
            timer: None,
        });
        Some((op, state.timeout_ms))
    })
    .flatten();
    let Some((op, timeout_ms)) = started else {
        return Err(RuntimeError::new("cannot accept(): the listener is closed"));
    };
    let timer = start_timer(vm, &resource, timeout_ms, op, listener_on_timeout)?;
    with_listener(&resource, |state| {
        if let Some(accept) = state.accepts.iter_mut().find(|accept| accept.op == op) {
            accept.timer = timer;
        }
    });
    run_listener(vm, &resource)?;
    Ok(promise)
}

/// `listener.setTimeout(ms)`: how long each later `accept()` may wait.
fn listener_set_timeout(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, LISTENER_RESOURCE, "setTimeout")?;
    let timeout = timeout_arg(args.get(1))?;
    with_listener(resource, |state| state.timeout_ms = timeout);
    Ok(Value::Null)
}

/// `listener.close()`: stops listening. `accept()` calls still waiting fail.
/// Closing a closed listener does nothing.
fn listener_close(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let resource = resource_arg(args, LISTENER_RESOURCE, "close")?.clone();
    let mut settlements = Vec::new();
    let mut timers = Vec::new();
    with_listener(&resource, |state| {
        if let Some(mut listener) = state.listener.take() {
            drop_registration(vm, &mut listener, &mut state.registration);
        }
        for accept in state.accepts.drain(..) {
            timers.push(accept.timer);
            settlements.push(Settle::Reject(
                accept.promise,
                "the listener was closed while accept() was waiting".to_string(),
            ));
        }
    });
    cancel(vm, timers);
    settle(vm, settlements)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(
        buffer: &[u8],
        eof: bool,
        kind: ReadKind,
    ) -> (Result<Option<Option<String>>, String>, Vec<u8>) {
        let mut buffer = buffer.to_vec();
        let result = take_read(&mut buffer, eof, &kind);
        (result, buffer)
    }

    #[test]
    fn read_line_strips_newline_and_carriage_return() {
        let (line, rest) = read(b"hi\r\nthere", false, ReadKind::Line);
        assert_eq!(line, Ok(Some(Some("hi".to_string()))));
        assert_eq!(rest, b"there");
    }

    #[test]
    fn read_line_waits_for_newline_then_returns_rest_at_eof() {
        assert_eq!(read(b"part", false, ReadKind::Line).0, Ok(None));
        assert_eq!(
            read(b"part", true, ReadKind::Line).0,
            Ok(Some(Some("part".to_string())))
        );
        assert_eq!(read(b"", true, ReadKind::Line).0, Ok(Some(None)));
    }

    #[test]
    fn read_never_splits_a_character() {
        // "é" is two bytes: a limit of 2 bytes after "a" keeps it for later.
        let (text, rest) = read("aé".as_bytes(), false, ReadKind::Bytes(2));
        assert_eq!(text, Ok(Some(Some("a".to_string()))));
        assert_eq!(rest, "é".as_bytes());
        // A limit smaller than one character returns the whole character.
        let (text, _) = read("é".as_bytes(), false, ReadKind::Bytes(1));
        assert_eq!(text, Ok(Some(Some("é".to_string()))));
        // Half a character waits for the other half.
        assert_eq!(
            read(&"é".as_bytes()[..1], false, ReadKind::Bytes(10)).0,
            Ok(None)
        );
    }

    #[test]
    fn read_at_end_of_stream_is_null() {
        assert_eq!(read(b"", true, ReadKind::Bytes(10)).0, Ok(Some(None)));
        assert_eq!(read(b"", false, ReadKind::Bytes(10)).0, Ok(None));
    }

    #[test]
    fn a_collected_resource_closes_its_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut state = new_socket_state(None);
        state.local = addr.to_string();
        let listener_state = ListenerState {
            listener: Some(mio::net::TcpListener::from_std(listener)),
            registration: None,
            accepts: VecDeque::new(),
            address: addr.to_string(),
            timeout_ms: None,
            next_op: 1,
        };
        let mut gc = pleno_value::gc::GCManager::new(1000);
        let resource = gc
            .allocate(NativeResource::new(
                LISTENER_RESOURCE.to_string(),
                Box::new(listener_state),
            ))
            .unwrap();
        // A stray reference outside the heap does not keep the socket open
        // once the collector finds nothing in the program refers to it.
        let stray = resource.clone();
        drop(resource);
        gc.collect_garbage(&[]);
        assert!(stray.borrow().inner.borrow().is_none());
        assert!(std::net::TcpStream::connect(addr).is_err());
    }
}

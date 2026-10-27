//! The HTTP/1.1 server: `Router`, `Server`, middleware and the `req`/`res`
//! objects.
//!
//! A request runs through a chain: every middleware (`use`, `static`) that
//! applies, in the order it was registered, then the matching route. Each
//! middleware gets a `next` function that runs the rest of the chain. The
//! response is written once the whole chain has finished: every handler has
//! returned and every promise a handler returned has settled. So middleware
//! can still add headers after `await next()`.
//!
//! The state of a request lives in a context map (traced by the collector),
//! kept in the server's `_active` map until the response is written, plus an
//! `Exchange` with the connection and counters.

use super::{cookies, static_files};
use indexmap::IndexMap;
use pleno_value::{
    ArrayState, BoundNativeMethodState, MapState, NativeFunction, NativeResource, RuntimeError,
    VMProvider, Value,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::rc::Rc;

/// The most a request body may hold; a larger one gets 413.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
/// The most the request line and headers may hold together; more gets 431.
const MAX_HEAD_BYTES: u64 = 64 * 1024;
/// The route methods a router keeps.
const ROUTE_METHODS: [&str; 5] = ["GET", "POST", "PUT", "DELETE", "PATCH"];

// --- Small helpers ---

fn string(text: impl Into<String>) -> Value {
    Value::String(Rc::new(text.into()))
}

fn new_map(
    vm: &mut dyn VMProvider,
    entries: IndexMap<String, Value>,
) -> Result<Value, RuntimeError> {
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))?))
}

fn get(map: &Value, key: &str) -> Option<Value> {
    match map {
        Value::Map(m) => m.borrow().entries.get(key).cloned(),
        _ => None,
    }
}

fn set(map: &Value, key: &str, value: Value) {
    if let Value::Map(m) = map {
        m.borrow_mut().entries.insert(key.to_string(), value);
    }
}

fn get_bool(map: &Value, key: &str) -> bool {
    matches!(get(map, key), Some(Value::Bool(true)))
}

fn bind(
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

fn add_methods(
    vm: &mut dyn VMProvider,
    receiver: &Value,
    methods: &[(&str, NativeFunction)],
) -> Result<(), RuntimeError> {
    for (name, function) in methods {
        let method = bind(vm, receiver, *function)?;
        set(receiver, name, method);
    }
    Ok(())
}

fn string_map(
    vm: &mut dyn VMProvider,
    entries: &IndexMap<String, String>,
) -> Result<Value, RuntimeError> {
    let map = entries
        .iter()
        .map(|(k, v)| (k.clone(), string(v.clone())))
        .collect();
    new_map(vm, map)
}

fn is_callable(value: &Value) -> bool {
    matches!(
        value,
        Value::Function(_)
            | Value::Closure(_)
            | Value::NativeFunction(_)
            | Value::BoundNativeMethod(_)
            | Value::BoundMethod(_)
            | Value::BoundFunction(_)
            | Value::StaticMethod(_)
            | Value::Partial(_)
    )
}

/// Whether `value` is a promise, as returned by an `async fn`.
fn is_promise(value: &Value) -> bool {
    let Value::Map(map) = value else {
        return false;
    };
    let map = map.borrow();
    matches!(map.entries.get("id"), Some(Value::Int(_)))
        && matches!(
            map.entries.get("variant").and_then(|v| v.as_str()),
            Some("Pending" | "Resolved" | "Rejected")
        )
}

/// `/api` for `/api/`, and an error for a prefix that is not a path.
fn normalize_prefix(function: &str, prefix: &str) -> Result<String, RuntimeError> {
    if !prefix.starts_with('/') {
        return Err(RuntimeError::new(format!(
            "router.{function}(): the path must start with '/', got '{prefix}'"
        )));
    }
    let trimmed = prefix.trim_end_matches('/');
    Ok(if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    })
}

/// The part of `path` below `prefix`, if `path` is `prefix` or inside it:
/// `/api` covers `/api` and `/api/users`, not `/apiary`.
fn strip_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix == "/" {
        return Some(path);
    }
    let rest = path.strip_prefix(prefix)?;
    (rest.is_empty() || rest.starts_with('/')).then_some(rest)
}

/// `Error: message` for an error object, the value as text otherwise.
fn describe_error(value: &Value) -> String {
    if let Value::Object(object) = value {
        let object = object.borrow();
        if let Some(message) = object.fields.get("message") {
            let name = object
                .fields
                .get("name")
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Error".to_string());
            return format!("{name}: {message}");
        }
    }
    value.to_string()
}

// --- Router ---

pub fn http_router(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let mut methods = IndexMap::new();
    for method in ROUTE_METHODS {
        methods.insert(method.to_string(), new_map(vm, IndexMap::new())?);
    }
    let mut router = IndexMap::new();
    router.insert("_routes".to_string(), new_map(vm, methods)?);
    router.insert(
        "_layers".to_string(),
        Value::Array(vm.get_gc().allocate(ArrayState::new(Vec::new()))?),
    );
    router.insert("_onError".to_string(), Value::Null);
    let router = new_map(vm, router)?;
    add_methods(
        vm,
        &router,
        &[
            ("get", router_get),
            ("post", router_post),
            ("put", router_put),
            ("delete", router_delete),
            ("patch", router_patch),
            ("use", router_use),
            ("static", router_static),
            ("onError", router_on_error),
        ],
    )?;
    Ok(router)
}

fn add_route(args: &[Value], method: &str) -> Result<Value, RuntimeError> {
    let name = method.to_lowercase();
    let (Some(router), Some(Value::String(path)), Some(handler)) =
        (args.first(), args.get(1), args.get(2))
    else {
        return Err(RuntimeError::new(format!(
            "router.{name}(path, handler) expects a path and a handler"
        )));
    };
    if !path.starts_with('/') {
        return Err(RuntimeError::new(format!(
            "router.{name}(): the path must start with '/', got '{path}'"
        )));
    }
    if !is_callable(handler) {
        return Err(RuntimeError::new(format!(
            "router.{name}(): the handler must be a function"
        )));
    }
    if let Some(routes) = get(router, "_routes")
        && let Some(method_routes) = get(&routes, method)
    {
        set(&method_routes, path, handler.clone());
    }
    Ok(router.clone())
}

fn router_get(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(args, "GET")
}
fn router_post(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(args, "POST")
}
fn router_put(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(args, "PUT")
}
fn router_delete(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(args, "DELETE")
}
fn router_patch(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(args, "PATCH")
}

fn push_layer(
    vm: &mut dyn VMProvider,
    router: &Value,
    layer: IndexMap<String, Value>,
) -> Result<(), RuntimeError> {
    let layer = new_map(vm, layer)?;
    if let Some(Value::Array(layers)) = get(router, "_layers") {
        layers.borrow_mut().elements.push(layer);
    }
    Ok(())
}

/// `router.use(fn(req, res, next) { ... })` or `router.use("/prefix", fn ...)`.
fn router_use(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let router = args.first().cloned().unwrap_or(Value::Null);
    let (prefix, handler) = match (args.get(1), args.get(2)) {
        (Some(Value::String(prefix)), Some(handler)) => {
            (normalize_prefix("use", prefix)?, handler.clone())
        }
        (Some(handler), None) => ("/".to_string(), handler.clone()),
        _ => {
            return Err(RuntimeError::new(
                "router.use() expects a middleware function, optionally after a path prefix",
            ));
        }
    };
    if !is_callable(&handler) {
        return Err(RuntimeError::new(
            "router.use(): the middleware must be a function taking (req, res, next)",
        ));
    }
    let mut layer = IndexMap::new();
    layer.insert("kind".to_string(), string("use"));
    layer.insert("prefix".to_string(), string(prefix));
    layer.insert("handler".to_string(), handler);
    push_layer(vm, &router, layer)?;
    Ok(router)
}

/// `router.static("/assets", "./public")`.
fn router_static(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let router = args.first().cloned().unwrap_or(Value::Null);
    let (Some(Value::String(prefix)), Some(Value::String(folder))) = (args.get(1), args.get(2))
    else {
        return Err(RuntimeError::new(
            "router.static(prefix, folder) expects a URL prefix and a folder",
        ));
    };
    let prefix = normalize_prefix("static", prefix)?;
    if !vm.is_read_allowed(folder) {
        return Err(RuntimeError::new(format!(
            "SecurityError: router.static(): read access to '{folder}' is not allowed"
        )));
    }
    let root = static_files::resolve_root(folder).map_err(RuntimeError::new)?;
    let mut layer = IndexMap::new();
    layer.insert("kind".to_string(), string("static"));
    layer.insert("prefix".to_string(), string(prefix));
    layer.insert("root".to_string(), string(root.to_string_lossy()));
    push_layer(vm, &router, layer)?;
    Ok(router)
}

/// `router.onError(fn(err, req, res) { ... })`: replaces the default 500 page.
fn router_on_error(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let router = args.first().cloned().unwrap_or(Value::Null);
    match args.get(1) {
        Some(handler) if is_callable(handler) => set(&router, "_onError", handler.clone()),
        _ => {
            return Err(RuntimeError::new(
                "router.onError() expects a function taking (err, req, res)",
            ));
        }
    }
    Ok(router)
}

// --- Server ---

pub fn http_server(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let router = match args.first() {
        Some(router) if get(router, "_routes").is_some() => router.clone(),
        _ => return Err(RuntimeError::new("Server(router) expects a Router")),
    };
    let mut server = IndexMap::new();
    server.insert("_router".to_string(), router);
    server.insert("_active".to_string(), new_map(vm, IndexMap::new())?);
    server.insert("_nextId".to_string(), Value::Int(0));
    server.insert("port".to_string(), Value::Null);
    let server = new_map(vm, server)?;
    add_methods(vm, &server, &[("listen", http_server_listen)])?;
    Ok(server)
}

pub fn http_server_listen(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] = this (Server), args[1] = port, args[2] = callback (optional)
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let server = args.first().cloned().unwrap_or(Value::Null);
    let port = match args.get(1) {
        Some(Value::Int(port)) if (0..=65535).contains(port) => *port as u16,
        _ => {
            return Err(RuntimeError::new(
                "listen() expects a port from 0 to 65535 (0 picks a free one)",
            ));
        }
    };
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let mut listener = mio::net::TcpListener::bind(addr)
        .map_err(|e| RuntimeError::new(format!("listen(): cannot bind port {port}: {e}")))?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    set(&server, "port", Value::Int(port as i64));
    println!("Server listening on http://0.0.0.0:{port}");

    let on_accept = bind(vm, &server, http_server_on_accept)?;
    vm.register_io(&mut listener, mio::Interest::READABLE, on_accept, true)?;
    let listener = NativeResource::new("http_listener".to_string(), Box::new(listener));
    let listener = Value::Resource(vm.get_gc().allocate(listener)?);
    set(&server, "_listener", listener);

    if let Some(callback) = args.get(2)
        && is_callable(callback)
    {
        vm.call(callback.clone(), vec![])?;
    }
    Ok(Value::Null)
}

pub fn http_server_on_accept(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let server = args.first().cloned().unwrap_or(Value::Null);
    let Some(Value::Resource(listener)) = get(&server, "_listener") else {
        return Ok(Value::Null);
    };
    loop {
        let accepted = {
            let state = listener.borrow();
            let mut inner = state.inner.borrow_mut();
            match inner
                .as_mut()
                .and_then(|l| l.downcast_mut::<mio::net::TcpListener>())
            {
                Some(l) => l.accept(),
                None => return Ok(Value::Null),
            }
        };
        let Ok((stream, peer)) = accepted else {
            break;
        };
        handle_connection(vm, &server, stream, peer)?;
    }
    Ok(Value::Null)
}

// --- Reading requests ---

struct HttpRequest {
    method: String,
    path: String,
    query: IndexMap<String, String>,
    headers: IndexMap<String, String>,
    body: String,
}

/// Why a request could not be read, and the status that says so.
enum ReadError {
    /// The client sent nothing, or went away: no response.
    Closed,
    Status(i64),
}

const BAD_REQUEST: ReadError = ReadError::Status(400);

/// Reads up to the next `\n` within `budget` bytes.
fn read_line(reader: &mut impl BufRead, budget: &mut u64) -> Result<String, ReadError> {
    let mut line = Vec::new();
    let read = reader
        .take(*budget)
        .read_until(b'\n', &mut line)
        .map_err(|_| ReadError::Closed)?;
    if read == 0 {
        return Err(ReadError::Closed);
    }
    if !line.ends_with(b"\n") {
        return Err(if read as u64 >= *budget {
            ReadError::Status(431)
        } else {
            ReadError::Closed
        });
    }
    *budget -= read as u64;
    while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        line.pop();
    }
    String::from_utf8(line).map_err(|_| BAD_REQUEST)
}

/// `key=value&...`, with `+` and `%XX` decoded. A repeated key keeps its last value.
fn parse_query(query: &str) -> IndexMap<String, String> {
    let decode =
        |text: &str| cookies::percent_decode(text, true).unwrap_or_else(|| text.to_string());
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

/// A `Transfer-Encoding: chunked` body.
fn read_chunked(reader: &mut impl BufRead) -> Result<Vec<u8>, ReadError> {
    let mut body = Vec::new();
    loop {
        let mut budget = 1024;
        let line = read_line(reader, &mut budget)?;
        let size = line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size, 16).map_err(|_| BAD_REQUEST)?;
        if size == 0 {
            // Trailer fields, up to the empty line.
            let mut budget = MAX_HEAD_BYTES;
            while !read_line(reader, &mut budget)?.is_empty() {}
            return Ok(body);
        }
        if body.len() + size > MAX_BODY_BYTES {
            return Err(ReadError::Status(413));
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader
            .read_exact(&mut body[start..])
            .map_err(|_| ReadError::Closed)?;
        let mut crlf = [0; 2];
        reader
            .read_exact(&mut crlf)
            .map_err(|_| ReadError::Closed)?;
        if &crlf != b"\r\n" {
            return Err(BAD_REQUEST);
        }
    }
}

/// Reads one HTTP/1.1 request.
fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ReadError> {
    let mut reader = BufReader::new(stream);
    let mut budget = MAX_HEAD_BYTES;
    let request_line = read_line(&mut reader, &mut budget)?;
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(BAD_REQUEST);
    };
    if !cookies::is_token(method) || !target.starts_with('/') || !version.starts_with("HTTP/1.") {
        return Err(BAD_REQUEST);
    }

    let mut headers: IndexMap<String, String> = IndexMap::new();
    loop {
        let line = read_line(&mut reader, &mut budget)?;
        if line.is_empty() {
            break;
        }
        // Folded lines (obsolete) and lines without a name are refused.
        let Some((name, value)) = line.split_once(':') else {
            return Err(BAD_REQUEST);
        };
        if !cookies::is_token(name) {
            return Err(BAD_REQUEST);
        }
        let name = name.to_ascii_lowercase();
        let value = value.trim();
        match headers.get_mut(&name) {
            Some(existing) => {
                existing.push_str(if name == "cookie" { "; " } else { ", " });
                existing.push_str(value);
            }
            None => {
                headers.insert(name, value.to_string());
            }
        }
    }

    let chunked = match headers.get("transfer-encoding") {
        None => false,
        Some(coding) if coding.eq_ignore_ascii_case("chunked") => true,
        Some(_) => return Err(ReadError::Status(501)),
    };
    // Both at once is how requests are smuggled past proxies.
    if chunked && headers.contains_key("content-length") {
        return Err(BAD_REQUEST);
    }
    let length = match headers.get("content-length") {
        None => 0,
        Some(length) if !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) => {
            length.parse::<usize>().unwrap_or(usize::MAX)
        }
        Some(_) => return Err(BAD_REQUEST),
    };
    if length > MAX_BODY_BYTES {
        return Err(ReadError::Status(413));
    }
    if (chunked || length > 0)
        && headers
            .get("expect")
            .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"))
    {
        let stream = reader.get_mut();
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").ok();
    }
    let body = if chunked {
        read_chunked(&mut reader)?
    } else {
        let mut body = vec![0; length];
        reader
            .read_exact(&mut body)
            .map_err(|_| ReadError::Closed)?;
        body
    };

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    Ok(HttpRequest {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        query: parse_query(query),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Matches a route pattern such as `/users/:id` against a request path,
/// returning the captured parameters, decoded.
fn match_route(pattern: &str, path: &str) -> Option<IndexMap<String, String>> {
    let pattern_parts: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let path_parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    if pattern_parts.len() != path_parts.len() {
        return None;
    }
    let mut params = IndexMap::new();
    for (expected, actual) in pattern_parts.iter().zip(&path_parts) {
        let actual = cookies::percent_decode(actual, false)?;
        if let Some(name) = expected.strip_prefix(':') {
            if actual.is_empty() {
                return None;
            }
            params.insert(name.to_string(), actual);
        } else if *expected != actual {
            return None;
        }
    }
    Some(params)
}

// --- Writing responses ---

fn reason_phrase(status: i64) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// Writes a response. For a `HEAD` request (`head_only`) the headers,
/// `Content-Length` included, are those of the full response, but no body.
/// 204 and 304 responses have neither a body nor a `Content-Length`.
fn write_response(
    stream: &mut TcpStream,
    status: i64,
    headers: &[(String, String)],
    body: &[u8],
    head_only: bool,
) {
    let mut head = format!("HTTP/1.1 {} {}\r\n", status, reason_phrase(status));
    let bodiless = status == 204 || status == 304;
    let has_content_type = headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));
    if !has_content_type && !bodiless && !body.is_empty() {
        head.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Date: {}\r\n",
        static_files::http_date(std::time::SystemTime::now())
    ));
    if !bodiless {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    let mut response = head.into_bytes();
    if !head_only && !bodiless {
        response.extend_from_slice(body);
    }
    stream.write_all(&response).ok();
    stream.flush().ok();
}

/// A response with a plain-text body, for errors the server answers itself.
fn write_plain(stream: &mut TcpStream, status: i64, head_only: bool) {
    write_response(
        stream,
        status,
        &[],
        reason_phrase(status).as_bytes(),
        head_only,
    );
}

// --- The request's state ---

/// What a request needs besides Pleno values: the connection and counters.
struct Exchange {
    stream: Option<TcpStream>,
    head_only: bool,
    /// `GET /path`, for messages.
    label: String,
    /// Handler calls and promises still running.
    pending: usize,
    /// The response was written.
    finished: bool,
    /// The chain got to the route (or to the 404/405 answer).
    reached_end: bool,
    /// The last middleware the chain entered, for the message when the chain
    /// stops there without a response.
    last_layer: Option<usize>,
    /// The `onError` hook has run for this request.
    error_hook_ran: bool,
    /// A file body from a static layer; it wins over `res.body`.
    file_body: Option<Vec<u8>>,
}

fn with_exchange<T>(ctx: &Value, f: impl FnOnce(&mut Exchange) -> T) -> Option<T> {
    let Some(Value::Resource(resource)) = get(ctx, "exchange") else {
        return None;
    };
    let state = resource.borrow();
    let mut inner = state.inner.borrow_mut();
    inner.as_mut()?.downcast_mut::<Exchange>().map(f)
}

fn handle_connection(
    vm: &mut dyn VMProvider,
    server: &Value,
    stream: mio::net::TcpStream,
    peer: std::net::SocketAddr,
) -> Result<(), RuntimeError> {
    // Serve each connection with blocking I/O; the listener stays on the event loop.
    let mut stream: TcpStream = stream.into();
    stream.set_nonblocking(false).ok();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(ReadError::Closed) => return Ok(()),
        Err(ReadError::Status(status)) => {
            write_plain(&mut stream, status, false);
            // Read some of what the client is still sending, so that closing
            // does not reset the connection before it reads the answer.
            stream.shutdown(std::net::Shutdown::Write).ok();
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(500)))
                .ok();
            std::io::copy(&mut (&stream).take(1024 * 1024), &mut std::io::sink()).ok();
            return Ok(());
        }
    };
    let head_only = request.method == "HEAD";
    let label = format!("{} {}", request.method, request.path);
    let cookies = request
        .headers
        .get("cookie")
        .map(|header| cookies::parse(header))
        .unwrap_or_default();

    let mut req = IndexMap::new();
    req.insert("method".to_string(), string(request.method));
    req.insert("path".to_string(), string(request.path));
    req.insert("params".to_string(), new_map(vm, IndexMap::new())?);
    req.insert("query".to_string(), string_map(vm, &request.query)?);
    req.insert("headers".to_string(), string_map(vm, &request.headers)?);
    req.insert("cookies".to_string(), string_map(vm, &cookies)?);
    req.insert("body".to_string(), string(request.body));
    req.insert("ip".to_string(), string(peer.ip().to_string()));
    let req = new_map(vm, req)?;
    let res = new_response(vm)?;

    let router = get(server, "_router").unwrap_or(Value::Null);
    let layers = match get(&router, "_layers") {
        Some(Value::Array(layers)) => layers.borrow().elements.clone(),
        _ => Vec::new(),
    };
    let exchange = Exchange {
        stream: Some(stream),
        head_only,
        label,
        pending: 1,
        finished: false,
        reached_end: false,
        last_layer: None,
        error_hook_ran: false,
        file_body: None,
    };
    let exchange = NativeResource::new("http_exchange".to_string(), Box::new(exchange));

    let id = get(server, "_nextId").and_then(|v| v.as_int()).unwrap_or(0);
    set(server, "_nextId", Value::Int(id + 1));
    let mut ctx = IndexMap::new();
    ctx.insert("id".to_string(), string(id.to_string()));
    ctx.insert("server".to_string(), server.clone());
    ctx.insert("router".to_string(), router);
    ctx.insert(
        "layers".to_string(),
        Value::Array(vm.get_gc().allocate(ArrayState::new(layers))?),
    );
    ctx.insert("req".to_string(), req);
    ctx.insert("res".to_string(), res.clone());
    ctx.insert(
        "exchange".to_string(),
        Value::Resource(vm.get_gc().allocate(exchange)?),
    );
    let ctx = new_map(vm, ctx)?;
    // Keeps the request's values reachable while promises are pending.
    if let Some(active) = get(server, "_active") {
        set(&active, &id.to_string(), ctx.clone());
    }

    let result = dispatch(vm, &ctx, 0);
    let fatal = match result {
        Err(e) if e.fatal => Some(e),
        _ => None,
    };
    settle_one(vm, &ctx)?;
    match fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Runs the chain from layer `start`: the first middleware there that
/// applies to the request, or else the route.
fn dispatch(vm: &mut dyn VMProvider, ctx: &Value, start: usize) -> Result<Value, RuntimeError> {
    let req = get(ctx, "req").unwrap_or(Value::Null);
    let res = get(ctx, "res").unwrap_or(Value::Null);
    let path = get(&req, "path").map(|p| p.to_string()).unwrap_or_default();
    let method = get(&req, "method")
        .map(|m| m.to_string())
        .unwrap_or_default();
    let layers = match get(ctx, "layers") {
        Some(Value::Array(layers)) => layers.borrow().elements.clone(),
        _ => Vec::new(),
    };

    for (index, layer) in layers.iter().enumerate().skip(start) {
        let prefix = get(layer, "prefix")
            .map(|p| p.to_string())
            .unwrap_or_default();
        let Some(rest) = strip_prefix(&path, &prefix) else {
            continue;
        };
        match get(layer, "kind")
            .and_then(|k| k.as_str().map(str::to_string))
            .as_deref()
        {
            Some("use") => {
                with_exchange(ctx, |ex| ex.last_layer = Some(index));
                let mut state = IndexMap::new();
                state.insert("ctx".to_string(), ctx.clone());
                state.insert("index".to_string(), Value::Int(index as i64 + 1));
                state.insert("called".to_string(), Value::Bool(false));
                let state = new_map(vm, state)?;
                let next = bind(vm, &state, call_next)?;
                let handler = get(layer, "handler").unwrap_or(Value::Null);
                let result = vm.call(handler, vec![req.clone(), res.clone(), next]);
                return track(vm, ctx, result, false);
            }
            Some("static") if (method == "GET" || method == "HEAD") && !get_bool(&res, "sent") => {
                let root = PathBuf::from(
                    get(layer, "root")
                        .map(|r| r.to_string())
                        .unwrap_or_default(),
                );
                let headers: IndexMap<String, String> = match get(&req, "headers") {
                    Some(Value::Map(h)) => h
                        .borrow()
                        .entries
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_string()))
                        .collect(),
                    _ => IndexMap::new(),
                };
                let allowed = |file: &str| vm.is_read_allowed(file);
                match static_files::serve(&root, &path, rest, &headers, &allowed) {
                    static_files::Outcome::Skip => continue,
                    static_files::Outcome::Serve {
                        status,
                        headers,
                        body,
                    } => {
                        set(&res, "statusCode", Value::Int(status));
                        for (name, value) in headers {
                            set_header(&res, &name, value);
                        }
                        with_exchange(ctx, |ex| {
                            ex.file_body = Some(body);
                            ex.reached_end = true;
                        });
                        set(&res, "sent", Value::Bool(true));
                    }
                    static_files::Outcome::Forbidden => {
                        finish_with_status(ctx, &res, 403);
                    }
                    static_files::Outcome::Redirect(location) => {
                        set(&res, "statusCode", Value::Int(301));
                        set_header(&res, "Location", location);
                        set(&res, "body", string(""));
                        set(&res, "sent", Value::Bool(true));
                        with_exchange(ctx, |ex| ex.reached_end = true);
                    }
                }
                return Ok(Value::Null);
            }
            _ => continue,
        }
    }
    route(vm, ctx, &req, &res, &method, &path)
}

/// Answers with `status` and its reason phrase as the body.
fn finish_with_status(ctx: &Value, res: &Value, status: i64) {
    set(res, "statusCode", Value::Int(status));
    set(res, "body", string(reason_phrase(status)));
    set(res, "sent", Value::Bool(true));
    with_exchange(ctx, |ex| ex.reached_end = true);
}

/// The end of the chain: the route for the method and path, or 404/405.
fn route(
    vm: &mut dyn VMProvider,
    ctx: &Value,
    req: &Value,
    res: &Value,
    method: &str,
    path: &str,
) -> Result<Value, RuntimeError> {
    with_exchange(ctx, |ex| ex.reached_end = true);
    let routes = get(ctx, "router").and_then(|r| get(&r, "_routes"));
    let find = |route_method: &str| -> Option<(Value, IndexMap<String, String>)> {
        let method_routes = routes.as_ref().and_then(|r| get(r, route_method))?;
        let Value::Map(method_routes) = method_routes else {
            return None;
        };
        let method_routes = method_routes.borrow();
        method_routes
            .entries
            .iter()
            .find_map(|(pattern, handler)| Some((handler.clone(), match_route(pattern, path)?)))
    };
    // A HEAD request is served by the `get` routes, without the body.
    let route_method = if method == "HEAD" { "GET" } else { method };
    if let Some((handler, params)) = find(route_method) {
        let params = string_map(vm, &params)?;
        set(req, "params", params);
        let result = vm.call(handler, vec![req.clone(), res.clone()]);
        return track(vm, ctx, result, true);
    }
    if get_bool(res, "sent") {
        return Ok(Value::Null);
    }
    // Another method has this path: 405 (or the answer to OPTIONS) with `Allow`.
    let mut allowed: Vec<&str> = ROUTE_METHODS
        .iter()
        .copied()
        .filter(|m| find(m).is_some())
        .collect();
    if allowed.contains(&"GET") {
        allowed.insert(1, "HEAD");
    }
    if allowed.is_empty() {
        finish_with_status(ctx, res, 404);
    } else {
        allowed.push("OPTIONS");
        set_header(res, "Allow", allowed.join(", "));
        if method == "OPTIONS" {
            set(res, "statusCode", Value::Int(204));
            set(res, "sent", Value::Bool(true));
        } else {
            finish_with_status(ctx, res, 405);
        }
    }
    Ok(Value::Null)
}

/// Follows what a handler call gave back. A promise is waited for (the
/// response waits with it); an error is recorded and passed on, so that a
/// middleware's `next()` throws it too.
fn track(
    vm: &mut dyn VMProvider,
    ctx: &Value,
    result: Result<Value, RuntimeError>,
    is_route: bool,
) -> Result<Value, RuntimeError> {
    match result {
        Ok(value) if is_promise(&value) => {
            with_exchange(ctx, |ex| ex.pending += 1);
            let mut info = IndexMap::new();
            info.insert("ctx".to_string(), ctx.clone());
            info.insert("route".to_string(), Value::Bool(is_route));
            let info = new_map(vm, info)?;
            let on_fulfilled = bind(vm, &info, handler_fulfilled)?;
            let on_rejected = bind(vm, &info, handler_rejected)?;
            crate::promise::register_callback(vm, &value, on_fulfilled, on_rejected, 0)?;
            Ok(value)
        }
        Ok(value) => {
            if is_route {
                use_returned_body(ctx, &value);
            }
            Ok(value)
        }
        Err(error) if error.fatal => Err(error),
        Err(error) => {
            // The value a `catch` (and `onError`) sees for the error.
            let message = error
                .message
                .strip_prefix("uncaught exception: ")
                .unwrap_or(&error.message);
            let reason = vm.error_value(message);
            record_error(ctx, reason);
            Err(error)
        }
    }
}

/// A route handler may return its body instead of calling `res.send`.
fn use_returned_body(ctx: &Value, value: &Value) {
    let res = get(ctx, "res").unwrap_or(Value::Null);
    if let Value::String(body) = value
        && !get_bool(&res, "sent")
    {
        set(&res, "body", Value::String(body.clone()));
        set(&res, "sent", Value::Bool(true));
    }
}

/// Records that a handler failed. A response sent before the failure is
/// dropped: a middleware that catches the error may send another.
fn record_error(ctx: &Value, reason: Value) {
    set(ctx, "error", reason);
    if let Some(res) = get(ctx, "res") {
        set(&res, "sent", Value::Bool(false));
    }
}

fn handler_fulfilled(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let info = args.first().cloned().unwrap_or(Value::Null);
    let ctx = get(&info, "ctx").unwrap_or(Value::Null);
    if get_bool(&info, "route") {
        use_returned_body(&ctx, args.get(1).unwrap_or(&Value::Null));
    }
    settle_one(vm, &ctx)?;
    Ok(Value::Null)
}

fn handler_rejected(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let info = args.first().cloned().unwrap_or(Value::Null);
    let ctx = get(&info, "ctx").unwrap_or(Value::Null);
    record_error(&ctx, args.get(1).cloned().unwrap_or(Value::Null));
    settle_one(vm, &ctx)?;
    Ok(Value::Null)
}

/// `next()` in a middleware: runs the rest of the chain and returns what it
/// returned (a promise when the rest is async, so it can be awaited).
/// `next(err)` fails the request with `err` instead.
fn call_next(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let state = args.first().cloned().unwrap_or(Value::Null);
    let ctx = get(&state, "ctx").unwrap_or(Value::Null);
    if with_exchange(&ctx, |ex| ex.finished).unwrap_or(true) {
        return Err(RuntimeError::new(
            "next() was called after the response was sent; call it before the middleware returns, or await it",
        ));
    }
    if get_bool(&state, "called") {
        return Err(RuntimeError::new(
            "next() was called twice in the same middleware",
        ));
    }
    set(&state, "called", Value::Bool(true));
    match args.get(1) {
        None | Some(Value::Null) => {}
        Some(error) => {
            record_error(&ctx, error.clone());
            return Ok(Value::Null);
        }
    }
    let start = get(&state, "index").and_then(|i| i.as_int()).unwrap_or(0) as usize;
    with_exchange(&ctx, |ex| ex.pending += 1);
    let result = dispatch(vm, &ctx, start);
    settle_one(vm, &ctx)?;
    result
}

/// One handler call or promise is done; once none is left, respond.
fn settle_one(vm: &mut dyn VMProvider, ctx: &Value) -> Result<(), RuntimeError> {
    let done = with_exchange(ctx, |ex| {
        ex.pending = ex.pending.saturating_sub(1);
        ex.pending == 0 && !ex.finished
    })
    .unwrap_or(false);
    if done {
        finish(vm, ctx)?;
    }
    Ok(())
}

/// Clears what handlers put on `res`, for the error handler: a 500 with the
/// plain reason as its body, unless the handler sends something else.
fn reset_response(vm: &mut dyn VMProvider, res: &Value) -> Result<(), RuntimeError> {
    set(res, "statusCode", Value::Int(500));
    set(res, "body", string(reason_phrase(500)));
    set(res, "sent", Value::Bool(false));
    set(res, "headers", new_map(vm, IndexMap::new())?);
    set(res, "_cookies", new_map(vm, IndexMap::new())?);
    Ok(())
}

/// The chain has finished: write the response, or the error response.
fn finish(vm: &mut dyn VMProvider, ctx: &Value) -> Result<(), RuntimeError> {
    let req = get(ctx, "req").unwrap_or(Value::Null);
    let res = get(ctx, "res").unwrap_or(Value::Null);
    let label = with_exchange(ctx, |ex| ex.label.clone()).unwrap_or_default();
    let sent = get_bool(&res, "sent");

    if let Some(error) = get(ctx, "error").filter(|_| !sent) {
        let hook = get(ctx, "router").and_then(|r| get(&r, "_onError"));
        let hook_ran = with_exchange(ctx, |ex| ex.error_hook_ran).unwrap_or(true);
        match hook {
            Some(hook) if is_callable(&hook) && !hook_ran => {
                with_exchange(ctx, |ex| {
                    ex.error_hook_ran = true;
                    ex.pending += 1;
                    ex.file_body = None;
                });
                if let Value::Map(m) = ctx {
                    m.borrow_mut().entries.shift_remove("error");
                }
                reset_response(vm, &res)?;
                let result = vm.call(hook, vec![error, req, res]);
                let fatal = match track(vm, ctx, result, true) {
                    Err(e) if e.fatal => Some(e),
                    _ => None,
                };
                settle_one(vm, ctx)?;
                return fatal.map_or(Ok(()), Err);
            }
            _ => {
                if hook_ran {
                    eprintln!(
                        "{label}: the onError handler failed: {}",
                        describe_error(&error)
                    );
                } else {
                    eprintln!("{label}: {}", describe_error(&error));
                }
                return write_final(ctx, 500, &[], reason_phrase(500).as_bytes().to_vec());
            }
        }
    }
    if !sent && !with_exchange(ctx, |ex| ex.reached_end).unwrap_or(false) {
        let layer = with_exchange(ctx, |ex| ex.last_layer)
            .flatten()
            .unwrap_or(0);
        let prefix = match get(ctx, "layers") {
            Some(Value::Array(layers)) => layers
                .borrow()
                .elements
                .get(layer)
                .and_then(|l| get(l, "prefix"))
                .map(|p| p.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        eprintln!(
            "{label}: middleware {} (use \"{prefix}\") returned without calling next() or sending a response",
            layer + 1
        );
        return write_final(ctx, 500, &[], reason_phrase(500).as_bytes().to_vec());
    }

    let status = get(&res, "statusCode")
        .and_then(|v| v.as_int())
        .unwrap_or(200);
    let mut headers: Vec<(String, String)> = match get(&res, "headers") {
        Some(Value::Map(h)) => h
            .borrow()
            .entries
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
        _ => Vec::new(),
    };
    if let Some(Value::Map(cookies)) = get(&res, "_cookies") {
        for cookie in cookies.borrow().entries.values() {
            headers.push(("Set-Cookie".to_string(), cookie.to_string()));
        }
    }
    let body = match with_exchange(ctx, |ex| ex.file_body.take()).flatten() {
        Some(body) => body,
        None => match get(&res, "body") {
            Some(Value::String(body)) => body.as_bytes().to_vec(),
            _ => Vec::new(),
        },
    };
    write_final(ctx, status, &headers, body)
}

fn write_final(
    ctx: &Value,
    status: i64,
    headers: &[(String, String)],
    body: Vec<u8>,
) -> Result<(), RuntimeError> {
    let taken = with_exchange(ctx, |ex| {
        ex.finished = true;
        ex.stream.take().map(|stream| (stream, ex.head_only))
    })
    .flatten();
    if let Some((mut stream, head_only)) = taken {
        write_response(&mut stream, status, headers, &body, head_only);
    }
    // The request is over: let its values go.
    if let (Some(server), Some(Value::String(id))) = (get(ctx, "server"), get(ctx, "id"))
        && let Some(Value::Map(active)) = get(&server, "_active")
    {
        active.borrow_mut().entries.shift_remove(id.as_str());
    }
    Ok(())
}

// --- The `res` object ---

/// The `res` object passed to handlers. `status`, `header`, `cookie` and
/// `clearCookie` return `res` for chaining.
fn new_response(vm: &mut dyn VMProvider) -> Result<Value, RuntimeError> {
    let mut entries = IndexMap::new();
    entries.insert("statusCode".to_string(), Value::Int(200));
    entries.insert("body".to_string(), Value::Null);
    entries.insert("sent".to_string(), Value::Bool(false));
    entries.insert("headers".to_string(), new_map(vm, IndexMap::new())?);
    entries.insert("_cookies".to_string(), new_map(vm, IndexMap::new())?);
    let res = new_map(vm, entries)?;
    add_methods(
        vm,
        &res,
        &[
            ("status", response_status),
            ("header", response_header),
            ("send", response_send),
            ("json", response_json),
            ("redirect", response_redirect),
            ("cookie", response_cookie),
            ("clearCookie", response_clear_cookie),
        ],
    )?;
    Ok(res)
}

fn this(args: &[Value]) -> Value {
    args.first().cloned().unwrap_or(Value::Null)
}

fn response_status(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let code = match args.get(1) {
        Some(Value::Int(code)) if (200..=599).contains(code) => *code,
        _ => {
            return Err(RuntimeError::new(
                "res.status(code) expects an integer status from 200 to 599",
            ));
        }
    };
    set(&args[0], "statusCode", Value::Int(code));
    Ok(this(args))
}

/// Sets a header, replacing one with the same name in any case.
fn set_header(res: &Value, name: &str, value: String) {
    if let Some(Value::Map(headers)) = get(res, "headers") {
        let mut headers = headers.borrow_mut();
        headers
            .entries
            .retain(|existing, _| !existing.eq_ignore_ascii_case(name));
        headers.entries.insert(name.to_string(), string(value));
    }
}

/// Header names and values `res.header` refuses, so that a value taken from
/// a request cannot add headers of its own or end the header section.
fn check_header(function: &str, name: &str, value: &str) -> Result<(), RuntimeError> {
    if name.contains(['\r', '\n']) || value.contains(['\r', '\n', '\0']) {
        return Err(RuntimeError::new(format!(
            "res.{function}(): header names and values cannot contain CR, LF or NUL"
        )));
    }
    if !cookies::is_token(name) {
        return Err(RuntimeError::new(format!(
            "res.{function}(): invalid header name '{name}'"
        )));
    }
    Ok(())
}

fn response_header(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (Some(Value::String(name)), Some(value)) = (args.get(1), args.get(2)) else {
        return Err(RuntimeError::new(
            "res.header(name, value) expects a name and a value",
        ));
    };
    let value = value.to_string();
    check_header("header", name, &value)?;
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "content-length" | "connection" | "transfer-encoding" | "date" => {
            return Err(RuntimeError::new(format!(
                "res.header(): {name} is set by the server"
            )));
        }
        "set-cookie" => {
            return Err(RuntimeError::new(
                "res.header(): use res.cookie(name, value, options) to set cookies",
            ));
        }
        _ => {}
    }
    set_header(&args[0], name, value);
    Ok(this(args))
}

fn mark_sent(function: &str, res: &Value) -> Result<(), RuntimeError> {
    if get_bool(res, "sent") {
        return Err(RuntimeError::new(format!(
            "res.{function}(): a response was already sent for this request"
        )));
    }
    set(res, "sent", Value::Bool(true));
    Ok(())
}

fn response_send(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let body = match args.get(1) {
        None | Some(Value::Null) => String::new(),
        Some(
            value @ (Value::String(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Char(_)),
        ) => value.to_string(),
        Some(_) => {
            return Err(RuntimeError::new(
                "res.send() expects a String; use res.json(value) for maps and arrays",
            ));
        }
    };
    mark_sent("send", &args[0])?;
    set(&args[0], "body", string(body));
    Ok(this(args))
}

fn response_json(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let json = crate::json::value_to_json_string(args.get(1).unwrap_or(&Value::Null))?;
    mark_sent("json", &args[0])?;
    set_header(&args[0], "Content-Type", "application/json".to_string());
    set(&args[0], "body", string(json));
    Ok(this(args))
}

/// `res.redirect(url)` (302) or `res.redirect(url, status)` with a 3xx status.
fn response_redirect(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let Some(Value::String(location)) = args.get(1) else {
        return Err(RuntimeError::new(
            "res.redirect(url, status?) expects a URL",
        ));
    };
    let status = match args.get(2) {
        None | Some(Value::Null) => 302,
        Some(Value::Int(code)) if (300..=399).contains(code) => *code,
        _ => {
            return Err(RuntimeError::new(
                "res.redirect(): the status must be a 3xx code such as 301, 302, 303 or 307",
            ));
        }
    };
    check_header("redirect", "Location", location)?;
    mark_sent("redirect", &args[0])?;
    set(&args[0], "statusCode", Value::Int(status));
    set_header(&args[0], "Location", location.to_string());
    set(&args[0], "body", string(""));
    Ok(this(args))
}

fn store_cookie(res: &Value, cookie: cookies::SetCookie) {
    if let Some(Value::Map(stored)) = get(res, "_cookies") {
        stored
            .borrow_mut()
            .entries
            .insert(cookie.key, string(cookie.header));
    }
}

fn response_cookie(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let name = args.get(1).unwrap_or(&Value::Null);
    let value = match args.get(2) {
        Some(Value::String(value)) => value.to_string(),
        Some(value @ (Value::Int(_) | Value::Float(_) | Value::Bool(_))) => value.to_string(),
        _ => {
            return Err(RuntimeError::new(
                "res.cookie(name, value, options?) expects a name and a String value",
            ));
        }
    };
    let cookie = cookies::serialize("cookie", name, &value, args.get(3), false)
        .map_err(RuntimeError::new)?;
    store_cookie(&args[0], cookie);
    Ok(this(args))
}

fn response_clear_cookie(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let name = args.get(1).unwrap_or(&Value::Null);
    let cookie = cookies::serialize("clearCookie", name, "", args.get(2), true)
        .map_err(RuntimeError::new)?;
    store_cookie(&args[0], cookie);
    Ok(this(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_match_whole_segments() {
        assert_eq!(strip_prefix("/api/users", "/api"), Some("/users"));
        assert_eq!(strip_prefix("/api", "/api"), Some(""));
        assert_eq!(strip_prefix("/apiary", "/api"), None);
        assert_eq!(strip_prefix("/x", "/"), Some("/x"));
    }

    #[test]
    fn decodes_query_strings_and_params() {
        let query = parse_query("q=a+b%26c&empty=&flag&=skipped&q2=%E2%9C%93");
        assert_eq!(query.get("q").map(String::as_str), Some("a b&c"));
        assert_eq!(query.get("empty").map(String::as_str), Some(""));
        assert_eq!(query.get("flag").map(String::as_str), Some(""));
        assert_eq!(query.get("q2").map(String::as_str), Some("\u{2713}"));
        assert_eq!(query.len(), 4);
        let params = match_route("/users/:name", "/users/Ann%20Lee").unwrap();
        assert_eq!(params.get("name").map(String::as_str), Some("Ann Lee"));
        assert!(match_route("/users/:name", "/users/").is_none());
    }
}

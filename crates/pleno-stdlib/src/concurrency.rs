use pleno_value::{ChannelState, MutexState, RuntimeError, VMProvider, Value};
use std::collections::VecDeque;

// Mutex
pub fn native_mutex_new(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_mutex_new called");
    let state = MutexState {
        locked: false,
        waiters: Vec::new(),
    };
    Ok(Value::Mutex(vm.get_gc().allocate(state)?))
}

pub fn native_mutex_lock(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_mutex_lock called with args: {:?}", args);
    let mutex_ref = match args.first() {
        Some(Value::Mutex(m)) => m.clone(),
        _ => return Err(RuntimeError::new("lock() expects Mutex")),
    };

    let mut mutex = mutex_ref.borrow_mut();
    if !mutex.locked {
        mutex.locked = true;
        Ok(Value::Null)
    } else {
        let (id, promise) = vm.create_promise()?;
        mutex.waiters.push(id as usize);
        Ok(promise)
    }
}

pub fn native_mutex_unlock(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_mutex_unlock called with args: {:?}", args);
    let mutex_ref = match args.first() {
        Some(Value::Mutex(m)) => m.clone(),
        _ => return Err(RuntimeError::new("unlock() expects Mutex")),
    };

    let mut mutex = mutex_ref.borrow_mut();
    if !mutex.locked {
        return Ok(Value::Null);
    }

    if let Some(waiter_id) = mutex.waiters.first().cloned() {
        mutex.waiters.remove(0);
        vm.resolve_promise(waiter_id as u64, Value::Null);
    } else {
        mutex.locked = false;
    }
    Ok(Value::Null)
}

// Channel
pub fn native_channel_new(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_channel_new called with args: {:?}", args);
    let capacity = match args.first() {
        Some(Value::Int(n)) if *n > 0 => *n as usize,
        _ => 0, // Unbounded if 0 or missing? Or synchronous? Let's say unbounded for now or default 1
    };

    // For now, let's implement unbounded if capacity is 0, or bounded if > 0.
    // Spec doesn't strictly specify, but Go channels are synchronous (cap 0) or buffered.
    // Let's implement Buffered(n) or Unbounded.
    // Implementation Plan said: buffer: VecDeque, capacity: usize.

    let state = ChannelState {
        buffer: VecDeque::new(),
        capacity: if capacity == 0 { usize::MAX } else { capacity },
        closed: false,
        send_waiters: Vec::new(),
        recv_waiters: Vec::new(),
    };
    Ok(Value::Channel(vm.get_gc().allocate(state)?))
}

pub fn native_channel_close(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    println!("native_channel_close called with args: {:?}", args);
    let chan_ref = match args.first() {
        Some(Value::Channel(c)) => c.clone(),
        _ => return Err(RuntimeError::new("close() expects Channel")),
    };

    let mut chan = chan_ref.borrow_mut();
    if chan.closed {
        return Ok(Value::Null);
    }
    chan.closed = true;

    // Reject all send waiters
    for (id, _) in chan.send_waiters.drain(..) {
        // Ideally we'd have a way to reject, but for now we resolve with Null/Error?
        // Let's resolve with Null or throw error?
        // Spec says send on closed channel should probably throw.
        // But since we are resolving a promise, we should resolve with an error value.
        vm.resolve_promise(id as u64, Value::Null); // Simplified
    }

    // Resolve all recv waiters with Missing if buffer is empty
    if chan.buffer.is_empty() {
        for id in chan.recv_waiters.drain(..) {
            vm.resolve_promise(id as u64, Value::Missing);
        }
    }

    Ok(Value::Null)
}

pub fn native_channel_send(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_channel_send called with args: {:?}", args);
    // args[0] is receiver (Channel), args[1] is value
    let chan_ref = match args.first() {
        Some(Value::Channel(c)) => c.clone(),
        _ => return Err(RuntimeError::new("send() expects Channel as receiver")),
    };
    let value = match args.get(1) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("send() expects value")),
    };

    let mut chan = chan_ref.borrow_mut();
    if chan.closed {
        return Err(RuntimeError::new("send on closed channel"));
    }

    // Check if there are receivers waiting
    if let Some(waiter_id) = chan.recv_waiters.first().cloned() {
        chan.recv_waiters.remove(0);
        // Direct handoff
        vm.resolve_promise(waiter_id as u64, value);
        return Ok(Value::Null);
    }

    // Check capacity
    if chan.buffer.len() < chan.capacity {
        chan.buffer.push_back(value);
        Ok(Value::Null)
    } else {
        // Suspend sender
        let (id, promise) = vm.create_promise()?;
        chan.send_waiters.push((id as usize, value));
        Ok(promise)
    }
}

pub fn native_channel_recv(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    println!("native_channel_recv called with args: {:?}", args);
    let chan_ref = match args.first() {
        Some(Value::Channel(c)) => c.clone(),
        _ => return Err(RuntimeError::new("recv() expects Channel")),
    };

    let mut chan = chan_ref.borrow_mut();

    if let Some(val) = chan.buffer.pop_front() {
        // If there are senders waiting, move one to buffer
        if !chan.send_waiters.is_empty() {
            let (sender_id, sender_val) = chan.send_waiters.remove(0);
            chan.buffer.push_back(sender_val);
            vm.resolve_promise(sender_id as u64, Value::Null);
        }
        Ok(val)
    } else if !chan.send_waiters.is_empty() {
        // Buffer empty, but sender waiting (capacity 0 case?)
        let (sender_id, sender_val) = chan.send_waiters.remove(0);
        vm.resolve_promise(sender_id as u64, Value::Null);
        Ok(sender_val)
    } else if chan.closed {
        // Closed and empty
        Ok(Value::Missing)
    } else {
        // Suspend receiver
        let (id, promise) = vm.create_promise()?;
        chan.recv_waiters.push(id as usize);
        Ok(promise)
    }
}

pub fn native_channel_iterator(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let chan_ref = match args.first() {
        Some(Value::Channel(c)) => c.clone(),
        _ => return Err(RuntimeError::new("iterator() expects Channel")),
    };
    Ok(Value::Channel(chan_ref))
}

pub fn native_channel_next(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // Channel iteration via next() - simply calls recv()
    native_channel_recv(vm, args)
}

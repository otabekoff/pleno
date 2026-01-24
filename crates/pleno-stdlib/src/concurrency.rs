use pleno_value::{ChannelState, MutexState, RuntimeError, VMProvider, Value};
use std::collections::VecDeque;

// Mutex
pub fn native_mutex_new(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let state = MutexState {
        locked: false,
        waiters: Vec::new(),
    };
    Ok(Value::Mutex(vm.get_gc().allocate(state)))
}

pub fn native_mutex_lock(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let mutex_ref = match args.first() {
        Some(Value::Mutex(m)) => m.clone(),
        _ => return Err(RuntimeError::new("lock() expects Mutex")),
    };

    let mut mutex = mutex_ref.borrow_mut();
    if !mutex.locked {
        mutex.locked = true;
        Ok(Value::Null)
    } else {
        let (id, promise) = vm.create_promise();
        mutex.waiters.push(id as usize);
        Ok(promise)
    }
}

pub fn native_mutex_unlock(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
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
        send_waiters: Vec::new(),
        recv_waiters: Vec::new(),
    };
    Ok(Value::Channel(vm.get_gc().allocate(state)))
}

pub fn native_channel_send(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
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
        let (id, promise) = vm.create_promise();
        chan.send_waiters.push((id as usize, value));
        Ok(promise)
    }
}

pub fn native_channel_recv(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let chan_ref = match args.first() {
        Some(Value::Channel(c)) => c.clone(),
        _ => return Err(RuntimeError::new("recv() expects Channel")),
    };

    let mut chan = chan_ref.borrow_mut();

    if let Some(val) = chan.buffer.pop_front() {
        // If there are senders waiting, move one to buffer (or handoff if buffer was full? wait, if buffer was full, it is now full-1.
        // If capacity is 1, buffer has 0. A sender can push.
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
    } else {
        // Suspend receiver
        let (id, promise) = vm.create_promise();
        chan.recv_waiters.push(id as usize);
        Ok(promise)
    }
}

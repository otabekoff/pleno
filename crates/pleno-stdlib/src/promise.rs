use indexmap::IndexMap;
use pleno_value::{
    ArrayState, BoundNativeMethodState, MapState, MultiWaiter, MultiWaiterMode, RuntimeError,
    VMProvider, Value,
};
// use std::collections::HashMap;
use std::rc::Rc;

pub fn promise_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("Promise() requires a callback"));
    }
    let executor = args[0].clone();

    // Create promise via VM which assigns an ID and registers it
    let (id, promise) = vm.create_promise()?;

    // Create bindable resolve/reject functions
    // We use BoundNativeMethod where the receiver is the promise itself (the map)
    // The native functions will extract the ID from the receiver and call vm.resolve_promise

    let gc = vm.get_gc();
    let resolve_fn = Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
        receiver: promise.clone(),
        function: native_promise_resolve,
    })?);

    let reject_fn = Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
        receiver: promise.clone(),
        function: native_promise_reject,
    })?);

    // Execute the executor immediately
    // If it throws, we catch it and reject the promise
    match vm.call(executor, vec![resolve_fn, reject_fn]) {
        Ok(_) => {}
        Err(e) => {
            // rejection logic could be here, but current VM error handling propagates up.
            // In a real Promise impl, we'd want to catch this and reject.
            // For now, let it propagate or manually reject if we could catch it.
            // Since vm.call returns Result, we can reject:
            vm.resolve_promise(id, Value::String(Rc::new(e.message))); // Simplified rejection
            // Ideally we set state to Rejected, but currently resolve_promise just sets value and Resolved state.
            // We need a proper reject mechanism in VM or just reuse resolve for now.
        }
    }

    Ok(promise)
}

fn native_promise_resolve(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] is the receiver (the Promise map)
    // args[1] is the value to resolve with
    if args.is_empty() {
        return Err(RuntimeError::new("resolve expects receiver"));
    }
    let receiver = &args[0];
    let value = args.get(1).cloned().unwrap_or(Value::Null);

    if let Value::Map(map) = receiver {
        let b = map.borrow();
        if let Some(Value::Int(id)) = b.get("id") {
            vm.resolve_promise(*id as u64, value);
        }
    }
    Ok(Value::Null)
}

fn native_promise_reject(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // Similar to resolve but perhaps wrapping in error or setting distinct state
    // For now simplistic implementation: resolve with error message or value
    // We ideally need vm.reject_promise
    if args.is_empty() {
        return Err(RuntimeError::new("reject expects receiver"));
    }
    let receiver = &args[0];
    let reason = args.get(1).cloned().unwrap_or(Value::Null);

    if let Value::Map(map) = receiver {
        let b = map.borrow();
        if let Some(Value::Int(id)) = b.get("id") {
            // For now treat rejection as resolution with the reason.
            // Real implementation needs 'Rejected' state.
            vm.resolve_promise(*id as u64, reason);
        }
    }
    Ok(Value::Null)
}

pub fn promise_then(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] is receiver (the promise)
    // args[1] is on_fulfilled
    // args[2] is on_rejected (optional)
    if args.len() < 2 {
        return Err(RuntimeError::new("then() requires callback"));
    }
    let parent = &args[0];
    let on_fulfilled = args[1].clone();
    let _on_rejected = args.get(2).cloned();

    // Create a new dependent promise
    let (child_id, child_promise) = vm.create_promise()?;

    if let Value::Map(map) = parent {
        let mut b = map.borrow_mut();

        // If parent is already resolved, schedule the task immediately
        if let Some(Value::String(s)) = b.get("variant")
            && s.as_str() == "Resolved"
        {
            let value = b.get("value").cloned().unwrap_or(Value::Null);
            // We can't immediately run callback here easily, or we can?
            // If we run it, we block.
            // Better to spawn a task or rely on VM loop.
            // Since we are in a builtin, we can perhaps just return the child promise
            // and let the VM handle it?
            // But we need to register the linkage.

            // Simplest: register an AsyncTask that will execute the callback
            // But we don't have public access to AsyncTask construction from here easily
            // unless we add a method to VMProvider.
            // For now, let's just assume we can simply attach to the parent's callbacks list.
            // Since it's resolved, maybe we just execute immediately?
            let result = vm.call(on_fulfilled, vec![value])?;
            vm.resolve_promise(child_id, result);
        } else {
            // Attach to parent's callbacks
            // We need to store this callback in the Promise map so VM picks it up on resolution
            // But Map stores Value, not native functions or closures easily accessible this way?
            // We can store a list of "then_callbacks" in the map.
            let callback_info = vec![on_fulfilled, Value::Int(child_id as i64)]; // Callback + Child ID

            // We need a list of these.
            // Since Value doesn't have a List type that holds arbitrary internal data easily,
            // we use Array<Value>.

            let mut callbacks = match b.get("callbacks") {
                Some(Value::Array(arr)) => arr.borrow().elements.clone(),
                _ => Vec::new(),
            };

            // Encode call info as an array: [callback, child_id]
            let info = Value::Array(vm.get_gc().allocate(ArrayState::new(callback_info))?);
            callbacks.push(info);

            b.insert(
                "callbacks".to_string(),
                Value::Array(vm.get_gc().allocate(ArrayState::new(callbacks))?),
            );
        }
    }

    Ok(child_promise)
}

pub fn async_spawn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("spawn() requires a function"));
    }
    let callee = args[0].clone();
    let call_args = args[1..].to_vec();
    // In this VM, calling an async function already starts it and returns a promise.
    // If it's a synchronous function, it will run to completion here.
    vm.call(callee, call_args)
}

pub fn promise_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let promises = match args.first() {
        Some(Value::Array(arr)) => arr.borrow().clone(),
        _ => return Err(RuntimeError::new("Promise.all() expects array of promises")),
    };

    if promises.is_empty() {
        let gc = vm.get_gc();
        let mut map = IndexMap::new();
        map.insert(
            "variant".to_string(),
            Value::String(Rc::new("Resolved".to_string())),
        );
        map.insert(
            "value".to_string(),
            Value::Array(gc.allocate(ArrayState::new(vec![]))?),
        );
        return Ok(Value::Map(gc.allocate(MapState::new(map))?));
    }

    let (master_id, master_promise) = vm.create_promise()?;
    let mut dependencies = Vec::new();
    let mut results = vec![Value::Null; promises.len()];
    let mut resolved_count = 0;

    for (i, p) in promises.iter().enumerate() {
        if let Value::Map(map) = p {
            let map_borrow = map.borrow();
            let variant = map_borrow.get("variant").and_then(|v| v.as_str());
            let id = map_borrow
                .get("id")
                .and_then(|v| v.as_int())
                .map(|i| i as u64);

            if variant == Some("Resolved") {
                results[i] = map_borrow.get("value").cloned().unwrap_or(Value::Null);
                resolved_count += 1;
            } else if let Some(id) = id {
                dependencies.push(id);
            } else {
                results[i] = p.clone();
                resolved_count += 1;
            }
        } else {
            results[i] = p.clone();
            resolved_count += 1;
        }
    }

    if resolved_count == promises.len() {
        let gc = vm.get_gc();
        let results_val = Value::Array(gc.allocate(ArrayState::new(results))?);
        vm.resolve_promise(master_id, results_val);
    } else {
        vm.register_multi_waiter(MultiWaiter {
            promise_id: master_id,
            dependencies,
            resolved_count,
            results,
            mode: MultiWaiterMode::All,
        });
    }

    Ok(master_promise)
}

pub fn promise_race(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let promises = match args.first() {
        Some(Value::Array(arr)) => arr.borrow().clone(),
        _ => {
            return Err(RuntimeError::new(
                "Promise.race() expects array of promises",
            ));
        }
    };

    if promises.is_empty() {
        return Err(RuntimeError::new("Promise.race() called on empty array"));
    }

    for p in &promises {
        let mut is_promise = false;
        if let Value::Map(map) = p {
            let map_borrow = map.borrow();
            let variant = map_borrow.get("variant").and_then(|v| v.as_str());
            if variant == Some("Resolved") {
                let val = map_borrow.get("value").cloned().unwrap_or(Value::Null);
                let (master_id, master_promise) = vm.create_promise()?;
                vm.resolve_promise(master_id, val);
                return Ok(master_promise);
            } else if variant == Some("Pending") {
                is_promise = true;
            }
        }

        if !is_promise {
            let (master_id, master_promise) = vm.create_promise()?;
            vm.resolve_promise(master_id, p.clone());
            return Ok(master_promise);
        }
    }

    let (master_id, master_promise) = vm.create_promise()?;
    let mut dependencies = Vec::new();

    for p in &promises {
        if let Value::Map(map) = p
            && let Some(id) = map.borrow().get("id").and_then(|v| v.as_int())
        {
            dependencies.push(id as u64);
        }
    }

    if dependencies.is_empty() {
        let val = promises[0].clone();
        vm.resolve_promise(master_id, val);
    } else {
        vm.register_multi_waiter(MultiWaiter {
            promise_id: master_id,
            dependencies,
            resolved_count: 0,
            results: Vec::new(),
            mode: MultiWaiterMode::Race,
        });
    }

    Ok(master_promise)
}

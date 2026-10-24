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
    // An executor that throws rejects the promise.
    if let Err(e) = vm.call(executor, vec![resolve_fn, reject_fn]) {
        let reason = e
            .message
            .strip_prefix("uncaught exception: ")
            .unwrap_or(&e.message)
            .to_string();
        vm.reject_promise(id, Value::String(Rc::new(reason)));
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
    if args.is_empty() {
        return Err(RuntimeError::new("reject expects receiver"));
    }
    let receiver = &args[0];
    let reason = args.get(1).cloned().unwrap_or(Value::Null);

    if let Value::Map(map) = receiver {
        let b = map.borrow();
        if let Some(Value::Int(id)) = b.get("id") {
            vm.reject_promise(*id as u64, reason);
        }
    }
    Ok(Value::Null)
}

pub fn promise_then(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args: [receiver, on_fulfilled, on_rejected?]
    if args.len() < 2 {
        return Err(RuntimeError::new("then() requires callback"));
    }
    let on_rejected = args.get(2).cloned().unwrap_or(Value::Null);
    register_callback(vm, &args[0], args[1].clone(), on_rejected, 0)
}

pub fn promise_catch(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args: [receiver, on_rejected]
    if args.len() < 2 {
        return Err(RuntimeError::new("catch() requires callback"));
    }
    register_callback(vm, &args[0], Value::Null, args[1].clone(), 0)
}

pub fn promise_finally(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args: [receiver, on_finally]
    if args.len() < 2 {
        return Err(RuntimeError::new("finally() requires callback"));
    }
    register_callback(vm, &args[0], args[1].clone(), Value::Null, 1)
}

/// `Promise.resolve(value)`: a promise already fulfilled with `value`.
pub fn promise_resolve_static(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let (id, promise) = vm.create_promise()?;
    vm.resolve_promise(id, args.first().cloned().unwrap_or(Value::Null));
    Ok(promise)
}

/// `Promise.reject(reason)`: a promise already rejected with `reason`.
pub fn promise_reject_static(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let (id, promise) = vm.create_promise()?;
    vm.reject_promise(id, args.first().cloned().unwrap_or(Value::Null));
    Ok(promise)
}

/// Registers a `then`/`catch`/`finally` callback on `parent` and returns the
/// dependent promise. `kind` is 0 for then/catch and 1 for finally.
fn register_callback(
    vm: &mut dyn VMProvider,
    parent: &Value,
    on_fulfilled: Value,
    on_rejected: Value,
    kind: i64,
) -> Result<Value, RuntimeError> {
    let (child_id, child_promise) = vm.create_promise()?;
    let info = vec![
        on_fulfilled,
        Value::Int(child_id as i64),
        on_rejected,
        Value::Int(kind),
    ];

    let Value::Map(map) = parent else {
        return Ok(child_promise);
    };

    let settled = {
        let b = map.borrow();
        match b.get("variant").and_then(|v| v.as_str()) {
            Some("Resolved") => Some((b.get("value").cloned().unwrap_or(Value::Null), false)),
            Some("Rejected") => Some((b.get("value").cloned().unwrap_or(Value::Null), true)),
            _ => None,
        }
    };

    if let Some((value, rejected)) = settled {
        if rejected && let Some(Value::Int(id)) = map.borrow().get("id") {
            vm.mark_rejection_handled(*id as u64);
        }
        vm.run_promise_callback(&info, &value, rejected);
    } else {
        // Stored on the promise; the VM runs these when it settles.
        let mut callbacks = match map.borrow().get("callbacks") {
            Some(Value::Array(arr)) => arr.borrow().elements.clone(),
            _ => Vec::new(),
        };
        callbacks.push(Value::Array(vm.get_gc().allocate(ArrayState::new(info))?));
        let callbacks = Value::Array(vm.get_gc().allocate(ArrayState::new(callbacks))?);
        map.borrow_mut().insert("callbacks".to_string(), callbacks);
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

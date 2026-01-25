use pleno_value::{
    ArrayState, MapState, MultiWaiter, MultiWaiterMode, RuntimeError, VMProvider, Value,
};
use std::collections::HashMap;
use std::rc::Rc;

pub fn promise_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Promise() requires a callback"));
    }
    let mut promise_map = HashMap::new();
    promise_map.insert(
        "variant".to_string(),
        Value::String(Rc::new("Pending".to_string())),
    );
    promise_map.insert("value".to_string(), Value::Null);
    Ok(Value::Map(gc.allocate(MapState::new(promise_map))))
}

pub fn promise_then(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("then() requires callback"));
    }
    Ok(args[0].clone())
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
        let mut map = HashMap::new();
        map.insert(
            "variant".to_string(),
            Value::String(Rc::new("Resolved".to_string())),
        );
        map.insert(
            "value".to_string(),
            Value::Array(gc.allocate(ArrayState::new(vec![]))),
        );
        return Ok(Value::Map(gc.allocate(MapState::new(map))));
    }

    let (master_id, master_promise) = vm.create_promise();
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
        let results_val = Value::Array(gc.allocate(ArrayState::new(results)));
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
                let (master_id, master_promise) = vm.create_promise();
                vm.resolve_promise(master_id, val);
                return Ok(master_promise);
            } else if variant == Some("Pending") {
                is_promise = true;
            }
        }

        if !is_promise {
            let (master_id, master_promise) = vm.create_promise();
            vm.resolve_promise(master_id, p.clone());
            return Ok(master_promise);
        }
    }

    let (master_id, master_promise) = vm.create_promise();
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

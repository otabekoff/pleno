use pleno_value::{ArrayState, MapState, RuntimeError, VMProvider, Value, ValueKey};
use std::rc::Rc;

pub fn map_put(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("put() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => return Err(RuntimeError::new("put() expects string key")),
    };
    let value = match args.get(2) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("put() expects value")),
    };
    map.borrow_mut().insert(key, value);
    Ok(Value::Null)
}

pub fn map_get(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("get() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("get() expects string key")),
    };
    Ok(map.borrow().get(key).cloned().unwrap_or(Value::Null))
}

pub fn map_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("size() expects Map")),
    };
    Ok(Value::Int(map.borrow().len() as i64))
}

pub fn map_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("isEmpty() expects Map")),
    };
    Ok(Value::Bool(map.borrow().is_empty()))
}

pub fn map_keys(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("keys() expects Map")),
    };
    let keys: Vec<Value> = map
        .borrow()
        .keys()
        .map(|k| Value::String(Rc::new(k.clone())))
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(keys))))
}

pub fn map_values(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("values() expects Map")),
    };
    let values: Vec<Value> = map.borrow().values().cloned().collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))))
}

pub fn map_entries(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("entries() expects Map")),
    };
    let mut entries = Vec::new();
    for (k, v) in map.borrow().iter() {
        let entry = vec![Value::String(Rc::new(k.clone())), v.clone()];
        entries.push(Value::Array(vm.get_gc().allocate(ArrayState::new(entry))));
    }
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(entries))))
}

pub fn map_contains_key(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("has() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("has() expects string key")),
    };
    Ok(Value::Bool(map.borrow().contains_key(key)))
}

pub fn map_remove(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("delete() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("delete() expects string key")),
    };
    let removed = map.borrow_mut().remove(key).is_some();
    Ok(Value::Bool(removed))
}

pub fn map_clear(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("clear() expects Map")),
    };
    map.borrow_mut().clear();
    Ok(Value::Null)
}

pub fn map_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("map() expects Map")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("map() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let entries: Vec<(String, Value)> = map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut results = std::collections::HashMap::new();
    for (key, value) in entries {
        let res = vm.call(
            callback.clone(),
            vec![Value::String(Rc::new(key.clone())), value],
        )?;
        results.insert(key, res);
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))))
}

pub fn map_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("filter() expects Map")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("filter() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let entries: Vec<(String, Value)> = map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut results = std::collections::HashMap::new();
    for (key, value) in entries {
        let should_include = vm.call(
            callback.clone(),
            vec![Value::String(Rc::new(key.clone())), value.clone()],
        )?;
        if matches!(should_include, Value::Bool(true)) {
            results.insert(key, value);
        }
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))))
}

pub fn map_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("forEach() expects Map")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("forEach() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let entries: Vec<(String, Value)> = map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, value) in entries {
        vm.call(callback.clone(), vec![Value::String(Rc::new(key)), value])?;
    }
    Ok(Value::Null)
}

// Set methods are here too
pub fn set_add(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("add() expects 1 argument"));
    }
    let key = ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    set.borrow_mut().insert(key);
    Ok(Value::Null)
}

pub fn set_remove(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("remove() expects 1 argument"));
    }
    let key = ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    set.borrow_mut().remove(&key);
    Ok(Value::Null)
}

pub fn set_clear(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow_mut().clear();
    Ok(Value::Null)
}

pub fn set_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let key = ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    Ok(Value::Bool(set.borrow().contains(&key)))
}

pub fn set_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("size() expects Set")),
    };
    Ok(Value::Int(set.borrow().len() as i64))
}

pub fn set_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("isEmpty() expects Set")),
    };
    Ok(Value::Bool(set.borrow().is_empty()))
}

pub fn set_union(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("union() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("union() expects second Set")),
    };
    let mut result = set1.borrow().clone();
    for item in set2.borrow().iter() {
        result.insert(item.clone());
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

pub fn set_intersection(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("intersection() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("intersection() expects second Set")),
    };
    let mut result = std::collections::HashSet::new();
    let s2 = set2.borrow();
    for item in set1.borrow().iter() {
        if s2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

pub fn set_difference(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("difference() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("difference() expects second Set")),
    };
    let mut result = std::collections::HashSet::new();
    let s2 = set2.borrow();
    for item in set1.borrow().iter() {
        if !s2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

pub fn set_to_array(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("toArray() expects Set")),
    };
    let values: Vec<Value> = set
        .borrow()
        .iter()
        .map(|key| match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(v) => Value::String(Rc::clone(v)),
        })
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))))
}

pub fn set_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("map() expects Set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("map() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let items: Vec<Value> = set
        .borrow()
        .iter()
        .map(|key| match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(v) => Value::String(Rc::clone(v)),
        })
        .collect();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let res = vm.call(callback.clone(), vec![item])?;
        results.push(res);
    }
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
}

pub fn set_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("filter() expects Set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("filter() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let mut result = std::collections::HashSet::new();
    for key in set.borrow().iter() {
        let item = match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(v) => Value::String(Rc::clone(v)),
        };
        let should_include = vm.call(callback.clone(), vec![item])?;
        if matches!(should_include, Value::Bool(true)) {
            result.insert(key.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

pub fn set_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("forEach() expects Set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("forEach() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    for key in set.borrow().iter() {
        let item = match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(v) => Value::String(Rc::clone(v)),
        };
        vm.call(callback.clone(), vec![item])?;
    }
    Ok(Value::Null)
}

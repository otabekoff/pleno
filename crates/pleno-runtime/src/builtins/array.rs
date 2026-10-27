use pleno_value::{Value, VMProvider, RuntimeError, ArrayState};
use std::rc::Rc;

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Missing => false,
        Value::Bool(value) => *value,
        _ => true,
    }
}

pub fn array_push(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("push() expects 1 argument"));
    }
    let val = args[1].clone();
    arr.borrow_mut().push(val);
    Ok(Value::Null)
}

pub fn array_pop(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    Ok(arr.borrow_mut().pop().unwrap_or(Value::Null))
}

pub fn array_clear(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow_mut().clear();
    Ok(Value::Null)
}

pub fn array_slice(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let array = arr.borrow();
    let len = array.len();

    let position = |arg: Option<&Value>| match arg {
        Some(Value::Int(i)) => Some(*i),
        _ => None,
    };
    // Like `arr[start..end]`: a negative position counts from the end.
    let (start, end) =
        pleno_value::text::slice_bounds(len, position(args.get(1)), position(args.get(2)), false);

    let slice: Vec<Value> = array[start..end].to_vec();
    Ok(Value::Array(gc.allocate(ArrayState::new(slice))))
}

pub fn array_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("map() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let res = vm.call(callback.clone(), vec![item])?;
        results.push(res);
    }
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
}

pub fn array_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("filter() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    let mut results = Vec::new();
    for item in items {
        let res = vm.call(callback.clone(), vec![item.clone()])?;
        if is_truthy(&res) {
            results.push(item);
        }
    }
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
}

pub fn array_reduce(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "reduce() expects at least 1 argument (callback)",
        ));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    if items.is_empty() {
        if args.len() >= 3 {
            return Ok(args[2].clone());
        } else {
            return Err(RuntimeError::new(
                "reduce() of empty array with no initial value",
            ));
        }
    }
    let (mut accumulator, start_idx) = if args.len() >= 3 {
        (args[2].clone(), 0)
    } else {
        (items[0].clone(), 1)
    };
    for item in items.iter().skip(start_idx) {
        accumulator = vm.call(callback.clone(), vec![accumulator, item.clone()])?;
    }
    Ok(accumulator)
}

pub fn array_reverse(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut array = arr.borrow_mut();
    array.reverse();
    Ok(Value::Null)
}

pub fn array_sort(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut items = arr.borrow().clone();
    items.sort_by_key(|a| a.to_string());
    Ok(Value::Array(vm.get_gc().allocate(items)))
}

pub fn array_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let array = arr.borrow();
    let search_value = &args[1];
    Ok(Value::Bool(array.contains(search_value)))
}

pub fn array_index_of(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("indexOf() expects 1 argument"));
    }
    let array = arr.borrow();
    let search_value = &args[1];
    for (i, item) in array.iter().enumerate() {
        if item == search_value {
            return Ok(Value::Int(i as i64));
        }
    }
    Ok(Value::Int(-1)) // Not found
}

pub fn array_shift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("shift() expects Array")),
    };
    let mut array = arr.borrow_mut();
    if array.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(array.remove(0))
    }
}

pub fn array_unshift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("unshift() expects Array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("unshift() expects value"));
    }
    arr.borrow_mut().insert(0, args[1].clone());
    Ok(Value::Null)
}

pub fn array_join(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("join() expects Array")),
    };
    let sep = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Null) | None => ",",
        _ => return Err(RuntimeError::new("join() expects string separator")),
    };
    let array = arr.borrow();
    let s: Vec<String> = array.iter().map(|v| v.to_string()).collect();
    Ok(Value::String(Rc::new(s.join(sep))))
}

pub fn array_find(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("find() expects Array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("find() expects callback"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    for item in items {
        let res = vm.call(callback.clone(), vec![item.clone()])?;
        if let Value::Bool(true) = res {
            return Ok(item);
        }
    }
    Ok(Value::Null)
}

pub fn array_insert(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("insert() expects Array")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("insert() expects index")),
    };
    let value = match args.get(2) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("insert() expects value")),
    };
    let mut array = arr.borrow_mut();
    if index > array.len() {
        return Err(RuntimeError::new("insert() index out of bounds"));
    }
    array.insert(index, value);
    Ok(Value::Null)
}

pub fn array_remove_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("remove() expects Array")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("remove() expects index")),
    };
    let mut array = arr.borrow_mut();
    if index >= array.len() {
        return Err(RuntimeError::new("remove() index out of bounds"));
    }
    Ok(array.remove(index))
}

pub fn array_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("isEmpty() expects Array")),
    };
    Ok(Value::Bool(arr.borrow().is_empty()))
}

pub fn array_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("forEach() expects Array")),
    };
    let callback = match args.get(1) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("forEach() expects callback")),
    };
    let items = arr.borrow().clone();
    for item in items {
        vm.call(callback.clone(), vec![item])?;
    }
    Ok(Value::Null)
}

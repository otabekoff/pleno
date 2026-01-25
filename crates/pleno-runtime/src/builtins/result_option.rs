use pleno_value::{Value, VMProvider, RuntimeError, MapState};
use std::collections::HashMap;
use std::rc::Rc;

// Result methods
pub fn result_unwrap(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrap() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => map_ref
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeError::new("Result::Ok missing value")),
        Some(Value::String(variant)) if variant.as_str() == "Err" => {
            Err(RuntimeError::new("called unwrap() on Result::Err"))
        }
        _ => Err(RuntimeError::new("invalid Result value")),
    }
}

pub fn result_unwrap_or(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("unwrapOr() requires a default value"));
    }
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrapOr() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => Ok(map_ref
            .get("value")
            .cloned()
            .unwrap_or_else(|| args[1].clone())),
        _ => Ok(args[1].clone()), // Return default for Err
    }
}

pub fn result_expect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("expect() requires a message"));
    }
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("expect() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => {
            map_ref.get("value").cloned().ok_or_else(|| {
                let msg = match &args[1] {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "Result::Err".to_string(),
                };
                RuntimeError::new(&msg)
            })
        }
        _ => {
            let msg = match &args[1] {
                Value::String(s) => s.as_str().to_string(),
                _ => "Result::Err".to_string(),
            };
            Err(RuntimeError::new(&msg))
        }
    }
}

pub fn result_is_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isOk() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Ok")),
        _ => Ok(Value::Bool(false)),
    }
}

pub fn result_is_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isErr() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Err")),
        _ => Ok(Value::Bool(false)),
    }
}

// Option methods
pub fn option_unwrap(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrap() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => map_ref
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeError::new("Option::Some missing value")),
        Some(Value::String(variant)) if variant.as_str() == "None" => {
            Err(RuntimeError::new("called unwrap() on Option::None"))
        }
        _ => Err(RuntimeError::new("invalid Option value")),
    }
}

pub fn option_unwrap_or(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("unwrapOr() requires a default value"));
    }
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrapOr() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => Ok(map_ref
            .get("value")
            .cloned()
            .unwrap_or_else(|| args[1].clone())),
        _ => Ok(args[1].clone()), // Return default for None
    }
}

pub fn option_expect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("expect() requires a message"));
    }
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("expect() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => {
            map_ref.get("value").cloned().ok_or_else(|| {
                let msg = match &args[1] {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "Option::None".to_string(),
                };
                RuntimeError::new(&msg)
            })
        }
        _ => {
            let msg = match &args[1] {
                Value::String(s) => s.as_str().to_string(),
                _ => "Option::None".to_string(),
            };
            Err(RuntimeError::new(&msg))
        }
    }
}

pub fn option_is_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isSome() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Some")),
        _ => Ok(Value::Bool(false)),
    }
}

pub fn option_is_none(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isNone() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "None")),
        _ => Ok(Value::Bool(false)),
    }
}

// Constructors (Ok, Err, Some)
pub fn result_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Ok() requires 1 argument"));
    }
    let mut result = HashMap::new();
    result.insert(
        "variant".to_string(),
        Value::String(Rc::new("Ok".to_string())),
    );
    result.insert("value".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(MapState::new(result))))
}

pub fn result_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Err() requires 1 argument"));
    }
    let mut result = HashMap::new();
    result.insert(
        "variant".to_string(),
        Value::String(Rc::new("Err".to_string())),
    );
    result.insert("error".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(MapState::new(result))))
}

pub fn option_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Some() requires 1 argument"));
    }
    let mut option = HashMap::new();
    option.insert(
        "variant".to_string(),
        Value::String(Rc::new("Some".to_string())),
    );
    option.insert("value".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(MapState::new(option))))
}

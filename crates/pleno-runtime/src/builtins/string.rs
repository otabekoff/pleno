use pleno_value::{Value, VMProvider, RuntimeError, ArrayState};
use std::rc::Rc;

pub fn string_upper(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_uppercase())))
}

pub fn string_lower(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_lowercase())))
}

pub fn string_trim(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.trim().to_string())))
}

pub fn string_split(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("split() expects 1 argument (separator)"));
    }
    let separator = match &args[1] {
        Value::String(sep) => sep.as_str(),
        _ => return Err(RuntimeError::new("split() separator must be string")),
    };
    let parts: Vec<Value> = s
        .split(separator)
        .map(|part| Value::String(Rc::new(part.to_string())))
        .collect();
    Ok(Value::Array(gc.allocate(ArrayState::new(parts))))
}

pub fn string_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let substr = match &args[1] {
        Value::String(sub) => sub.as_str(),
        _ => return Err(RuntimeError::new("contains() argument must be string")),
    };
    Ok(Value::Bool(s.contains(substr)))
}

pub fn string_starts_with(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("startsWith() expects 1 argument"));
    }
    let prefix = match &args[1] {
        Value::String(pre) => pre.as_str(),
        _ => return Err(RuntimeError::new("startsWith() argument must be string")),
    };
    Ok(Value::Bool(s.starts_with(prefix)))
}

pub fn string_ends_with(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("endsWith() expects 1 argument"));
    }
    let suffix = match &args[1] {
        Value::String(suf) => suf.as_str(),
        _ => return Err(RuntimeError::new("endsWith() argument must be string")),
    };
    Ok(Value::Bool(s.ends_with(suffix)))
}

pub fn string_replace(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "replace() expects 2 arguments (old, new)",
        ));
    }
    let old_str = match &args[1] {
        Value::String(old) => old.as_str(),
        _ => return Err(RuntimeError::new("replace() old argument must be string")),
    };
    let new_str = match &args[2] {
        Value::String(new) => new.as_str(),
        _ => return Err(RuntimeError::new("replace() new argument must be string")),
    };
    Ok(Value::String(Rc::new(s.replace(old_str, new_str))))
}

pub fn string_substring(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("substring() expects String")),
    };
    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("substring() expects start index")),
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) | None => s.len(),
        _ => return Err(RuntimeError::new("substring() expects end index or null")),
    };

    if start > s.len() || end > s.len() || start > end {
        return Err(RuntimeError::new("substring() out of bounds"));
    }

    Ok(Value::String(Rc::new(s[start..end].to_string())))
}

pub fn string_char_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("charAt() expects String")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("charAt() expects index")),
    };
    if index >= s.len() {
        return Ok(Value::Null);
    }
    Ok(Value::String(Rc::new(s[index..index + 1].to_string())))
}

pub fn string_index_of(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("indexOf() expects String")),
    };
    let search = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("indexOf() expects string to search")),
    };
    match s.find(search) {
        Some(i) => Ok(Value::Int(i as i64)),
        None => Ok(Value::Int(-1)),
    }
}

pub fn string_repeat(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("repeat() expects String")),
    };
    let count = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("repeat() expects count")),
    };
    Ok(Value::String(Rc::new(s.repeat(count))))
}

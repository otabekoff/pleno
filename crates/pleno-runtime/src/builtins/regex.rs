use pleno_value::{ArrayState, RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn regex_is_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("isMatch() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(text)))
}

pub fn regex_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("match() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    match re.captures(text) {
        Some(caps) => {
            let mut groups = Vec::new();
            for i in 0..caps.len() {
                match caps.get(i) {
                    Some(m) => groups.push(Value::String(Rc::new(m.as_str().to_string()))),
                    None => groups.push(Value::Null),
                }
            }
            Ok(Value::Array(gc.allocate(ArrayState::new(groups))))
        }
        None => Ok(Value::Null),
    }
}

pub fn regex_find_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("findAll() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    let mut matches = Vec::new();

    for caps in re.captures_iter(text) {
        let mut groups = Vec::new();
        for i in 0..caps.len() {
            match caps.get(i) {
                Some(m) => groups.push(Value::String(Rc::new(m.as_str().to_string()))),
                None => groups.push(Value::Null),
            }
        }
        matches.push(Value::Array(gc.allocate(ArrayState::new(groups))));
    }
    Ok(Value::Array(gc.allocate(ArrayState::new(matches))))
}

pub fn regex_replace(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "replace() requires pattern, text, and replacement",
        ));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };
    let replacement = match &args[2] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("replacement must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    let result = re.replace_all(text, replacement);
    Ok(Value::String(Rc::new(result.to_string())))
}

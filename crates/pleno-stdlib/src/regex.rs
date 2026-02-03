use indexmap::IndexMap;
use pleno_value::{ArrayState, BoundNativeMethodState, MapState, RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn regex_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let pattern = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => {
            return Err(RuntimeError::new(
                "Regex constructor expects pattern string",
            ));
        }
    };

    let mut map = IndexMap::new();
    map.insert(
        "variant".to_string(),
        Value::String(Rc::new("Regex".to_string())),
    );
    map.insert("pattern".to_string(), Value::String(pattern));

    let map_val = Value::Map(vm.get_gc().allocate(MapState::new(map))?);

    // Bind methods
    if let Value::Map(state) = &map_val {
        let mut state = state.borrow_mut();
        let gc = vm.get_gc();
        state.entries.insert(
            "test".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_test,
            })?),
        );
        state.entries.insert(
            "match".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_match,
            })?),
        );
        state.entries.insert(
            "findAll".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_find_all,
            })?),
        );
        state.entries.insert(
            "replace".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_replace,
            })?),
        );
    }

    Ok(map_val)
}

pub fn regex_test(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let this = match args.first() {
        Some(Value::Map(m)) => m.borrow(),
        _ => return Err(RuntimeError::new("Regex.test: invalid receiver")),
    };
    let pattern = match this.get("pattern") {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex instance missing pattern")),
    };
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.test expects text string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(text)))
}

pub fn regex_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let this = match args.first() {
        Some(Value::Map(m)) => m.borrow(),
        _ => return Err(RuntimeError::new("Regex.match: invalid receiver")),
    };
    let pattern = match this.get("pattern") {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex instance missing pattern")),
    };
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.match expects text string")),
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
            Ok(Value::Array(gc.allocate(ArrayState::new(groups))?))
        }
        None => Ok(Value::Null),
    }
}

pub fn regex_find_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let this = match args.first() {
        Some(Value::Map(m)) => m.borrow(),
        _ => return Err(RuntimeError::new("Regex.findAll: invalid receiver")),
    };
    let pattern = match this.get("pattern") {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex instance missing pattern")),
    };
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.findAll expects text string")),
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
        matches.push(Value::Array(gc.allocate(ArrayState::new(groups))?));
    }
    Ok(Value::Array(gc.allocate(ArrayState::new(matches))?))
}

pub fn regex_replace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let this = match args.first() {
        Some(Value::Map(m)) => m.borrow(),
        _ => return Err(RuntimeError::new("Regex.replace: invalid receiver")),
    };
    let pattern = match this.get("pattern") {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex instance missing pattern")),
    };
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.replace expects text string")),
    };
    let replacement = match args.get(2) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Regex.replace expects replacement string",
            ));
        }
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    let result = re.replace_all(text, replacement);
    Ok(Value::String(Rc::new(result.to_string())))
}

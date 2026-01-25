use pleno_value::{ArrayState, MapState, RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn json_parse(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let json_str = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("JSON.parse expects string")),
    };

    let serde_val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => return Err(RuntimeError::new(format!("JSON parse error: {}", e))),
    };

    convert_serde_to_value(vm, &serde_val)
}

fn convert_serde_to_value(
    vm: &mut dyn VMProvider,
    val: &serde_json::Value,
) -> Result<Value, RuntimeError> {
    match val {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(RuntimeError::new("Infinite or NaN number in JSON"))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(Rc::new(s.clone()))),
        serde_json::Value::Array(arr) => {
            let mut elements = Vec::new();
            for item in arr {
                elements.push(convert_serde_to_value(vm, item)?);
            }
            Ok(Value::Array(
                vm.get_gc().allocate(ArrayState::new(elements)),
            ))
        }
        serde_json::Value::Object(obj) => {
            let mut entries = std::collections::HashMap::new();
            for (k, v) in obj {
                entries.insert(k.clone(), convert_serde_to_value(vm, v)?);
            }
            Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))))
        }
    }
}

pub fn json_stringify(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    // Rough serialization - better to implement properly later
    // For now, use debug or simple string conversion?
    // Actually we need Value -> serde_json::Value or custom serializer.
    // For v1 MVP, let's just support primitives and arrays/maps if possible, or implement a basic recursive serializer.

    // Quick implementation:
    let s = value_to_json_string(val)?;
    Ok(Value::String(Rc::new(s)))
}

fn value_to_json_string(val: &Value) -> Result<String, RuntimeError> {
    match val {
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Int(i) => Ok(i.to_string()),
        Value::Float(f) => Ok(f.to_string()),
        Value::String(s) => Ok(format!("{:?}", s)), // simple quoting
        Value::Array(arr) => {
            let mut s = String::from("[");
            let arr = arr.borrow();
            for (i, elem) in arr.elements.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&value_to_json_string(elem)?);
            }
            s.push(']');
            Ok(s)
        }
        Value::Map(map) => {
            let mut s = String::from("{");
            let map = map.borrow();
            for (i, (k, v)) in map.entries.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("{:?}:{}", k, value_to_json_string(v)?));
            }
            s.push('}');
            Ok(s)
        }
        _ => Ok("\"<unsupported>\"".to_string()),
    }
}

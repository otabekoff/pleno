use indexmap::IndexMap;
use pleno_value::Value;
use std::rc::Rc;

pub fn json_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let json_str = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("JSON.parse expects string")),
    };

    let serde_val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            return Err(pleno_value::RuntimeError::new(format!(
                "JSON parse error: {}",
                e
            )));
        }
    };

    convert_serde_to_value(vm, &serde_val)
}

pub(crate) fn convert_serde_to_value(
    vm: &mut dyn pleno_value::VMProvider,
    val: &serde_json::Value,
) -> Result<Value, pleno_value::RuntimeError> {
    match val {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(pleno_value::RuntimeError::new(
                    "Infinite or NaN number in JSON",
                ))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(Rc::new(s.clone()))),
        serde_json::Value::Array(arr) => {
            let mut elements = Vec::new();
            for item in arr {
                elements.push(convert_serde_to_value(vm, item)?);
            }
            Ok(Value::Array(
                vm.get_gc()
                    .allocate(pleno_value::ArrayState::new(elements))?,
            ))
        }
        serde_json::Value::Object(obj) => {
            let mut entries = IndexMap::new();
            for (k, v) in obj {
                entries.insert(k.clone(), convert_serde_to_value(vm, v)?);
            }
            Ok(Value::Map(
                vm.get_gc().allocate(pleno_value::MapState::new(entries))?,
            ))
        }
    }
}

pub fn json_stringify(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    // Rough serialization - better to implement properly later
    // For now, use debug or simple string conversion?
    // Actually we need Value -> serde_json::Value or custom serializer.
    // For v1 MVP, let's just support primitives and arrays/maps if possible, or implement a basic recursive serializer.

    // Quick implementation:
    let s = value_to_json_string(val)?;
    Ok(Value::String(Rc::new(s)))
}

/// `JSON.stringifyPretty(value, indent = 2)`: JSON on several lines, nested
/// values indented by `indent` spaces.
pub fn json_stringify_pretty(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    // The indent may be passed by name: `stringifyPretty(value, indent: 4)`.
    let named_indent = vm.get_named_args().get("indent").cloned();
    let indent = match args.get(1).or(named_indent.as_ref()) {
        None | Some(Value::Missing) => 2,
        Some(Value::Int(n)) if (0..=16).contains(n) => *n as usize,
        Some(_) => {
            return Err(pleno_value::RuntimeError::new(
                "stringifyPretty indent must be an Int from 0 to 16",
            ));
        }
    };
    let compact = value_to_json_string(val)?;
    let parsed: serde_json::Value = serde_json::from_str(&compact)
        .map_err(|e| pleno_value::RuntimeError::new(format!("stringifyPretty failed: {e}")))?;
    let pretty = serde_json::to_string_pretty(&parsed)
        .map_err(|e| pleno_value::RuntimeError::new(format!("stringifyPretty failed: {e}")))?;
    // serde_json indents by two spaces; JSON strings never contain a raw
    // newline, so re-indenting line by line is safe.
    let unit = " ".repeat(indent);
    let lines: Vec<String> = pretty
        .lines()
        .map(|line| {
            let body = line.trim_start_matches(' ');
            let depth = (line.len() - body.len()) / 2;
            format!("{}{}", unit.repeat(depth), body)
        })
        .collect();
    Ok(Value::String(Rc::new(lines.join("\n"))))
}

pub(crate) fn value_to_json_string(val: &Value) -> Result<String, pleno_value::RuntimeError> {
    match val {
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Int(i) => Ok(i.to_string()),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                Ok("null".to_string())
            } else {
                Ok(f.to_string())
            }
        }
        Value::String(s) => {
            // Use serde_json for proper string escaping
            Ok(serde_json::to_string(s.as_str()).unwrap_or_else(|_| format!("\"{}\"", s)))
        }
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
                // Keys in JSON must be strings
                s.push_str(&serde_json::to_string(k).unwrap_or_else(|_| format!("\"{}\"", k)));
                s.push(':');
                s.push_str(&value_to_json_string(v)?);
            }
            s.push('}');
            Ok(s)
        }
        _ => Ok("null".to_string()), // Functions, Classes, etc. serialize to null in JSON
    }
}

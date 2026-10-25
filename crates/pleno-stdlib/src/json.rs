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

/// How deeply `stringify` follows nested values before it stops; a value that
/// contains itself would otherwise recurse until the stack overflows.
const MAX_JSON_DEPTH: usize = 512;

pub(crate) fn value_to_json_string(val: &Value) -> Result<String, pleno_value::RuntimeError> {
    let mut out = String::new();
    write_json(val, &mut out, 0)?;
    Ok(out)
}

fn write_json_key(key: &str, out: &mut String) {
    out.push_str(&serde_json::to_string(key).unwrap_or_else(|_| format!("\"{}\"", key)));
    out.push(':');
}

fn write_json(
    val: &Value,
    out: &mut String,
    depth: usize,
) -> Result<(), pleno_value::RuntimeError> {
    if depth > MAX_JSON_DEPTH {
        return Err(pleno_value::RuntimeError::new(
            "JSON.stringify: value is nested too deeply (does it contain itself?)",
        ));
    }
    match val {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(&b.to_string()),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                out.push_str("null");
            } else {
                out.push_str(&f.to_string());
            }
        }
        Value::String(s) => {
            // Use serde_json for proper string escaping
            out.push_str(
                &serde_json::to_string(s.as_str()).unwrap_or_else(|_| format!("\"{}\"", s)),
            )
        }
        Value::Char(c) => out.push_str(&serde_json::to_string(&c.to_string()).unwrap_or_default()),
        Value::Array(arr) => {
            out.push('[');
            let elements = arr.borrow().elements.clone();
            for (i, elem) in elements.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(elem, out, depth + 1)?;
            }
            out.push(']');
        }
        // A set is written as an array of its elements, in insertion order.
        Value::Set(set) => {
            out.push('[');
            let elements: Vec<Value> = set.borrow().elements.iter().map(|k| k.to_value()).collect();
            for (i, elem) in elements.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(elem, out, depth + 1)?;
            }
            out.push(']');
        }
        Value::Map(map) => {
            out.push('{');
            let entries: Vec<(String, Value)> = map
                .borrow()
                .entries
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_key(k, out);
                write_json(v, out, depth + 1)?;
            }
            out.push('}');
        }
        // A class instance is written as an object of its public fields, in
        // the order they were assigned. Private, protected and internal
        // fields are left out.
        Value::Object(object) => {
            out.push('{');
            let object = object.borrow();
            let fields: Vec<(String, Value)> = object
                .fields
                .iter()
                .filter(|(name, _)| is_public_field(object.class.as_ref(), name))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            drop(object);
            for (i, (k, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_key(k, out);
                write_json(v, out, depth + 1)?;
            }
            out.push('}');
        }
        // Functions, classes and other values have no JSON form.
        _ => out.push_str("null"),
    }
    Ok(())
}

/// Whether instance field `name` is public: the nearest class in the chain
/// that declares it says so, or no class declares it.
fn is_public_field(class: Option<&pleno_value::gc::GcRc<pleno_value::Class>>, name: &str) -> bool {
    let mut current = class.cloned();
    while let Some(candidate) = current {
        if let Some(visibility) = candidate.borrow().fields.get(name) {
            return matches!(visibility, pleno_value::MethodVisibility::Public);
        }
        current = candidate.borrow().superclass.clone();
    }
    true
}

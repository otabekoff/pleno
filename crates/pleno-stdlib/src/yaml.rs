use indexmap::IndexMap;
use pleno_value::{RuntimeError, Value};
use std::rc::Rc;

pub fn yaml_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("yaml.parse expects string")),
    };

    let yaml_val: serde_yaml::Value = serde_yaml::from_str(input)
        .map_err(|e| RuntimeError::new(format!("YAML parse error: {}", e)))?;

    convert_yaml_to_value(vm, &yaml_val)
}

pub fn yaml_stringify(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    let yaml_val = convert_value_to_yaml_value(val)?;
    match serde_yaml::to_string(&yaml_val) {
        Ok(s) => Ok(Value::String(Rc::new(s))),
        Err(e) => Err(RuntimeError::new(format!("YAML stringify error: {}", e))),
    }
}

fn convert_value_to_yaml_value(val: &Value) -> Result<serde_yaml::Value, RuntimeError> {
    match val {
        Value::Null => Ok(serde_yaml::Value::Null),
        Value::Bool(b) => Ok(serde_yaml::Value::Bool(*b)),
        Value::Int(i) => Ok(serde_yaml::Value::Number((*i).into())),
        Value::Float(f) => Ok(serde_yaml::Value::Number(serde_yaml::Number::from(*f))),
        Value::String(s) => Ok(serde_yaml::Value::String(s.as_str().to_string())),
        Value::Array(arr) => {
            let mut seq = Vec::new();
            for item in &arr.borrow().elements {
                seq.push(convert_value_to_yaml_value(item)?);
            }
            Ok(serde_yaml::Value::Sequence(seq))
        }
        Value::Map(map) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in &map.borrow().entries {
                mapping.insert(
                    serde_yaml::Value::String(k.clone()),
                    convert_value_to_yaml_value(v)?,
                );
            }
            Ok(serde_yaml::Value::Mapping(mapping))
        }
        _ => Err(RuntimeError::new(format!(
            "Cannot serialize {} to YAML",
            val.type_name()
        ))),
    }
}

fn convert_yaml_to_value(
    vm: &mut dyn pleno_value::VMProvider,
    val: &serde_yaml::Value,
) -> Result<Value, RuntimeError> {
    match val {
        serde_yaml::Value::Null => Ok(Value::Null),
        serde_yaml::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(RuntimeError::new("Infinite or NaN number in YAML"))
            }
        }
        serde_yaml::Value::String(s) => Ok(Value::String(Rc::new(s.clone()))),
        serde_yaml::Value::Sequence(seq) => {
            let mut elements = Vec::new();
            for item in seq {
                elements.push(convert_yaml_to_value(vm, item)?);
            }
            Ok(Value::Array(
                vm.get_gc()
                    .allocate(pleno_value::ArrayState::new(elements))?,
            ))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut entries = IndexMap::new();
            for (k, v) in map {
                // Keys in Pleno maps must be strings for now?
                // Pleno maps support any value as key ideally, but implementation might strict.
                // Value::Map uses IndexMap<String, Value>. So keys MUST be strings.
                let key_str = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    _ => {
                        return Err(RuntimeError::new(
                            "YAML map keys must be strings (or convertible)",
                        ));
                    }
                };
                entries.insert(key_str, convert_yaml_to_value(vm, v)?);
            }
            Ok(Value::Map(
                vm.get_gc().allocate(pleno_value::MapState::new(entries))?,
            ))
        }
        serde_yaml::Value::Tagged(tv) => {
            // Treat tagged values as their inner value for now, or maybe wrap?
            // Just unwrap for now.
            convert_yaml_to_value(vm, &tv.value)
        }
    }
}

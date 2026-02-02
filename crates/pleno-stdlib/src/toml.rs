use indexmap::IndexMap;
use pleno_value::{RuntimeError, Value};
use std::rc::Rc;
use toml::Value as TomlValue;

pub fn toml_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("toml.parse expects string")),
    };

    let toml_val: TomlValue = input
        .parse()
        .map_err(|e| RuntimeError::new(format!("TOML parse error: {}", e)))?;

    convert_toml_to_value(vm, &toml_val)
}

pub fn toml_stringify(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    let toml_val = convert_value_to_toml_value(val)?;
    match toml::to_string(&toml_val) {
        Ok(s) => Ok(Value::String(Rc::new(s))),
        Err(e) => Err(RuntimeError::new(format!("TOML stringify error: {}", e))),
    }
}

fn convert_value_to_toml_value(val: &Value) -> Result<TomlValue, RuntimeError> {
    match val {
        Value::Bool(b) => Ok(TomlValue::Boolean(*b)),
        Value::Int(i) => Ok(TomlValue::Integer(*i)),
        Value::Float(f) => Ok(TomlValue::Float(*f)),
        Value::String(s) => Ok(TomlValue::String(s.as_str().to_string())),
        Value::Array(arr) => {
            let mut seq = Vec::new();
            for item in &arr.borrow().elements {
                seq.push(convert_value_to_toml_value(item)?);
            }
            Ok(TomlValue::Array(seq))
        }
        Value::Map(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in &map.borrow().entries {
                table.insert(k.clone(), convert_value_to_toml_value(v)?);
            }
            Ok(TomlValue::Table(table))
        }
        _ => Err(RuntimeError::new(format!(
            "Cannot serialize {} to TOML",
            val.type_name()
        ))),
    }
}

fn convert_toml_to_value(
    vm: &mut dyn pleno_value::VMProvider,
    val: &TomlValue,
) -> Result<Value, RuntimeError> {
    match val {
        TomlValue::String(s) => Ok(Value::String(Rc::new(s.clone()))),
        TomlValue::Integer(i) => Ok(Value::Int(*i)),
        TomlValue::Float(f) => Ok(Value::Float(*f)),
        TomlValue::Boolean(b) => Ok(Value::Bool(*b)),
        TomlValue::Datetime(d) => Ok(Value::String(Rc::new(d.to_string()))), // TOML datetime -> String for now
        TomlValue::Array(arr) => {
            let mut elements = Vec::new();
            for item in arr {
                elements.push(convert_toml_to_value(vm, item)?);
            }
            Ok(Value::Array(
                vm.get_gc()
                    .allocate(pleno_value::ArrayState::new(elements))?,
            ))
        }
        TomlValue::Table(table) => {
            let mut entries = IndexMap::new();
            for (k, v) in table {
                entries.insert(k.clone(), convert_toml_to_value(vm, v)?);
            }
            Ok(Value::Map(
                vm.get_gc().allocate(pleno_value::MapState::new(entries))?,
            ))
        }
    }
}

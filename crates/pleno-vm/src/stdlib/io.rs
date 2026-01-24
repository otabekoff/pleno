use crate::Value;
use std::fs;
use std::rc::Rc;

pub fn io_file_read(
    _vm: &mut dyn crate::VMProvider,
    args: &[Value],
) -> Result<Value, crate::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(crate::RuntimeError::new("File.read expects path string")),
    };

    match fs::read_to_string(path) {
        Ok(content) => Ok(Value::String(Rc::new(content))),
        Err(e) => Err(crate::RuntimeError::new(format!("File.read failed: {}", e))),
    }
}

pub fn io_file_write(
    _vm: &mut dyn crate::VMProvider,
    args: &[Value],
) -> Result<Value, crate::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(crate::RuntimeError::new("File.write expects path string")),
    };
    let content = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(crate::RuntimeError::new(
                "File.write expects content string",
            ));
        }
    };

    match fs::write(path, content) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(crate::RuntimeError::new(format!(
            "File.write failed: {}",
            e
        ))),
    }
}

pub fn io_file_exists(
    _vm: &mut dyn crate::VMProvider,
    args: &[Value],
) -> Result<Value, crate::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(crate::RuntimeError::new("File.exists expects path string")),
    };

    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

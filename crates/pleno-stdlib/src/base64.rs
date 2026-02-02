use ::base64::{Engine as _, engine::general_purpose};
use pleno_value::{RuntimeError, Value};
use std::rc::Rc;

pub fn base64_encode(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("base64.encode expects string")),
    };
    let encoded = general_purpose::STANDARD.encode(input);
    Ok(Value::String(Rc::new(encoded)))
}

pub fn base64_decode(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("base64.decode expects string")),
    };
    match general_purpose::STANDARD.decode(input) {
        Ok(bytes) => {
            // Try to convert to UTF-8 string
            match String::from_utf8(bytes) {
                Ok(s) => Ok(Value::String(Rc::new(s))),
                Err(_) => Err(RuntimeError::new(
                    "base64.decode result is not valid UTF-8 string",
                )),
            }
        }
        Err(e) => Err(RuntimeError::new(format!("base64.decode failed: {}", e))),
    }
}

use pleno_value::{RuntimeError, Value};
use std::rc::Rc;
use uuid::Uuid;

pub fn uuid_v4(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    let id = Uuid::new_v4();
    Ok(Value::String(Rc::new(id.to_string())))
}

pub fn uuid_v7(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    let id = Uuid::now_v7();
    Ok(Value::String(Rc::new(id.to_string())))
}

pub fn uuid_parse(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("uuid.parse expects string")),
    };

    match Uuid::parse_str(input) {
        Ok(id) => Ok(Value::String(Rc::new(id.to_string()))),
        Err(e) => Err(RuntimeError::new(format!("Invalid UUID: {}", e))),
    }
}

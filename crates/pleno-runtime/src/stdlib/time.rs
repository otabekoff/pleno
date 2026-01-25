use pleno_value::{RuntimeError, VMProvider, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn time_now(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let start = SystemTime::now();
    let since_epoch = start
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeError::new("Time went backwards"))?;
    Ok(Value::Float(since_epoch.as_secs_f64()))
}

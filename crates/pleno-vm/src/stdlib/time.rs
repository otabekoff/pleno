use crate::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn time_now(
    _vm: &mut dyn crate::VMProvider,
    _args: &[Value],
) -> Result<Value, crate::RuntimeError> {
    let start = SystemTime::now();
    let since_epoch = start
        .duration_since(UNIX_EPOCH)
        .map_err(|_| crate::RuntimeError::new("Time went backwards"))?;
    Ok(Value::Float(since_epoch.as_secs_f64()))
}

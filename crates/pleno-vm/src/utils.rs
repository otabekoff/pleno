use pleno_value::{RuntimeError, Value};

#[inline]
pub fn decode_offset(b: u8, c: u8) -> i16 {
    let hi = (b as i16) << 8;
    let lo = c as i16;
    hi | lo
}

#[inline]
pub fn apply_offset(ip: usize, offset: i16) -> Result<usize, RuntimeError> {
    let next = ip as isize + offset as isize;
    if next < 0 {
        return Err(RuntimeError::new("jump offset out of bounds"));
    }
    Ok(next as usize)
}

pub fn expect_int_or_null(value: &Value) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Int(value) => Ok(Some(*value)),
        Value::Null => Ok(None),
        _ => Err(RuntimeError::new("expected integer or null")),
    }
}

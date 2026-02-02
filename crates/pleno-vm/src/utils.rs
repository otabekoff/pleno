use pleno_value::{RuntimeError, Value};

pub fn decode_offset(b: u8, c: u8) -> i16 {
    let hi = (b as i16) << 8;
    let lo = c as i16;
    hi | lo
}

pub fn apply_offset(ip: usize, offset: i16) -> Result<usize, RuntimeError> {
    let next = ip as isize + offset as isize;
    if next < 0 {
        return Err(RuntimeError::new("jump offset out of bounds"));
    }
    Ok(next as usize)
}

pub fn resolve_index(index: Option<i64>, len: i64, default_val: i64) -> usize {
    let idx = index.unwrap_or(default_val);
    if idx < 0 {
        (len + idx).max(0) as usize
    } else {
        idx.min(len) as usize
    }
}

pub fn expect_int_or_null(value: &Value) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Int(value) => Ok(Some(*value)),
        Value::Null => Ok(None),
        _ => Err(RuntimeError::new("expected integer or null")),
    }
}

pub fn array_slice(
    elements: &[Value],
    start: Option<Value>,
    end: Option<Value>,
    step: Option<Value>,
) -> Result<Vec<Value>, RuntimeError> {
    let len = elements.len() as i64;
    let step = match step {
        Some(Value::Int(s)) => s,
        Some(Value::Null) | None => 1,
        _ => return Err(RuntimeError::new("step must be integer")),
    };

    if step == 0 {
        return Err(RuntimeError::new("slice step cannot be zero"));
    }

    let start = match start {
        Some(Value::Int(s)) => resolve_index(Some(s), len, if step > 0 { 0 } else { len - 1 }),
        Some(Value::Null) | None => {
            if step > 0 {
                0
            } else {
                (len - 1) as usize
            }
        }
        _ => return Err(RuntimeError::new("start must be integer")),
    };

    let end = match end {
        Some(Value::Int(e)) => resolve_index(Some(e), len, if step > 0 { len } else { -1 }),
        Some(Value::Null) | None => {
            if step > 0 {
                len as usize
            } else {
                // For negative step, if end is null, it should go past the first element
                0 // resolve_index(-1, len, -1) = len-1... wait.
            }
        }
        _ => return Err(RuntimeError::new("end must be integer")),
    };

    let mut result = Vec::new();
    if step > 0 {
        let mut i = start;
        while i < end && i < elements.len() {
            result.push(elements[i].clone());
            i += step as usize;
        }
    } else {
        // Correct handling for negative step if end is null/omitted is tricky.
        // If end is null and step < 0, it should include elements down to index 0.
        let end_limit = match end {
            _ if end == 0 && step < 0 => -1, // Include index 0
            _ => end as i64,
        };
        let mut i = start as i64;
        while i > end_limit && i >= 0 {
            result.push(elements[i as usize].clone());
            i += step;
        }
    }
    Ok(result)
}

pub fn string_slice(
    s: &str,
    start: Option<Value>,
    end: Option<Value>,
    step: Option<Value>,
) -> Result<String, RuntimeError> {
    let chars: Vec<char> = s.chars().collect();
    let slice_elements: Vec<Value> = chars.iter().map(|&c| Value::Char(c)).collect();
    let result_values = array_slice(&slice_elements, start, end, step)?;
    let mut result_str = String::new();
    for v in result_values {
        if let Value::Char(c) = v {
            result_str.push(c);
        }
    }
    Ok(result_str)
}

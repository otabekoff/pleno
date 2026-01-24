use chrono::{TimeZone, Utc};
use pleno_value::MapState;
use pleno_value::Value;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn time_now(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let start = SystemTime::now();
    let since_epoch = start
        .duration_since(UNIX_EPOCH)
        .map_err(|_| pleno_value::RuntimeError::new("Time went backwards"))?;
    Ok(Value::Float(since_epoch.as_secs_f64()))
}

pub fn time_format(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "time.format requires timestamp and format string",
        ));
    }

    let timestamp = match args[0] {
        Value::Float(f) => f,
        Value::Int(i) => i as f64,
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected timestamp as first argument",
            ));
        }
    };

    let fmt = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected format string as second argument",
            ));
        }
    };

    let seconds = timestamp as i64;
    let nanoseconds = ((timestamp - (seconds as f64)) * 1_000_000_000.0) as u32;

    let dt = Utc
        .timestamp_opt(seconds, nanoseconds)
        .single()
        .ok_or_else(|| pleno_value::RuntimeError::new("Invalid timestamp"))?;

    Ok(Value::String(Rc::new(dt.format(fmt).to_string())))
}

pub fn time_parse(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "time.parse requires string and format",
        ));
    }

    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected string as first argument",
            ));
        }
    };

    let fmt = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected format string as second argument",
            ));
        }
    };

    let dt = chrono::NaiveDateTime::parse_from_str(s, fmt)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Parse error: {}", e)))?;
    let dt = Utc.from_utc_datetime(&dt);

    Ok(Value::Float(
        dt.timestamp() as f64 + (dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0),
    ))
}

pub fn time_parts(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "time.parts requires a timestamp",
        ));
    }

    let timestamp = match args[0] {
        Value::Float(f) => f,
        Value::Int(i) => i as f64,
        _ => return Err(pleno_value::RuntimeError::new("Expected timestamp")),
    };

    let seconds = timestamp as i64;
    let nanoseconds = ((timestamp - (seconds as f64)) * 1_000_000_000.0) as u32;

    let dt = Utc
        .timestamp_opt(seconds, nanoseconds)
        .single()
        .ok_or_else(|| pleno_value::RuntimeError::new("Invalid timestamp"))?;

    use chrono::{Datelike, Timelike};

    let mut entries = HashMap::new();
    entries.insert("year".to_string(), Value::Int(dt.year() as i64));
    entries.insert("month".to_string(), Value::Int(dt.month() as i64));
    entries.insert("day".to_string(), Value::Int(dt.day() as i64));
    entries.insert("hour".to_string(), Value::Int(dt.hour() as i64));
    entries.insert("minute".to_string(), Value::Int(dt.minute() as i64));
    entries.insert("second".to_string(), Value::Int(dt.second() as i64));
    entries.insert(
        "millisecond".to_string(),
        Value::Int((nanoseconds / 1_000_000) as i64),
    );

    Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))))
}

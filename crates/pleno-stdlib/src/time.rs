use chrono::{TimeZone, Utc};
use indexmap::IndexMap;
use pleno_value::{BoundNativeMethodState, MapState, RuntimeError, Value};
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

pub fn time_now_ns(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let start = SystemTime::now();
    let since_epoch = start
        .duration_since(UNIX_EPOCH)
        .map_err(|_| pleno_value::RuntimeError::new("Time went backwards"))?;
    Ok(Value::Int(since_epoch.as_nanos() as i64))
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

    let mut entries = IndexMap::new();
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

    Ok(Value::Map(vm.get_gc().allocate(MapState::new(entries))?))
}

pub fn time_instance_format(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let this = match args.first() {
        Some(Value::Map(m)) => m.borrow(),
        _ => return Err(RuntimeError::new("DateTime.format: invalid receiver")),
    };
    let timestamp = match this.get("_timestamp") {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(i)) => *i as f64,
        _ => return Err(RuntimeError::new("DateTime instance missing timestamp")),
    };

    let fmt = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Expected format string")),
    };

    let seconds = timestamp as i64;
    let nanoseconds = ((timestamp - (seconds as f64)) * 1_000_000_000.0) as u32;

    let dt = Utc
        .timestamp_opt(seconds, nanoseconds)
        .single()
        .ok_or_else(|| pleno_value::RuntimeError::new("Invalid timestamp"))?;

    Ok(Value::String(Rc::new(dt.format(fmt).to_string())))
}

pub fn time_datetime_now(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| pleno_value::RuntimeError::new("Time went backwards"))?;
    datetime_value(vm, since_epoch.as_secs_f64())
}

/// `DateTime.parse(text, format)`: a DateTime in UTC. A format without a time
/// of day gives midnight.
pub fn time_datetime_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let (Some(Value::String(text)), Some(Value::String(fmt))) = (args.first(), args.get(1)) else {
        return Err(RuntimeError::new(
            "DateTime.parse expects a string and a format",
        ));
    };
    let naive = chrono::NaiveDateTime::parse_from_str(text, fmt)
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(text, fmt)
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap_or_default())
        })
        .map_err(|e| RuntimeError::new(format!("DateTime.parse: {e}")))?;
    let dt = Utc.from_utc_datetime(&naive);
    let timestamp = dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1e9;
    datetime_value(vm, timestamp)
}

fn utc(timestamp: f64) -> Result<chrono::DateTime<Utc>, RuntimeError> {
    let seconds = timestamp.floor() as i64;
    let nanoseconds = ((timestamp - seconds as f64) * 1e9) as u32;
    Utc.timestamp_opt(seconds, nanoseconds)
        .single()
        .ok_or_else(|| RuntimeError::new("Invalid timestamp"))
}

/// A DateTime: a map of its UTC parts, with methods bound to it.
fn datetime_value(
    vm: &mut dyn pleno_value::VMProvider,
    timestamp: f64,
) -> Result<Value, RuntimeError> {
    use chrono::{Datelike, Timelike};
    let dt = utc(timestamp)?;

    let mut entries = IndexMap::new();
    entries.insert("_timestamp".to_string(), Value::Float(timestamp));
    entries.insert("year".to_string(), Value::Int(dt.year() as i64));
    entries.insert("month".to_string(), Value::Int(dt.month() as i64));
    entries.insert("day".to_string(), Value::Int(dt.day() as i64));
    entries.insert("hour".to_string(), Value::Int(dt.hour() as i64));
    entries.insert("minute".to_string(), Value::Int(dt.minute() as i64));
    entries.insert("second".to_string(), Value::Int(dt.second() as i64));
    entries.insert(
        "millisecond".to_string(),
        Value::Int((dt.timestamp_subsec_nanos() / 1_000_000) as i64),
    );
    let map_val = Value::Map(vm.get_gc().allocate(MapState::new(entries))?);

    let methods: [(&str, pleno_value::NativeFunction); 7] = [
        ("format", time_instance_format),
        ("toISO", datetime_to_iso),
        ("getTimestamp", datetime_get_timestamp),
        ("add", datetime_add),
        ("subtract", datetime_subtract),
        ("isBefore", datetime_is_before),
        ("isAfter", datetime_is_after),
    ];
    for (name, function) in methods {
        let method = vm.get_gc().allocate(BoundNativeMethodState {
            receiver: map_val.clone(),
            function,
        })?;
        if let Value::Map(state) = &map_val {
            state
                .borrow_mut()
                .entries
                .insert(name.to_string(), Value::BoundNativeMethod(method));
        }
    }
    Ok(map_val)
}

fn timestamp_of(value: Option<&Value>) -> Result<f64, RuntimeError> {
    if let Some(Value::Map(map)) = value {
        match map.borrow().get("_timestamp") {
            Some(Value::Float(f)) => return Ok(*f),
            Some(Value::Int(i)) => return Ok(*i as f64),
            _ => {}
        }
    }
    Err(RuntimeError::new("expected a DateTime"))
}

/// A Duration's length in seconds; a plain number counts as seconds.
fn seconds_of(value: Option<&Value>) -> Result<f64, RuntimeError> {
    match value {
        Some(Value::Int(i)) => Ok(*i as f64),
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Map(map)) => match map.borrow().get("_seconds") {
            Some(Value::Float(f)) => Ok(*f),
            _ => Err(RuntimeError::new("expected a Duration")),
        },
        _ => Err(RuntimeError::new("expected a Duration")),
    }
}

fn datetime_to_iso(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let dt = utc(timestamp_of(args.first())?)?;
    Ok(Value::String(Rc::new(
        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )))
}

/// Seconds since the Unix epoch.
fn datetime_get_timestamp(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    Ok(Value::Int(timestamp_of(args.first())?.floor() as i64))
}

fn datetime_add(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let timestamp = timestamp_of(args.first())? + seconds_of(args.get(1))?;
    datetime_value(vm, timestamp)
}

fn datetime_subtract(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let timestamp = timestamp_of(args.first())? - seconds_of(args.get(1))?;
    datetime_value(vm, timestamp)
}

fn datetime_is_before(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(
        timestamp_of(args.first())? < timestamp_of(args.get(1))?,
    ))
}

fn datetime_is_after(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(
        timestamp_of(args.first())? > timestamp_of(args.get(1))?,
    ))
}

/// A Duration: a map holding its length in seconds, with `getSeconds()`.
fn duration_value(
    vm: &mut dyn pleno_value::VMProvider,
    seconds: f64,
) -> Result<Value, RuntimeError> {
    let mut entries = IndexMap::new();
    entries.insert("_seconds".to_string(), Value::Float(seconds));
    let map_val = Value::Map(vm.get_gc().allocate(MapState::new(entries))?);
    let method = vm.get_gc().allocate(BoundNativeMethodState {
        receiver: map_val.clone(),
        function: duration_get_seconds,
    })?;
    if let Value::Map(state) = &map_val {
        state
            .borrow_mut()
            .entries
            .insert("getSeconds".to_string(), Value::BoundNativeMethod(method));
    }
    Ok(map_val)
}

fn duration_get_seconds(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    Ok(Value::Int(seconds_of(args.first())?.trunc() as i64))
}

fn duration_of(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
    unit_seconds: f64,
) -> Result<Value, RuntimeError> {
    let amount = match args.first() {
        Some(Value::Int(i)) => *i as f64,
        Some(Value::Float(f)) => *f,
        _ => return Err(RuntimeError::new("Duration expects a number")),
    };
    duration_value(vm, amount * unit_seconds)
}

pub fn duration_milliseconds(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    duration_of(vm, args, 0.001)
}

pub fn duration_seconds(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    duration_of(vm, args, 1.0)
}

pub fn duration_minutes(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    duration_of(vm, args, 60.0)
}

pub fn duration_hours(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    duration_of(vm, args, 3600.0)
}

pub fn duration_days(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    duration_of(vm, args, 86400.0)
}

pub fn native_resolve_timer(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if let Some(Value::Int(id)) = args.first() {
        vm.resolve_promise(*id as u64, Value::Null);
    }
    Ok(Value::Null)
}

pub fn set_timeout(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "setTimeout expects at least 2 arguments: callback and delay",
        ));
    }

    let callback = args[0].clone();
    let delay = match args[1] {
        Value::Int(ms) => ms.max(0) as u64,
        _ => return Err(RuntimeError::new("delay must be an integer")),
    };

    let callback_args = args[2..].to_vec();
    let id = vm.add_timer(delay, callback, callback_args);
    Ok(Value::Int(id as i64))
}

pub fn time_sleep(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new(
            "sleep expects at least 1 argument: delay",
        ));
    }

    let ms = match args[0] {
        Value::Int(ms) => ms.max(0) as u64,
        _ => return Err(RuntimeError::new("delay must be an integer")),
    };

    let (id, promise) = vm.create_promise()?;
    vm.add_timer(
        ms,
        Value::NativeFunction(native_resolve_timer),
        vec![Value::Int(id as i64)],
    );

    Ok(promise)
}

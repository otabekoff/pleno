use chrono::{TimeZone, Utc};
use pleno_value::{MapState, RuntimeError, VMProvider, Value};
use std::io::Write;
use std::rc::Rc;

pub fn print(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    vm.check_purity()?;
    let _gc = vm.get_gc();
    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{}", value));
    }
    println!("{out}");
    let _ = std::io::stdout().flush();
    Ok(Value::Null)
}

pub fn id(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.first().cloned().unwrap_or(Value::Null))
}

pub fn sleep(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let ms = match args.first() {
        Some(Value::Int(m)) => *m as u64,
        _ => return Err(RuntimeError::new("sleep() expects integer milliseconds")),
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(Value::Null)
}

pub fn time_now_ns(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RuntimeError::new(format!("time error: {}", e)))?;
    Ok(Value::Int(now.as_nanos() as i64))
}

pub fn datetime_now(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(Utc::now().timestamp()))
}

pub fn datetime_format(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let timestamp = match args.first() {
        Some(Value::Int(t)) => *t,
        _ => return Err(RuntimeError::new("format() expects integer timestamp")),
    };
    let fmt = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("format() expects format string")),
    };

    let dt = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or_else(|| RuntimeError::new("invalid timestamp"))?;
    Ok(Value::String(Rc::new(dt.format(fmt).to_string())))
}

pub fn datetime_parse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let date_str = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("parse() expects date string")),
    };
    let fmt = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("parse() expects format string")),
    };

    let dt = chrono::DateTime::parse_from_str(date_str, fmt)
        .map_err(|e| RuntimeError::new(format!("date parse error: {}", e)))?;
    Ok(Value::Int(dt.with_timezone(&Utc).timestamp()))
}

pub fn gc_collect(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    vm.collect();
    Ok(Value::Null)
}

pub fn gc_stats(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc_manager = vm.get_gc();
    let mut stats_map = std::collections::HashMap::new();
    stats_map.insert(
        "heap_size".to_string(),
        Value::Int(gc_manager.gc.get_heap_size() as i64),
    );
    stats_map.insert(
        "threshold".to_string(),
        Value::Int(gc_manager.gc.allocation_threshold as i64),
    );
    stats_map.insert(
        "current_allocation".to_string(),
        Value::Int(gc_manager.current_allocation as i64),
    );
    let gc = vm.get_gc();
    Ok(Value::Map(gc.allocate(MapState::new(stats_map))))
}

//! Logging module for Pleno standard library
//!
//! Provides leveled logging with customizable output.

use pleno_value::{RuntimeError, VMProvider, Value};
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Log levels (lower = more verbose)
/// 0: TRACE, 1: DEBUG, 2: INFO, 3: WARN, 4: ERROR, 5: FATAL, 6: OFF
static LOG_LEVEL: AtomicU8 = AtomicU8::new(2); // Default: INFO

fn level_name(level: u8) -> &'static str {
    match level {
        0 => "TRACE",
        1 => "DEBUG",
        2 => "INFO",
        3 => "WARN",
        4 => "ERROR",
        5 => "FATAL",
        _ => "OFF",
    }
}

fn level_color(level: u8) -> &'static str {
    match level {
        0 => "\x1b[90m", // Gray
        1 => "\x1b[36m", // Cyan
        2 => "\x1b[32m", // Green
        3 => "\x1b[33m", // Yellow
        4 => "\x1b[31m", // Red
        5 => "\x1b[35m", // Magenta
        _ => "\x1b[0m",
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let borrowed = arr.borrow();
            let items: Vec<String> = borrowed.elements.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Map(map) => {
            let borrowed = map.borrow();
            let items: Vec<String> = borrowed
                .entries
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        _ => format!("{:?}", value),
    }
}

fn log_message(level: u8, args: &[Value]) {
    let current_level = LOG_LEVEL.load(Ordering::Relaxed);
    if level < current_level {
        return;
    }

    let message: String = args.iter().map(format_value).collect::<Vec<_>>().join(" ");
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let color = level_color(level);
    let reset = "\x1b[0m";
    let level_str = level_name(level);

    eprintln!(
        "{}{} [{}]{} {}",
        color, timestamp, level_str, reset, message
    );
}

pub fn log_trace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(0, args);
    Ok(Value::Null)
}

pub fn log_debug(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(1, args);
    Ok(Value::Null)
}

pub fn log_info(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(2, args);
    Ok(Value::Null)
}

pub fn log_warn(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(3, args);
    Ok(Value::Null)
}

pub fn log_error(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(4, args);
    Ok(Value::Null)
}

pub fn log_fatal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(5, args);
    Ok(Value::Null)
}

/// Set log level: "trace", "debug", "info", "warn", "error", "fatal", "off"
pub fn log_set_level(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let level_str = match args.first() {
        Some(Value::String(s)) => s.as_str().to_lowercase(),
        Some(Value::Int(i)) => {
            let level = (*i as u8).min(6);
            LOG_LEVEL.store(level, Ordering::Relaxed);
            return Ok(Value::Null);
        }
        _ => return Err(RuntimeError::new("setLevel expects level name or number")),
    };

    let level = match level_str.as_str() {
        "trace" => 0,
        "debug" => 1,
        "info" => 2,
        "warn" | "warning" => 3,
        "error" => 4,
        "fatal" => 5,
        "off" | "none" => 6,
        _ => {
            return Err(RuntimeError::new(format!(
                "Unknown log level: {}",
                level_str
            )));
        }
    };

    LOG_LEVEL.store(level, Ordering::Relaxed);
    Ok(Value::Null)
}

/// Get current log level as string
pub fn log_get_level(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let level = LOG_LEVEL.load(Ordering::Relaxed);
    Ok(Value::String(Rc::new(level_name(level).to_lowercase())))
}

/// Create a child logger with a prefix
pub fn log_create(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let prefix = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        _ => return Err(RuntimeError::new("create() expects prefix string")),
    };

    let gc = vm.get_gc();
    let mut logger = std::collections::HashMap::new();

    // Store prefix for use in closures (we'll use a simple approach)
    logger.insert("prefix".to_string(), Value::String(Rc::new(prefix)));
    logger.insert("trace".to_string(), Value::NativeFunction(log_trace));
    logger.insert("debug".to_string(), Value::NativeFunction(log_debug));
    logger.insert("info".to_string(), Value::NativeFunction(log_info));
    logger.insert("warn".to_string(), Value::NativeFunction(log_warn));
    logger.insert("error".to_string(), Value::NativeFunction(log_error));
    logger.insert("fatal".to_string(), Value::NativeFunction(log_fatal));

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(logger))?))
}

//! Logging module for Pleno standard library
//!
//! Provides leveled logging that integrates with the Rust logging ecosystem.

use indexmap::IndexMap;
use pleno_value::{RuntimeError, VMProvider, Value};
use std::rc::Rc;

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

fn log_message(level: log::Level, args: &[Value]) {
    let message: String = args.iter().map(format_value).collect::<Vec<_>>().join(" ");
    log::log!(level, "{}", message);
}

pub fn log_trace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(log::Level::Trace, args);
    Ok(Value::Null)
}

pub fn log_debug(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(log::Level::Debug, args);
    Ok(Value::Null)
}

pub fn log_info(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(log::Level::Info, args);
    Ok(Value::Null)
}

pub fn log_warn(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(log::Level::Warn, args);
    Ok(Value::Null)
}

pub fn log_error(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    log_message(log::Level::Error, args);
    Ok(Value::Null)
}

pub fn log_fatal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // Rust log crate doesn't have Fatal, using Error
    log_message(log::Level::Error, args);
    Ok(Value::Null)
}

/// Set log level (legacy/stub for backward compatibility as env_logger is controlled externally)
pub fn log_set_level(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // Note: Logging levels are now controlled via RUST_LOG environment variable
    Ok(Value::Null)
}

/// Get current log level (stub)
pub fn log_get_level(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::String(Rc::new("managed by caller".to_string())))
}

/// Create a child logger with a prefix
pub fn log_create(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _prefix = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        _ => return Err(RuntimeError::new("create() expects prefix string")),
    };

    let gc = vm.get_gc();
    let mut logger = IndexMap::new();

    // In a full implementation, we'd handle prefixes properly.
    // For now, we reuse the same functions.
    logger.insert("trace".to_string(), Value::NativeFunction(log_trace));
    logger.insert("debug".to_string(), Value::NativeFunction(log_debug));
    logger.insert("info".to_string(), Value::NativeFunction(log_info));
    logger.insert("warn".to_string(), Value::NativeFunction(log_warn));
    logger.insert("error".to_string(), Value::NativeFunction(log_error));
    logger.insert("fatal".to_string(), Value::NativeFunction(log_fatal));

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(logger))?))
}

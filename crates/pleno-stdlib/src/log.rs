//! `std:log`: leveled logging to stderr.
//!
//! The level is the script's own (`Log.setLevel`), independent of `RUST_LOG`,
//! which only controls the interpreter's internal logging.

use indexmap::IndexMap;
use pleno_value::{BoundNativeMethodState, NativeFunction, RuntimeError, VMProvider, Value};
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

const LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];
const OFF: u8 = LEVELS.len() as u8;

/// Messages below this level are dropped. Starts at `info`.
static MIN_LEVEL: AtomicU8 = AtomicU8::new(2);

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

fn emit(level: u8, prefix: Option<&str>, args: &[Value]) {
    if level < MIN_LEVEL.load(Ordering::Relaxed) {
        return;
    }
    let message = args.iter().map(format_value).collect::<Vec<_>>().join(" ");
    let message = match prefix {
        Some(prefix) => format!("{prefix} {message}"),
        None => message,
    };
    eprintln!(
        "[{}] {:<5} - {}",
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
        LEVELS[level as usize].to_uppercase(),
        message
    );
}

/// For a logger from `Log.create(prefix)`: the prefix is the bound receiver.
fn emit_prefixed(level: u8, args: &[Value]) {
    match args.split_first() {
        Some((Value::String(prefix), rest)) => emit(level, Some(prefix), rest),
        _ => emit(level, None, args),
    }
}

pub fn log_trace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(0, None, args);
    Ok(Value::Null)
}

pub fn log_debug(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(1, None, args);
    Ok(Value::Null)
}

pub fn log_info(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(2, None, args);
    Ok(Value::Null)
}

pub fn log_warn(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(3, None, args);
    Ok(Value::Null)
}

pub fn log_error(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(4, None, args);
    Ok(Value::Null)
}

pub fn log_fatal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit(5, None, args);
    Ok(Value::Null)
}

fn prefixed_trace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(0, args);
    Ok(Value::Null)
}

fn prefixed_debug(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(1, args);
    Ok(Value::Null)
}

fn prefixed_info(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(2, args);
    Ok(Value::Null)
}

fn prefixed_warn(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(3, args);
    Ok(Value::Null)
}

fn prefixed_error(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(4, args);
    Ok(Value::Null)
}

fn prefixed_fatal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    emit_prefixed(5, args);
    Ok(Value::Null)
}

/// `Log.setLevel(level)`: a name (`"trace"` to `"fatal"`, or `"off"`) or a
/// number from 0 (trace) to 6 (off).
pub fn log_set_level(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let level = match args.first() {
        Some(Value::String(name)) => match name.to_lowercase().as_str() {
            "off" | "none" => Some(OFF),
            name => LEVELS.iter().position(|level| *level == name).map(|i| i as u8),
        },
        Some(Value::Int(n)) if (0..=OFF as i64).contains(n) => Some(*n as u8),
        _ => None,
    }
    .ok_or_else(|| {
        RuntimeError::new(
            "setLevel expects trace, debug, info, warn, error, fatal or off, or a number from 0 to 6",
        )
    })?;
    MIN_LEVEL.store(level, Ordering::Relaxed);
    Ok(Value::Null)
}

/// `Log.getLevel()`: the current level's name.
pub fn log_get_level(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let level = MIN_LEVEL.load(Ordering::Relaxed) as usize;
    let name = LEVELS.get(level).copied().unwrap_or("off");
    Ok(Value::String(Rc::new(name.to_string())))
}

/// `Log.create(prefix)`: a logger whose messages start with `prefix`.
pub fn log_create(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let prefix = match args.first() {
        Some(prefix @ Value::String(_)) => prefix.clone(),
        _ => return Err(RuntimeError::new("create() expects prefix string")),
    };

    let methods: [(&str, NativeFunction); 6] = [
        ("trace", prefixed_trace),
        ("debug", prefixed_debug),
        ("info", prefixed_info),
        ("warn", prefixed_warn),
        ("error", prefixed_error),
        ("fatal", prefixed_fatal),
    ];
    let mut logger = IndexMap::new();
    for (name, function) in methods {
        let method = vm.get_gc().allocate(BoundNativeMethodState {
            receiver: prefix.clone(),
            function,
        })?;
        logger.insert(name.to_string(), Value::BoundNativeMethod(method));
    }

    Ok(Value::Map(
        vm.get_gc().allocate(pleno_value::MapState::new(logger))?,
    ))
}

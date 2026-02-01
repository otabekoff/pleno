use pleno_value::{RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn os_platform(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else if cfg!(target_os = "openbsd") {
        "openbsd"
    } else if cfg!(target_os = "netbsd") {
        "netbsd"
    } else if cfg!(target_os = "dragonfly") {
        "dragonfly"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "unknown"
    };
    Ok(Value::String(Rc::new(platform.to_string())))
}

pub fn os_arch(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let arch = if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    Ok(Value::String(Rc::new(arch.to_string())))
}

pub fn os_homedir(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // For now, return Null or use environment variable
    match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(path) => Ok(Value::String(Rc::new(path))),
        Err(_) => Ok(Value::Null),
    }
}

pub fn os_tmpdir(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::String(Rc::new(
        std::env::temp_dir().to_string_lossy().into_owned(),
    )))
}

pub fn os_hostname(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // Placeholder to avoid hostname crate dependency for now
    Ok(Value::Null)
}

pub fn os_uptime(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // Basic uptime using system_stat or similar would be better, but for now
    // let's just return a placeholder or use a simpler method if available.
    // Rust doesn't have a built-in for uptime.
    Ok(Value::Float(0.0))
}

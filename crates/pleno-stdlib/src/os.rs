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

/// The machine's host name, or null if it cannot be found.
pub fn os_hostname(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(host_name()
        .map(|name| Value::String(Rc::new(name)))
        .unwrap_or(Value::Null))
}

fn host_name() -> Option<String> {
    let non_empty = |name: String| {
        let name = name.trim().to_string();
        (!name.is_empty()).then_some(name)
    };
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|var| std::env::var(var).ok().and_then(non_empty))
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .and_then(non_empty)
        })
        .or_else(|| {
            let output = std::process::Command::new("hostname").output().ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).to_string())
                .and_then(non_empty)
        })
}

/// Seconds since the system booted, as a Float, or null where unknown.
pub fn os_uptime(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(uptime_seconds().map(Value::Float).unwrap_or(Value::Null))
}

#[cfg(target_os = "linux")]
fn uptime_seconds() -> Option<f64> {
    // "12345.67 54321.00": seconds up, then idle time.
    std::fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(target_os = "windows")]
fn uptime_seconds() -> Option<f64> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount64() -> u64;
    }
    // SAFETY: GetTickCount64 takes no arguments, cannot fail, and is in
    // kernel32 on every supported Windows version.
    let milliseconds = unsafe { GetTickCount64() };
    Some(milliseconds as f64 / 1000.0)
}

#[cfg(target_os = "macos")]
fn uptime_seconds() -> Option<f64> {
    // `sysctl -n kern.boottime` prints "{ sec = 1700000000, usec = 0 } ...".
    let output = std::process::Command::new("sysctl")
        .args(["-n", "kern.boottime"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let boot: u64 = text
        .split("sec = ")
        .nth(1)?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(now.saturating_sub(boot) as f64)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn uptime_seconds() -> Option<f64> {
    None
}

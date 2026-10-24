use pleno_value::Value;
use pleno_value::{RuntimeError, VMProvider};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub fn path_join(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let mut path = PathBuf::new();

    for arg in args {
        match arg {
            Value::String(s) => path.push(s.as_str()),
            _ => return Err(RuntimeError::new("path components must be strings")),
        }
    }

    Ok(Value::String(Rc::new(path.to_string_lossy().into_owned())))
}

pub fn path_dirname(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("dirname expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            let parent = path.parent().unwrap_or(Path::new(""));
            let parent_str = if parent.as_os_str().is_empty() {
                if path.is_absolute() {
                    // Root directory parent is itself on some systems or empty
                    // For "/" parent is "/"
                    s.as_str()
                } else {
                    "."
                }
            } else {
                parent.to_str().unwrap_or("")
            };
            Ok(Value::String(Rc::new(parent_str.to_string())))
        }
        _ => Err(RuntimeError::new("dirname expects string")),
    }
}

pub fn path_basename(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("basename expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            Ok(Value::String(Rc::new(file_name)))
        }
        _ => Err(RuntimeError::new("basename expects string")),
    }
}

pub fn path_extname(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("extname expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            Ok(Value::String(Rc::new(ext)))
        }
        _ => Err(RuntimeError::new("extname expects string")),
    }
}

pub fn path_exists(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("exists expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            Ok(Value::Bool(path.exists()))
        }
        _ => Err(RuntimeError::new("exists expects string")),
    }
}

pub fn path_is_file(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("isFile expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            Ok(Value::Bool(path.is_file()))
        }
        _ => Err(RuntimeError::new("isFile expects string")),
    }
}

pub fn path_is_dir(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("isDir expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            Ok(Value::Bool(path.is_dir()))
        }
        _ => Err(RuntimeError::new("isDir expects string")),
    }
}

/// On Windows, `canonicalize` returns `\\?\C:\...` paths; the prefix is
/// noise to users and other programs, so drive paths drop it.
fn without_verbatim_prefix(path: String) -> String {
    match path.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with(r"UNC\") => rest.to_string(),
        _ => path,
    }
}

pub fn path_resolve(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("resolve expects 1 argument"));
    }

    match &args[0] {
        Value::String(s) => {
            let path = Path::new(s.as_str());
            match std::fs::canonicalize(path) {
                Ok(abs_path) => Ok(Value::String(Rc::new(without_verbatim_prefix(
                    abs_path.to_string_lossy().into_owned(),
                )))),
                Err(_) => {
                    // Fallback: if file doesn't exist, try to resolve using cwd
                    if path.is_absolute() {
                        Ok(Value::String(s.clone()))
                    } else {
                        let mut abs_path = std::env::current_dir()
                            .map_err(|e| RuntimeError::new(format!("failed to get cwd: {}", e)))?;
                        // Component by component, so the separators match the platform.
                        for component in path.components() {
                            abs_path.push(component);
                        }
                        Ok(Value::String(Rc::new(
                            abs_path.to_string_lossy().into_owned(),
                        )))
                    }
                }
            }
        }
        _ => Err(RuntimeError::new("resolve expects string")),
    }
}

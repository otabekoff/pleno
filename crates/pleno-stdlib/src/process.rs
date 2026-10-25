use pleno_value::Value;
use pleno_value::{RuntimeError, VMProvider};
use std::rc::Rc;

pub fn process_cwd(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_io_allowed() {
        return Err(RuntimeError::new("SecurityError: IO access denied"));
    }
    match std::env::current_dir() {
        Ok(path) => Ok(Value::String(Rc::new(path.to_string_lossy().into_owned()))),
        Err(e) => Err(RuntimeError::new(format!("failed to get cwd: {}", e))),
    }
}

pub fn process_exit(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let code = if args.is_empty() {
        0
    } else {
        match args[0] {
            Value::Int(code) => code as i32,
            _ => return Err(RuntimeError::new("exit code must be integer")),
        }
    };
    std::process::exit(code);
}

pub fn process_args(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let args: Vec<Value> = std::env::args()
        .map(|arg| Value::String(Rc::new(arg)))
        .collect();
    Ok(Value::Array(
        gc.allocate(pleno_value::ArrayState::new(args))?,
    ))
}

pub fn process_env(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() != 1 {
        return Err(RuntimeError::new("env expects 1 argument"));
    }

    match &args[0] {
        Value::String(key) => match std::env::var(key.as_str()) {
            Ok(value) => Ok(Value::String(Rc::new(value))),
            Err(_) => Ok(Value::Null),
        },
        _ => Err(RuntimeError::new("env key must be string")),
    }
}

pub fn process_pid(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    Ok(Value::Int(std::process::id() as i64))
}

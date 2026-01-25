use pleno_value::{ArrayState, RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn os_exit(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let code = match args.first() {
        Some(Value::Int(code)) => *code as i32,
        _ => 0,
    };
    std::process::exit(code);
}

pub fn os_getenv(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let name = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("getenv expects string argument")),
    };
    match std::env::var(name) {
        Ok(val) => Ok(Value::String(Rc::new(val))),
        Err(_) => Ok(Value::Null),
    }
}

pub fn os_args(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let args: Vec<Value> = std::env::args()
        .map(|arg| Value::String(Rc::new(arg)))
        .collect();
    Ok(Value::Array(gc.allocate(ArrayState::new(args))))
}

pub fn os_cwd(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let cwd = std::env::current_dir()
        .map_err(|e| RuntimeError::new(format!("failed to get cwd: {}", e)))?;
    Ok(Value::String(Rc::new(cwd.to_string_lossy().into_owned())))
}

pub fn os_platform(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    Ok(Value::String(Rc::new(std::env::consts::OS.to_string())))
}

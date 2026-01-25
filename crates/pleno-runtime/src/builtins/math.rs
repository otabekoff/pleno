use pleno_value::{RuntimeError, VMProvider, Value};

pub fn math_abs(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Err(RuntimeError::new("abs() expects a number")),
    }
}

pub fn math_pow(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let base = match args.first() {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        _ => return Err(RuntimeError::new("pow() base must be a number")),
    };
    let exp = match args.get(1) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        _ => return Err(RuntimeError::new("pow() exponent must be a number")),
    };
    Ok(Value::Float(base.powf(exp)))
}

pub fn math_sqrt(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        _ => Err(RuntimeError::new("sqrt() expects a number")),
    }
}

pub fn math_random(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    use rand::Rng;
    Ok(Value::Float(rand::thread_rng().r#gen()))
}

pub fn math_random_int(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    use rand::Rng;
    let min = match args.first() {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::new("randomInt() min must be an integer")),
    };
    let max = match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::new("randomInt() max must be an integer")),
    };
    if min >= max {
        return Err(RuntimeError::new("randomInt() min must be less than max"));
    }
    Ok(Value::Int(rand::thread_rng().gen_range(min..max)))
}

pub fn math_sin(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sin())),
        _ => Err(RuntimeError::new("sin() expects a number")),
    }
}

pub fn math_cos(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cos())),
        _ => Err(RuntimeError::new("cos() expects a number")),
    }
}

pub fn math_tan(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tan())),
        _ => Err(RuntimeError::new("tan() expects a number")),
    }
}

pub fn math_log(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ln())),
        Some(Value::Float(n)) => Ok(Value::Float(n.ln())),
        _ => Err(RuntimeError::new("log() expects a number")),
    }
}

pub fn math_exp(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).exp())),
        Some(Value::Float(n)) => Ok(Value::Float(n.exp())),
        _ => Err(RuntimeError::new("exp() expects a number")),
    }
}

pub fn math_floor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.floor() as i64)),
        _ => Err(RuntimeError::new("floor() expects a number")),
    }
}

pub fn math_ceil(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.ceil() as i64)),
        _ => Err(RuntimeError::new("ceil() expects a number")),
    }
}

pub fn math_round(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.round() as i64)),
        _ => Err(RuntimeError::new("round() expects a number")),
    }
}

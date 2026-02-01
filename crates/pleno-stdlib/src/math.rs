use pleno_value::Value;

pub fn math_abs(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.abs requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Int(i.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_min(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "Math.min requires 2 arguments",
        ));
    }
    let a = match args[0] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number")),
    };
    let b = match args[1] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number")),
    };
    Ok(Value::Float(a.min(b)))
}

pub fn math_max(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "Math.max requires 2 arguments",
        ));
    }
    let a = match args[0] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number")),
    };
    let b = match args[1] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number")),
    };
    Ok(Value::Float(a.max(b)))
}

pub fn math_floor(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.floor requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Float(f) => Ok(Value::Int(f.floor() as i64)),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_ceil(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.ceil requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Float(f) => Ok(Value::Int(f.ceil() as i64)),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_trunc(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.trunc requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Float(f) => Ok(Value::Int(f.trunc() as i64)),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_sign(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.sign requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => {
            if i > 0 {
                Ok(Value::Int(1))
            } else if i < 0 {
                Ok(Value::Int(-1))
            } else {
                Ok(Value::Int(0))
            }
        }
        Value::Float(f) => {
            if f > 0.0 {
                Ok(Value::Int(1))
            } else if f < 0.0 {
                Ok(Value::Int(-1))
            } else if f == 0.0 {
                Ok(Value::Int(0))
            } else {
                Ok(Value::Float(f64::NAN))
            }
        }
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_round(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.round requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Float(f) => Ok(Value::Int(f.round() as i64)),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_sqrt(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.sqrt requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).sqrt())),
        Value::Float(f) => Ok(Value::Float(f.sqrt())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_pow(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "Math.pow requires 2 arguments",
        ));
    }
    let base = match args[0] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number base")),
    };
    let exp = match args[1] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number exponent")),
    };
    Ok(Value::Float(base.powf(exp)))
}

pub fn math_atan2(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "Math.atan2 requires 2 arguments (y, x)",
        ));
    }
    let y = match args[0] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number for y")),
    };
    let x = match args[1] {
        Value::Int(i) => i as f64,
        Value::Float(f) => f,
        _ => return Err(pleno_value::RuntimeError::new("Expected number for x")),
    };
    Ok(Value::Float(y.atan2(x)))
}

pub fn math_sin(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.sin requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).sin())),
        Value::Float(f) => Ok(Value::Float(f.sin())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_cos(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.cos requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).cos())),
        Value::Float(f) => Ok(Value::Float(f.cos())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_tan(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.tan requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).tan())),
        Value::Float(f) => Ok(Value::Float(f.tan())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_log(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.log requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).ln())),
        Value::Float(f) => Ok(Value::Float(f.ln())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_log10(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.log10 requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).log10())),
        Value::Float(f) => Ok(Value::Float(f.log10())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_exp(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.is_empty() {
        return Err(pleno_value::RuntimeError::new(
            "Math.exp requires 1 argument",
        ));
    }
    match args[0] {
        Value::Int(i) => Ok(Value::Float((i as f64).exp())),
        Value::Float(f) => Ok(Value::Float(f.exp())),
        _ => Err(pleno_value::RuntimeError::new("Expected number")),
    }
}

pub fn math_random(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    use rand::Rng;
    Ok(Value::Float(rand::thread_rng().r#gen()))
}

pub fn math_random_int(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    use rand::Rng;
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "Math.randomInt requires min and max arguments",
        ));
    }
    let min = match args[0] {
        Value::Int(i) => i,
        _ => return Err(pleno_value::RuntimeError::new("Expected integer min")),
    };
    let max = match args[1] {
        Value::Int(i) => i,
        _ => return Err(pleno_value::RuntimeError::new("Expected integer max")),
    };

    if min >= max {
        return Err(pleno_value::RuntimeError::new(
            "Math.randomInt: min must be less than max",
        ));
    }

    Ok(Value::Int(rand::thread_rng().gen_range(min..=max)))
}

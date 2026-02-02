use pleno_value::{RuntimeError, Value};
use rand::Rng;
use rand::seq::SliceRandom;

pub fn random(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    let mut rng = rand::thread_rng();
    Ok(Value::Float(rng.r#gen()))
}

pub fn randint(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError::new("randint expects 2 arguments: min, max"));
    }
    let min = match &args[0] {
        Value::Int(i) => *i,
        _ => return Err(RuntimeError::new("min must be an integer")),
    };
    let max = match &args[1] {
        Value::Int(i) => *i,
        _ => return Err(RuntimeError::new("max must be an integer")),
    };

    if min > max {
        return Err(RuntimeError::new("min must be less than or equal to max"));
    }

    let mut rng = rand::thread_rng();
    Ok(Value::Int(rng.gen_range(min..=max)))
}

pub fn choice(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("choice expects 1 argument: array"));
    }
    match &args[0] {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            if elements.is_empty() {
                return Ok(Value::Null);
            }
            let mut rng = rand::thread_rng();
            Ok(elements.choose(&mut rng).cloned().unwrap_or(Value::Null))
        }
        _ => Err(RuntimeError::new("choice expects an array")),
    }
}

pub fn shuffle(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("shuffle expects 1 argument: array"));
    }
    match &args[0] {
        Value::Array(arr) => {
            if arr.borrow().is_immutable {
                return Err(RuntimeError::new("Cannot shuffle immutable array"));
            }
            let mut rng = rand::thread_rng();
            arr.borrow_mut().elements.shuffle(&mut rng);
            Ok(Value::Null)
        }
        _ => Err(RuntimeError::new("shuffle expects an array")),
    }
}

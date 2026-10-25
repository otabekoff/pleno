use pleno_bytecode::Opcode;
use pleno_value::nan_value::NanValue;
use pleno_value::{RuntimeError, Value};
use std::rc::Rc;

pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Missing => false,
        Value::Bool(value) => *value,
        _ => true,
    }
}

pub fn is_truthy_nan(val: NanValue) -> bool {
    if val.is_f64() {
        return true;
    }
    match val.tag() {
        2 => val.as_bool().unwrap_or(false), // TAG_BOOL
        3 | 4 => false,                      // TAG_NULL, TAG_MISSING
        _ => true,
    }
}

/// Two numeric operands, with an `Int` and a `Float` promoted to `Float`.
enum Numbers {
    Ints(i64, i64),
    Floats(f64, f64),
}

fn numbers(a: &Value, b: &Value) -> Option<Numbers> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Some(Numbers::Ints(*a, *b)),
        (Value::Float(a), Value::Float(b)) => Some(Numbers::Floats(*a, *b)),
        (Value::Int(a), Value::Float(b)) => Some(Numbers::Floats(*a as f64, *b)),
        (Value::Float(a), Value::Int(b)) => Some(Numbers::Floats(*a, *b as f64)),
        _ => None,
    }
}

fn operand_error(op: &str, a: &Value, b: &Value) -> RuntimeError {
    RuntimeError::new(format!(
        "invalid operands for {op}: {} and {}",
        a.type_name(),
        b.type_name()
    ))
}

fn overflow(operation: &str) -> RuntimeError {
    RuntimeError::new(format!("integer overflow in {operation}"))
}

fn concat(
    vm: &mut dyn pleno_value::VMProvider,
    left: &str,
    right: &str,
) -> Result<Value, RuntimeError> {
    let len = left.len() + right.len();
    vm.get_gc().track_allocation(len)?;
    let mut res = String::with_capacity(len);
    res.push_str(left);
    res.push_str(right);
    Ok(Value::String(Rc::new(res)))
}

pub fn add_values(
    vm: &mut dyn pleno_value::VMProvider,
    a: Value,
    b: Value,
) -> Result<Value, RuntimeError> {
    if let Some(numbers) = numbers(&a, &b) {
        return match numbers {
            Numbers::Ints(a, b) => a
                .checked_add(b)
                .map(Value::Int)
                .ok_or_else(|| overflow("addition")),
            Numbers::Floats(a, b) => Ok(Value::Float(a + b)),
        };
    }
    match (&a, &b) {
        (Value::String(a), Value::String(b)) => concat(vm, a, b),
        (Value::String(a), other) => concat(vm, a, &other.to_string()),
        (other, Value::String(b)) => concat(vm, &other.to_string(), b),
        (Value::Char(a), Value::Char(b)) => Ok(Value::String(Rc::new(format!("{a}{b}")))),
        (a, b) => Err(RuntimeError::new(format!(
            "invalid operands for +: {} and {} (use .toString() for string concatenation)",
            a.type_name(),
            b.type_name()
        ))),
    }
}

pub fn sub_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match numbers(&a, &b) {
        Some(Numbers::Ints(a, b)) => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| overflow("subtraction")),
        Some(Numbers::Floats(a, b)) => Ok(Value::Float(a - b)),
        None => Err(operand_error("-", &a, &b)),
    }
}

pub fn mul_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match numbers(&a, &b) {
        Some(Numbers::Ints(a, b)) => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| overflow("multiplication")),
        Some(Numbers::Floats(a, b)) => Ok(Value::Float(a * b)),
        None => Err(operand_error("*", &a, &b)),
    }
}

/// `/` always gives a `Float`; dividing by zero is an error for every
/// combination of `Int` and `Float`.
pub fn div_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    let (a, b) = match numbers(&a, &b) {
        Some(Numbers::Ints(a, b)) => (a as f64, b as f64),
        Some(Numbers::Floats(a, b)) => (a, b),
        None => return Err(operand_error("/", &a, &b)),
    };
    if b == 0.0 {
        return Err(RuntimeError::new("division by zero"));
    }
    Ok(Value::Float(a / b))
}

pub fn mod_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match numbers(&a, &b) {
        Some(Numbers::Ints(_, 0)) => Err(RuntimeError::new("modulo by zero")),
        // `i64::MIN % -1` overflows.
        Some(Numbers::Ints(a, b)) => a
            .checked_rem(b)
            .map(Value::Int)
            .ok_or_else(|| overflow("modulo")),
        Some(Numbers::Floats(_, 0.0)) => Err(RuntimeError::new("modulo by zero")),
        Some(Numbers::Floats(a, b)) => Ok(Value::Float(a % b)),
        None => Err(operand_error("%", &a, &b)),
    }
}

pub fn neg_value(value: Value) -> Result<Value, RuntimeError> {
    match value {
        Value::Int(value) => value
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| overflow("negation")),
        Value::Float(value) => Ok(Value::Float(-value)),
        _ => Err(RuntimeError::new("invalid operand for unary -")),
    }
}

fn compare<T: PartialOrd>(opcode: Opcode, a: T, b: T) -> Result<bool, RuntimeError> {
    Ok(match opcode {
        Opcode::Lt => a < b,
        Opcode::Le => a <= b,
        Opcode::Gt => a > b,
        Opcode::Ge => a >= b,
        _ => return Err(RuntimeError::new("invalid comparison opcode")),
    })
}

/// `<`, `<=`, `>` and `>=`: numbers by value (Int and Float mixed), strings
/// and characters by Unicode code point, one character at a time.
pub fn compare_values(opcode: Opcode, left: Value, right: Value) -> Result<bool, RuntimeError> {
    match numbers(&left, &right) {
        Some(Numbers::Ints(a, b)) => compare(opcode, a, b),
        Some(Numbers::Floats(a, b)) => compare(opcode, a, b),
        None => match (&left, &right) {
            (Value::String(a), Value::String(b)) => compare(opcode, a.as_str(), b.as_str()),
            (Value::Char(a), Value::Char(b)) => compare(opcode, a, b),
            _ => Err(operand_error("comparison", &left, &right)),
        },
    }
}

/// The fast paths below handle two `Float`s or two `Int`s without decoding;
/// everything else goes through the `Value` versions above.
pub fn add_nan(
    vm: &mut dyn pleno_value::VMProvider,
    a: NanValue,
    b: NanValue,
) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        return Ok(NanValue::from_f64(a.as_f64() + b.as_f64()));
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int()) {
        return a
            .checked_add(b)
            .map(NanValue::from_int)
            .ok_or_else(|| overflow("addition"));
    }
    Ok(NanValue::from_value(&add_values(
        vm,
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn sub_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        return Ok(NanValue::from_f64(a.as_f64() - b.as_f64()));
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int()) {
        return a
            .checked_sub(b)
            .map(NanValue::from_int)
            .ok_or_else(|| overflow("subtraction"));
    }
    Ok(NanValue::from_value(&sub_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn mul_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        return Ok(NanValue::from_f64(a.as_f64() * b.as_f64()));
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int()) {
        return a
            .checked_mul(b)
            .map(NanValue::from_int)
            .ok_or_else(|| overflow("multiplication"));
    }
    Ok(NanValue::from_value(&mul_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn div_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() && b.as_f64() != 0.0 {
        return Ok(NanValue::from_f64(a.as_f64() / b.as_f64()));
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int())
        && b != 0
    {
        return Ok(NanValue::from_f64(a as f64 / b as f64));
    }
    Ok(NanValue::from_value(&div_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn mod_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() && b.as_f64() != 0.0 {
        return Ok(NanValue::from_f64(a.as_f64() % b.as_f64()));
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int())
        && let Some(rem) = a.checked_rem(b)
    {
        return Ok(NanValue::from_int(rem));
    }
    Ok(NanValue::from_value(&mod_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn compare_nan(opcode: Opcode, a: NanValue, b: NanValue) -> Result<bool, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        return compare(opcode, a.as_f64(), b.as_f64());
    }
    if let (Some(a), Some(b)) = (a.as_int(), b.as_int()) {
        return compare(opcode, a, b);
    }
    compare_values(opcode, a.to_value(), b.to_value())
}

pub fn neg_nan(value: NanValue) -> Result<NanValue, RuntimeError> {
    if value.is_f64() {
        return Ok(NanValue::from_f64(-value.as_f64()));
    }
    Ok(NanValue::from_value(&neg_value(value.to_value())?))
}

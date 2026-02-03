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
        0 => true,                           // Float (handled above but just in case)
        2 => val.as_bool().unwrap_or(false), // TAG_BOOL
        3 | 4 => false,                      // TAG_NULL, TAG_MISSING
        _ => true,
    }
}

pub fn add_values(
    vm: &mut dyn pleno_value::VMProvider,
    a: Value,
    b: Value,
) -> Result<Value, RuntimeError> {
    match (&a, &b) {
        (Value::Int(a_val), Value::Int(b_val)) => a_val
            .checked_add(*b_val)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in addition")),
        (Value::Float(a_val), Value::Float(b_val)) => Ok(Value::Float(a_val + b_val)),
        (Value::Int(a_val), Value::Float(b_val)) => Ok(Value::Float(*a_val as f64 + b_val)),
        (Value::Float(a_val), Value::Int(b_val)) => Ok(Value::Float(a_val + *b_val as f64)),
        (Value::String(a_str), Value::String(b_str)) => {
            let a_s = a_str.as_str();
            let b_s = b_str.as_str();
            let len = a_s.len() + b_s.len();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
            res.push_str(a_s);
            res.push_str(b_s);
            Ok(Value::String(Rc::new(res)))
        }
        (Value::String(a_str), other) => {
            let a_s = a_str.as_str();
            let b_s = format!("{}", other);
            let len = a_s.len() + b_s.len();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
            res.push_str(a_s);
            res.push_str(&b_s);
            Ok(Value::String(Rc::new(res)))
        }
        (other, Value::String(b_str)) => {
            let a_s = format!("{}", other);
            let b_s = b_str.as_str();
            let len = a_s.len() + b_s.len();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
            res.push_str(&a_s);
            res.push_str(b_s);
            Ok(Value::String(Rc::new(res)))
        }
        (Value::Char(a_char), Value::Char(b_char)) => {
            let mut res = String::with_capacity(a_char.len_utf8() + b_char.len_utf8());
            res.push(*a_char);
            res.push(*b_char);
            Ok(Value::String(Rc::new(res)))
        }
        _ => Err(RuntimeError::new(
            "invalid operands for + (use .toString() for string concatenation)",
        )),
    }
}

pub fn sub_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in subtraction")),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),
        _ => Err(RuntimeError::new("invalid operands for -")),
    }
}

pub fn mul_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in multiplication")),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),
        _ => Err(RuntimeError::new("invalid operands for *")),
    }
}

pub fn div_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(_), Value::Int(0)) => Err(RuntimeError::new("division by zero")),
        (Value::Float(_), Value::Float(0.0)) => Err(RuntimeError::new("division by zero")),
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(a as f64 / b as f64)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / b as f64)),
        _ => Err(RuntimeError::new("invalid operands for /")),
    }
}

pub fn mod_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(_), Value::Int(0)) => Err(RuntimeError::new("modulo by zero")),
        (Value::Float(_), Value::Float(0.0)) => Err(RuntimeError::new("modulo by zero")),
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 % b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a % b as f64)),
        _ => Err(RuntimeError::new("invalid operands for %")),
    }
}

pub fn neg_value(value: Value) -> Result<Value, RuntimeError> {
    match value {
        Value::Int(value) => Ok(Value::Int(-value)),
        Value::Float(value) => Ok(Value::Float(-value)),
        _ => Err(RuntimeError::new("invalid operand for unary -")),
    }
}

pub fn compare_values(opcode: Opcode, left: Value, right: Value) -> Result<bool, RuntimeError> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Ok(match opcode {
            Opcode::Lt => a < b,
            Opcode::Le => a <= b,
            Opcode::Gt => a > b,
            Opcode::Ge => a >= b,
            _ => return Err(RuntimeError::new("invalid comparison opcode")),
        }),
        (Value::Float(a), Value::Float(b)) => Ok(match opcode {
            Opcode::Lt => a < b,
            Opcode::Le => a <= b,
            Opcode::Gt => a > b,
            Opcode::Ge => a >= b,
            _ => return Err(RuntimeError::new("invalid comparison opcode")),
        }),
        (Value::Int(a), Value::Float(b)) => {
            let af = a as f64;
            Ok(match opcode {
                Opcode::Lt => af < b,
                Opcode::Le => af <= b,
                Opcode::Gt => af > b,
                Opcode::Ge => af >= b,
                _ => return Err(RuntimeError::new("invalid comparison opcode")),
            })
        }
        (Value::Float(a), Value::Int(b)) => {
            let bf = b as f64;
            Ok(match opcode {
                Opcode::Lt => a < bf,
                Opcode::Le => a <= bf,
                Opcode::Gt => a > bf,
                Opcode::Ge => a >= bf,
                _ => return Err(RuntimeError::new("invalid comparison opcode")),
            })
        }
        _ => Err(RuntimeError::new("invalid operands for comparison")),
    }
}

pub fn add_nan(
    vm: &mut dyn pleno_value::VMProvider,
    a: NanValue,
    b: NanValue,
) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        return Ok(NanValue::from_f64(a.as_f64() + b.as_f64()));
    }
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        let res: i64 = ai
            .checked_add(bi)
            .ok_or_else(|| RuntimeError::new("integer overflow in addition"))?;
        return Ok(NanValue::from_int(res));
    }
    if let (Some(af), Some(bi)) = (if a.is_f64() { Some(a.as_f64()) } else { None }, b.as_int()) {
        return Ok(NanValue::from_f64(af + bi as f64));
    }
    if let (Some(ai), Some(bf)) = (a.as_int(), if b.is_f64() { Some(b.as_f64()) } else { None }) {
        return Ok(NanValue::from_f64(ai as f64 + bf));
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
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        let res: i64 = ai
            .checked_sub(bi)
            .ok_or_else(|| RuntimeError::new("integer overflow in subtraction"))?;
        return Ok(NanValue::from_int(res));
    }
    if let (Some(af), Some(bi)) = (if a.is_f64() { Some(a.as_f64()) } else { None }, b.as_int()) {
        return Ok(NanValue::from_f64(af - bi as f64));
    }
    if let (Some(ai), Some(bf)) = (a.as_int(), if b.is_f64() { Some(b.as_f64()) } else { None }) {
        return Ok(NanValue::from_f64(ai as f64 - bf));
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
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        let res: i64 = ai
            .checked_mul(bi)
            .ok_or_else(|| RuntimeError::new("integer overflow in multiplication"))?;
        return Ok(NanValue::from_int(res));
    }
    if let (Some(af), Some(bi)) = (if a.is_f64() { Some(a.as_f64()) } else { None }, b.as_int()) {
        return Ok(NanValue::from_f64(af * bi as f64));
    }
    if let (Some(ai), Some(bf)) = (a.as_int(), if b.is_f64() { Some(b.as_f64()) } else { None }) {
        return Ok(NanValue::from_f64(ai as f64 * bf));
    }
    Ok(NanValue::from_value(&mul_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn div_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        if b.as_f64() == 0.0 {
            return Err(RuntimeError::new("division by zero"));
        }
        return Ok(NanValue::from_f64(a.as_f64() / b.as_f64()));
    }
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        if bi == 0 {
            return Err(RuntimeError::new("division by zero"));
        }
        return Ok(NanValue::from_f64(ai as f64 / bi as f64));
    }
    Ok(NanValue::from_value(&div_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn mod_nan(a: NanValue, b: NanValue) -> Result<NanValue, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        if b.as_f64() == 0.0 {
            return Err(RuntimeError::new("modulo by zero"));
        }
        return Ok(NanValue::from_f64(a.as_f64() % b.as_f64()));
    }
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        if bi == 0 {
            return Err(RuntimeError::new("modulo by zero"));
        }
        return Ok(NanValue::from_int(ai % bi));
    }
    Ok(NanValue::from_value(&mod_values(
        a.to_value(),
        b.to_value(),
    )?))
}

pub fn compare_nan(opcode: Opcode, a: NanValue, b: NanValue) -> Result<bool, RuntimeError> {
    if a.is_f64() && b.is_f64() {
        let (av, bv) = (a.as_f64(), b.as_f64());
        return Ok(match opcode {
            Opcode::Lt => av < bv,
            Opcode::Le => av <= bv,
            Opcode::Gt => av > bv,
            Opcode::Ge => av >= bv,
            _ => return Err(RuntimeError::new("invalid comparison opcode")),
        });
    }
    if let (Some(ai), Some(bi)) = (a.as_int(), b.as_int()) {
        return Ok(match opcode {
            Opcode::Lt => ai < bi,
            Opcode::Le => ai <= bi,
            Opcode::Gt => ai > bi,
            Opcode::Ge => ai >= bi,
            _ => return Err(RuntimeError::new("invalid comparison opcode")),
        });
    }
    compare_values(opcode, a.to_value(), b.to_value())
}

pub fn neg_nan(value: NanValue) -> Result<NanValue, RuntimeError> {
    if value.is_f64() {
        return Ok(NanValue::from_f64(-value.as_f64()));
    }
    if let Some(i) = value.as_int() {
        return Ok(NanValue::from_int(-i));
    }
    Err(RuntimeError::new("invalid operand for unary -"))
}

use pleno_bytecode::Opcode;
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
        (Value::String(a_str), Value::Char(b_char)) => {
            let a_s = a_str.as_str();
            let len = a_s.len() + b_char.len_utf8();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
            res.push_str(a_s);
            res.push(*b_char);
            Ok(Value::String(Rc::new(res)))
        }
        (Value::Char(a_char), Value::String(b_str)) => {
            let b_s = b_str.as_str();
            let len = a_char.len_utf8() + b_s.len();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
            res.push(*a_char);
            res.push_str(b_s);
            Ok(Value::String(Rc::new(res)))
        }
        (Value::Char(a_char), Value::Char(b_char)) => {
            let len = a_char.len_utf8() + b_char.len_utf8();
            vm.get_gc().track_allocation(len)?;
            let mut res = String::with_capacity(len);
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

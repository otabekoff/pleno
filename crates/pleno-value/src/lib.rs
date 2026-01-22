use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub type NativeFunction = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Range(Range),
    Class(Rc<Class>),
    Function(usize),
    BoundMethod {
        receiver: Rc<RefCell<Object>>,
        function: usize,
        owner: Rc<String>,
        visibility: MethodVisibility,
    },
    StaticMethod {
        function: usize,
        owner: Rc<String>,
        visibility: MethodVisibility,
    },
    Closure {
        function: usize,
        captures: Vec<Value>,
    },
    NativeFunction(NativeFunction),
    BoundNativeMethod {
        receiver: Box<Value>,
        function: NativeFunction,
    },
    Map(Rc<RefCell<HashMap<String, Value>>>),
    Array(Rc<RefCell<Vec<Value>>>),
    Set(Rc<RefCell<HashSet<ValueKey>>>),
    Object(Rc<RefCell<Object>>),
    Partial {
        function: usize,
        args: Vec<Value>,
        placeholders: Vec<usize>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub inclusive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodInfo>,
    pub static_methods: HashMap<String, MethodInfo>,
    pub constructor: Option<usize>,
    pub superclass: Option<Rc<Class>>,
    pub static_fields: HashMap<String, Value>,
    pub fields: HashMap<String, MethodVisibility>,
    pub static_field_visibility: HashMap<String, MethodVisibility>,
    pub module: Option<Rc<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodVisibility {
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodInfo {
    pub function: usize,
    pub visibility: MethodVisibility,
    pub owner: Rc<String>,
    pub module: Option<Rc<String>>,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Missing, Value::Missing) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Range(a), Value::Range(b)) => a == b,
            (Value::Class(a), Value::Class(b)) => Rc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => a == b,
            (Value::BoundMethod { receiver: a, function: fa, owner: oa, visibility: va }, Value::BoundMethod { receiver: b, function: fb, owner: ob, visibility: vb }) => {
                Rc::ptr_eq(a, b) && fa == fb && oa == ob && va == vb
            }
            (Value::StaticMethod { function: fa, owner: oa, visibility: va }, Value::StaticMethod { function: fb, owner: ob, visibility: vb }) => {
                fa == fb && oa == ob && va == vb
            }
            (Value::Closure { function: fa, captures: ca }, Value::Closure { function: fb, captures: cb }) => {
                fa == fb && ca == cb
            }
            (Value::NativeFunction(a), Value::NativeFunction(b)) => {
                (*a as usize) == (*b as usize)
            }
            (
                Value::BoundNativeMethod {
                    receiver: ra,
                    function: fa,
                },
                Value::BoundNativeMethod {
                    receiver: rb,
                    function: fb,
                },
            ) => ra == rb && (*fa as usize) == (*fb as usize),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b),
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Set(a), Value::Set(b)) => Rc::ptr_eq(a, b),
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (
                Value::Partial {
                    function: fa,
                    args: aa,
                    placeholders: pa,
                },
                Value::Partial {
                    function: fb,
                    args: ab,
                    placeholders: pb,
                },
            ) => fa == fb && aa == ab && pa == pb,
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Missing => write!(f, "<missing>"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "{}", v),
            Value::Range(r) => {
                if let Some(start) = r.start {
                    write!(f, "{}", start)?;
                }
                if r.inclusive {
                    write!(f, "..=")?;
                } else {
                    write!(f, "..")?;
                }
                if let Some(end) = r.end {
                    write!(f, "{}", end)?;
                }
                Ok(())
            }
            Value::Class(c) => write!(f, "<class {}>", c.name),
            Value::Function(_) => write!(f, "<fn>"),
            Value::BoundMethod { owner, .. } => write!(f, "<method of {}>", owner),
            Value::StaticMethod { owner, .. } => write!(f, "<static method of {}>", owner),
            Value::Closure { .. } => write!(f, "<closure>"),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::BoundNativeMethod { .. } => write!(f, "<native method>"),
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Set(s) => {
                write!(f, "{{")?;
                for (i, v) in s.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match v {
                        ValueKey::Null => write!(f, "null")?,
                        ValueKey::Bool(v) => write!(f, "{}", v)?,
                        ValueKey::Int(v) => write!(f, "{}", v)?,
                        ValueKey::Float(bits) => write!(f, "{}", f64::from_bits(*bits))?,
                        ValueKey::String(v) => write!(f, "\"{}\"", v)?,
                    }
                }
                write!(f, "}}")
            }
            Value::Object(o) => {
                if let Some(class) = &o.borrow().class {
                    write!(f, "<instance of {}>", class.name)
                } else {
                    write!(f, "<object>")
                }
            }
            Value::Partial { .. } => write!(f, "<partial>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: Option<Rc<Class>>,
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(Rc<String>),
}

impl ValueKey {
    pub fn try_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Null => Some(ValueKey::Null),
            Value::Bool(value) => Some(ValueKey::Bool(*value)),
            Value::Int(value) => Some(ValueKey::Int(*value)),
            Value::Float(value) => Some(ValueKey::Float(value.to_bits())),
            Value::String(value) => Some(ValueKey::String(Rc::clone(value))),
            _ => None,
        }
    }
}

pub mod gc;

use gc::{GcObject, GcRc, MarkAndSweepGC};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub trait VMProvider {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError>;
    fn get_gc(&mut self) -> &mut gc::GCManager;
}

pub type NativeFunction =
    fn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError>;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Range(Range),
    Class(GcRc<Class>),
    Function(usize),
    BoundMethod {
        receiver: GcRc<Object>,
        function: usize,
        owner: GcRc<Class>,
        visibility: MethodVisibility,
    },
    StaticMethod {
        function: usize,
        owner: GcRc<Class>,
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
    Map(GcRc<HashMap<String, Value>>),
    Array(GcRc<Vec<Value>>),
    Set(GcRc<HashSet<ValueKey>>),
    Object(GcRc<Object>),
    Partial {
        function: usize,
        args: Vec<Value>,
        placeholders: Vec<usize>,
    },
    /// Array operation that requires callback (map, filter, reduce)
    ArrayOp {
        kind: ArrayOpKind,
        array: GcRc<Vec<Value>>,
    },
}

impl GcObject for Value {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        match self {
            Value::Class(c) => gc.mark_object(&c.to_gc_object()),
            Value::BoundMethod { receiver, .. } => gc.mark_object(&receiver.to_gc_object()),
            Value::Closure { captures, .. } => {
                for v in captures {
                    v.mark(gc);
                }
            }
            Value::BoundNativeMethod { receiver, .. } => receiver.mark(gc),
            Value::Map(m) => gc.mark_object(&m.to_gc_object()),
            Value::Array(a) => gc.mark_object(&a.to_gc_object()),
            Value::Set(s) => gc.mark_object(&s.to_gc_object()),
            Value::Object(o) => gc.mark_object(&o.to_gc_object()),
            Value::Partial { args, .. } => {
                for v in args {
                    v.mark(gc);
                }
            }
            Value::ArrayOp { array, .. } => gc.mark_object(&array.to_gc_object()),
            _ => {}
        }
    }
}

impl GcObject for Vec<Value> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in self {
            v.mark(gc);
        }
    }
}

impl GcObject for HashMap<String, Value> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in self.values() {
            v.mark(gc);
        }
    }
}

impl GcObject for HashSet<ValueKey> {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayOpKind {
    Map,
    Filter,
    Reduce,
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

#[derive(Debug, Clone)]
pub struct Class {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodInfo>,
    pub static_methods: HashMap<String, MethodInfo>,
    pub constructor: Option<usize>,
    pub native_constructor: Option<NativeFunction>,
    pub superclass: Option<GcRc<Class>>,
    pub static_fields: HashMap<String, Value>,
    pub fields: HashMap<String, MethodVisibility>,
    pub static_field_visibility: HashMap<String, MethodVisibility>,
    pub module: Option<Rc<String>>,
}

impl GcObject for Class {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        if let Some(sc) = &self.superclass {
            gc.mark_object(&sc.to_gc_object());
        }
        for v in self.static_fields.values() {
            v.mark(gc);
        }
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.methods == other.methods
            && self.static_methods == other.static_methods
            && self.constructor == other.constructor
            && match (self.native_constructor, other.native_constructor) {
                (Some(a), Some(b)) => (a as usize) == (b as usize),
                (None, None) => true,
                _ => false,
            }
            && self.superclass == other.superclass
            && self.static_fields == other.static_fields
            && self.fields == other.fields
            && self.static_field_visibility == other.static_field_visibility
            && self.module == other.module
    }
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
    pub owner: GcRc<Class>,
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
            (Value::Class(a), Value::Class(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Function(a), Value::Function(b)) => a == b,
            (
                Value::BoundMethod {
                    receiver: a,
                    function: fa,
                    owner: oa,
                    visibility: va,
                },
                Value::BoundMethod {
                    receiver: b,
                    function: fb,
                    owner: ob,
                    visibility: vb,
                },
            ) => a.as_ptr() == b.as_ptr() && fa == fb && oa == ob && va == vb,
            (
                Value::StaticMethod {
                    function: fa,
                    owner: oa,
                    visibility: va,
                },
                Value::StaticMethod {
                    function: fb,
                    owner: ob,
                    visibility: vb,
                },
            ) => fa == fb && oa == ob && va == vb,
            (
                Value::Closure {
                    function: fa,
                    captures: ca,
                },
                Value::Closure {
                    function: fb,
                    captures: cb,
                },
            ) => fa == fb && ca == cb,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => (*a as usize) == (*b as usize),
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
            (Value::Map(a), Value::Map(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Array(a), Value::Array(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Set(a), Value::Set(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Object(a), Value::Object(b)) => a.as_ptr() == b.as_ptr(),
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
            (
                Value::ArrayOp {
                    kind: ka,
                    array: aa,
                },
                Value::ArrayOp {
                    kind: kb,
                    array: ab,
                },
            ) => ka == kb && aa.as_ptr() == ab.as_ptr(),
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
            Value::Class(c) => write!(f, "<class {}>", c.borrow().name),
            Value::Function(_) => write!(f, "<fn>"),
            Value::BoundMethod { owner, .. } => write!(f, "<method of {}>", owner.borrow().name),
            Value::StaticMethod { owner, .. } => {
                write!(f, "<static method of {}>", owner.borrow().name)
            }
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
                    write!(f, "<instance of {}>", class.borrow().name)
                } else {
                    write!(f, "<object>")
                }
            }
            Value::Partial { .. } => write!(f, "<partial>"),
            Value::ArrayOp { kind, .. } => {
                let name = match kind {
                    ArrayOpKind::Map => "map",
                    ArrayOpKind::Filter => "filter",
                    ArrayOpKind::Reduce => "reduce",
                };
                write!(f, "<array.{}>", name)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: Option<GcRc<Class>>,
    pub fields: HashMap<String, Value>,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "Null",
            Value::Missing => "Missing",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Range(_) => "Range",
            Value::Class(_) => "Class",
            Value::Function(_) => "Function",
            Value::BoundMethod { .. } => "Method",
            Value::StaticMethod { .. } => "StaticMethod",
            Value::Closure { .. } => "Closure",
            Value::NativeFunction(_) => "NativeFunction",
            Value::BoundNativeMethod { .. } => "NativeMethod",
            Value::Map(_) => "Map",
            Value::Array(_) => "Array",
            Value::Set(_) => "Set",
            Value::Object(_) => "Object",
            Value::Partial { .. } => "Partial",
            Value::ArrayOp { .. } => "ArrayOp",
        }
    }
}

impl GcObject for Object {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        if let Some(c) = &self.class {
            gc.mark_object(&c.to_gc_object());
        }
        for v in self.fields.values() {
            v.mark(gc);
        }
    }
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

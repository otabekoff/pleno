pub mod gc;

use gc::{GcObject, GcRc, MarkAndSweepGC};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub trait VMProvider {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError>;
    fn get_gc(&mut self) -> &mut gc::GCManager;
    fn create_promise(&mut self) -> (u64, Value);
    fn resolve_promise(&mut self, id: u64, value: Value);
    fn register_multi_waiter(&mut self, waiter: MultiWaiter);
    fn check_purity(&mut self) -> Result<(), RuntimeError>;
    fn collect(&mut self);
}

#[derive(Debug, Clone)]
pub enum MultiWaiterMode {
    All,
    Race,
}

#[derive(Debug, Clone)]
pub struct MultiWaiter {
    pub promise_id: u64,
    pub dependencies: Vec<u64>,
    pub resolved_count: usize,
    pub results: Vec<Value>,
    pub mode: MultiWaiterMode,
}

impl GcObject for MultiWaiter {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in &self.results {
            v.mark(gc);
        }
    }
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
    Interface(GcRc<Interface>),
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
    Closure(GcRc<ClosureState>),
    NativeFunction(NativeFunction),
    BoundNativeMethod {
        receiver: Box<Value>,
        function: NativeFunction,
    },
    Map(GcRc<MapState>),
    Array(GcRc<ArrayState>),
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
        array: GcRc<ArrayState>,
    },
    Mutex(GcRc<MutexState>),
    Channel(GcRc<ChannelState>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

#[derive(Debug, Clone)]
pub struct MutexState {
    pub locked: bool,
    pub waiters: Vec<usize>, // Promise IDs waiting for lock
}

#[derive(Debug, Clone)]
pub struct ChannelState {
    pub buffer: std::collections::VecDeque<Value>,
    pub capacity: usize,
    pub send_waiters: Vec<(usize, Value)>, // (Promise ID, Value to send)
    pub recv_waiters: Vec<usize>,          // Promise IDs waiting for value
}

impl GcObject for MutexState {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {
        // Waiters are just IDs, no GC references
    }
}

impl GcObject for ChannelState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in &self.buffer {
            v.mark(gc);
        }
        for (_, v) in &self.send_waiters {
            v.mark(gc);
        }
    }
}

impl GcObject for Value {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        match self {
            Value::Class(c) => gc.mark_object(&c.to_gc_object()),
            Value::Interface(i) => gc.mark_object(&i.to_gc_object()),
            Value::BoundMethod { receiver, .. } => gc.mark_object(&receiver.to_gc_object()),
            Value::Closure(c) => gc.mark_object(&c.to_gc_object()),
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
            Value::Mutex(m) => gc.mark_object(&m.to_gc_object()),
            Value::Channel(c) => gc.mark_object(&c.to_gc_object()),
            _ => {}
        }
    }

    fn clear_refs(&mut self) {
        if let Value::BoundNativeMethod { receiver, .. } = self {
            receiver.clear_refs();
        }
    }
}

impl GcObject for ArrayState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in &self.elements {
            v.mark(gc);
        }
    }

    fn clear_refs(&mut self) {
        self.elements.clear();
    }
}

impl GcObject for MapState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in self.entries.values() {
            v.mark(gc);
        }
    }

    fn clear_refs(&mut self) {
        self.entries.clear();
    }
}

impl GcObject for HashSet<ValueKey> {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
    fn clear_refs(&mut self) {
        self.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayOpKind {
    Map,
    Filter,
    Reduce,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackFrame {
    pub function_name: String,
    pub span: Option<pleno_common::Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<pleno_common::Span>,
    pub stack_trace: Vec<StackFrame>,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            stack_trace: Vec::new(),
        }
    }

    pub fn with_span(mut self, span: pleno_common::Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_stack_trace(mut self, stack_trace: Vec<StackFrame>) -> Self {
        self.stack_trace = stack_trace;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub inclusive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodSignature {
    pub name: String,
    pub arity: usize,
    pub is_static: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodSignature>,
    pub getters: HashMap<String, MethodSignature>,
    pub setters: HashMap<String, MethodSignature>,
}

impl GcObject for Interface {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
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
    pub getters: HashMap<String, MethodInfo>,
    pub setters: HashMap<String, MethodInfo>,
    pub static_getters: HashMap<String, MethodInfo>,
    pub static_setters: HashMap<String, MethodInfo>,
    pub module: Option<Rc<String>>,
    pub is_abstract: bool,
    pub implements: Vec<Value>,
}

impl GcObject for Class {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        if let Some(sc) = &self.superclass {
            gc.mark_object(&sc.to_gc_object());
        }
        for v in self.static_fields.values() {
            v.mark(gc);
        }
        for v in &self.implements {
            v.mark(gc);
        }
    }

    fn clear_refs(&mut self) {
        self.superclass = None;
        self.static_fields.clear();
        self.methods.clear();
        self.static_methods.clear();
        self.getters.clear();
        self.setters.clear();
        self.static_getters.clear();
        self.static_setters.clear();
        self.implements.clear();
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
            && self.getters == other.getters
            && self.setters == other.setters
            && self.static_getters == other.static_getters
            && self.static_setters == other.static_setters
            && self.module == other.module
            && self.is_abstract == other.is_abstract
            && self.implements == other.implements
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
            (Value::Interface(a), Value::Interface(b)) => a.as_ptr() == b.as_ptr(),
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
            (Value::Closure(ca), Value::Closure(cb)) => ca == cb,
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
            ) => *ka == *kb && aa.as_ptr() == ab.as_ptr(),
            (Value::Mutex(a), Value::Mutex(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Channel(a), Value::Channel(b)) => a.as_ptr() == b.as_ptr(),
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
            Value::Interface(i) => write!(f, "<interface {}>", i.borrow().name),
            Value::Function(_) => write!(f, "<fn>"),
            Value::BoundMethod { owner, .. } => write!(f, "<method of {}>", owner.borrow().name),
            Value::StaticMethod { owner, .. } => {
                write!(f, "<static method of {}>", owner.borrow().name)
            }
            Value::Closure(c) => write!(f, "<closure {}>", c.borrow().function),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::BoundNativeMethod { .. } => write!(f, "<native method>"),
            Value::Map(m) => {
                write!(f, "{{")?;
                let entries = &m.borrow().entries;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Array(a) => {
                write!(f, "[")?;
                let elements = &a.borrow().elements;
                for (i, v) in elements.iter().enumerate() {
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
            Value::Mutex(_) => write!(f, "<mutex>"),
            Value::Channel(_) => write!(f, "<channel>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayState {
    pub elements: Vec<Value>,
    pub is_immutable: bool,
}

impl ArrayState {
    pub fn new(elements: Vec<Value>) -> Self {
        Self {
            elements,
            is_immutable: false,
        }
    }

    pub fn ensure_mutable(&self) -> Result<(), RuntimeError> {
        if self.is_immutable {
            Err(RuntimeError::new("cannot modify immutable array"))
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for ArrayState {
    type Target = Vec<Value>;
    fn deref(&self) -> &Self::Target {
        &self.elements
    }
}

impl std::ops::DerefMut for ArrayState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.elements
    }
}

impl IntoIterator for ArrayState {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}
#[derive(Debug, Clone)]
pub struct ClosureState {
    pub function: usize,
    pub captures: Vec<Value>,
}

impl ClosureState {
    pub fn new(function: usize, captures: Vec<Value>) -> Self {
        Self { function, captures }
    }
}

impl GcObject for ClosureState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in &self.captures {
            v.mark(gc);
        }
    }
}

impl<'a> IntoIterator for &'a ArrayState {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapState {
    pub entries: HashMap<String, Value>,
    pub is_immutable: bool,
}

impl MapState {
    pub fn new(entries: HashMap<String, Value>) -> Self {
        Self {
            entries,
            is_immutable: false,
        }
    }

    pub fn ensure_mutable(&self) -> Result<(), RuntimeError> {
        if self.is_immutable {
            Err(RuntimeError::new("cannot modify immutable map"))
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for MapState {
    type Target = HashMap<String, Value>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl std::ops::DerefMut for MapState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

impl IntoIterator for MapState {
    type Item = (String, Value);
    type IntoIter = std::collections::hash_map::IntoIter<String, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a MapState {
    type Item = (&'a String, &'a Value);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: Option<GcRc<Class>>,
    pub fields: HashMap<String, Value>,
    pub is_immutable: bool,
}

impl Object {
    pub fn new(class: Option<GcRc<Class>>) -> Self {
        Self {
            class,
            fields: HashMap::new(),
            is_immutable: false,
        }
    }

    pub fn ensure_mutable(&self) -> Result<(), RuntimeError> {
        if self.is_immutable {
            Err(RuntimeError::new("cannot modify immutable object"))
        } else {
            Ok(())
        }
    }
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
            Value::Interface(_) => "Interface",
            Value::Function(_) => "Function",
            Value::BoundMethod { .. } => "Method",
            Value::StaticMethod { .. } => "StaticMethod",
            Value::Closure(_) => "Closure",
            Value::NativeFunction(_) => "NativeFunction",
            Value::BoundNativeMethod { .. } => "NativeMethod",
            Value::Map(_) => "Map",
            Value::Array(_) => "Array",
            Value::Set(_) => "Set",
            Value::Object(_) => "Object",
            Value::Partial { .. } => "Partial",
            Value::ArrayOp { .. } => "ArrayOp",
            Value::Mutex(_) => "Mutex",
            Value::Channel(_) => "Channel",
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

    fn clear_refs(&mut self) {
        self.class = None;
        self.fields.clear();
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

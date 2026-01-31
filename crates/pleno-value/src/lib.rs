pub mod gc;

use gc::{GcObject, GcRc, MarkAndSweepGC};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::rc::Rc;

pub trait VMProvider {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError>;
    fn spawn(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError>;
    fn get_gc(&mut self) -> &mut gc::GCManager;
    fn create_promise(&mut self) -> (u64, Value);
    fn resolve_promise(&mut self, id: u64, value: Value);
    fn register_multi_waiter(&mut self, waiter: MultiWaiter);
    fn check_purity(&mut self) -> Result<(), RuntimeError>;
    fn collect(&mut self);
    fn register_module(&mut self, name: String, value: Value);
    fn register_global(&mut self, name: String, value: Value);
    fn set_builtin_resolver(&mut self, resolver: fn(&str, &str) -> Option<NativeFunction>);
    fn check_is_instance(&self, value: &Value, type_val: &Value) -> bool;
    fn get_named_args(&self) -> &HashMap<String, Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiWaiterMode {
    All,
    Race,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// Helper to provide default for Skipped Fields
#[allow(dead_code)]
fn default_native_function() -> Option<NativeFunction> {
    None
}

#[allow(dead_code)]
fn skip_native_function<S>(_nf: &Option<NativeFunction>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_none()
}

#[allow(dead_code)]
fn deserialize_native_function<'de, D>(deserializer: D) -> Result<Option<NativeFunction>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let _ = Option::<()>::deserialize(deserializer)?;
    Ok(None)
}

fn dummy_native_function(_: &mut dyn VMProvider, _: &[Value]) -> Result<Value, RuntimeError> {
    Err(RuntimeError::new(
        "deserialized native function calls are not supported",
    ))
}

fn default_native_function_val() -> NativeFunction {
    dummy_native_function
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        receiver: Box<Value>,
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
    Thunk(GcRc<ThunkState>),
    NativeFunction(#[serde(skip, default = "default_native_function_val")] NativeFunction),
    BoundNativeMethod {
        receiver: Box<Value>,
        #[serde(skip, default = "default_native_function_val")]
        function: NativeFunction,
    },
    Map(GcRc<MapState>),
    Array(GcRc<ArrayState>),
    Set(GcRc<SetState>),
    Object(GcRc<Object>),
    Partial {
        callee: Box<Value>,
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
    Tuple(GcRc<TupleState>),
    Iterator(GcRc<IteratorState>),
    BoundClass {
        class: GcRc<Class>,
        type_args: Vec<Value>,
    },
    Enum(GcRc<EnumType>),
    Variant {
        enum_name: Rc<String>,
        variant_name: Rc<String>,
        data: Vec<Value>,
    },
    VariantConstructor {
        enum_type: GcRc<EnumType>,
        variant_name: Rc<String>,
    },
    BoundInterface {
        interface: GcRc<Interface>,
        type_args: Vec<Value>,
    },
    BoundFunction {
        function: usize,
        type_args: Vec<Value>,
    },
    File(GcRc<FileState>),
    Cell(GcRc<CellState>),
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
            Value::Thunk(_) => "Thunk",
            Value::Cell(_) => "Cell",
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
            Value::Tuple(_) => "Tuple",
            Value::Iterator(_) => "Iterator",
            Value::BoundClass { .. } => "Class",
            Value::Enum(_) => "Enum",
            Value::Variant { .. } => "Variant",
            Value::VariantConstructor { .. } => "VariantConstructor",
            Value::BoundInterface { .. } => "Interface",
            Value::BoundFunction { .. } => "Function",
            Value::File(_) => "File",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutexState {
    pub locked: bool,
    pub waiters: Vec<usize>, // Promise IDs waiting for lock
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub buffer: std::collections::VecDeque<Value>,
    pub capacity: usize,
    #[serde(skip)]
    pub send_waiters: Vec<(usize, Value)>, // (Promise ID, Value to send) - cannot serialize easily due to tuple
    pub recv_waiters: Vec<usize>, // Promise IDs waiting for value
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

pub struct FileState {
    pub inner: RefCell<Option<Box<dyn Write>>>,
    pub name: String,
}

impl std::fmt::Debug for FileState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileState({{ name: {} }})", self.name)
    }
}

impl Serialize for FileState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // We only serialize the name, file handle is lost
        let mut state = serializer.serialize_struct("FileState", 1)?;
        state.serialize_field("name", &self.name)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for FileState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FileStateData {
            name: String,
        }
        let data = FileStateData::deserialize(deserializer)?;
        Ok(FileState {
            inner: RefCell::new(None), // Deserialized files are closed
            name: data.name,
        })
    }
}

impl GcObject for FileState {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {
        // No references to other values
    }
}

impl GcObject for Value {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        match self {
            Value::Class(c) => gc.mark_object(&c.to_gc_object()),
            Value::Interface(i) => gc.mark_object(&i.to_gc_object()),
            Value::BoundMethod { receiver, .. } => receiver.mark(gc),
            Value::Closure(c) => gc.mark_object(&c.to_gc_object()),
            Value::Thunk(t) => gc.mark_object(&t.to_gc_object()),
            Value::BoundNativeMethod { receiver, .. } => receiver.mark(gc),
            Value::Map(m) => gc.mark_object(&m.to_gc_object()),
            Value::Array(a) => gc.mark_object(&a.to_gc_object()),
            Value::Set(s) => gc.mark_object(&s.to_gc_object()),
            Value::Object(o) => gc.mark_object(&o.to_gc_object()),
            Value::Partial { callee, args, .. } => {
                callee.mark(gc);
                for v in args {
                    v.mark(gc);
                }
            }
            Value::ArrayOp { array, .. } => gc.mark_object(&array.to_gc_object()),
            Value::Mutex(m) => gc.mark_object(&m.to_gc_object()),
            Value::Channel(c) => gc.mark_object(&c.to_gc_object()),
            Value::Tuple(t) => gc.mark_object(&t.to_gc_object()),
            Value::Iterator(i) => gc.mark_object(&i.to_gc_object()),
            Value::BoundClass { class, type_args } => {
                gc.mark_object(&class.to_gc_object());
                for arg in type_args {
                    arg.mark(gc);
                }
            }
            Value::Enum(e) => gc.mark_object(&e.to_gc_object()),
            Value::Variant { data, .. } => {
                for val in data {
                    val.mark(gc);
                }
            }
            Value::VariantConstructor { enum_type, .. } => {
                gc.mark_object(&enum_type.to_gc_object());
            }
            Value::BoundInterface {
                interface,
                type_args,
            } => {
                gc.mark_object(&interface.to_gc_object());
                for arg in type_args {
                    arg.mark(gc);
                }
            }
            Value::BoundFunction {
                function: _, // usize, no marking needed
                type_args,
            } => {
                for arg in type_args {
                    arg.mark(gc);
                }
            }
            Value::File(f) => gc.mark_object(&f.to_gc_object()),
            Value::Cell(c) => gc.mark_object(&c.to_gc_object()),
            _ => {}
        }
    }

    fn clear_refs(&mut self) {
        if let Value::BoundNativeMethod { receiver, .. } = self {
            receiver.clear_refs();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrayState {
    pub elements: Vec<Value>,
    pub is_immutable: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapState {
    pub entries: HashMap<String, Value>,
    pub is_immutable: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetState {
    pub elements: HashSet<ValueKey>,
    pub is_immutable: bool,
}

impl GcObject for SetState {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
    fn clear_refs(&mut self) {
        self.elements.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArrayOpKind {
    Map,
    Filter,
    Reduce,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackFrame {
    pub function_name: String,
    pub span: Option<pleno_common::Span>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<pleno_common::Span>,
    pub stack_trace: Vec<StackFrame>,
    pub handled: bool,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            stack_trace: Vec::new(),
            handled: false,
        }
    }

    pub fn handled() -> Self {
        Self {
            message: "exception handled".to_string(),
            span: None,
            stack_trace: Vec::new(),
            handled: true,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodSignature {
    pub name: String,
    pub arity: usize,
    pub is_static: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodSignature>,
    pub getters: HashMap<String, MethodSignature>,
    pub setters: HashMap<String, MethodSignature>,
    pub type_params: Vec<String>,
    pub type_constraints: HashMap<String, Vec<String>>,
}

impl GcObject for Interface {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumType {
    pub name: Rc<String>,
    pub variants: HashMap<String, EnumVariantDef>,
    pub module: Option<Rc<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariantDef {
    pub index: usize,
    pub param_count: usize,
}

impl GcObject for EnumType {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class {
    pub name: Rc<String>,
    pub methods: HashMap<String, MethodInfo>,
    pub static_methods: HashMap<String, MethodInfo>,
    pub operators: HashMap<String, MethodInfo>,
    pub constructor: Option<usize>,
    #[serde(skip, default)]
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
    pub is_pooled: bool,
    pub implements: Vec<Value>,
    pub type_params: Vec<String>,
    pub type_constraints: HashMap<String, Vec<String>>,
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
        self.operators.clear();
        self.type_constraints.clear();
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
            && self.is_pooled == other.is_pooled
            && self.implements == other.implements
            && self.type_params == other.type_params
            && self.type_constraints == other.type_constraints
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodVisibility {
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
                    receiver: ra,
                    function: fa,
                    owner: oa,
                    visibility: va,
                },
                Value::BoundMethod {
                    receiver: rb,
                    function: fb,
                    owner: ob,
                    visibility: vb,
                },
            ) => ra == rb && fa == fb && oa == ob && va == vb,
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
                    callee: ca,
                    args: aa,
                    placeholders: pa,
                },
                Value::Partial {
                    callee: cb,
                    args: ab,
                    placeholders: pb,
                },
            ) => ca == cb && aa == ab && pa == pb,
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
            (Value::Tuple(a), Value::Tuple(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Iterator(a), Value::Iterator(b)) => a.as_ptr() == b.as_ptr(),
            (
                Value::BoundClass {
                    class: ca,
                    type_args: ta,
                },
                Value::BoundClass {
                    class: cb,
                    type_args: tb,
                },
            ) => ca.as_ptr() == cb.as_ptr() && ta == tb,
            (Value::Enum(a), Value::Enum(b)) => a.as_ptr() == b.as_ptr(),
            (
                Value::Variant {
                    enum_name: ea,
                    variant_name: va,
                    data: da,
                },
                Value::Variant {
                    enum_name: eb,
                    variant_name: vb,
                    data: db,
                },
            ) => ea == eb && va == vb && da == db,
            (
                Value::VariantConstructor {
                    enum_type: ea,
                    variant_name: va,
                },
                Value::VariantConstructor {
                    enum_type: eb,
                    variant_name: vb,
                },
            ) => ea.as_ptr() == eb.as_ptr() && va == vb,
            (
                Value::BoundFunction {
                    function: fa,
                    type_args: ta,
                },
                Value::BoundFunction {
                    function: fb,
                    type_args: tb,
                },
            ) => fa == fb && ta == tb,
            (Value::Cell(a), Value::Cell(b)) => a.as_ptr() == b.as_ptr(),
            (Value::File(a), Value::File(b)) => a.as_ptr() == b.as_ptr(),
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
            Value::File(v) => write!(f, "<file {}>", v.borrow().name),
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
            Value::BoundClass { class, type_args } => {
                write!(f, "<class {}", class.borrow().name)?;
                if !type_args.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ">")?;
                }
                write!(f, ">")
            }
            Value::Interface(i) => write!(f, "<interface {}>", i.borrow().name),
            Value::BoundFunction { type_args, .. } => {
                write!(f, "<fn")?;
                if !type_args.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ">")?;
                }
                write!(f, ">")
            }
            Value::Function(_) => write!(f, "<fn>"),
            Value::BoundMethod { owner, .. } => write!(f, "<method of {}>", owner.borrow().name),
            Value::StaticMethod { owner, .. } => {
                write!(f, "<static method of {}>", owner.borrow().name)
            }
            Value::Closure(c) => write!(f, "<closure {}>", c.borrow().function),
            Value::Thunk(_) => write!(f, "<thunk>"),
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
            Value::Tuple(t) => {
                write!(f, "(")?;
                let t = t.borrow();
                for (i, v) in t.elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if let Some(name) = t
                        .names
                        .as_ref()
                        .and_then(|names| names.iter().find(|p| *p.1 == i).map(|p| p.0))
                    {
                        write!(f, "{}: ", name)?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }
            Value::Iterator(_) => write!(f, "<iterator>"),
            Value::Enum(e) => write!(f, "<enum {}>", e.borrow().name),
            Value::Variant {
                enum_name,
                variant_name,
                data,
            } => {
                write!(f, "{}.{}", enum_name, variant_name)?;
                if !data.is_empty() {
                    write!(f, "(")?;
                    for (i, v) in data.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", v)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Value::VariantConstructor {
                enum_type,
                variant_name,
            } => write!(
                f,
                "<{}.{} constructor>",
                enum_type.borrow().name,
                variant_name
            ),
            Value::BoundInterface {
                interface,
                type_args,
            } => {
                write!(f, "{}<", interface.borrow().name)?;
                for (i, arg) in type_args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
            Value::Cell(val) => write!(f, "<cell {}>", val.borrow().value),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunkState {
    pub callable: Value,
    pub value: Option<Value>,
}

impl ThunkState {
    pub fn new(callable: Value) -> Self {
        Self {
            callable,
            value: None,
        }
    }
}

impl GcObject for ThunkState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.callable.mark(gc);
        if let Some(value) = &self.value {
            value.mark(gc);
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

impl SetState {
    pub fn new(elements: HashSet<ValueKey>) -> Self {
        Self {
            elements,
            is_immutable: false,
        }
    }

    pub fn ensure_mutable(&self) -> Result<(), RuntimeError> {
        if self.is_immutable {
            Err(RuntimeError::new("cannot modify immutable set"))
        } else {
            Ok(())
        }
    }
}

impl std::ops::Deref for SetState {
    type Target = HashSet<ValueKey>;
    fn deref(&self) -> &Self::Target {
        &self.elements
    }
}

impl std::ops::DerefMut for SetState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.elements
    }
}

impl IntoIterator for SetState {
    type Item = ValueKey;
    type IntoIter = std::collections::hash_set::IntoIter<ValueKey>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<'a> IntoIterator for &'a SetState {
    type Item = &'a ValueKey;
    type IntoIter = std::collections::hash_set::Iter<'a, ValueKey>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub class: Option<GcRc<Class>>,
    pub fields: HashMap<String, Value>,
    pub is_immutable: bool,
    pub type_args: Option<Vec<Value>>,
}

impl Object {
    pub fn new(class: Option<GcRc<Class>>, type_args: Option<Vec<Value>>) -> Self {
        Self {
            class,
            fields: HashMap::new(),
            is_immutable: false,
            type_args,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TupleState {
    pub elements: Vec<Value>,
    pub names: Option<HashMap<String, usize>>,
}

impl TupleState {
    pub fn new(elements: Vec<Value>, names: Option<HashMap<String, usize>>) -> Self {
        Self { elements, names }
    }
}

impl GcObject for TupleState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for v in &self.elements {
            v.mark(gc);
        }
    }

    fn clear_refs(&mut self) {
        self.elements.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IteratorState {
    Array {
        array: GcRc<ArrayState>,
        index: usize,
        is_entries: bool,
    },
    Tuple {
        tuple: GcRc<TupleState>,
        index: usize,
    },
    Range {
        range: Range,
        current: i64,
    },
    Object {
        iterator: GcRc<Object>,
    },
    Map {
        map: GcRc<MapState>,
        keys: Vec<String>,
        index: usize,
        is_entries: bool,
    },
    Set {
        set: GcRc<SetState>,
        values: Vec<ValueKey>,
        index: usize,
    },
}

impl GcObject for IteratorState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        match self {
            IteratorState::Array { array, .. } => gc.mark_object(&array.to_gc_object()),
            IteratorState::Tuple { tuple, .. } => gc.mark_object(&tuple.to_gc_object()),
            IteratorState::Range { .. } => {}
            IteratorState::Object { iterator } => gc.mark_object(&iterator.to_gc_object()),
            IteratorState::Map { map, .. } => gc.mark_object(&map.to_gc_object()),
            IteratorState::Set { set, .. } => gc.mark_object(&set.to_gc_object()),
        }
    }

    fn clear_refs(&mut self) {
        match self {
            IteratorState::Array { array, .. } => array.borrow_mut().clear_refs(),
            IteratorState::Tuple { tuple, .. } => tuple.borrow_mut().clear_refs(),
            IteratorState::Range { .. } => {}
            IteratorState::Object { iterator } => iterator.borrow_mut().clear_refs(),
            IteratorState::Map { map, .. } => map.borrow_mut().clear_refs(),
            IteratorState::Set { set, .. } => set.borrow_mut().clear(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellState {
    pub value: Value,
}

impl CellState {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

impl GcObject for CellState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.value.mark(gc);
    }

    fn clear_refs(&mut self) {
        self.value = Value::Null;
    }
}

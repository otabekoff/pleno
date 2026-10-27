pub mod gc;
pub mod nan_value;
pub mod suggest;
pub mod text;

pub use gc::{GcObject, GcRc, MarkAndSweepGC};
use indexmap::{IndexMap, IndexSet};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
// use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::rc::Rc;

pub trait VMProvider {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.call_ext(callee, args, CallOptions::default())
    }
    /// `call` with borrowed arguments, for callbacks run once per element:
    /// the VM avoids building an argument vector for each call.
    fn call_with(&mut self, callee: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
        self.call_ext_with(callee.clone(), args, CallOptions::default())
    }
    /// `call_ext` with borrowed arguments.
    fn call_ext_with(
        &mut self,
        callee: Value,
        args: &[Value],
        opts: CallOptions,
    ) -> Result<Value, RuntimeError> {
        self.call_ext(callee, args.to_vec(), opts)
    }
    fn print(&mut self, text: &str) {
        print!("{}", text);
    }
    fn call_ext(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        opts: CallOptions,
    ) -> Result<Value, RuntimeError>;
    fn spawn(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError>;
    fn get_gc(&mut self) -> &mut gc::GCManager;
    fn create_promise(&mut self) -> Result<(u64, Value), RuntimeError>;
    fn resolve_promise(&mut self, id: u64, value: Value);
    fn reject_promise(&mut self, id: u64, reason: Value);
    /// Records that a rejected promise now has a handler (a late `.catch` or `.then`).
    fn mark_rejection_handled(&mut self, id: u64);
    /// Runs one `.then`/`.catch`/`.finally` registration against a settled promise.
    /// `info` is `[on_fulfilled, child_id, on_rejected, kind]` (kind 1 = finally).
    fn run_promise_callback(&mut self, info: &[Value], value: &Value, rejected: bool);
    fn register_multi_waiter(&mut self, waiter: MultiWaiter);
    fn check_purity(&mut self) -> Result<(), RuntimeError>;
    fn collect(&mut self);
    fn register_module(&mut self, name: String, value: Value);
    fn register_global(&mut self, name: String, value: Value);
    fn set_builtin_resolver(&mut self, resolver: fn(&str, &str) -> Option<NativeFunction>);
    /// Every method name `set_builtin_resolver`'s resolver knows, for any type:
    /// the names "did you mean" suggestions choose from.
    fn set_builtin_method_names(&mut self, _names: &'static [&'static str]) {}
    fn add_timer(&mut self, delay_ms: u64, callback: Value, args: Vec<Value>) -> u64;
    /// Removes a timer `add_timer` made, if it has not fired yet, so that it
    /// no longer keeps the program running.
    fn cancel_timer(&mut self, _id: u64) {}
    /// An `Error` instance for `message`, to reject a promise with. A
    /// `Name: rest` message whose name ends in `Error` (`TimeoutError: ...`)
    /// gives an error with that `name`, as a thrown native error does.
    fn error_value(&mut self, message: &str) -> Value {
        Value::String(Rc::new(message.to_string()))
    }
    #[cfg(feature = "net")]
    fn register_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        interest: mio::Interest,
        callback: Value,
        persistent: bool,
    ) -> Result<mio::Token, RuntimeError>;
    #[cfg(feature = "net")]
    fn unregister_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        token: mio::Token,
    ) -> Result<(), RuntimeError>;
    fn check_is_instance(&self, value: &Value, type_val: &Value) -> bool;
    fn get_named_args(&self) -> &IndexMap<String, Value>;
    fn is_io_allowed(&self) -> bool {
        true
    }
    fn is_network_allowed(&self) -> bool {
        true
    }
    fn is_read_allowed(&self, _path: &str) -> bool {
        true
    }
    fn is_write_allowed(&self, _path: &str) -> bool {
        true
    }
    fn get_max_memory(&self) -> usize {
        0 // 0 means unlimited
    }
    fn get_max_instructions(&self) -> u64 {
        0 // 0 means unlimited
    }
    /// How many instructions the VM has executed so far.
    fn instruction_count(&self) -> u64 {
        0
    }
    // Added for sandboxing
    fn create_sandbox(
        &self,
        _config: SandboxConfig,
    ) -> std::result::Result<Box<dyn VMProvider>, RuntimeError> {
        Err(RuntimeError::new("Sandbox creation not supported"))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub allow_io: bool,
    pub allow_network: bool,
    pub max_memory: usize,
    pub max_instructions: u64,
    pub allowed_read_paths: Vec<String>,
    pub allowed_write_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiWaiterMode {
    All,
    Race,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiWaiter {
    pub promise_id: u64,
    /// The promises still being waited on.
    pub dependencies: Vec<u64>,
    /// For `All`: the index in `results` of each dependency.
    pub slots: Vec<usize>,
    /// For `All`: how many of `results` are filled in.
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

    fn release_refs(&mut self) {
        self.results.clear();
    }
}

pub type NativeFunction =
    fn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError>;

fn dummy_native_function(_: &mut dyn VMProvider, _: &[Value]) -> Result<Value, RuntimeError> {
    Err(RuntimeError::new(
        "deserialized native function calls are not supported",
    ))
}

fn default_native_function_val() -> NativeFunction {
    dummy_native_function
}

/// A value. Arrays, maps and objects hold their elements as `Value`s, so the
/// type is kept to 16 bytes: every variant carries at most one machine word
/// (larger payloads, such as a range, are behind an `Rc`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Null,
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Range(Rc<Range>),
    Class(GcRc<Class>),
    Interface(GcRc<Interface>),
    Function(usize),
    BoundMethod(GcRc<BoundMethodState>),
    StaticMethod(GcRc<StaticMethodState>),
    Closure(GcRc<ClosureState>),
    Thunk(GcRc<ThunkState>),
    NativeFunction(#[serde(skip, default = "default_native_function_val")] NativeFunction),
    BoundNativeMethod(GcRc<BoundNativeMethodState>),
    Map(GcRc<MapState>),
    Array(GcRc<ArrayState>),
    Set(GcRc<SetState>),
    Object(GcRc<Object>),
    Partial(GcRc<PartialState>),
    /// Array operation that requires callback (map, filter, reduce)
    ArrayOp {
        kind: ArrayOpKind,
        array: GcRc<ArrayState>,
    },
    Mutex(GcRc<MutexState>),
    Channel(GcRc<ChannelState>),
    Tuple(GcRc<TupleState>),
    Iterator(GcRc<IteratorState>),
    BoundClass(GcRc<BoundClassState>),
    Enum(GcRc<EnumType>),
    Variant(GcRc<VariantState>),
    VariantConstructor(GcRc<VariantConstructorState>),
    BoundInterface(GcRc<BoundInterfaceState>),
    BoundFunction(GcRc<BoundFunctionState>),
    File(GcRc<FileState>),
    Database(GcRc<DatabaseState>),
    Resource(GcRc<NativeResource>),
    Cell(GcRc<CellState>),
    Char(char),
}

/// Optional parameters for function calls, reducing argument count.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub return_override: Option<Value>,
    pub closure_state: Option<GcRc<ClosureState>>,
    pub current_owner: Option<MethodOwner>,
    pub type_args: Vec<Value>,
    pub promise_id: Option<u64>,
}

impl CallOptions {
    pub fn new() -> Self {
        Self::default()
    }
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
            Value::Database(_) => "Database",
            Value::Resource(_) => "Resource",
            Value::Char(_) => "Char",
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
    pub closed: bool,
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

    fn release_refs(&mut self) {
        self.buffer.clear();
        self.send_waiters.clear();
    }
}

pub struct FileState {
    pub inner: RefCell<Option<Box<dyn Write>>>,
    pub name: String,
    /// Set for files opened in read mode (`File.open(path, "r")`).
    pub read_path: Option<String>,
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
            read_path: None,
        })
    }
}

pub struct DatabaseState {
    pub inner: RefCell<Option<Box<dyn std::any::Any>>>, // Boxed connection (rusqlite::Connection)
    pub connection_string: String,
    pub driver: String,
}

impl DatabaseState {
    pub fn new(driver: String, connection_string: String) -> Self {
        Self {
            inner: RefCell::new(None),
            connection_string,
            driver,
        }
    }
}

impl std::fmt::Debug for DatabaseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DatabaseState({{ driver: {}, connection_string: {} }})",
            self.driver, self.connection_string
        )
    }
}

impl Serialize for DatabaseState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("DatabaseState", 2)?;
        state.serialize_field("driver", &self.driver)?;
        state.serialize_field("connection_string", &self.connection_string)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for DatabaseState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DatabaseStateData {
            driver: String,
            connection_string: String,
        }
        let data = DatabaseStateData::deserialize(deserializer)?;
        Ok(DatabaseState {
            inner: RefCell::new(None),
            driver: data.driver,
            connection_string: data.connection_string,
        })
    }
}

pub struct NativeResource {
    pub inner: RefCell<Option<Box<dyn std::any::Any>>>,
    pub name: String,
}

impl NativeResource {
    pub fn new(name: String, resource: Box<dyn std::any::Any>) -> Self {
        Self {
            inner: RefCell::new(Some(resource)),
            name,
        }
    }
}

impl std::fmt::Debug for NativeResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NativeResource({{ name: {} }})", self.name)
    }
}

impl Serialize for NativeResource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // We only serialize the name, internal handle is lost
        let mut state = serializer.serialize_struct("NativeResource", 1)?;
        state.serialize_field("name", &self.name)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for NativeResource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NativeResourceData {
            name: String,
        }
        let data = NativeResourceData::deserialize(deserializer)?;
        Ok(NativeResource {
            inner: RefCell::new(None),
            name: data.name,
        })
    }
}

/// A resource nothing can reach any more is closed at once, even if a stray
/// reference keeps the `NativeResource` itself alive: dropping the handle
/// closes the socket or file it wraps.
impl GcObject for NativeResource {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}

    fn clear_refs(&mut self) {
        self.inner.get_mut().take();
    }
}

impl GcObject for DatabaseState {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {}
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
            Value::Function(_) => {}
            Value::BoundMethod(m) => gc.mark_object(&m.to_gc_object()),
            Value::StaticMethod(m) => gc.mark_object(&m.to_gc_object()),
            Value::Closure(c) => gc.mark_object(&c.to_gc_object()),
            Value::Thunk(t) => gc.mark_object(&t.to_gc_object()),
            Value::BoundNativeMethod(m) => gc.mark_object(&m.to_gc_object()),
            Value::Map(m) => gc.mark_object(&m.to_gc_object()),
            Value::Array(a) => gc.mark_object(&a.to_gc_object()),
            Value::Set(s) => gc.mark_object(&s.to_gc_object()),
            Value::Object(o) => gc.mark_object(&o.to_gc_object()),
            Value::Partial(p) => gc.mark_object(&p.to_gc_object()),
            Value::BoundFunction(f) => gc.mark_object(&f.to_gc_object()),
            Value::ArrayOp { array, .. } => gc.mark_object(&array.to_gc_object()),
            Value::Mutex(m) => gc.mark_object(&m.to_gc_object()),
            Value::Channel(c) => gc.mark_object(&c.to_gc_object()),
            Value::Tuple(t) => gc.mark_object(&t.to_gc_object()),
            Value::Iterator(i) => gc.mark_object(&i.to_gc_object()),
            Value::BoundClass(c) => gc.mark_object(&c.to_gc_object()),
            Value::Enum(e) => gc.mark_object(&e.to_gc_object()),
            Value::Variant(v) => gc.mark_object(&v.to_gc_object()),
            Value::VariantConstructor(v) => gc.mark_object(&v.to_gc_object()),
            Value::BoundInterface(i) => gc.mark_object(&i.to_gc_object()),
            Value::File(f) => gc.mark_object(&f.to_gc_object()),
            Value::Database(d) => gc.mark_object(&d.to_gc_object()),
            Value::Resource(r) => gc.mark_object(&r.to_gc_object()),
            Value::Cell(c) => gc.mark_object(&c.to_gc_object()),
            Value::Char(_) => {}
            _ => {}
        }
    }

    fn clear_refs(&mut self) {
        // GcRc's clear themselves when drop count hits 0 and they are swept.
        // If we need manual clearing of cycles, we do it in their state structs.
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

    fn byte_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.elements.len() * std::mem::size_of::<Value>()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapState {
    pub entries: IndexMap<String, Value>,
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

    fn byte_size(&self) -> usize {
        // Rough estimate for HashMap: entries * (size_of String + size_of Value)
        std::mem::size_of::<Self>()
            + self.entries.len() * (std::mem::size_of::<String>() + std::mem::size_of::<Value>())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetState {
    pub elements: IndexSet<ValueKey>,
    pub is_immutable: bool,
}

impl GcObject for SetState {
    fn mark(&self, _gc: &mut MarkAndSweepGC) {
        for _ in &self.elements {
            // ValueKey doesn't have GC'd objects currently, but we mark just in case
            // actually ValueKey is Null, Bool, Int, Float, String, Char.
            // String is Rc<String>, not GcRc.
        }
    }

    fn clear_refs(&mut self) {
        self.elements.clear();
    }

    fn byte_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.elements.len() * std::mem::size_of::<ValueKey>()
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

impl StackFrame {
    /// The line that stands for the frames a deep backtrace leaves out.
    pub fn omitted(count: usize) -> Self {
        Self {
            function_name: format!("... {count} more frames"),
            span: None,
        }
    }

    /// Whether this is an `omitted` line rather than a function.
    pub fn is_omitted(&self) -> bool {
        self.span.is_none() && self.function_name.starts_with("... ")
    }

    /// `at f`, or the `omitted` line as it is.
    pub fn describe(&self) -> String {
        if self.is_omitted() {
            self.function_name.clone()
        } else {
            format!("at {}", self.function_name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeError {
    pub message: String,
    pub span: Option<pleno_common::Span>,
    pub stack_trace: Vec<StackFrame>,
    pub handled: bool,
    /// Ends the program and cannot be caught: a sandbox limit was reached.
    pub fatal: bool,
    /// Set by `exit(code)`: the program asked to end with this status. It
    /// unwinds as a `SystemExit` error, which runs `finally` blocks and `with`
    /// closes but which an untyped `catch` lets through; when nothing catches
    /// it, the host decides what ending means (the `pleno` CLI exits the
    /// process with the code).
    #[serde(default)]
    pub exit_code: Option<i32>,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            stack_trace: Vec::new(),
            handled: false,
            fatal: false,
            exit_code: None,
        }
    }

    pub fn handled() -> Self {
        Self {
            message: "exception handled".to_string(),
            span: None,
            stack_trace: Vec::new(),
            handled: true,
            fatal: false,
            exit_code: None,
        }
    }

    /// An error that `catch` cannot stop, such as a sandbox limit: catching it
    /// would let a program run on past the limit.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            fatal: true,
            ..Self::new(message)
        }
    }

    /// `exit(code)`: ends the program with `code` (see `exit_code`). The
    /// message is what a `SystemExit` error is made from wherever the error
    /// becomes a Pleno exception (`split_error_message` gives its name).
    pub fn exit(code: i32) -> Self {
        Self {
            exit_code: Some(code),
            ..Self::new(format!("{SYSTEM_EXIT_CLASS}: exit({code})"))
        }
    }

    /// The status `exit(code)` asked for, if this error is an exit: its
    /// `exit_code`, or the code in a message that went through a promise or
    /// a callback as text.
    pub fn exit_status(&self) -> Option<i32> {
        self.exit_code.or_else(|| Self::exit_code_in(&self.message))
    }

    /// The code of a `SystemExit: exit(N)` message, also after it was wrapped
    /// (`uncaught exception: SystemExit: exit(N)`).
    pub fn exit_code_in(message: &str) -> Option<i32> {
        let rest = message.split("SystemExit: exit(").nth(1)?;
        rest.split(')').next()?.parse().ok()
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

impl pleno_diagnostics::ToDiagnostic for RuntimeError {
    fn to_diagnostic(&self) -> pleno_diagnostics::Diagnostic {
        let mut diagnostic = pleno_diagnostics::Diagnostic::error(&self.message);
        if let Some(span) = self.span {
            diagnostic = diagnostic.with_label(span, "runtime error");
        }
        // Add stack trace as notes
        for frame in &self.stack_trace {
            if let Some(_span) = frame.span {
                // We can't easily add multiple spans to one diagnostic yet (Diagnostic support?),
                // or maybe we just add them as notes for now.
                // Actually DiagnosticBuilder has `with_label`.
                // But stack frames are in different files or locations.
                // SourceMap handles one file?
                // Current SourceMap handles one file.
                // So we can only properly report the top-most error if it matches the current file.
                // For now, let's just use the main span.
                // We can format stack trace into the message or notes.
            }
            diagnostic = diagnostic.with_note(frame.describe());
        }
        diagnostic
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenericParamMetadata {
    pub name: String,
    pub variance: pleno_common::Variance,
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
    pub methods: IndexMap<String, MethodSignature>,
    pub getters: IndexMap<String, MethodSignature>,
    pub setters: IndexMap<String, MethodSignature>,
    pub type_params: Vec<GenericParamMetadata>,
    pub type_constraints: IndexMap<String, Vec<String>>,
    pub annotations: Vec<RuntimeAnnotation>,
    pub module: Option<Rc<String>>,
}

impl GcObject for Interface {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        mark_annotations(&self.annotations, gc);
    }

    fn release_refs(&mut self) {
        self.annotations.clear();
    }
}

fn mark_annotations(annotations: &[RuntimeAnnotation], gc: &mut MarkAndSweepGC) {
    for annotation in annotations {
        for arg in &annotation.args {
            arg.mark(gc);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundClassState {
    pub class: GcRc<Class>,
    pub type_args: Vec<Value>,
}

impl GcObject for BoundClassState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        gc.mark_object(&self.class.to_gc_object());
        for arg in &self.type_args {
            arg.mark(gc);
        }
    }

    // A released class clears its own references too.
    fn release_refs(&mut self) {
        self.type_args.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantState {
    pub enum_type: GcRc<EnumType>,
    pub variant_name: Rc<String>,
    pub data: Vec<Value>,
}

impl GcObject for VariantState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        gc.mark_object(&self.enum_type.to_gc_object());
        for arg in &self.data {
            arg.mark(gc);
        }
    }

    fn release_refs(&mut self) {
        self.data.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantConstructorState {
    pub enum_type: GcRc<EnumType>,
    pub variant_name: Rc<String>,
}

impl GcObject for VariantConstructorState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        gc.mark_object(&self.enum_type.to_gc_object());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundInterfaceState {
    pub interface: GcRc<Interface>,
    pub type_args: Vec<Value>,
}

impl GcObject for BoundInterfaceState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        gc.mark_object(&self.interface.to_gc_object());
        for arg in &self.type_args {
            arg.mark(gc);
        }
    }

    fn release_refs(&mut self) {
        self.type_args.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumType {
    pub name: Rc<String>,
    pub variants: IndexMap<String, EnumVariantDef>,
    pub methods: IndexMap<String, MethodInfo>,
    pub static_methods: IndexMap<String, MethodInfo>,
    pub module: Option<Rc<String>>,
    #[serde(default)]
    pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariantDef {
    pub index: usize,
    pub param_count: usize,
    #[serde(default)]
    pub annotations: Vec<RuntimeAnnotation>,
    /// Explicit value, as in `Active = 1`.
    #[serde(default)]
    pub value: Option<Value>,
}

impl GcObject for EnumType {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for m in self.methods.values() {
            m.mark(gc);
        }
        for m in self.static_methods.values() {
            m.mark(gc);
        }
        for variant in self.variants.values() {
            if let Some(value) = &variant.value {
                value.mark(gc);
            }
            mark_annotations(&variant.annotations, gc);
        }
        mark_annotations(&self.annotations, gc);
    }

    fn clear_refs(&mut self) {
        self.methods.clear();
        self.static_methods.clear();
    }

    fn release_refs(&mut self) {
        self.clear_refs();
        self.variants.clear();
        self.annotations.clear();
    }
}

impl PartialEq for EnumType {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.variants == other.variants
            && self.methods == other.methods
            && self.static_methods == other.static_methods
            && self.module == other.module
            && self.annotations == other.annotations
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class {
    pub name: Rc<String>,
    pub superclass: Option<GcRc<Class>>,
    pub static_fields: IndexMap<String, Value>,
    pub methods: IndexMap<String, MethodInfo>,
    pub static_methods: IndexMap<String, MethodInfo>,
    pub constructor: Option<MethodInfo>,
    #[serde(skip, default)]
    pub native_constructor: Option<NativeFunction>,
    pub operators: IndexMap<String, MethodInfo>,
    pub fields: IndexMap<String, MethodVisibility>,
    pub field_types: IndexMap<String, Option<String>>,
    pub static_field_visibility: IndexMap<String, MethodVisibility>,
    pub static_field_types: IndexMap<String, Option<String>>,
    pub getters: IndexMap<String, MethodInfo>,
    pub setters: IndexMap<String, MethodInfo>,
    pub static_getters: IndexMap<String, MethodInfo>,
    pub static_setters: IndexMap<String, MethodInfo>,
    pub module: Option<Rc<String>>,
    pub is_abstract: bool,
    /// Methods this class declares `abstract`: a subclass must define them
    /// before it can be instantiated.
    #[serde(default)]
    pub abstract_methods: Vec<String>,
    /// Instance fields declared `const`: only a constructor may assign them.
    #[serde(default)]
    pub const_fields: Vec<String>,
    /// Static fields declared `const`: set once, when the class is declared.
    #[serde(default)]
    pub const_static_fields: Vec<String>,
    pub is_pooled: bool,
    pub implements: Vec<Value>,
    pub type_params: Vec<GenericParamMetadata>,
    pub type_constraints: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub annotations: Vec<RuntimeAnnotation>,
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
        for m in self.methods.values() {
            m.mark(gc);
        }
        for m in self.static_methods.values() {
            m.mark(gc);
        }
        for m in self.getters.values() {
            m.mark(gc);
        }
        for m in self.setters.values() {
            m.mark(gc);
        }
        for m in self.static_getters.values() {
            m.mark(gc);
        }
        for m in self.static_setters.values() {
            m.mark(gc);
        }
        for m in self.operators.values() {
            m.mark(gc);
        }
        if let Some(constructor) = &self.constructor {
            constructor.mark(gc);
        }
        mark_annotations(&self.annotations, gc);
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

    fn release_refs(&mut self) {
        self.clear_refs();
        self.constructor = None;
        self.annotations.clear();
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
            && self.annotations == other.annotations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodVisibility {
    Public,
    Private,
    Protected,
    Internal,
}

/// The annotation that marks the built-in `Comparable` interface.
pub const BUILTIN_COMPARABLE: &str = "__builtin_comparable";

impl Interface {
    /// Whether this is the built-in `Comparable`, not one a program declared.
    pub fn is_builtin_comparable(&self) -> bool {
        self.annotations
            .iter()
            .any(|a| a.name == BUILTIN_COMPARABLE)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeAnnotation {
    pub name: String,
    pub args: Vec<Value>,
}

impl RuntimeAnnotation {
    pub fn from_value(val: &Value) -> Option<Self> {
        match val {
            Value::Object(obj) => {
                let borrowed = obj.borrow();
                let name = borrowed.fields.get("name")?.as_str()?.to_string();
                let args = match borrowed.fields.get("args")? {
                    Value::Array(arr) => arr.borrow().elements.clone(),
                    _ => Vec::new(),
                };
                Some(Self { name, args })
            }
            Value::Map(map) => {
                let borrowed = map.borrow();
                let name = borrowed.entries.get("name")?.as_str()?.to_string();
                let args = match borrowed.entries.get("args")? {
                    Value::Array(arr) => arr.borrow().elements.clone(),
                    _ => Vec::new(),
                };
                Some(Self { name, args })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MethodOwner {
    Class(GcRc<Class>),
    Enum(GcRc<EnumType>),
    Interface(GcRc<Interface>),
    /// An extension method on a built-in type such as `String` (`extend String { ... }`).
    Builtin(Rc<String>),
}

impl MethodOwner {
    pub fn name(&self) -> Rc<String> {
        match self {
            MethodOwner::Class(c) => c.borrow().name.clone(),
            MethodOwner::Enum(e) => e.borrow().name.clone(),
            MethodOwner::Interface(i) => i.borrow().name.clone(),
            MethodOwner::Builtin(name) => name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodInfo {
    pub function: Value,
    pub visibility: MethodVisibility,
    pub owner: MethodOwner,
    pub module: Option<Rc<String>>,
    #[serde(default)]
    pub annotations: Vec<RuntimeAnnotation>,
}

impl MethodInfo {
    pub fn mark(&self, gc: &mut MarkAndSweepGC) {
        mark_owner(&self.owner, gc);
        self.function.mark(gc);
        mark_annotations(&self.annotations, gc);
    }
}

impl PartialEq for PartialState {
    fn eq(&self, other: &Self) -> bool {
        self.callee == other.callee
            && self.args == other.args
            && self.placeholders == other.placeholders
    }
}

impl PartialEq for BoundMethodState {
    fn eq(&self, other: &Self) -> bool {
        self.receiver == other.receiver
            && self.function == other.function
            && self.owner == other.owner
            && self.visibility == other.visibility
    }
}

impl PartialEq for StaticMethodState {
    fn eq(&self, other: &Self) -> bool {
        self.function == other.function
            && self.owner == other.owner
            && self.visibility == other.visibility
    }
}

impl PartialEq for BoundNativeMethodState {
    fn eq(&self, other: &Self) -> bool {
        self.receiver == other.receiver && (self.function as usize) == (other.function as usize)
    }
}

impl PartialEq for BoundFunctionState {
    fn eq(&self, other: &Self) -> bool {
        self.function == other.function && self.type_args == other.type_args
    }
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
            (Value::BoundMethod(a), Value::BoundMethod(b)) => a == b,
            (Value::StaticMethod(a), Value::StaticMethod(b)) => a == b,
            (Value::Closure(ca), Value::Closure(cb)) => ca == cb,
            (Value::NativeFunction(a), Value::NativeFunction(b)) => (*a as usize) == (*b as usize),
            (Value::BoundNativeMethod(a), Value::BoundNativeMethod(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Array(a), Value::Array(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Set(a), Value::Set(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Object(a), Value::Object(b)) => a.as_ptr() == b.as_ptr(),
            (Value::Partial(a), Value::Partial(b)) => a == b,
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
            (Value::BoundFunction(a), Value::BoundFunction(b)) => a == b,
            (Value::BoundClass(a), Value::BoundClass(b)) => a == b,
            (Value::Enum(a), Value::Enum(b)) => a == b,
            // Variants are values: `Status.Active == Status.Active` even though each
            // access constructs a new variant.
            (Value::Variant(a), Value::Variant(b)) => {
                let (a, b) = (a.borrow(), b.borrow());
                a.enum_type == b.enum_type && a.variant_name == b.variant_name && a.data == b.data
            }
            (Value::VariantConstructor(a), Value::VariantConstructor(b)) => a == b,
            (Value::BoundInterface(a), Value::BoundInterface(b)) => a == b,
            (Value::Cell(a), Value::Cell(b)) => a == b,
            (Value::File(a), Value::File(b)) => a == b,
            (Value::Database(a), Value::Database(b)) => a == b,
            (Value::Resource(a), Value::Resource(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            _ => false,
        }
    }
}

/// How deep `print` follows nested containers. Deeper values, and a container
/// that contains itself, print as `[...]` (or `{...}`, `(...)`) instead of
/// overflowing the stack.
const MAX_DISPLAY_DEPTH: usize = 200;

thread_local! {
    static DISPLAYING: RefCell<Vec<*const ()>> = const { RefCell::new(Vec::new()) };
}

struct DisplayGuard;

impl DisplayGuard {
    fn enter(ptr: *const ()) -> Option<Self> {
        DISPLAYING.with(|stack| {
            let mut stack = stack.borrow_mut();
            if stack.len() >= MAX_DISPLAY_DEPTH || stack.contains(&ptr) {
                return None;
            }
            stack.push(ptr);
            Some(DisplayGuard)
        })
    }
}

impl Drop for DisplayGuard {
    fn drop(&mut self) {
        DISPLAYING.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// Writes a value that sits inside a container. Strings and characters are
/// quoted there, so `["1", 1]` prints differently from `[1, 1]`.
fn write_nested(f: &mut std::fmt::Formatter<'_>, value: &Value) -> std::fmt::Result {
    match value {
        Value::String(s) => write_quoted_str(f, s),
        Value::Char(c) => write_quoted_char(f, *c),
        other => write!(f, "{}", other),
    }
}

fn write_escaped_char(f: &mut std::fmt::Formatter<'_>, c: char, quote: char) -> std::fmt::Result {
    match c {
        '\\' => f.write_str("\\\\"),
        '\n' => f.write_str("\\n"),
        '\r' => f.write_str("\\r"),
        '\t' => f.write_str("\\t"),
        '\0' => f.write_str("\\0"),
        c if c == quote => write!(f, "\\{}", c),
        c if c.is_control() => write!(f, "\\u{{{:x}}}", c as u32),
        c => write!(f, "{}", c),
    }
}

/// Writes `s` as a double-quoted string literal that Pleno reads back as `s`.
pub fn write_quoted_str(f: &mut std::fmt::Formatter<'_>, s: &str) -> std::fmt::Result {
    f.write_str("\"")?;
    for c in s.chars() {
        write_escaped_char(f, c, '"')?;
    }
    f.write_str("\"")
}

fn write_quoted_char(f: &mut std::fmt::Formatter<'_>, c: char) -> std::fmt::Result {
    f.write_str("'")?;
    write_escaped_char(f, c, '\'')?;
    f.write_str("'")
}

/// `Ok(1)`, `Err("x")`, `Some(2)` and `None` are maps underneath; they print
/// the way they are written.
fn write_result_or_option(
    f: &mut std::fmt::Formatter<'_>,
    entries: &IndexMap<String, Value>,
) -> Option<std::fmt::Result> {
    let Some(Value::String(variant)) = entries.get("variant") else {
        return None;
    };
    let payload = match (variant.as_str(), entries.len()) {
        ("None", 1) => return Some(f.write_str("None")),
        ("Ok" | "Some", 2) => entries.get("value")?,
        ("Err", 2) => entries.get("error")?,
        _ => return None,
    };
    Some(
        write!(f, "{}(", variant)
            .and_then(|_| write_nested(f, payload))
            .and_then(|_| f.write_str(")")),
    )
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Missing => write!(f, "<missing>"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Int(v) => write!(f, "{}", v),
            // `{:?}` keeps a `.0` on whole numbers (`3.0`), gives the shortest
            // text that reads back as the same Float, and switches to an
            // exponent for very large and very small numbers (`1e21`, `1e-7`).
            Value::Float(v) => write!(f, "{:?}", v),
            Value::String(v) => write!(f, "{}", v),
            Value::Char(v) => write!(f, "{}", v),
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
            Value::BoundClass(state) => {
                let state = state.borrow();
                let class = &state.class;
                let type_args = &state.type_args;
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
            Value::BoundInterface(state) => {
                let state = state.borrow();
                let interface = &state.interface;
                let type_args = &state.type_args;
                write!(f, "<interface {}", interface.borrow().name)?;
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
            Value::BoundFunction(state) => {
                let state = state.borrow();
                write!(f, "<bound function {}>", state.function)
            }
            Value::Function(_) => write!(f, "<fn>"),
            Value::BoundMethod(state) => {
                let state = state.borrow();
                write!(f, "<method of {}>", state.owner.name())
            }
            Value::StaticMethod(state) => {
                let state = state.borrow();
                write!(f, "<static method of {}>", state.owner.name())
            }
            Value::Closure(c) => write!(f, "<closure {}>", c.borrow().function),
            Value::Thunk(_) => write!(f, "<thunk>"),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::BoundNativeMethod(_) => write!(f, "<native method>"),
            Value::Map(m) => {
                let Some(_guard) = DisplayGuard::enter(m.as_ptr() as *const ()) else {
                    return write!(f, "{{...}}");
                };
                let map = m.borrow();
                if let Some(result) = write_result_or_option(f, &map.entries) {
                    return result;
                }
                write!(f, "{{")?;
                for (i, (k, v)) in map.entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write_quoted_str(f, k)?;
                    write!(f, ": ")?;
                    write_nested(f, v)?;
                }
                write!(f, "}}")
            }
            Value::Array(a) => {
                let Some(_guard) = DisplayGuard::enter(a.as_ptr() as *const ()) else {
                    return write!(f, "[...]");
                };
                write!(f, "[")?;
                let elements = &a.borrow().elements;
                for (i, v) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write_nested(f, v)?;
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
                        ValueKey::Float(bits) => write!(f, "{:?}", f64::from_bits(*bits))?,
                        ValueKey::String(v) => write_quoted_str(f, v)?,
                        ValueKey::Char(v) => write_quoted_char(f, *v)?,
                    }
                }
                write!(f, "}}")
            }
            Value::Object(o) => {
                if let Some(text) = o.borrow().error_text() {
                    write!(f, "{}", text)
                } else if let Some(class) = &o.borrow().class {
                    write!(f, "<instance of {}>", class.borrow().name)
                } else {
                    write!(f, "<object>")
                }
            }
            Value::Partial(_) => write!(f, "<partial>"),
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
                let Some(_guard) = DisplayGuard::enter(t.as_ptr() as *const ()) else {
                    return write!(f, "(...)");
                };
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
                    write_nested(f, v)?;
                }
                // `(1,)`: a tuple of one element prints the way it is written.
                if t.elements.len() == 1 && t.names.is_none() {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            Value::Iterator(_) => write!(f, "<iterator>"),
            Value::Enum(e) => write!(f, "<enum {}>", e.borrow().name),
            Value::Variant(state) => {
                let state = state.borrow();
                write!(
                    f,
                    "{}.{}",
                    state.enum_type.borrow().name,
                    state.variant_name
                )?;
                if !state.data.is_empty() {
                    write!(f, "(")?;
                    for (i, v) in state.data.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write_nested(f, v)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Value::VariantConstructor(state) => {
                let state = state.borrow();
                write!(
                    f,
                    "<{}.{} constructor>",
                    state.enum_type.borrow().name,
                    state.variant_name
                )
            }
            Value::Cell(val) => write!(f, "<cell {}>", val.borrow().value),
            Value::Database(db) => write!(
                f,
                "<database {} ({})>",
                db.borrow().driver,
                db.borrow().connection_string
            ),
            Value::Resource(r) => write!(f, "<resource {}>", r.borrow().name),
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

    fn release_refs(&mut self) {
        self.captures.clear();
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

    fn release_refs(&mut self) {
        self.callable = Value::Null;
        self.value = None;
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
    pub fn new(entries: IndexMap<String, Value>) -> Self {
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
    type Target = IndexMap<String, Value>;
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
    type IntoIter = indexmap::map::IntoIter<String, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a MapState {
    type Item = (&'a String, &'a Value);
    type IntoIter = indexmap::map::Iter<'a, String, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl SetState {
    pub fn new(elements: IndexSet<ValueKey>) -> Self {
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
    type Target = IndexSet<ValueKey>;
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
    type IntoIter = indexmap::set::IntoIter<ValueKey>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<'a> IntoIterator for &'a SetState {
    type Item = &'a ValueKey;
    type IntoIter = indexmap::set::Iter<'a, ValueKey>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub class: Option<GcRc<Class>>,
    pub fields: IndexMap<String, Value>,
    pub is_immutable: bool,
    pub type_args: Option<Vec<Value>>,
}

impl Object {
    pub fn new(class: Option<GcRc<Class>>, type_args: Option<Vec<Value>>) -> Self {
        Self {
            class,
            fields: IndexMap::new(),
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

    /// The error's name, `Error` for the built-in class or the subclass name
    /// unless the `name` field says otherwise. `None` if this is not an error.
    pub fn error_name(&self) -> Option<String> {
        let class = self.class.as_ref()?;
        if !class.borrow().extends_error() {
            return None;
        }
        Some(match self.fields.get("name") {
            Some(Value::String(name)) => name.to_string(),
            _ => class.borrow().name.to_string(),
        })
    }

    /// How an error prints: `name: message`, or only the name without a message.
    pub fn error_text(&self) -> Option<String> {
        let name = self.error_name()?;
        Some(match self.fields.get("message") {
            None | Some(Value::Null) => name,
            Some(Value::String(message)) if message.is_empty() => name,
            Some(message) => format!("{name}: {message}"),
        })
    }

    /// How an uncaught error is reported: like [`Object::error_text`], but a
    /// plain `Error` shows only its message, as the runtime always has.
    pub fn error_report(&self) -> Option<String> {
        let plain = self
            .class
            .as_ref()
            .is_some_and(|class| class.borrow().is_builtin_error())
            && matches!(self.fields.get("name"), Some(Value::String(name)) if name.as_str() == ERROR_CLASS);
        match self.fields.get("message") {
            Some(Value::String(message)) if plain && !message.is_empty() => {
                Some(message.to_string())
            }
            _ => self.error_text(),
        }
    }
}

/// The name of the class every error extends.
pub const ERROR_CLASS: &str = "Error";
/// The built-in subclass of `Error` that `exit(code)` throws.
pub const SYSTEM_EXIT_CLASS: &str = "SystemExit";

impl Class {
    /// A class named `name` without members, as `class Name {}` declares.
    pub fn empty(name: &str) -> Self {
        Self {
            name: Rc::new(name.to_string()),
            superclass: None,
            static_fields: IndexMap::new(),
            methods: IndexMap::new(),
            static_methods: IndexMap::new(),
            constructor: None,
            native_constructor: None,
            operators: IndexMap::new(),
            fields: IndexMap::new(),
            field_types: IndexMap::new(),
            static_field_visibility: IndexMap::new(),
            static_field_types: IndexMap::new(),
            getters: IndexMap::new(),
            setters: IndexMap::new(),
            static_getters: IndexMap::new(),
            static_setters: IndexMap::new(),
            module: None,
            is_abstract: false,
            abstract_methods: Vec::new(),
            const_fields: Vec::new(),
            const_static_fields: Vec::new(),
            is_pooled: false,
            implements: Vec::new(),
            type_params: Vec::new(),
            type_constraints: IndexMap::new(),
            annotations: Vec::new(),
        }
    }

    /// The built-in `Error` class: `Error(message)` makes an instance with
    /// `message` and `name` fields, and classes extend it for their own errors.
    pub fn error() -> Self {
        let mut class = Self::empty(ERROR_CLASS);
        class.native_constructor = Some(error_constructor);
        for field in ["message", "name"] {
            class
                .fields
                .insert(field.to_string(), MethodVisibility::Public);
            class
                .field_types
                .insert(field.to_string(), Some("String".to_string()));
        }
        class
    }

    /// The built-in `SystemExit` class, a subclass of `error` with an Int
    /// `code` field.
    pub fn system_exit(error: GcRc<Class>) -> Self {
        let mut class = Self::empty(SYSTEM_EXIT_CLASS);
        class.superclass = Some(error);
        class
            .fields
            .insert("code".to_string(), MethodVisibility::Public);
        class
            .field_types
            .insert("code".to_string(), Some("Int".to_string()));
        class
    }

    /// Whether this is the built-in `Error` class (not a subclass or a
    /// user class that happens to share its name).
    pub fn is_builtin_error(&self) -> bool {
        self.superclass.is_none()
            && self.name.as_str() == ERROR_CLASS
            && self.native_constructor.is_some()
    }

    /// Whether this is the built-in `Error` class or extends it.
    pub fn extends_error(&self) -> bool {
        if self.is_builtin_error() {
            return true;
        }
        let mut current = self.superclass.clone();
        while let Some(class) = current {
            if class.borrow().is_builtin_error() {
                return true;
            }
            current = class.borrow().superclass.clone();
        }
        false
    }

    /// The native constructor of this class or the nearest ancestor that has one.
    pub fn resolve_native_constructor(&self) -> Option<NativeFunction> {
        if let Some(constructor) = self.native_constructor {
            return Some(constructor);
        }
        let mut current = self.superclass.clone();
        while let Some(class) = current {
            if let Some(constructor) = class.borrow().native_constructor {
                return Some(constructor);
            }
            current = class.borrow().superclass.clone();
        }
        None
    }
}

/// Splits a runtime error message such as `TypeError: expected Int` into its
/// name (`TypeError`) and the rest; messages without such a prefix are `Error`s.
pub fn split_error_message(message: &str) -> (&str, &str) {
    // `exit(code)` (see `RuntimeError::exit`) is the one name not ending in Error.
    if let Some(rest) = message
        .strip_prefix(SYSTEM_EXIT_CLASS)
        .and_then(|rest| rest.strip_prefix(": "))
    {
        return (SYSTEM_EXIT_CLASS, rest);
    }
    if let Some((name, rest)) = message.split_once(": ")
        && name.len() > ERROR_CLASS.len()
        && name.ends_with(ERROR_CLASS)
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return (name, rest);
    }
    (ERROR_CLASS, message)
}

/// `Error(message)`: called with the new instance first, it sets `message`
/// (a String, empty when omitted) and `name` (the instance's class name).
fn error_constructor(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let Some(Value::Object(object)) = args.first() else {
        return Err(RuntimeError::new(
            "Error constructor called without an instance",
        ));
    };
    let message = match args.get(1) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(message)) => message.to_string(),
        Some(other) => other.to_string(),
    };
    let mut object_ref = object.borrow_mut();
    let name = object_ref
        .class
        .as_ref()
        .map(|class| class.borrow().name.to_string())
        .unwrap_or_else(|| ERROR_CLASS.to_string());
    object_ref
        .fields
        .insert("message".to_string(), Value::String(Rc::new(message)));
    object_ref
        .fields
        .insert("name".to_string(), Value::String(Rc::new(name)));
    drop(object_ref);
    Ok(Value::Object(object.clone()))
}

/// Whether an Int and a Float are the same number, exactly: `5 == 5.0`, but
/// `9007199254740993` is not the Float it rounds to.
pub fn int_equals_float(int: i64, float: f64) -> bool {
    // 2^63 is exactly representable; every Float in [-2^63, 2^63) converts to i64.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    float.fract() == 0.0 && (-LIMIT..LIMIT).contains(&float) && float as i64 == int
}

/// Decides `==` for two different objects (see `values_equal_by`): `Some`
/// with a class's `operator ==` result, `None` to compare them by identity.
pub type ObjectEquality<'a> = dyn FnMut(&Value, &Value) -> Result<Option<bool>, RuntimeError> + 'a;

/// `==` as programs see it, for values without a class `operator ==` (see
/// `values_equal_by`).
pub fn values_equal(a: &Value, b: &Value) -> bool {
    values_equal_by(a, b, &mut |_, _| Ok(None)).unwrap_or(false)
}

/// `==` as programs see it. Numbers compare by value across Int and Float
/// (`5 == 5.0`). Arrays, tuples, maps and sets compare by contents: arrays
/// and tuples element by element (a named tuple's names too), maps by their
/// keys and values in any order, sets by their elements. Other values,
/// class instances among them, are equal when they are the same value.
///
/// `objects` is asked first about two different objects, inside containers
/// too, and returns `Some` when a class's `operator ==` decides. The walk
/// uses a worklist, so deep nesting cannot overflow the stack, and treats a
/// pair of containers it is already comparing as equal, so values that
/// contain themselves compare without looping.
pub fn values_equal_by(
    a: &Value,
    b: &Value,
    objects: &mut ObjectEquality<'_>,
) -> Result<bool, RuntimeError> {
    // Most comparisons are of two plain values: no worklist for those.
    let containers = matches!(
        (a, b),
        (Value::Array(_), Value::Array(_))
            | (Value::Tuple(_), Value::Tuple(_))
            | (Value::Map(_), Value::Map(_))
    );
    if !containers {
        return match (a, b) {
            (Value::Int(int), Value::Float(float)) | (Value::Float(float), Value::Int(int)) => {
                Ok(int_equals_float(*int, *float))
            }
            (Value::Set(x), Value::Set(y)) => {
                let x = x.borrow();
                let y = y.borrow();
                Ok(x.elements.len() == y.elements.len()
                    && x.elements.iter().all(|key| y.elements.contains(key)))
            }
            (Value::Object(x), Value::Object(y)) if x.as_ptr() != y.as_ptr() => {
                Ok(objects(a, b)?.unwrap_or(false))
            }
            _ => Ok(a == b),
        };
    }
    let mut pending = vec![(a.clone(), b.clone())];
    let mut seen = std::collections::HashSet::new();
    while let Some((a, b)) = pending.pop() {
        let pair = match (&a, &b) {
            (Value::Array(x), Value::Array(y)) => Some((x.as_ptr() as usize, y.as_ptr() as usize)),
            (Value::Tuple(x), Value::Tuple(y)) => Some((x.as_ptr() as usize, y.as_ptr() as usize)),
            (Value::Map(x), Value::Map(y)) => Some((x.as_ptr() as usize, y.as_ptr() as usize)),
            _ => None,
        };
        if let Some((x, y)) = pair
            && (x == y || !seen.insert((x, y)))
        {
            continue;
        }
        match (&a, &b) {
            (Value::Int(int), Value::Float(float)) | (Value::Float(float), Value::Int(int)) => {
                if !int_equals_float(*int, *float) {
                    return Ok(false);
                }
            }
            (Value::Array(x), Value::Array(y)) => {
                // Copied out first: an `operator ==` may change either array.
                let xs = x.borrow().elements.clone();
                let ys = y.borrow().elements.clone();
                if xs.len() != ys.len() {
                    return Ok(false);
                }
                pending.extend(xs.into_iter().zip(ys).rev());
            }
            (Value::Tuple(x), Value::Tuple(y)) => {
                let (xs, x_names) = {
                    let x = x.borrow();
                    (x.elements.clone(), x.names.clone())
                };
                let (ys, y_names) = {
                    let y = y.borrow();
                    (y.elements.clone(), y.names.clone())
                };
                if xs.len() != ys.len() || x_names != y_names {
                    return Ok(false);
                }
                pending.extend(xs.into_iter().zip(ys).rev());
            }
            (Value::Map(x), Value::Map(y)) => {
                let xs = x.borrow().entries.clone();
                let y = y.borrow();
                if xs.len() != y.entries.len() {
                    return Ok(false);
                }
                let mut pairs = Vec::with_capacity(xs.len());
                for (key, value) in xs {
                    let Some(other) = y.entries.get(&key) else {
                        return Ok(false);
                    };
                    pairs.push((value, other.clone()));
                }
                pending.extend(pairs.into_iter().rev());
            }
            (Value::Set(x), Value::Set(y)) => {
                if x.as_ptr() != y.as_ptr() {
                    let x = x.borrow();
                    let y = y.borrow();
                    if x.elements.len() != y.elements.len()
                        || !x.elements.iter().all(|key| y.elements.contains(key))
                    {
                        return Ok(false);
                    }
                }
            }
            (Value::Object(x), Value::Object(y)) if x.as_ptr() != y.as_ptr() => {
                if !objects(&a, &b)?.unwrap_or(false) {
                    return Ok(false);
                }
            }
            _ => {
                if a != b {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// A Set element. Numbers compare by value, as with `==`: a Float with an
/// integral value is the same element as that Int (`{5, 5.0}` has one element).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(Rc<String>),
    Char(char),
}

impl ValueKey {
    /// The Int a Float key equals, if it has an integral value.
    fn as_integral(&self) -> Option<i64> {
        match self {
            ValueKey::Int(int) => Some(*int),
            ValueKey::Float(bits) => {
                let float = f64::from_bits(*bits);
                let int = float as i64;
                int_equals_float(int, float).then_some(int)
            }
            _ => None,
        }
    }
}

impl PartialEq for ValueKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueKey::Null, ValueKey::Null) => true,
            (ValueKey::Bool(a), ValueKey::Bool(b)) => a == b,
            (ValueKey::String(a), ValueKey::String(b)) => a == b,
            (ValueKey::Char(a), ValueKey::Char(b)) => a == b,
            (ValueKey::Float(a), ValueKey::Float(b)) => a == b,
            (ValueKey::Int(_) | ValueKey::Float(_), ValueKey::Int(_) | ValueKey::Float(_)) => {
                self.as_integral().is_some() && self.as_integral() == other.as_integral()
            }
            _ => false,
        }
    }
}

impl Eq for ValueKey {}

impl std::hash::Hash for ValueKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Equal numbers hash alike whether they are Ints or Floats.
        if let Some(int) = self.as_integral() {
            state.write_u8(2);
            int.hash(state);
            return;
        }
        match self {
            ValueKey::Null => state.write_u8(0),
            ValueKey::Bool(value) => {
                state.write_u8(1);
                value.hash(state);
            }
            ValueKey::Int(value) => {
                state.write_u8(2);
                value.hash(state);
            }
            ValueKey::Float(bits) => {
                state.write_u8(3);
                bits.hash(state);
            }
            ValueKey::String(value) => {
                state.write_u8(4);
                value.hash(state);
            }
            ValueKey::Char(value) => {
                state.write_u8(5);
                value.hash(state);
            }
        }
    }
}

/// The map key a value stands for. Map keys are strings; a number or Bool
/// key is the string it prints as, so `m[1]`, `m["1"]` and `{1: x}` agree.
pub fn map_key(value: &Value) -> Option<std::borrow::Cow<'_, str>> {
    match value {
        Value::String(s) => Some(std::borrow::Cow::Borrowed(s.as_str())),
        Value::Int(_) | Value::Bool(_) => Some(std::borrow::Cow::Owned(value.to_string())),
        // A whole Float names the key of the Int it equals: `m[1.0]` is `m[1]`.
        Value::Float(f) => Some(std::borrow::Cow::Owned(f.to_string())),
        _ => None,
    }
}

impl ValueKey {
    pub fn try_from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Null => Some(ValueKey::Null),
            Value::Bool(value) => Some(ValueKey::Bool(*value)),
            Value::Int(value) => Some(ValueKey::Int(*value)),
            // -0.0 is the same element as 0.0 (and 0).
            Value::Float(value) if *value == 0.0 => Some(ValueKey::Float(0.0f64.to_bits())),
            Value::Float(value) => Some(ValueKey::Float(value.to_bits())),
            Value::String(value) => Some(ValueKey::String(Rc::clone(value))),
            Value::Char(value) => Some(ValueKey::Char(*value)),
            _ => None,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(s) => Value::String(Rc::clone(s)),
            ValueKey::Char(c) => Value::Char(*c),
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
        for v in self.type_args.iter().flatten() {
            v.mark(gc);
        }
    }

    fn clear_refs(&mut self) {
        self.class = None;
        self.fields.clear();
    }

    fn release_refs(&mut self) {
        self.clear_refs();
        self.type_args = None;
    }

    fn byte_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.fields.len() * (std::mem::size_of::<String>() + std::mem::size_of::<Value>())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TupleState {
    pub elements: Vec<Value>,
    pub names: Option<IndexMap<String, usize>>,
}

impl TupleState {
    pub fn new(elements: Vec<Value>, names: Option<IndexMap<String, usize>>) -> Self {
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

    fn byte_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.elements.len() * std::mem::size_of::<Value>()
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
        index: usize,
        is_entries: bool,
    },
    Set {
        set: GcRc<SetState>,
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

    /// Drops the iterator's reference to what it iterates, which is not
    /// emptied: other values may still use it.
    fn clear_refs(&mut self) {
        *self = IteratorState::Range {
            range: Range {
                start: Some(0),
                end: Some(0),
                inclusive: false,
            },
            current: 0,
        };
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

#[derive(Debug, Serialize, Deserialize)]
pub struct PartialState {
    pub callee: Value,
    pub args: Vec<Value>,
    pub placeholders: Vec<usize>,
}

impl GcObject for PartialState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.callee.mark(gc);
        for arg in &self.args {
            arg.mark(gc);
        }
    }

    fn release_refs(&mut self) {
        self.callee = Value::Null;
        self.args.clear();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundMethodState {
    pub receiver: Value,
    pub function: Value,
    pub owner: MethodOwner,
    pub visibility: MethodVisibility,
}

impl GcObject for BoundMethodState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.receiver.mark(gc);
        self.function.mark(gc);
        mark_owner(&self.owner, gc);
    }

    fn release_refs(&mut self) {
        self.receiver = Value::Null;
        self.function = Value::Null;
        self.owner = released_owner();
    }
}

fn mark_owner(owner: &MethodOwner, gc: &mut MarkAndSweepGC) {
    match owner {
        MethodOwner::Class(c) => gc.mark_object(&c.to_gc_object()),
        MethodOwner::Enum(e) => gc.mark_object(&e.to_gc_object()),
        MethodOwner::Interface(i) => gc.mark_object(&i.to_gc_object()),
        MethodOwner::Builtin(_) => {}
    }
}

/// The owner left in a released method, which refers to nothing.
fn released_owner() -> MethodOwner {
    MethodOwner::Builtin(Rc::new(String::new()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticMethodState {
    pub function: Value,
    pub owner: MethodOwner,
    pub visibility: MethodVisibility,
}

impl GcObject for StaticMethodState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.function.mark(gc);
        mark_owner(&self.owner, gc);
    }

    fn release_refs(&mut self) {
        self.function = Value::Null;
        self.owner = released_owner();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundNativeMethodState {
    pub receiver: Value,
    #[serde(skip, default = "default_native_function_val")]
    pub function: NativeFunction,
}

impl GcObject for BoundNativeMethodState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.receiver.mark(gc);
    }

    fn release_refs(&mut self) {
        self.receiver = Value::Null;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundFunctionState {
    pub function: Value,
    pub type_args: Vec<Value>,
}

impl GcObject for BoundFunctionState {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.function.mark(gc);
        for arg in &self.type_args {
            arg.mark(gc);
        }
    }

    fn release_refs(&mut self) {
        self.function = Value::Null;
        self.type_args.clear();
    }
}

#[cfg(test)]
mod value_size_tests {
    #[test]
    fn values_stay_two_words() {
        assert_eq!(std::mem::size_of::<super::Value>(), 16);
    }
}

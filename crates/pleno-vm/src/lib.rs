use indexmap::{IndexMap, IndexSet};
use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    ArrayState, BoundMethodState, BoundNativeMethodState, CallOptions, Class, ClosureState,
    EnumType, EnumVariantDef, GcRc, MapState, MethodOwner, MultiWaiter, MultiWaiterMode,
    NativeFunction, Object, Range, RuntimeError, SetState, StaticMethodState, ThunkState,
    TupleState, VMProvider, Value, ValueKey, gc::GcObject, nan_value::NanValue,
};
// use std::collections::HashSet;
use std::fs;
use std::rc::Rc;

mod access;
pub mod debug;
pub mod event_loop;
mod fast;
mod method_cache;
mod models;
mod utils;
mod value_ops;

pub use access::*;
pub use debug::*;
pub use event_loop::*;
pub use models::*;
pub use utils::*;
pub use value_ops::*;

// Core VM implementation

// Removed local SandboxConfig, utilizing pleno_value::SandboxConfig

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Finished(Value),
    Paused,
}

pub struct VM {
    pub sandbox_config: Option<pleno_value::SandboxConfig>,
    /// Whether simple instructions may run in the small loop of `run_fast`
    /// (on by default). Turning it off runs everything in the general loop,
    /// which must give the same results: tests and the fuzzer compare both.
    pub fast_loop: bool,
    /// Names of deprecated functions already warned about.
    deprecation_warnings: std::collections::HashSet<String>,
    /// How deeply calls from native code back into Pleno can nest before a
    /// `RecursionError`; lower it where the stack is small (WebAssembly).
    pub max_nested_runs: usize,
    pub gc: pleno_value::gc::GCManager,
    pub registers: Vec<NanValue>,
    pub frames: Vec<CallFrame>,
    pub globals: IndexMap<String, Value>,
    pub modules: IndexMap<String, Value>,
    /// The top-level names of each imported module, keyed by the module's
    /// resolved path. Code in a module reads its own names first and then
    /// `globals`; only the names an `import` lists reach the importer.
    pub module_globals: IndexMap<String, IndexMap<String, Value>>,
    /// Top-level names the main program defined, which modules do not see.
    pub program_globals: ProgramGlobals,
    /// Methods added to built-in types with `extend`, keyed by type name.
    pub builtin_extensions: IndexMap<String, IndexMap<String, pleno_value::MethodInfo>>,
    /// Rejected promises nothing has handled yet, reported when the program ends.
    pub unhandled_rejections: IndexMap<u64, Value>,
    pub functions: Vec<Function>,
    pub pending_tasks: Vec<AsyncTask>,
    pub suspended_frames: IndexMap<u64, SuspendedFrame>,
    pub promises: IndexMap<u64, Value>,
    pub next_promise_id: u64,
    pub multi_waiters: Vec<MultiWaiter>,
    /// The file being run: relative imports in it resolve from its folder,
    /// not from the working directory.
    pub main_path: Option<std::path::PathBuf>,
    /// Promises passed to `Promise.all` or `Promise.race`: those observe their
    /// rejections, even after the combined promise has settled.
    combined_promises: std::collections::HashSet<u64>,
    pub builtin_resolver: Option<fn(&str, &str) -> Option<NativeFunction>>,
    /// What `builtin_resolver` found for each type and method name read so far.
    builtin_methods: method_cache::BuiltinMethodCache,
    /// A spare argument vector for calls of native methods (empty between calls).
    native_args: Vec<Value>,
    pub run_depths: Vec<usize>,
    pub current_named_args: IndexMap<String, Value>,
    pub instruction_count: u64,
    pub event_loop: event_loop::EventLoop,
    pub debug_context: DebugContext,
    pub output: Option<String>,
    /// The built-in `Error` class, which runtime errors are instances of even
    /// when a program shadows the global `Error`.
    pub error_class: GcRc<Class>,
    /// Where `GetGlobal` found each name last time, per function and constant
    /// index: an index into `globals` plus one (0 = not looked up yet). The
    /// name at that index is checked before use, so a stale entry only costs
    /// a normal lookup.
    global_slots: Vec<Vec<u32>>,
    /// The built-in method each `GetMethod` found in `run_fast`, by function
    /// and instruction, for the resolver `method_sites_resolver`.
    method_sites: Vec<Vec<Option<(bool, NativeFunction)>>>,
    method_sites_resolver: Option<usize>,
    /// Values of globals that `run_fast` read, with the `globals_epoch` they
    /// were read in (by function and name operand).
    global_values: Vec<Vec<(u64, NanValue)>>,
    /// Changes whenever a global may have changed, which forgets every value
    /// in `global_values`.
    globals_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Rc<String>,
    pub chunk: Chunk,
    pub arity: usize,
    pub params: Vec<ParamSpec>,
    pub module: Option<Rc<String>>,
    pub module_path: Option<Rc<String>>,
    pub is_async: bool,
    pub is_pure: bool,
    pub type_params: Vec<pleno_value::GenericParamMetadata>,
    pub type_constraints: IndexMap<String, Vec<String>>,
    pub annotations: Vec<pleno_value::RuntimeAnnotation>,
    /// The argument counts a call can pass straight into the parameters
    /// (see `plain_params`), worked out once from `params` and `arity`.
    plain_args: (usize, usize),
    /// Set by `@deprecated` or `@deprecated("use g instead")`: calling the
    /// function prints a warning (once per name), with this note if given.
    deprecation: Option<Rc<String>>,
}

impl VMProvider for VM {
    fn print(&mut self, text: &str) {
        if let Some(buf) = &mut self.output {
            buf.push_str(text);
        } else {
            print!("{}", text);
        }
    }

    fn call_ext(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        opts: CallOptions,
    ) -> Result<Value, RuntimeError> {
        self.call_ext_with(callee, &args, opts)
    }

    /// A native calling back into Pleno (`map`, `filter`, `sort`, ...): a
    /// plain call of a bytecode function or closure gets its frame directly;
    /// anything else goes through `call_ext_with`.
    fn call_with(&mut self, callee: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
        self.check_call_depth()?;
        let (function_index, closure_state) = match callee {
            Value::Function(index) => (*index, None),
            Value::Closure(state) => (state.borrow().function, Some(state.clone())),
            _ => return self.call_ext_with(callee.clone(), args, CallOptions::default()),
        };
        self.warn_if_deprecated(function_index);
        let Some(function) = self.functions.get(function_index) else {
            return self.call_ext_with(callee.clone(), args, CallOptions::default());
        };
        if function.is_async || !plain_params(function, args.len()) {
            return self.call_ext_with(callee.clone(), args, CallOptions::default());
        }
        let param_count = function.params.len();
        let register_count = function.chunk.max_registers.max(function.arity);
        let depth = self.frames.len();
        // As in `call_ext_with`, the result register is the callee's first one;
        // the frame at `depth` returns its value instead of storing it.
        let base = self.registers.len();
        self.registers.reserve(register_count);
        for arg in args.iter().take(register_count) {
            self.registers.push(NanValue::from_value(arg));
        }
        for _ in args.len()..param_count.min(register_count) {
            self.registers.push(NanValue::missing());
        }
        // A loop over the reserved space: `resize` calls `memset`, which costs
        // more than the few registers a call usually has.
        while self.registers.len() < base + register_count {
            self.registers.push(NanValue::null());
        }
        // Written in place, as in `try_push_register_frame`.
        self.frames.reserve(1);
        let len = self.frames.len();
        // SAFETY: `reserve` made room for one more frame at index `len`, which
        // `set_len` then covers.
        unsafe {
            self.frames.as_mut_ptr().add(len).write(CallFrame {
                function_index,
                ip: 0,
                base,
                return_reg: base,
                return_override: None,
                closure_state,
                current_owner: None,
                type_args: Vec::new(),
                exception_handlers: Vec::new(),
            });
            self.frames.set_len(len + 1);
        }
        // Short callbacks (`|x| => x * 2`) often run entirely in `run_fast`;
        // their return then needs no general run.
        if !self.debug_context.is_active() && self.fast_loop {
            self.run_fast(depth)?;
            if let Some(result) = self.finish_plain_return(depth) {
                return Ok(result.to_value());
            }
        }
        match self.run_until(depth)? {
            ExecutionStatus::Finished(value) => Ok(value),
            ExecutionStatus::Paused => Err(RuntimeError::new("Paused during call_function")),
        }
    }

    fn call_ext_with(
        &mut self,
        callee: Value,
        args: &[Value],
        opts: CallOptions,
    ) -> Result<Value, RuntimeError> {
        let return_reg_idx = self.registers.len();
        // Execute until the callee frame returns
        let depth = self.frames.len();
        match callee {
            Value::Function(handle) => {
                self.call_function(
                    Value::Function(handle),
                    None,
                    args,
                    return_reg_idx,
                    false,
                    opts,
                )?;
            }
            Value::Closure(state) => {
                let borrowed = state.borrow();
                let mut call_opts = opts;
                call_opts.closure_state = Some(state.clone());
                self.call_function(
                    Value::Function(borrowed.function),
                    None,
                    args,
                    return_reg_idx,
                    false,
                    call_opts,
                )?;
            }
            Value::BoundMethod(m) => {
                let m = m.borrow();
                let mut call_opts = opts;
                call_opts.current_owner = Some(m.owner.clone());
                self.call_function(
                    m.function.clone(),
                    Some(m.receiver.clone()),
                    args,
                    return_reg_idx,
                    false,
                    call_opts,
                )?;
            }
            Value::NativeFunction(f) => {
                return f(self, args);
            }
            Value::BoundNativeMethod(m) => {
                let m = m.borrow();
                let mut final_args = vec![m.receiver.clone()];
                final_args.extend_from_slice(args);
                let result = (m.function)(self, &final_args)?;
                return Ok(result);
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call_ext_with(callable, args, opts);
                } else {
                    return Err(RuntimeError::new("Map is not callable"));
                }
            }
            Value::Object(obj) => {
                let callable = obj.borrow().fields.get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call_ext_with(callable, args, opts);
                } else {
                    return Err(RuntimeError::new("Object is not callable"));
                }
            }
            other => {
                return Err(RuntimeError::new(format!(
                    "{} is not callable",
                    other.type_name()
                )));
            }
        }

        // If it's an async function call, call_ext will have placed a promise map in return_reg_idx.
        // We want to capture that map before run_until might potentially truncate it (on sync return).
        // Only an async callee has one: for any other, that register is its
        // first argument (which may well be a map with a "variant" key).
        let callee_is_async = self
            .frames
            .get(depth)
            .is_some_and(|frame| self.functions[frame.function_index].is_async);
        let maybe_promise = callee_is_async
            .then(|| self.registers.get(return_reg_idx).cloned())
            .flatten();

        let sync_result = match self.run_until(depth)? {
            ExecutionStatus::Finished(v) => v,
            ExecutionStatus::Paused => {
                return Err(RuntimeError::new("Paused during call_function"));
            }
        };

        // If run_until returned Null (could be suspension), and we have a promise map, return the map.
        // Only a map can be the promise (other values may have been freed by a
        // collection during the call; their register was not kept).
        if let Some(p) = maybe_promise
            && sync_result == Value::Null
            && p.type_name() == "Map"
        {
            let p_val = p.to_value();
            let is_promise = if let Value::Map(m) = &p_val {
                m.borrow().entries.contains_key("variant")
            } else {
                false
            };
            if is_promise {
                return Ok(p_val);
            }
        }

        Ok(sync_result)
    }

    fn get_gc(&mut self) -> &mut pleno_value::gc::GCManager {
        &mut self.gc
    }

    fn add_timer(&mut self, delay_ms: u64, callback: Value, args: Vec<Value>) -> u64 {
        self.event_loop.add_timer(delay_ms, callback, args)
    }

    #[cfg(feature = "net")]
    fn register_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        interest: mio::Interest,
        callback: Value,
        persistent: bool,
    ) -> Result<mio::Token, RuntimeError> {
        self.event_loop
            .register_io(source, interest, callback, persistent)
            .map_err(|e| RuntimeError::new(format!("IO registration failed: {}", e)))
    }

    #[cfg(feature = "net")]
    fn unregister_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        token: mio::Token,
    ) -> Result<(), RuntimeError> {
        self.event_loop
            .unregister_io(source, token)
            .map_err(|e| RuntimeError::new(format!("IO unregistration failed: {}", e)))
    }

    fn create_promise(&mut self) -> Result<(u64, Value), RuntimeError> {
        let id = self.next_promise_id;
        self.next_promise_id += 1;

        let mut map = IndexMap::new();
        map.insert("id".to_string(), Value::Int(id as i64));
        map.insert(
            "variant".to_string(),
            Value::String(Rc::new("Pending".to_string())),
        );
        map.insert("value".to_string(), Value::Null);
        let promise = Value::Map(self.gc.allocate(MapState::new(map))?);
        self.promises.insert(id, promise.clone());
        Ok((id, promise))
    }

    fn resolve_promise(&mut self, id: u64, value: Value) {
        self.settle_promise(id, value, false);
    }

    fn reject_promise(&mut self, id: u64, reason: Value) {
        self.settle_promise(id, reason, true);
    }

    fn mark_rejection_handled(&mut self, id: u64) {
        self.unhandled_rejections.swap_remove(&id);
    }

    fn run_promise_callback(&mut self, info: &[Value], value: &Value, rejected: bool) {
        let Some(Value::Int(child_id)) = info.get(1) else {
            return;
        };
        let child_id = *child_id as u64;
        let on_fulfilled = info.first().cloned().unwrap_or(Value::Null);
        let on_rejected = info.get(2).cloned().unwrap_or(Value::Null);

        if matches!(info.get(3), Some(Value::Int(1))) {
            // `finally`: run the callback, then pass the original outcome through.
            match self.call(on_fulfilled, vec![]) {
                Ok(_) => self.settle_promise(child_id, value.clone(), rejected),
                Err(e) => {
                    let reason = self.rejection_reason(e);
                    self.reject_promise(child_id, reason)
                }
            }
            return;
        }

        let handler = if rejected { on_rejected } else { on_fulfilled };
        if matches!(handler, Value::Null) {
            self.settle_promise(child_id, value.clone(), rejected);
            return;
        }
        match self.call(handler, vec![value.clone()]) {
            Ok(res) => self.resolve_promise(child_id, res),
            Err(e) => {
                let reason = self.rejection_reason(e);
                self.reject_promise(child_id, reason)
            }
        }
    }

    fn spawn(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let (id, promise) = self.create_promise()?;
        let closure_state = if let Value::Closure(state) = &callee {
            Some(state.clone())
        } else {
            None
        };
        self.pending_tasks.push(AsyncTask {
            promise_id: id,
            value: Value::Null,
            callee: Some(callee),
            args: Some(args),
            closure_state,
            is_async: true,
            is_resolved: false,
            is_rejected: false,
        });
        Ok(promise)
    }

    fn register_multi_waiter(&mut self, waiter: MultiWaiter) {
        self.combined_promises
            .extend(waiter.dependencies.iter().copied());
        self.multi_waiters.push(waiter);
    }

    fn check_purity(&mut self) -> Result<(), RuntimeError> {
        if let Some(frame) = self.frames.last()
            && self.functions[frame.function_index].is_pure
        {
            return Err(RuntimeError::new("pure function cannot have side effects"));
        }
        Ok(())
    }

    fn collect(&mut self) {
        self.gc.collect_garbage_with(|gc| {
            // Mark all reachable objects
            for val in self.registers.iter() {
                val.mark(gc);
            }
            for val in self.globals.values() {
                val.mark(gc);
            }
            for val in self.modules.values() {
                val.mark(gc);
            }
            for val in self
                .module_globals
                .values()
                .flat_map(|names| names.values())
            {
                val.mark(gc);
            }
            for val in self.program_globals.replaced.values() {
                val.mark(gc);
            }
            for task in &self.pending_tasks {
                task.mark(gc);
            }
            for promise in self.promises.values() {
                promise.mark(gc);
            }
            for frame in self.suspended_frames.values() {
                frame.mark(gc);
            }
            for waiter in &self.multi_waiters {
                waiter.mark(gc);
            }
            for methods in self.builtin_extensions.values() {
                for info in methods.values() {
                    info.mark(gc);
                }
            }
            for frame in &self.frames {
                frame.mark(gc);
            }
        });
    }

    fn register_module(&mut self, name: String, value: Value) {
        self.modules.insert(name, value);
    }

    fn register_global(&mut self, name: String, value: Value) {
        self.globals_epoch += 1;
        self.globals.insert(name, value);
    }

    fn set_builtin_resolver(&mut self, resolver: fn(&str, &str) -> Option<NativeFunction>) {
        self.builtin_resolver = Some(resolver);
    }

    fn check_is_instance(&self, value: &Value, type_val: &Value) -> bool {
        self.is_instance(value, type_val)
    }

    fn get_named_args(&self) -> &IndexMap<String, Value> {
        &self.current_named_args
    }

    fn is_io_allowed(&self) -> bool {
        self.sandbox_config
            .as_ref()
            .map(|c| c.allow_io)
            .unwrap_or(true)
    }

    fn is_network_allowed(&self) -> bool {
        self.sandbox_config
            .as_ref()
            .map(|c| c.allow_network)
            .unwrap_or(true)
    }

    fn is_read_allowed(&self, path: &str) -> bool {
        if let Some(config) = &self.sandbox_config {
            if !config.allow_io {
                return false;
            }
            if !config.allowed_read_paths.is_empty() {
                return path_allowed(path, &config.allowed_read_paths);
            }
        }
        true
    }

    fn is_write_allowed(&self, path: &str) -> bool {
        if let Some(config) = &self.sandbox_config {
            if !config.allow_io {
                return false;
            }
            if !config.allowed_write_paths.is_empty() {
                return path_allowed(path, &config.allowed_write_paths);
            }
        }
        true
    }

    fn get_max_memory(&self) -> usize {
        self.sandbox_config
            .as_ref()
            .map(|c| c.max_memory)
            .unwrap_or(0)
    }

    fn instruction_count(&self) -> u64 {
        self.instruction_count
    }

    fn get_max_instructions(&self) -> u64 {
        self.sandbox_config
            .as_ref()
            .map(|c| c.max_instructions)
            .unwrap_or(0)
    }

    fn create_sandbox(
        &self,
        config: pleno_value::SandboxConfig,
    ) -> std::result::Result<Box<dyn VMProvider>, RuntimeError> {
        let mut new_vm = VM::new();
        new_vm.sandbox_config = Some(config);
        // The parent VM's registers are not visible to the sandbox's collector.
        new_vm.gc.sweeps_boxed_values = false;

        // Copy functionality to allow sandbox to actually run code
        new_vm.functions = self.functions.clone();
        new_vm.modules = self.modules.clone();
        new_vm.module_globals = self.module_globals.clone();

        // Globals: We might want access to "Object", "String", etc.
        // Assuming they are in globals or we re-register them?
        // Cloning globals is simplest for v1 to ensure stdlib access.
        new_vm.globals = self.globals.clone();

        Ok(Box::new(new_vm))
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn evaluate_condition(&mut self, condition: &str) -> Result<bool, RuntimeError> {
        let val = self.evaluate_expression(condition)?;
        Ok(crate::value_ops::is_truthy(&val))
    }

    pub fn evaluate_expression(&mut self, expression: &str) -> Result<Value, RuntimeError> {
        let expression = expression.trim();
        if expression.is_empty() {
            return Ok(Value::Null);
        }

        // Handle basic literals first (to avoid conflict with identifiers)
        if let Ok(i) = expression.parse::<i64>() {
            return Ok(Value::Int(i));
        }
        if let Ok(f) = expression.parse::<f64>() {
            return Ok(Value::Float(f));
        }
        if expression == "true" {
            return Ok(Value::Bool(true));
        }
        if expression == "false" {
            return Ok(Value::Bool(false));
        }
        if expression == "null" {
            return Ok(Value::Null);
        }

        // Simplified parser for property access: base.prop1[index].prop2
        // We'll split by '.' and handle '[' ']' separately.
        let mut parts = Vec::new();
        let mut current_part = String::new();
        let mut in_bracket = false;

        for c in expression.chars() {
            match c {
                '.' if !in_bracket => {
                    if !current_part.is_empty() {
                        parts.push(current_part.clone());
                        current_part.clear();
                    }
                }
                '[' if !in_bracket => {
                    if !current_part.is_empty() {
                        parts.push(current_part.clone());
                        current_part.clear();
                    }
                    in_bracket = true;
                }
                ']' if in_bracket => {
                    if !current_part.is_empty() {
                        parts.push(current_part.clone());
                        current_part.clear();
                    }
                    in_bracket = false;
                }
                _ => {
                    current_part.push(c);
                }
            }
        }
        if !current_part.is_empty() {
            parts.push(current_part);
        }

        if parts.is_empty() {
            return Err(RuntimeError::new("invalid expression"));
        }

        let mut val = self.resolve_base_value(&parts[0])?;

        for part in &parts[1..] {
            let key = part.trim().trim_matches(|c| c == '"' || c == '\'');
            val = self.get_member(&val, key)?;
        }

        Ok(val)
    }

    /// Global `name` as the running code sees it: its module's own top-level
    /// name, or the shared global.
    fn visible_global(&self, name: &str) -> Option<Value> {
        let module_path = self
            .frames
            .last()
            .and_then(|frame| self.functions[frame.function_index].module_path.as_deref());
        lookup_global_in(
            &self.globals,
            &self.module_globals,
            &self.program_globals,
            module_path,
            name,
        )
    }

    /// The class, interface or enum a type annotation names. Besides the
    /// names the running code sees, this finds types that stay inside the
    /// module that declares them, so a value of such a type passed to other
    /// code still matches its annotation.
    fn lookup_type_name(&self, name: &str) -> Option<Value> {
        self.visible_global(name).or_else(|| {
            self.module_globals
                .values()
                .filter_map(|table| table.get(name))
                .find(|value| {
                    matches!(
                        value,
                        Value::Class(_) | Value::Interface(_) | Value::Enum(_)
                    )
                })
                .cloned()
        })
    }

    fn resolve_base_value(&self, name: &str) -> Result<Value, RuntimeError> {
        let name = name.trim();
        if let Some(frame) = self.frames.last() {
            let func = &self.functions[frame.function_index];
            let ip = frame.ip;

            // Check locals
            for local in &func.chunk.locals {
                if local.name == name && ip >= local.start_ip && ip <= local.end_ip {
                    return Ok(self.registers[frame.base + local.register as usize].to_value());
                }
            }

            // Check params
            for (i, param) in func.params.iter().enumerate() {
                if param.name == name {
                    return Ok(self.registers[frame.base + i].to_value());
                }
            }
        }

        // Check globals
        if let Some(val) = self.visible_global(name) {
            return Ok(val);
        }

        Err(RuntimeError::new(format!(
            "cannot resolve base value: {}",
            name
        )))
    }

    fn get_member(&self, val: &Value, key: &str) -> Result<Value, RuntimeError> {
        match val {
            Value::Object(obj) => {
                if let Some(v) = obj.borrow().fields.get(key) {
                    return Ok(v.clone());
                }
                Err(RuntimeError::new(format!("property '{}' not found", key)))
            }
            Value::Map(map) => {
                if let Some(v) = map.borrow().entries.get(key) {
                    return Ok(v.clone());
                }
                Err(RuntimeError::new(format!("map key '{}' not found", key)))
            }
            Value::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    let b = arr.borrow();
                    if idx < b.elements.len() {
                        return Ok(b.elements[idx].clone());
                    }
                    return Err(RuntimeError::new("array index out of bounds"));
                }
                Err(RuntimeError::new("invalid array index"))
            }
            Value::Tuple(t) => {
                if let Ok(idx) = key.parse::<usize>() {
                    let b = t.borrow();
                    if idx < b.elements.len() {
                        return Ok(b.elements[idx].clone());
                    }
                    return Err(RuntimeError::new("tuple index out of bounds"));
                }
                Err(RuntimeError::new("invalid tuple index"))
            }
            _ => Err(RuntimeError::new("value is not an object, map, or array")),
        }
    }

    pub fn get_variable_children(&mut self, var_ref: u64) -> Vec<(String, Value)> {
        let val = if let Some(v) = self.debug_context.variable_tracker.get(var_ref) {
            v.clone()
        } else {
            return Vec::new();
        };

        match val {
            Value::Array(arr) => arr
                .borrow()
                .elements
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v.clone()))
                .collect(),
            Value::Map(map) => map
                .borrow()
                .entries
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            Value::Object(obj) => obj
                .borrow()
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            Value::Tuple(t) => t
                .borrow()
                .elements
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn process_debug_log(&mut self, message: &str) {
        let mut expanded = String::new();
        let mut start = 0;
        while let Some(open) = message[start..].find('{') {
            expanded.push_str(&message[start..start + open]);
            let actual_open = start + open;
            if let Some(close) = message[actual_open..].find('}') {
                let expr = &message[actual_open + 1..actual_open + close];
                let val = self.evaluate_expression(expr).unwrap_or(Value::Null);
                expanded.push_str(&format!("{:?}", val));
                start = actual_open + close + 1;
            } else {
                expanded.push('{');
                start = actual_open + 1;
            }
        }
        expanded.push_str(&message[start..]);
        log::info!("[DAP] {}", expanded);
    }

    pub fn new() -> Self {
        let gc = pleno_value::gc::GCManager::new(4 * 1024 * 1024);
        let mut globals = IndexMap::new();
        let modules = IndexMap::new();
        // Never handed to the collector: it lives as long as the VM.
        let error_class = GcRc::new(Class::error());
        globals.insert(
            pleno_value::ERROR_CLASS.to_string(),
            Value::Class(error_class.clone()),
        );

        Self {
            sandbox_config: None,
            fast_loop: true,
            deprecation_warnings: std::collections::HashSet::new(),
            max_nested_runs: DEFAULT_MAX_NESTED_RUNS,
            gc,
            registers: vec![NanValue::null(); 256],
            frames: Vec::new(),
            globals,
            modules,
            module_globals: IndexMap::new(),
            program_globals: ProgramGlobals::default(),
            builtin_extensions: IndexMap::new(),
            unhandled_rejections: IndexMap::new(),
            functions: Vec::new(),
            pending_tasks: Vec::new(),
            suspended_frames: IndexMap::new(),
            promises: IndexMap::new(),
            next_promise_id: 1,
            multi_waiters: Vec::new(),
            main_path: None,
            combined_promises: std::collections::HashSet::new(),
            builtin_resolver: None,
            builtin_methods: Default::default(),
            native_args: Vec::new(),
            run_depths: Vec::new(),
            current_named_args: IndexMap::new(),
            instruction_count: 0,
            event_loop: event_loop::EventLoop::new(),
            debug_context: DebugContext::new(),
            output: None,
            error_class,
            global_slots: Vec::new(),
            method_sites: Vec::new(),
            method_sites_resolver: None,
            global_values: Vec::new(),
            globals_epoch: 1,
        }
    }

    pub fn apply_config(&mut self, config: pleno_value::SandboxConfig) {
        self.gc.max_bytes = config.max_memory;
        self.sandbox_config = Some(config);
    }

    pub fn load_script(&mut self, chunk: Chunk) {
        self.load_script_with_path(chunk, None);
    }

    /// Loads the main script, recording its file path so breakpoints set on
    /// that file match and relative imports resolve from its directory.
    pub fn load_script_with_path(&mut self, chunk: Chunk, path: Option<String>) {
        // Code outside a run may have changed `globals` directly.
        self.globals_epoch += 1;
        let path = path.map(Rc::new);
        let max_regs = chunk.max_registers;
        let function = Function {
            name: Rc::new("<script>".to_string()),
            chunk,
            arity: 0,
            params: Vec::new(),
            module: None,
            module_path: path.clone(),
            is_async: false,
            is_pure: false,
            type_params: Vec::new(),
            type_constraints: IndexMap::new(),
            annotations: Vec::new(),
            plain_args: (0, 0),
            deprecation: None,
        };

        self.functions.push(function);
        let func_idx = self.functions.len() - 1;

        // Create main script closure
        let closure = ClosureState {
            function: func_idx,
            captures: Vec::new(),
        };

        let closure_gc = self.gc.allocate(closure).expect("GC allocation failed");

        // Push initial frame
        let call_frame = CallFrame {
            function_index: func_idx,
            ip: 0,
            base: 0,
            return_reg: 0,
            return_override: None,
            closure_state: Some(closure_gc),
            current_owner: None,
            type_args: Vec::new(),
            exception_handlers: Vec::new(),
        };

        self.frames.push(call_frame);

        if self.registers.len() < max_regs {
            self.registers.resize(max_regs, NanValue::null());
        }
    }

    pub fn execute(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        self.load_script(chunk);
        let script_index = self.functions.len() - 1;

        let mut last_result = Value::Null;
        let mut script_finished = false;

        loop {
            // 1. Run synchronous code until we hit a suspension point or finish
            if !self.frames.is_empty() {
                let result = match self.run_until(0) {
                    Ok(ExecutionStatus::Finished(val)) => val,
                    Ok(ExecutionStatus::Paused) => {
                        return Err(RuntimeError::new("VM paused during execute()"));
                    }
                    Err(mut err) => {
                        if err.handled {
                            continue;
                        }
                        self.enrich_error(&mut err);
                        return Err(err);
                    }
                };
                if self.frames.is_empty() && !script_finished {
                    last_result = result;
                    script_finished = true;
                }
            }

            // 2. Process pending tasks (Promises resolved by previous sync code)
            if !self.pending_tasks.is_empty() {
                if let Err(mut err) = self.process_tasks() {
                    self.enrich_error(&mut err);
                    return Err(err);
                }
                continue; // Task processing might have resumed frames, so loop back
            }

            // 3. If we have frames (resumed) or tasks (new), loop back immediately
            if !self.frames.is_empty() || !self.pending_tasks.is_empty() {
                continue;
            }

            // 4. Check Event Loop (Timers/IO)
            if !self.event_loop.is_empty()
                && let Some(task) = self.event_loop.tick()
            {
                // Convert EventLoop Task to VM pending task
                self.pending_tasks.push(AsyncTask {
                    promise_id: task.promise_id.unwrap_or(0),
                    value: Value::Null,
                    callee: Some(task.callback),
                    args: Some(task.args),
                    closure_state: None,
                    is_async: true,
                    is_resolved: false,
                    is_rejected: false,
                });
                continue;
            }

            // 5. If no frames, no pending tasks, and event loop empty -> Done
            break;
        }
        // Nothing is left that could settle the promise the script awaits, so
        // the rest of the script would silently never run.
        if self
            .suspended_frames
            .values()
            .any(|suspended| suspended.frame.function_index == script_index)
        {
            return Err(RuntimeError::new(
                "the program is waiting for a promise that can never settle",
            ));
        }
        // A rejection nothing handled is an error, like an uncaught exception:
        // otherwise a failing async task would end the program successfully.
        let rejections = std::mem::take(&mut self.unhandled_rejections);
        if !rejections.is_empty() {
            let reasons: Vec<String> = rejections.values().map(describe_exception).collect();
            return Err(RuntimeError::new(format!(
                "unhandled promise rejection: {}",
                reasons.join("; ")
            )));
        }
        Ok(last_result)
    }

    /// Runs until the next breakpoint or the end of the program. Called again
    /// after a pause, it resumes from where execution stopped.
    pub fn run_debug(&mut self) -> Result<ExecutionStatus, RuntimeError> {
        self.globals_epoch += 1;
        self.debug_context.resume();
        self.run_until(0)
    }

    /// Stops at the next line, entering called functions.
    pub fn step_into(&mut self) -> Result<ExecutionStatus, RuntimeError> {
        self.step_until(|start, now| now != start)
    }

    /// Stops at the next line of the current function, or its caller once it returns.
    pub fn step_over(&mut self) -> Result<ExecutionStatus, RuntimeError> {
        self.step_until(|(start_depth, start_line), (depth, line)| {
            depth < start_depth || (depth == start_depth && line != start_line)
        })
    }

    /// Runs until the current function returns to its caller.
    pub fn step_out(&mut self) -> Result<ExecutionStatus, RuntimeError> {
        self.step_until(|(start_depth, _), (depth, _)| depth < start_depth)
    }

    /// The call depth and source line of the next instruction to run.
    pub fn current_location(&self) -> Option<(usize, u32)> {
        let frame = self.frames.last()?;
        let line = *self.functions[frame.function_index]
            .chunk
            .lines
            .get(frame.ip)?;
        Some((self.frames.len(), line))
    }

    /// Executes instructions until `arrived(start, now)` holds for the
    /// (call depth, line) locations, a breakpoint is hit, or the program ends.
    fn step_until(
        &mut self,
        arrived: impl Fn((usize, u32), (usize, u32)) -> bool,
    ) -> Result<ExecutionStatus, RuntimeError> {
        self.globals_epoch += 1;
        let Some(start) = self.current_location() else {
            return Ok(ExecutionStatus::Finished(Value::Null));
        };
        self.debug_context.resume();
        self.debug_context.stepping = true;
        let result = self.step_until_inner(start, arrived);
        self.debug_context.stepping = false;
        result
    }

    fn step_until_inner(
        &mut self,
        start: (usize, u32),
        arrived: impl Fn((usize, u32), (usize, u32)) -> bool,
    ) -> Result<ExecutionStatus, RuntimeError> {
        loop {
            match self.run_step(0) {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(err) if err.handled => {}
                Err(err) if err.fatal || err.message.contains("uncaught exception") => {
                    return Err(err);
                }
                Err(err) => {
                    // Let `catch` blocks see runtime errors, as normal execution does.
                    let unwound = self.rethrow(err);
                    if !unwound.handled {
                        return Err(unwound);
                    }
                }
            }
            match self.current_location() {
                Some(now) if arrived(start, now) => {
                    self.debug_context.pause();
                    return Ok(ExecutionStatus::Paused);
                }
                Some(_) => {}
                None => return Ok(ExecutionStatus::Finished(Value::Null)),
            }
        }
    }

    fn enrich_error(&self, err: &mut RuntimeError) {
        if err.span.is_none()
            && let Some(frame) = self.frames.last()
            && let Some(func) = self.functions.get(frame.function_index)
        {
            let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 };
            if let Some(span) = func.chunk.spans.get(ip) {
                err.span = Some(*span);
            }
        }
        if err.stack_trace.is_empty() {
            err.stack_trace = self.generate_backtrace();
        }
    }

    fn force_thunk(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Thunk(state) => {
                if let Some(cached) = state.borrow().value.clone() {
                    return Ok(cached);
                }
                let (callable,) = {
                    let s = state.borrow();
                    (s.callable.clone(),)
                };
                let result = self.call(callable, Vec::new())?;
                state.borrow_mut().value = Some(result.clone());
                Ok(result)
            }
            other => Ok(other),
        }
    }

    /// The call stack, innermost frame first. A deep stack keeps its innermost
    /// `BACKTRACE_INNER` and outermost `BACKTRACE_OUTER` frames with a
    /// `... N more frames` line between them, so an error costs the same at
    /// any depth: a `catch` that throws again at each of 100,000 levels would
    /// otherwise copy billions of frames.
    fn generate_backtrace(&self) -> Vec<pleno_value::StackFrame> {
        let stack_frame = |frame: &CallFrame| {
            let func = self.functions.get(frame.function_index)?;
            let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 };
            Some(pleno_value::StackFrame {
                function_name: (*func.name).clone(),
                span: func.chunk.spans.get(ip).copied(),
            })
        };
        let count = self.frames.len();
        if count <= BACKTRACE_INNER + BACKTRACE_OUTER {
            return self.frames.iter().rev().filter_map(stack_frame).collect();
        }
        let mut trace: Vec<_> = self.frames[count - BACKTRACE_INNER..]
            .iter()
            .rev()
            .filter_map(stack_frame)
            .collect();
        trace.push(pleno_value::StackFrame::omitted(
            count - BACKTRACE_INNER - BACKTRACE_OUTER,
        ));
        trace.extend(
            self.frames[..BACKTRACE_OUTER]
                .iter()
                .rev()
                .filter_map(stack_frame),
        );
        trace
    }

    /// `value.toString()` for built-in values: the same text `print` shows.
    fn bind_to_string(
        &mut self,
        method_at: Option<usize>,
        receiver: Value,
    ) -> Result<Value, RuntimeError> {
        fn to_string(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
            let text = args.first().map(|v| v.to_string()).unwrap_or_default();
            Ok(Value::String(Rc::new(text)))
        }
        self.native_method_value(method_at, receiver, to_string)
    }

    /// A method added to `receiver`'s built-in type with `extend`, bound to it
    /// (see `bytecode_method_value` for `method_at`).
    fn extension_method(
        &mut self,
        method_at: Option<usize>,
        receiver: &Value,
        name: &str,
    ) -> Result<Option<Value>, RuntimeError> {
        let Some(info) = self
            .builtin_extensions
            .get(receiver.type_name())
            .and_then(|methods| methods.get(name))
            .cloned()
        else {
            return Ok(None);
        };
        self.bytecode_method_value(method_at, receiver.clone(), &info)
            .map(Some)
    }

    /// The native method `function` of `receiver` as a property: a bound method.
    /// For `GetMethod` (`method_at` is the register of the method) it is the
    /// function itself, with `true` (a native method) two registers after
    /// `method_at`, so that no bound method is allocated. `GetMethod` has
    /// already stored `receiver`, the value it read the property of, in the
    /// register between them.
    fn native_method_value(
        &mut self,
        method_at: Option<usize>,
        receiver: Value,
        function: NativeFunction,
    ) -> Result<Value, RuntimeError> {
        if let Some(at) = method_at {
            self.registers[at + 2] = NanValue::from_bool(true);
            return Ok(Value::NativeFunction(function));
        }
        Ok(Value::BoundNativeMethod(
            self.gc
                .allocate(BoundNativeMethodState { receiver, function })?,
        ))
    }

    /// The bytecode method `info` of `receiver` as a property: a bound method.
    /// For `GetMethod` it is the method's function, with the method's owner two
    /// registers after `method_at` (and the receiver between them, as for
    /// `native_method_value`).
    fn bytecode_method_value(
        &mut self,
        method_at: Option<usize>,
        receiver: Value,
        info: &pleno_value::MethodInfo,
    ) -> Result<Value, RuntimeError> {
        if let Some(at) = method_at {
            self.registers[at + 2] = owner_to_nan(&info.owner);
            return Ok(info.function.clone());
        }
        Ok(Value::BoundMethod(self.gc.allocate(BoundMethodState {
            receiver,
            function: info.function.clone(),
            owner: info.owner.clone(),
            visibility: info.visibility.clone(),
        })?))
    }

    /// A function's `.name`: its declared name.
    fn function_name_value(&self, index: usize) -> Value {
        self.functions
            .get(index)
            .map(|function| Value::String(function.name.clone()))
            .unwrap_or(Value::Null)
    }

    fn handle_property_error(&self, name: &str, type_name: &str) -> RuntimeError {
        RuntimeError::new(format!("cannot read property '{name}' of {type_name}"))
    }

    /// Checks `value` against a type annotation written as text, such as `Int`,
    /// `String?`, `Int | String`, `Int[]`, `Map<String, Int>`, `(Int) => Int`
    /// or the name of a class, interface or enum.
    fn matches_type_name(&self, value: &Value, ty: &str) -> bool {
        let ty = ty.trim();
        if let Some(parts) = split_top_level(ty, '|') {
            return parts.iter().any(|part| self.matches_type_name(value, part));
        }
        // `A & B`: the value must satisfy every part.
        if let Some(parts) = split_top_level(ty, '&') {
            return parts.iter().all(|part| self.matches_type_name(value, part));
        }
        if let Some(inner) = ty.strip_suffix('?') {
            return matches!(value, Value::Null) || self.matches_type_name(value, inner);
        }
        if let Some(element_types) = tuple_type_elements(ty) {
            let Value::Tuple(tuple) = value else {
                return false;
            };
            let elements = tuple.borrow().elements.clone();
            return elements.len() == element_types.len()
                && elements
                    .iter()
                    .zip(&element_types)
                    .all(|(element, ty)| self.matches_type_name(element, ty));
        }
        if ty == "Function" || (ty.starts_with('(') && ty.contains("=>")) {
            return self.is_callable(value);
        }
        if let Some(elem) = ty.strip_suffix("[]") {
            return self.elements_match(value, elem);
        }
        if let Some((base, args)) = ty.split_once('<')
            && let Some(args) = args.strip_suffix('>')
        {
            let args = split_top_level(args, ',').unwrap_or_else(|| vec![args.to_string()]);
            return match (base.trim(), value) {
                ("Array", Value::Array(_)) => self.elements_match(value, &args[0]),
                ("Set", Value::Set(_)) => true,
                ("Map", Value::Map(map)) => {
                    let values: Vec<Value> = map.borrow().entries.values().cloned().collect();
                    let value_ty = args.get(1).cloned().unwrap_or_else(|| "Any".to_string());
                    values.iter().all(|v| self.matches_type_name(v, &value_ty))
                }
                (base, _) => self.matches_type_name(value, base),
            };
        }
        let primitive = matches!(
            (ty, value),
            ("Int", Value::Int(_))
                | ("Float", Value::Float(_) | Value::Int(_))
                | ("Number", Value::Float(_) | Value::Int(_))
                | ("Bool", Value::Bool(_))
                | ("String", Value::String(_))
                | ("Array", Value::Array(_))
                | ("Tuple", Value::Tuple(_))
                | ("Map", Value::Map(_))
                | ("Set", Value::Set(_))
                | ("Object", Value::Map(_) | Value::Object(_))
                | ("Mutex", Value::Mutex(_))
                | ("Channel", Value::Channel(_))
                | ("File", Value::File(_))
                | ("Null" | "Void", Value::Null)
                | ("Char", Value::Char(_))
                | ("Any", _)
        );
        if primitive {
            return true;
        }
        // `Ok`/`Err` and `Some`/`None` values are maps tagged with their variant.
        if let ("Result" | "Option", Value::Map(map)) = (ty, value)
            && let Some(Value::String(variant)) = map.borrow().entries.get("variant")
            && matches!(
                (ty, variant.as_str()),
                ("Result", "Ok" | "Err") | ("Option", "Some" | "None")
            )
        {
            return true;
        }
        if let Some(resolved) = self.resolve_type_param(ty) {
            return self.is_instance(value, &resolved);
        }
        // A user-defined class, interface or enum referenced by name.
        match self.lookup_type_name(ty) {
            Some(
                target @ (Value::Class(_)
                | Value::Interface(_)
                | Value::Enum(_)
                | Value::BoundClass(_)),
            ) => self.is_instance(value, &target),
            _ => false,
        }
    }

    fn elements_match(&self, value: &Value, elem_ty: &str) -> bool {
        let Value::Array(array) = value else {
            return false;
        };
        let elements = array.borrow().elements.clone();
        elements.iter().all(|e| self.matches_type_name(e, elem_ty))
    }

    fn settle_promise(&mut self, id: u64, value: Value, rejected: bool) {
        self.pending_tasks.push(AsyncTask {
            promise_id: id,
            value,
            callee: None,
            args: None,
            closure_state: None,
            is_async: true,
            is_resolved: true,
            is_rejected: rejected,
        });
    }

    fn process_tasks(&mut self) -> Result<(), RuntimeError> {
        let tasks = std::mem::take(&mut self.pending_tasks);
        for task in tasks {
            if !task.is_resolved {
                if let Some(callee) = task.callee {
                    let args = task.args.unwrap_or_default();
                    let opts = CallOptions {
                        closure_state: task.closure_state,
                        promise_id: Some(task.promise_id),
                        ..Default::default()
                    };
                    match self.call_ext(callee, args, opts) {
                        Ok(res) => {
                            // If callee was async, call() returns the promise map.
                            // If we provide promise_id, then task.promise_id IS that promise.
                            // So we don't need to resolve it again with the promise map itself.
                            // Unless callee was NOT async, then we resolve it with the result.
                            let is_promise = if let Value::Map(m) = &res {
                                m.borrow().get("variant").is_some()
                            } else {
                                false
                            };
                            if !is_promise {
                                self.resolve_promise(task.promise_id, res);
                            }
                        }
                        Err(e) => {
                            let reason = self.rejection_reason(e);
                            self.reject_promise(task.promise_id, reason)
                        }
                    }
                }
                continue;
            }
            // Update Promise map if it exists
            let rejected = task.is_rejected;
            let mut handled = false;
            if let Some(Value::Map(map)) = self.promises.swap_remove(&task.promise_id) {
                {
                    let mut b = map.borrow_mut();
                    let state = if rejected { "Rejected" } else { "Resolved" };
                    b.insert(
                        "variant".to_string(),
                        Value::String(Rc::new(state.to_string())),
                    );
                    b.insert("value".to_string(), task.value.clone());
                }

                // Handle chained callbacks (.then)
                let callbacks = {
                    let b = map.borrow();
                    b.get("callbacks").cloned()
                };

                if let Some(Value::Array(arr)) = callbacks {
                    let elements = arr.borrow().elements.clone();
                    handled |= !elements.is_empty();
                    for cb_info_val in elements {
                        if let Value::Array(info) = cb_info_val {
                            let info = info.borrow().elements.clone();
                            self.run_promise_callback(&info, &task.value, rejected);
                        }
                    }
                }
            }

            if let Some(suspended) = self.suspended_frames.swap_remove(&task.promise_id) {
                handled = true;
                let frame = suspended.frame;
                if rejected {
                    // Throw the reason inside the awaiting frame: its handler catches it,
                    // or, in an async function, it rejects that function's promise.
                    self.frames.push(frame);
                    match self.unwind_to_handler(task.value.clone()) {
                        Err(err) if !err.handled => return Err(err),
                        _ => {}
                    }
                } else {
                    self.registers[frame.base + suspended.await_reg] =
                        NanValue::from_value(&task.value);
                    self.frames.push(frame);
                }
            }

            // Check multi-waiters
            let mut i = 0;
            let mut resolutions = Vec::new();
            let mut rejections = Vec::new();
            while i < self.multi_waiters.len() {
                let mut waiter = self.multi_waiters[i].clone();
                let mut resolved = false;

                if let Some(pos) = waiter
                    .dependencies
                    .iter()
                    .position(|&id| id == task.promise_id)
                {
                    handled = true;
                    match waiter.mode {
                        _ if rejected => {
                            rejections.push((waiter.promise_id, task.value.clone()));
                            resolved = true;
                        }
                        MultiWaiterMode::All => {
                            // Each dependency fills its own slot once.
                            waiter.dependencies.swap_remove(pos);
                            let slot = waiter.slots.swap_remove(pos);
                            waiter.results[slot] = task.value.clone();
                            waiter.resolved_count += 1;
                            if waiter.resolved_count == waiter.results.len() {
                                let gc = self.get_gc();
                                let results_val = Value::Array(
                                    gc.allocate(ArrayState::new(waiter.results.clone()))?,
                                );
                                resolutions.push((waiter.promise_id, results_val));
                                resolved = true;
                            }
                        }
                        MultiWaiterMode::Race => {
                            resolutions.push((waiter.promise_id, task.value.clone()));
                            resolved = true;
                        }
                    }
                }

                if resolved {
                    self.multi_waiters.remove(i);
                } else {
                    self.multi_waiters[i] = waiter;
                    i += 1;
                }
            }

            for (id, val) in resolutions {
                self.resolve_promise(id, val);
            }
            for (id, reason) in rejections {
                self.reject_promise(id, reason);
            }
            handled |= self.combined_promises.remove(&task.promise_id);
            if rejected && !handled {
                self.unhandled_rejections
                    .insert(task.promise_id, task.value.clone());
            }
        }
        Ok(())
    }

    fn run_until(&mut self, depth: usize) -> Result<ExecutionStatus, RuntimeError> {
        // Each nested run (a callback from `map`, an operator, a test body) uses
        // stack space in the Rust code that runs it, so unbounded recursion
        // through callbacks would overflow that stack instead of failing.
        if self.run_depths.len() >= self.max_nested_runs {
            return Err(RuntimeError::new(format!(
                "RecursionError: maximum call depth exceeded ({} nested callbacks)",
                self.max_nested_runs
            )));
        }
        self.run_depths.push(depth);
        let res = self.run_until_internal(depth);
        self.run_depths.pop();
        res
    }

    fn run_until_internal(&mut self, depth: usize) -> Result<ExecutionStatus, RuntimeError> {
        loop {
            match self.run_step(depth) {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => continue,
                Err(err) => {
                    if err.handled {
                        continue;
                    }
                    if err.fatal || err.message.contains("uncaught exception") {
                        return Err(err);
                    }
                    let message = err.message.clone();
                    let unwound = self.throw_runtime_error(&message);
                    if unwound.handled {
                        continue;
                    }
                    return Err(unwound);
                }
            }
        }
    }
    /// Pauses at a breakpoint on the instruction at `ip` of function
    /// `func_index`, or while the debugger has paused execution.
    fn check_breakpoint(&mut self, func_index: usize, ip: usize) -> Option<ExecutionStatus> {
        if !self
            .debug_context
            .should_pause(func_index, ip, &self.functions)
        {
            return None;
        }
        if let DebugState::Paused(_) = self.debug_context.state {
            return Some(ExecutionStatus::Paused);
        }

        let (path, line) = {
            let function = &self.functions[func_index];
            let line = function.chunk.lines[ip] as usize;
            (function.module_path.as_deref().map(|s| s.to_string()), line)
        };

        if let Some(p) = path {
            let hits = self
                .debug_context
                .hit_counts
                .entry((p.clone(), line))
                .or_insert(0);
            *hits += 1;
            let current_hits = *hits;

            let (condition, hit_condition, log_message) = {
                if let Some(bp) = self
                    .debug_context
                    .get_breakpoint(func_index, ip, &self.functions)
                {
                    (
                        bp.condition.clone(),
                        bp.hit_condition.clone(),
                        bp.log_message.clone(),
                    )
                } else {
                    (None, None, None)
                }
            };

            // 1. Condition
            let mut hit = if let Some(cond) = condition {
                self.evaluate_condition(&cond).unwrap_or(false)
            } else {
                true
            };

            // 2. Hit Condition
            if hit
                && let Some(hit_cond) = hit_condition
                && let Ok(target) = hit_cond.parse::<usize>()
            {
                hit = current_hits == target;
                // For now we only support numeric hit conditions like "5"
            }

            if hit {
                // 3. Log Message
                if let Some(log_msg) = log_message {
                    self.process_debug_log(&log_msg);
                    // Log messages don't pause execution
                } else {
                    self.debug_context.state = DebugState::Paused(PauseReason::Breakpoint);
                    return Some(ExecutionStatus::Paused);
                }
            }
        }
        None
    }

    /// Runs instructions until the frame at `depth` returns, the program pauses
    /// or an instruction asks the caller to check its state (`Ok(None)`).
    fn run_step(&mut self, depth: usize) -> Result<Option<ExecutionStatus>, RuntimeError> {
        let mut first = true;
        // The instructions of the function last run, kept between instructions
        // so that fetching one does not go through the frame, the function
        // table and the chunk each time. A function's code is never changed
        // or freed once it is in `self.functions` (functions are only ever
        // appended, and growing the vector moves the `Function` values but
        // not the instruction buffers they own), so the pointer stays valid.
        let mut code_function = usize::MAX;
        let mut code: *const pleno_bytecode::Instruction = std::ptr::null();
        let mut code_len = 0;
        // Whether a debugger may stop execution. Only the debugger changes this,
        // between runs (a pause inside the run returns from it), so it is read
        // once; it stays true while stepping even after the flags clear.
        let debugging = self.debug_context.is_active();
        // The instruction `run_fast` last stopped at without running anything.
        let mut fast_skip: Option<(usize, usize)> = None;
        // The instruction after the one at `ip`, for instructions that also run
        // the following one they produce the input of (fused, saving a round
        // through this loop). Not while a debugger may stop at it.
        macro_rules! next_instruction {
            ($ip:expr) => {
                if $ip + 1 < code_len && !debugging {
                    // SAFETY: `code` points to the `code_len` instructions of
                    // function `code_function` (see above).
                    Some(unsafe { *code.add($ip + 1) })
                } else {
                    None
                }
            };
        }
        // After a comparison at index `$at` that wrote `$result` to register
        // `$dest`: runs a conditional jump on that register that follows it.
        macro_rules! fused_jump {
            ($frame:ident, $at:expr, $dest:expr, $result:expr) => {
                if let Some(jump) = next_instruction!($at)
                    && matches!(jump.opcode(), Opcode::JumpIfNot | Opcode::JumpIf)
                    && jump.a() as usize == $dest
                {
                    self.instruction_count += 1;
                    $frame.ip = $at + 2;
                    if $result == (jump.opcode() == Opcode::JumpIf) {
                        let offset = decode_offset(jump.b(), jump.c());
                        $frame.ip = apply_offset($frame.ip, offset)?;
                    }
                }
            };
        }
        loop {
            // Between instructions every live value is in a register, a frame or
            // another root, so this is where the collector may run: after
            // enough objects, or (checked now and then) after enough new strings
            // and big integers, which only a collection frees from their tables.
            if self.gc.current_allocation >= self.gc.gc.allocation_threshold
                || (self.instruction_count.is_multiple_of(128)
                    && self.gc.sweeps_boxed_values
                    && pleno_value::nan_value::boxed_values_added()
                        > BOXED_VALUES_PER_COLLECTION.max(self.gc.current_allocation))
            {
                let roots = VmRoots {
                    registers: &self.registers,
                    frames: &self.frames,
                    globals: &self.globals,
                    modules: &self.modules,
                    module_globals: &self.module_globals,
                    replaced_globals: &self.program_globals.replaced,
                    builtin_extensions: &self.builtin_extensions,
                    pending_tasks: &self.pending_tasks,
                    suspended_frames: &self.suspended_frames,
                    promises: &self.promises,
                    multi_waiters: &self.multi_waiters,
                };
                self.gc.collect_garbage(&[&roots]);
            }

            if self.frames.len() == depth {
                return Ok(Some(ExecutionStatus::Finished(Value::Null)));
            }

            // Breakpoints, pauses and stepping; without a debugger attached this
            // is one check.
            if debugging {
                // The debugger's stepping commands look at each instruction.
                if !std::mem::take(&mut first) && self.debug_context.stepping {
                    return Ok(None);
                }
                let (func_index, ip) = {
                    let frame = self
                        .frames
                        .last()
                        .ok_or_else(|| RuntimeError::new("no frame"))?;
                    (frame.function_index, frame.ip)
                };
                if let Some(status) = self.check_breakpoint(func_index, ip) {
                    return Ok(Some(status));
                }
            }

            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| RuntimeError::new("no frame"))?;
            let ip = frame.ip;

            let function = &self.functions[frame.function_index];
            if frame.function_index != code_function {
                code_function = frame.function_index;
                code = function.chunk.code.as_ptr();
                code_len = function.chunk.code.len();
            }
            if ip >= code_len {
                return Err(self.throw_runtime_error("program counter out of bounds"));
            }
            // SAFETY: `ip < code_len`, and `code` points to the `code_len`
            // instructions of function `code_function` (see above).
            let instruction = unsafe { *code.add(ip) };
            let opcode = instruction.opcode();

            // Simple instructions run in the smaller loop of `run_fast`. When it
            // stops at once (an addition of strings, say), this instruction runs
            // here instead.
            if !debugging && self.fast_loop && fast::handles(opcode) {
                let at = (code_function, ip);
                if fast_skip != Some(at) {
                    let before = self.instruction_count;
                    self.run_fast(depth)?;
                    if self.instruction_count == before {
                        fast_skip = Some(at);
                    }
                    continue;
                }
                fast_skip = None;
            }

            #[cfg(debug_assertions)]
            log::trace!("VM::run_step: ip={:04} {:?}", ip, opcode);
            frame.ip += 1;

            self.instruction_count += 1;
            if let Some(config) = &self.sandbox_config
                && self.instruction_count.is_multiple_of(100)
            {
                if config.max_instructions > 0 && self.instruction_count > config.max_instructions {
                    return Err(self.sandbox_limit_error("instruction limit exceeded"));
                }
                if config.max_memory > 0
                    && self.gc.allocated_bytes > config.max_memory + self.gc.grace_bytes
                {
                    return Err(self.sandbox_limit_error("memory limit exceeded"));
                }
            }

            let a = instruction.a() as usize;
            let b = instruction.b() as usize;
            let c = instruction.c() as usize;
            let base = frame.base;

            match opcode {
                Opcode::GetIterator => {
                    let source_nan = self.registers[base + b];
                    let source = source_nan.to_value();
                    let wants_entries = c as u8 == pleno_bytecode::ITER_ENTRIES
                        || (c as u8 == pleno_bytecode::ITER_MAP_ENTRIES
                            && matches!(source, Value::Map(_)));
                    let name = if wants_entries {
                        "entries".to_string()
                    } else {
                        "iterator".to_string()
                    };
                    let func_reg = base + a;

                    // Look up iterator method
                    if let Some(native) = self
                        .builtin_resolver
                        .and_then(|r| r(source.type_name(), &name))
                    {
                        match native(self, &[source]) {
                            Ok(iter) => {
                                self.registers[base + a] = NanValue::from_value(&iter);
                            }
                            Err(err) => {
                                return Err(self.rethrow(err));
                            }
                        }
                    } else if let Value::Object(obj) = &source {
                        if let Some(class) = &obj.borrow().class
                            && let Some(method_info) = class.borrow().methods.get(&name)
                        {
                            self.call_function(
                                method_info.function.clone(),
                                Some(Value::Object(obj.clone())),
                                &[],
                                func_reg,
                                false,
                                CallOptions {
                                    current_owner: Some(method_info.owner.clone()),
                                    ..Default::default()
                                },
                            )?;
                            continue;
                        }
                        return Err(RuntimeError::new(format!(
                            "property '{}' not found on Object",
                            name
                        )));
                    } else {
                        return Err(RuntimeError::new(format!(
                            "type '{}' is not iterable",
                            source.type_name()
                        )));
                    }
                }
                Opcode::IterNext => {
                    let iter_nan = self.registers[base + b];
                    let iter = iter_nan.to_value();
                    let name = "next".to_string();
                    let func_reg = base + a;

                    // Look up next method
                    if let Some(native) = self
                        .builtin_resolver
                        .and_then(|r| r(iter.type_name(), &name))
                    {
                        let is_channel = matches!(iter, Value::Channel(_));
                        match native(self, &[iter]) {
                            // An empty channel hands back a promise for its next
                            // value: wait for it, as `await` does.
                            Ok(val) if is_channel && pending_promise_id(&val).is_some() => {
                                let id = pending_promise_id(&val).unwrap_or_default();
                                let frame = self
                                    .frames
                                    .pop()
                                    .ok_or_else(|| RuntimeError::new("VM stack underflow"))?;
                                self.suspended_frames.insert(
                                    id,
                                    SuspendedFrame {
                                        frame,
                                        await_reg: a,
                                    },
                                );
                                // Callers below keep running; the run ends once it is back at `depth`.
                                return Ok(None);
                            }
                            Ok(val) => {
                                self.registers[base + a] = NanValue::from_value(&val);
                            }
                            Err(err) => {
                                return Err(self.rethrow(err));
                            }
                        }
                    } else if let Value::Object(obj) = &iter {
                        if let Some(class) = &obj.borrow().class
                            && let Some(method_info) = class.borrow().methods.get(&name)
                        {
                            self.call_function(
                                method_info.function.clone(),
                                Some(Value::Object(obj.clone())),
                                &[],
                                func_reg,
                                false,
                                CallOptions {
                                    current_owner: Some(method_info.owner.clone()),
                                    ..Default::default()
                                },
                            )?;
                            continue;
                        }
                        return Err(RuntimeError::new(format!(
                            "property '{}' not found on Object",
                            name
                        )));
                    } else {
                        return Err(RuntimeError::new(format!(
                            "type '{}' is not an iterator",
                            iter.type_name()
                        )));
                    }
                }
                Opcode::Move => {
                    let value = self.registers[base + b];
                    self.registers[base + a] = value;
                    // The copy of the callee just before a call: make the call
                    // here when it is a plain call of a bytecode function.
                    if let Some(call) = next_instruction!(ip)
                        && call.opcode() == Opcode::CallRegs
                        && call.b() as usize == a
                        && base + a + 1 + call.c() as usize <= self.registers.len()
                    {
                        frame.ip = ip + 2;
                        let return_reg = base + call.a() as usize;
                        if self.try_call_regs(value, base + a + 1, call.c() as usize, return_reg)? {
                            self.instruction_count += 1;
                            continue;
                        }
                        // Not a plain call: run `CallRegs` itself next.
                        if let Some(frame) = self.frames.last_mut() {
                            frame.ip = ip + 1;
                        }
                    }
                }
                Opcode::LoadConst => {
                    // A 16-bit constant index: b is the low byte, c the high byte.
                    let Some(value) = function.chunk.constants.get(b | (c << 8)) else {
                        return Err(RuntimeError::new("constant index out of range"));
                    };
                    self.registers[base + a] = NanValue::from_value(value);
                }
                Opcode::LoadMissing => {
                    self.registers[base + a] = NanValue::missing();
                }
                Opcode::LoadNull => {
                    self.registers[base + a] = NanValue::null();
                }
                Opcode::LoadTrue => {
                    self.registers[base + a] = NanValue::from_bool(true);
                }
                Opcode::LoadFalse => {
                    self.registers[base + a] = NanValue::from_bool(false);
                }
                Opcode::LoadInt => {
                    let value = (b as i8) as i64;
                    self.registers[base + a] = NanValue::from_int(value);
                    // `x < 2`: the comparison of an integer with this constant
                    // (and a jump on its result) runs here too.
                    if let Some(compare) = next_instruction!(ip)
                        && matches!(
                            compare.opcode(),
                            Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge
                        )
                        && compare.c() as usize == a
                        && let Some(left) =
                            self.registers[base + compare.b() as usize].as_inline_int()
                    {
                        let result = match compare.opcode() {
                            Opcode::Lt => left < value,
                            Opcode::Le => left <= value,
                            Opcode::Gt => left > value,
                            _ => left >= value,
                        };
                        let dest = compare.a() as usize;
                        self.registers[base + dest] = NanValue::from_bool(result);
                        self.instruction_count += 1;
                        frame.ip = ip + 2;
                        fused_jump!(frame, ip + 1, dest, result);
                    }
                }
                Opcode::Closure => {
                    let proto = function
                        .chunk
                        .functions
                        .get(b)
                        .ok_or_else(|| RuntimeError::new("unknown function prototype"))?;
                    let func_index = self.functions.len();
                    self.functions.push(Function {
                        name: Rc::new(proto.name.clone()),
                        chunk: proto.chunk.clone(),
                        arity: proto.arity as usize,
                        params: proto.params.clone(),
                        // A function belongs to the module it is defined in.
                        module: function.module.clone(),
                        module_path: function.module_path.as_ref().cloned(),
                        is_async: proto.is_async,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
                        plain_args: plain_arg_range(&proto.params, proto.arity as usize),
                        deprecation: deprecation_note(&proto.annotations),
                    });
                    let captures = vec![Value::Null; c];
                    let state = self.gc.allocate(ClosureState::new(func_index, captures))?;
                    self.registers[base + a] = NanValue::from_value(&Value::Closure(state));
                }
                Opcode::AsyncFunction => {
                    let proto = function
                        .chunk
                        .functions
                        .get(b)
                        .ok_or_else(|| RuntimeError::new("unknown function prototype"))?;
                    let func_index = self.functions.len();
                    self.functions.push(Function {
                        name: Rc::new(proto.name.clone()),
                        chunk: proto.chunk.clone(),
                        arity: proto.arity as usize,
                        params: proto.params.clone(),
                        // A function belongs to the module it is defined in.
                        module: function.module.clone(),
                        module_path: function.module_path.as_ref().cloned(),
                        is_async: true,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
                        plain_args: plain_arg_range(&proto.params, proto.arity as usize),
                        deprecation: deprecation_note(&proto.annotations),
                    });
                    self.registers[base + a] = NanValue::from_value(&Value::Function(func_index));
                }
                Opcode::FunctionRef => {
                    let proto = function
                        .chunk
                        .functions
                        .get(b)
                        .ok_or_else(|| RuntimeError::new("unknown function prototype"))?;
                    let func_index = self.functions.len();
                    self.functions.push(Function {
                        name: Rc::new(proto.name.clone()),
                        chunk: proto.chunk.clone(),
                        arity: proto.arity as usize,
                        params: proto.params.clone(),
                        // A function belongs to the module it is defined in.
                        module: function.module.clone(),
                        module_path: function.module_path.as_ref().cloned(),
                        is_async: proto.is_async,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
                        plain_args: plain_arg_range(&proto.params, proto.arity as usize),
                        deprecation: deprecation_note(&proto.annotations),
                    });
                    self.registers[base + a] = NanValue::from_value(&Value::Function(func_index));
                }
                Opcode::Capture => {
                    let value = self.registers[base + b].to_value();
                    if let Value::Closure(state) = &self.registers[base + a].to_value() {
                        let mut state = state.borrow_mut();
                        if c >= state.captures.len() {
                            return Err(RuntimeError::new("capture index out of bounds"));
                        }
                        state.captures[c] = value;
                    } else {
                        return Err(RuntimeError::new("capture on non-closure"));
                    }
                }
                Opcode::LoadCapture => {
                    let idx = b;
                    if let Some(state) = frame.closure_state.as_ref().cloned() {
                        let state = state.borrow();
                        if idx >= state.captures.len() {
                            return Err(RuntimeError::new("capture index out of bounds"));
                        }
                        let val = state.captures[idx].clone();
                        log::trace!("LoadCapture: index={} value={:?}", idx, val);
                        self.registers[base + a] = NanValue::from_value(&val);
                    } else {
                        return Err(RuntimeError::new("not in a closure"));
                    }
                }
                Opcode::SetCapture => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (captured state)",
                        ));
                    }
                    let idx = b;
                    let value = self.registers[base + a].to_value();
                    if let Some(state) = frame.closure_state.as_ref().cloned() {
                        let mut state = state.borrow_mut();
                        if idx >= state.captures.len() {
                            return Err(RuntimeError::new("capture index out of bounds"));
                        }
                        state.captures[idx] = value;
                    } else {
                        return Err(RuntimeError::new("not in a closure"));
                    }
                }
                Opcode::MakeCell => {
                    let value = self.registers[base + b].to_value();
                    let cell = self.gc.allocate(pleno_value::CellState::new(value))?;
                    self.registers[base + a] = NanValue::from_value(&Value::Cell(cell));
                }
                Opcode::LoadCell => {
                    let val = if let Value::Cell(cell) = &self.registers[base + b].to_value() {
                        cell.borrow().value.clone()
                    } else {
                        return Err(RuntimeError::new("LoadCell from non-cell value"));
                    };
                    self.registers[base + a] = NanValue::from_value(&val);
                }
                Opcode::SetCell => {
                    let value = self.registers[base + b].to_value();
                    if let Value::Cell(cell) = &self.registers[base + a].to_value() {
                        cell.borrow_mut().value = value;
                    } else {
                        return Err(RuntimeError::new("SetCell on non-cell value"));
                    }
                }
                Opcode::MakeThunk => {
                    let callable = self.registers[base + b].to_value();
                    let thunk = self.gc.allocate(ThunkState::new(callable))?;
                    self.registers[base + a] = NanValue::from_value(&Value::Thunk(thunk));
                }
                Opcode::ForceThunk => {
                    let value_nan = self.registers[base + b];
                    if !value_nan.is_thunk() {
                        self.registers[base + a] = value_nan;
                        continue;
                    }
                    let value = value_nan.to_value();
                    let forced = self.force_thunk(value)?;
                    self.registers[base + a] = NanValue::from_value(&forced);
                }
                Opcode::BuildRange => {
                    let start = expect_int_or_null(&self.registers[base + b].to_value())?;
                    let end = expect_int_or_null(&self.registers[base + c].to_value())?;
                    self.registers[base + a] =
                        NanValue::from_value(&Value::Range(Rc::new(Range {
                            start,
                            end,
                            inclusive: false,
                        })));
                }
                Opcode::BuildRangeInclusive => {
                    let start = expect_int_or_null(&self.registers[base + b].to_value())?;
                    let end = expect_int_or_null(&self.registers[base + c].to_value())?;
                    self.registers[base + a] =
                        NanValue::from_value(&Value::Range(Rc::new(Range {
                            start,
                            end,
                            inclusive: true,
                        })));
                }
                Opcode::NewMap => {
                    self.registers[base + a] = NanValue::from_value(&Value::Map(
                        self.gc.allocate(MapState::new(IndexMap::new()))?,
                    ));
                }
                Opcode::NewArray => {
                    self.registers[base + a] = NanValue::from_value(&Value::Array(
                        self.gc.allocate(ArrayState::new(Vec::new()))?,
                    ));
                }
                Opcode::NewSet => {
                    self.registers[base + a] = NanValue::from_value(&Value::Set(
                        self.gc.allocate(SetState::new(IndexSet::new()))?,
                    ));
                }
                Opcode::SetAdd => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Set(set) => {
                            if set.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable set"));
                            }
                            let key = value_to_key(&value)?;
                            set.borrow_mut().insert(key);
                        }
                        _ => return Err(RuntimeError::new("set add expects set")),
                    }
                }
                Opcode::SetHas => {
                    let value = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Set(set) => {
                            let key = value_to_key(&value)?;
                            let has = set.borrow().contains(&key);
                            self.registers[base + a] = NanValue::from_bool(has);
                        }
                        _ => return Err(RuntimeError::new("set has on non-set value")),
                    }
                }
                Opcode::SetLen => match self.registers[base + b].to_value() {
                    Value::Set(set) => {
                        let len = set.borrow().len() as i64;
                        self.registers[base + a] = NanValue::from_int(len);
                    }
                    _ => return Err(RuntimeError::new("set len on non-set value")),
                },
                Opcode::SetValues => match self.registers[base + b].to_value() {
                    Value::Set(set) => {
                        let values: Vec<Value> = set.borrow().iter().map(key_to_value).collect();
                        self.registers[base + a] = NanValue::from_value(&Value::Array(
                            self.gc.allocate(ArrayState::new(values))?,
                        ));
                    }
                    _ => return Err(RuntimeError::new("set values on non-set value")),
                },
                Opcode::ArrayAppend => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Array(array) => {
                            if array.borrow().is_immutable {
                                return Err(
                                    self.throw_runtime_error("cannot modify immutable array")
                                );
                            }
                            self.gc.track_allocation(std::mem::size_of::<Value>())?;
                            array.borrow_mut().elements.push(value);
                        }
                        _ => return Err(RuntimeError::new("array append expects array")),
                    }
                }
                Opcode::NewTuple => {
                    let tuple = TupleState::new(Vec::new(), None);
                    self.registers[base + a] =
                        NanValue::from_value(&Value::Tuple(self.gc.allocate(tuple)?));
                }
                Opcode::TupleAppend => {
                    let val = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Tuple(t) => {
                            t.borrow_mut().elements.push(val);
                        }
                        _ => return Err(RuntimeError::new("tuple append expects tuple")),
                    }
                }
                Opcode::TupleNamedAppend => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("tuple property name must be string")),
                    };
                    let value = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Tuple(t) => {
                            let mut t = t.borrow_mut();
                            let idx = t.elements.len();
                            t.elements.push(value);
                            if t.names.is_none() {
                                t.names = Some(IndexMap::new());
                            }
                            t.names.as_mut().unwrap().insert(name, idx);
                        }
                        _ => return Err(RuntimeError::new("tuple named append expects tuple")),
                    }
                }
                Opcode::ArraySpread => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match (self.registers[base + a].to_value(), value) {
                        (Value::Array(target), Value::Array(source)) => {
                            if target.borrow().is_immutable {
                                return Err(
                                    self.throw_runtime_error("cannot modify immutable array")
                                );
                            }
                            target
                                .borrow_mut()
                                .elements
                                .extend(source.borrow().elements.iter().cloned());
                        }
                        _ => return Err(RuntimeError::new("array spread expects array values")),
                    }
                }
                Opcode::MapSpread => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match (self.registers[base + a].to_value(), value) {
                        (Value::Map(target), Value::Map(source)) => {
                            if target.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable map"));
                            }
                            let source_len = source.borrow().entries.len();
                            self.gc.track_allocation(
                                source_len * (std::mem::size_of::<Value>() + 16),
                            )?; // 16 for approx key size
                            for (k, v) in source.borrow().entries.iter() {
                                target.borrow_mut().entries.insert(k.clone(), v.clone());
                            }
                        }
                        _ => return Err(RuntimeError::new("map spread expects map values")),
                    }
                }
                Opcode::MapRemove => {
                    let key_val = self.registers[base + b].to_value();
                    let name = match key_val {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("map remove key must be string")),
                    };
                    match self.registers[base + a].to_value() {
                        Value::Map(map) => {
                            if map.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable map"));
                            }
                            map.borrow_mut().entries.swap_remove(&name);
                        }
                        _ => return Err(RuntimeError::new("map remove expects map")),
                    }
                }
                Opcode::SetSpread => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match (self.registers[base + a].to_value(), value) {
                        (Value::Set(target), Value::Set(source)) => {
                            for k in source.borrow().iter() {
                                target.borrow_mut().insert(k.clone());
                            }
                        }
                        _ => return Err(RuntimeError::new("set spread expects set values")),
                    }
                }
                Opcode::ArrayExtend => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].to_value();
                    match (self.registers[base + a].to_value(), value) {
                        (Value::Array(target), Value::Array(source)) => {
                            let source_len = source.borrow().elements.len();
                            self.gc
                                .track_allocation(source_len * std::mem::size_of::<Value>())?;
                            target
                                .borrow_mut()
                                .elements
                                .extend(source.borrow().elements.iter().cloned());
                        }
                        _ => return Err(RuntimeError::new("array extend expects array values")),
                    }
                }
                Opcode::NewObject => {
                    self.registers[base + a] = NanValue::from_value(&Value::Object(
                        self.gc.allocate(Object::new(None, None))?,
                    ));
                }
                Opcode::SetField => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let mut value = self.registers[base + a].to_value();
                    match self.registers[base + b].to_value() {
                        Value::Object(object) => {
                            if object.borrow().is_immutable {
                                return Err(
                                    self.throw_runtime_error("cannot modify immutable object")
                                );
                            }

                            let class = object.borrow().class.clone();
                            if let Some(class) = class {
                                let ty_name = class.borrow().field_types.get(&name).cloned();
                                if let Some(Some(ty_name)) = ty_name {
                                    let ty_val = Value::String(Rc::new(ty_name));
                                    if !self.is_instance(&value, &ty_val) {
                                        let actual_type_name = value.type_name();
                                        let msg = format!(
                                            "TypeError: field '{}' expected {}, got {}",
                                            name,
                                            ty_val.as_str().unwrap_or_default(),
                                            actual_type_name
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                    value = widen_to_annotation(value, &ty_val);
                                }
                            }

                            self.gc
                                .track_allocation(std::mem::size_of::<Value>() + name.len())?;
                            object.borrow_mut().fields.insert(name, value);
                        }
                        _ => return Err(RuntimeError::new("field set on non-object value")),
                    }
                }
                Opcode::GetField => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let value = match self.registers[base + b].to_value() {
                        Value::Object(object) => object
                            .borrow()
                            .fields
                            .get(&name)
                            .cloned()
                            .unwrap_or(Value::Null),
                        Value::Array(array) => {
                            if name == "length" {
                                Value::Int(array.borrow().elements.len() as i64)
                            } else {
                                return Err(RuntimeError::new("unknown array field"));
                            }
                        }
                        Value::String(s) => {
                            if name == "length" {
                                Value::Int(pleno_value::text::char_len(&s) as i64)
                            } else {
                                return Err(RuntimeError::new("unknown string field"));
                            }
                        }
                        _ => return Err(RuntimeError::new("field access on non-object value")),
                    };
                    self.registers[base + a] = NanValue::from_value(&value);
                }
                Opcode::MapKeys => {
                    let keys = match self.registers[base + b].to_value() {
                        Value::Map(map) => map
                            .borrow()
                            .entries
                            .keys()
                            .cloned()
                            .map(|key| Value::String(Rc::new(key)))
                            .collect::<Vec<_>>(),
                        Value::Object(object) => object
                            .borrow()
                            .fields
                            .keys()
                            .cloned()
                            .map(|key| Value::String(Rc::new(key)))
                            .collect::<Vec<_>>(),
                        _ => return Err(RuntimeError::new("MapKeys expects map or object")),
                    };
                    self.registers[base + a] = NanValue::from_value(&Value::Array(
                        self.gc.allocate(ArrayState::new(keys))?,
                    ));
                }
                Opcode::IterValues => {
                    let value = match self.registers[base + b].to_value() {
                        Value::Array(array) => Ok(Value::Array(array.clone())),
                        Value::Map(map) => {
                            let values = map
                                .borrow()
                                .entries
                                .keys()
                                .cloned()
                                .map(|key| Value::String(Rc::new(key)))
                                .collect::<Vec<_>>();
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(values))?))
                        }
                        Value::Set(set) => {
                            let values = set.borrow().iter().map(key_to_value).collect::<Vec<_>>();
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(values))?))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    }?;
                    self.registers[base + a] = NanValue::from_value(&value);
                }
                Opcode::IterEntries => {
                    let value = match self.registers[base + b].to_value() {
                        Value::Array(array) => {
                            let entries = array
                                .borrow()
                                .elements
                                .iter()
                                .enumerate()
                                .map(|(idx, val)| {
                                    Ok(Value::Array(self.gc.allocate(ArrayState::new(vec![
                                        Value::Int(idx as i64),
                                        val.clone(),
                                    ]))?))
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(entries))?))
                        }
                        Value::Map(map) => {
                            let entries =
                                map.borrow()
                                    .entries
                                    .iter()
                                    .map(|(key, val)| {
                                        Ok(Value::Array(self.gc.allocate(ArrayState::new(
                                            vec![Value::String(Rc::new(key.clone())), val.clone()],
                                        ))?))
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(entries))?))
                        }
                        Value::Set(set) => {
                            let entries =
                                set.borrow()
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, val)| {
                                        let val = key_to_value(val);
                                        Ok(Value::Array(self.gc.allocate(ArrayState::new(
                                            vec![Value::Int(idx as i64), val],
                                        ))?))
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(entries))?))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    }?;
                    self.registers[base + a] = NanValue::from_value(&value);
                }
                Opcode::ArrayLen => {
                    let len = match self.registers[base + b].to_value() {
                        Value::Array(array) => array.borrow().elements.len() as i64,
                        Value::Tuple(tuple) => tuple.borrow().elements.len() as i64,
                        _ => {
                            return Err(RuntimeError::new("array length on non-array/tuple value"));
                        }
                    };
                    self.registers[base + a] = NanValue::from_int(len);
                }
                Opcode::Await => {
                    let value = self.registers[base + b].to_value();
                    log::trace!("Opcode::Await on value: {:?}", value);
                    match value {
                        Value::Map(map) => {
                            let variant = map.borrow().entries.get("variant").cloned();
                            match variant {
                                Some(Value::String(s)) if s.as_str() == "Pending" => {
                                    let id = map
                                        .borrow()
                                        .entries
                                        .get("id")
                                        .and_then(|v| match v {
                                            Value::Int(i) => Some(*i as u64),
                                            _ => None,
                                        })
                                        .unwrap_or(self.next_promise_id);

                                    let frame = self
                                        .frames
                                        .pop()
                                        .ok_or_else(|| RuntimeError::new("VM stack underflow"))?;
                                    self.suspended_frames.insert(
                                        id,
                                        SuspendedFrame {
                                            frame,
                                            await_reg: a,
                                        },
                                    );
                                    // Suspended: callers below keep running, as after a return.
                                    return Ok(None);
                                }
                                Some(Value::String(s)) if s.as_str() == "Resolved" => {
                                    let val = map
                                        .borrow()
                                        .entries
                                        .get("value")
                                        .cloned()
                                        .unwrap_or(Value::Null);
                                    self.registers[base + a] = NanValue::from_value(&val);
                                }
                                Some(Value::String(s)) if s.as_str() == "Rejected" => {
                                    if let Some(Value::Int(id)) = map.borrow().entries.get("id") {
                                        self.unhandled_rejections.swap_remove(&(*id as u64));
                                    }
                                    let reason = map
                                        .borrow()
                                        .entries
                                        .get("value")
                                        .cloned()
                                        .unwrap_or(Value::Null);
                                    match self.unwind_to_handler(reason) {
                                        Ok(_) => unreachable!(),
                                        Err(err) => return Err(err),
                                    }
                                }
                                _ => {
                                    self.registers[base + a] =
                                        NanValue::from_value(&Value::Map(map));
                                }
                            }
                        }
                        _ => {
                            self.registers[base + a] = NanValue::from_value(&value);
                        }
                    }
                }
                Opcode::Is => {
                    let value = self.registers[base + b];
                    let type_val = self.registers[base + c];
                    let result = self.is_instance_nan(&value, &type_val);
                    self.registers[base + a] = NanValue::from_bool(result);
                }
                Opcode::TypeOf => {
                    // Instances report their class and variants their enum.
                    let type_name = match self.registers[base + b].to_value() {
                        Value::Object(object) => object
                            .borrow()
                            .class
                            .as_ref()
                            .map(|class| class.borrow().name.clone())
                            .unwrap_or_else(|| Rc::new("Object".to_string())),
                        Value::Variant(variant) => variant.borrow().enum_type.borrow().name.clone(),
                        other => Rc::new(other.type_name().to_string()),
                    };
                    self.registers[base + a] = NanValue::from_value(&Value::String(type_name));
                }
                // `b + c` and `b - c` for a small integer constant c: the same
                // as `Add` and `Sub` with an `Int` operand, for every type of b.
                Opcode::AddInt => {
                    let left = self.registers[base + b];
                    self.add_registers(base + a, left, NanValue::from_int(c as i64))?;
                }
                Opcode::SubInt => {
                    let left = self.registers[base + b];
                    self.sub_registers(base + a, left, NanValue::from_int(c as i64))?;
                }
                Opcode::MulInt => {
                    let val = match self.registers[base + b].as_int() {
                        Some(i) => i,
                        _ => return Err(RuntimeError::new("MulInt expects integer")),
                    };
                    let result = val
                        .checked_mul(c as i64)
                        .ok_or_else(|| RuntimeError::new("integer overflow in multiplication"))?;
                    self.registers[base + a] = NanValue::from_int(result);
                }
                Opcode::DivInt => {
                    let val = match self.registers[base + b].as_int() {
                        Some(i) => i,
                        _ => return Err(RuntimeError::new("DivInt expects integer")),
                    };
                    if c == 0 {
                        return Err(RuntimeError::new("division by zero"));
                    }
                    // `c` is a non-zero u8, so this cannot overflow.
                    self.registers[base + a] = NanValue::from_int(val / c as i64);
                }
                Opcode::ModInt => {
                    let val = match self.registers[base + b].as_int() {
                        Some(i) => i,
                        _ => return Err(RuntimeError::new("ModInt expects integer")),
                    };
                    if c == 0 {
                        return Err(RuntimeError::new("division by zero"));
                    }
                    self.registers[base + a] = NanValue::from_int(val % c as i64);
                }

                Opcode::SetProperty => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let value = self.registers[base + a];
                    let current_owner = frame.current_owner.clone();
                    let current_class = match &current_owner {
                        Some(MethodOwner::Class(c)) => Some(c.clone()),
                        _ => None,
                    };
                    let current_module = function.module.as_ref().cloned();
                    let target = self.registers[base + b].to_value();
                    match target {
                        Value::Map(map) => {
                            if map.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable map"));
                            }
                            self.gc
                                .track_allocation(std::mem::size_of::<Value>() + name.len())?;
                            map.borrow_mut().insert(name, value.to_value());
                        }
                        Value::Object(object) => {
                            if object.borrow().is_immutable {
                                return Err(
                                    self.throw_runtime_error("cannot modify immutable object")
                                );
                            }
                            let class = object.borrow().class.clone();
                            if let Some(class) = class.as_ref() {
                                if let Some(info) = resolve_setter(class, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("property access denied")
                                        );
                                    }
                                    self.call_function(
                                        info.function.clone(),
                                        Some(Value::Object(object.clone())),
                                        &[value.to_value()],
                                        base + a,
                                        false,
                                        CallOptions {
                                            current_owner: Some(info.owner.clone()),
                                            ..Default::default()
                                        },
                                    )?;
                                    continue;
                                }
                                if let Some((owner, visibility)) = find_field_owner(class, &name) {
                                    if !can_access_field(
                                        &visibility,
                                        &owner,
                                        current_class.as_ref(),
                                        current_module.as_ref(),
                                    ) {
                                        let msg = field_access_error(
                                            "assign to",
                                            &visibility,
                                            "field",
                                            &name,
                                            &class.borrow().name,
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                    // A `const` field is set by a constructor of its class
                                    // (or of a subclass) and never changes afterwards.
                                    let in_constructor =
                                        self.functions[frame.function_index].name.as_str()
                                            == "constructor"
                                            && current_class
                                                .as_ref()
                                                .is_some_and(|c| is_subclass_of(c, &owner));
                                    if !in_constructor
                                        && owner.borrow().const_fields.contains(&name)
                                    {
                                        let msg = format!(
                                            "cannot assign to constant field '{}' of {} outside its constructor",
                                            name,
                                            class.borrow().name
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                }

                                let ty_name = class.borrow().field_types.get(&name).cloned();
                                if let Some(Some(ty_name)) = ty_name {
                                    let ty_val = Value::String(Rc::new(ty_name));
                                    if !self.is_instance(&value.to_value(), &ty_val) {
                                        let actual_type_name = value.type_name();
                                        let msg = format!(
                                            "TypeError: field '{}' expected {}, got {}",
                                            name,
                                            ty_val.as_str().unwrap_or_default(),
                                            actual_type_name
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                    let widened = widen_to_annotation(value.to_value(), &ty_val);
                                    object.borrow_mut().fields.insert(name, widened);
                                    continue;
                                }
                            }
                            object.borrow_mut().fields.insert(name, value.to_value());
                        }
                        Value::Class(class) => {
                            if let Some(info) = resolve_static_setter(&class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static property access denied")
                                    );
                                }
                                self.call_function(
                                    info.function.clone(),
                                    None,
                                    &[value.to_value()],
                                    base + a,
                                    false,
                                    CallOptions {
                                        current_owner: Some(info.owner.clone()),
                                        ..Default::default()
                                    },
                                )?;
                                continue;
                            }
                            let visibility =
                                class.borrow().static_field_visibility.get(&name).cloned();
                            if let Some(visibility) = visibility {
                                if !can_access_field(
                                    &visibility,
                                    &class,
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                ) {
                                    let msg = field_access_error(
                                        "assign to",
                                        &visibility,
                                        "static field",
                                        &name,
                                        &class.borrow().name,
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                                if class.borrow().const_static_fields.contains(&name) {
                                    let msg = format!(
                                        "cannot assign to constant {}.{}",
                                        class.borrow().name,
                                        name
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                            }

                            let mut stored = value.to_value();
                            let ty_name = class.borrow().static_field_types.get(&name).cloned();
                            if let Some(Some(ty_name)) = ty_name {
                                let ty_val = Value::String(Rc::new(ty_name));
                                if !self.is_instance(&stored, &ty_val) {
                                    let actual_type_name = value.type_name();
                                    let msg = format!(
                                        "TypeError: static field '{}' expected {}, got {}",
                                        name,
                                        ty_val.as_str().unwrap_or_default(),
                                        actual_type_name
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                                stored = widen_to_annotation(stored, &ty_val);
                            }

                            class.borrow_mut().static_fields.insert(name, stored);
                        }
                        _ => return Err(RuntimeError::new("field set on non-object")),
                    }
                }
                Opcode::GetProperty | Opcode::GetMethod => {
                    // For `GetMethod`, where the method goes (see `Opcode::GetMethod`).
                    let method_at = (opcode == Opcode::GetMethod).then_some(base + a);
                    if let Some(at) = method_at {
                        if at + 2 >= self.registers.len() {
                            return Err(RuntimeError::new("method registers out of range"));
                        }
                        self.registers[at + 1] = self.registers[base + b];
                        self.registers[at + 2] = NanValue::from_bool(false);
                    }
                    let name_rc = match &function.chunk.constants[c] {
                        Value::String(value) => Rc::clone(value),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let name = name_rc.as_str();
                    let current_owner = frame.current_owner.clone();
                    let current_class = match &current_owner {
                        Some(MethodOwner::Class(c)) => Some(c.clone()),
                        _ => None,
                    };
                    let value_nan = self.registers[base + b];
                    let value = value_nan.to_value();
                    log::trace!("GetProperty: object={:?} name={}", value, name);

                    let result_val = match value {
                        Value::Map(map) => {
                            let map_ref = map.borrow();
                            if let Some(Value::String(variant)) = map_ref.get("variant") {
                                if (variant.as_str() == "Ok" || variant.as_str() == "Err")
                                    && self
                                        .builtin_methods
                                        .get(self.builtin_resolver, "Result", &name_rc)
                                        .is_some()
                                {
                                    drop(map_ref);
                                    let func = self
                                        .builtin_methods
                                        .get(self.builtin_resolver, "Result", &name_rc)
                                        .unwrap();
                                    let method = self.native_method_value(
                                        method_at,
                                        Value::Map(map.clone()),
                                        func,
                                    )?;
                                    self.registers[base + a] = NanValue::from_value(&method);
                                    continue;
                                } else if matches!(
                                    variant.as_str(),
                                    "Pending" | "Resolved" | "Rejected"
                                ) && map_ref.get("id").is_some()
                                    && let Some(func) = self.builtin_methods.get(
                                        self.builtin_resolver,
                                        "Promise",
                                        &name_rc,
                                    )
                                {
                                    drop(map_ref);
                                    let method = self.native_method_value(
                                        method_at,
                                        Value::Map(map.clone()),
                                        func,
                                    )?;
                                    self.registers[base + a] = NanValue::from_value(&method);
                                    continue;
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && self
                                        .builtin_methods
                                        .get(self.builtin_resolver, "Option", &name_rc)
                                        .is_some()
                                {
                                    drop(map_ref);
                                    let func = self
                                        .builtin_methods
                                        .get(self.builtin_resolver, "Option", &name_rc)
                                        .unwrap();
                                    let method = self.native_method_value(
                                        method_at,
                                        Value::Map(map.clone()),
                                        func,
                                    )?;
                                    self.registers[base + a] = NanValue::from_value(&method);
                                    continue;
                                }
                            }
                            let val = map_ref.entries.get(name).cloned();
                            drop(map_ref);

                            if let Some(val) = val {
                                val
                            } else if let Some(func) =
                                self.builtin_methods
                                    .get(self.builtin_resolver, "Map", &name_rc)
                            {
                                self.native_method_value(method_at, Value::Map(map.clone()), func)?
                            } else if let Some(method) =
                                self.extension_method(method_at, &Value::Map(map.clone()), name)?
                            {
                                method
                            } else if name == "toString" {
                                self.bind_to_string(method_at, Value::Map(map.clone()))?
                            } else {
                                // A missing key reads as null, like `map.get(key)`.
                                Value::Null
                            }
                        }
                        Value::Enum(enum_type) => {
                            let et = enum_type.borrow();
                            if let Some(def) = et.variants.get(name) {
                                let val = if def.param_count == 0 {
                                    Value::Variant(self.gc.allocate(pleno_value::VariantState {
                                        enum_type: enum_type.clone(),
                                        variant_name: name_rc.clone(),
                                        data: Vec::new(),
                                    })?)
                                } else {
                                    Value::VariantConstructor(self.gc.allocate(
                                        pleno_value::VariantConstructorState {
                                            enum_type: enum_type.clone(),
                                            variant_name: name_rc.clone(),
                                        },
                                    )?)
                                };
                                self.registers[base + a] = NanValue::from_value(&val);
                                continue;
                            } else if let Some(info) = resolve_enum_static_method(&enum_type, name)
                            {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    function.module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static method access denied")
                                    );
                                }
                                self.registers[base + a] = NanValue::from_value(
                                    &Value::StaticMethod(self.gc.allocate(StaticMethodState {
                                        function: info.function.clone(),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    })?),
                                );
                                continue;
                            } else {
                                return Err(RuntimeError::new(format!(
                                    "Enum '{}' has no variant or static method '{}'",
                                    et.name, name
                                )));
                            }
                        }
                        Value::Object(object) => {
                            let class = object.borrow().class.clone();
                            let field_value = object.borrow().fields.get(name).cloned();
                            if let Some(class) = class.as_ref() {
                                if let Some(info) = resolve_getter(class, name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        function.module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("property access denied")
                                        );
                                    }
                                    self.call_function(
                                        info.function.clone(),
                                        Some(Value::Object(object.clone())),
                                        &[],
                                        base + a,
                                        false,
                                        CallOptions {
                                            current_owner: Some(info.owner.clone()),
                                            ..Default::default()
                                        },
                                    )?;
                                    continue;
                                }
                                if let Some((owner, visibility)) = find_field_owner(class, name) {
                                    if !can_access_field(
                                        &visibility,
                                        &owner,
                                        current_class.as_ref(),
                                        function.module.as_ref(),
                                    ) {
                                        let msg = field_access_error(
                                            "read",
                                            &visibility,
                                            "field",
                                            name,
                                            &class.borrow().name,
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                    field_value.unwrap_or(Value::Null)
                                } else if let Some(v) = field_value {
                                    v
                                } else if let Some(info) = resolve_method(class, name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        function.module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    self.bytecode_method_value(
                                        method_at,
                                        Value::Object(object.clone()),
                                        &info,
                                    )?
                                } else if name == "toString" && class.borrow().extends_error() {
                                    // `Error: message`, as errors print.
                                    self.bind_to_string(method_at, Value::Object(object.clone()))?
                                } else {
                                    return Err(self.handle_property_error(name, "Object"));
                                }
                            } else {
                                field_value.unwrap_or(Value::Null)
                            }
                        }
                        Value::Class(class) => {
                            if let Some(info) = resolve_static_getter(&class, name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    function.module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static property access denied")
                                    );
                                }
                                self.call_function(
                                    info.function.clone(),
                                    None,
                                    &[],
                                    base + a,
                                    false,
                                    CallOptions {
                                        current_owner: Some(info.owner.clone()),
                                        ..Default::default()
                                    },
                                )?;
                                continue;
                            }
                            let visibility =
                                class.borrow().static_field_visibility.get(name).cloned();
                            if let Some(visibility) = visibility {
                                if !can_access_field(
                                    &visibility,
                                    &class,
                                    current_class.as_ref(),
                                    function.module.as_ref(),
                                ) {
                                    let msg = field_access_error(
                                        "read",
                                        &visibility,
                                        "static field",
                                        name,
                                        &class.borrow().name,
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                                class
                                    .borrow()
                                    .static_fields
                                    .get(name)
                                    .cloned()
                                    .unwrap_or(Value::Null)
                            } else if let Some(v) = class.borrow().static_fields.get(name).cloned()
                            {
                                v
                            } else if let Some(info) = resolve_static_method(&class, name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    function.module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static method access denied")
                                    );
                                }
                                Value::StaticMethod(self.gc.allocate(StaticMethodState {
                                    function: info.function.clone(),
                                    owner: info.owner.clone(),
                                    visibility: info.visibility,
                                })?)
                            } else {
                                return Err(self.handle_property_error(name, "Class"));
                            }
                        }
                        Value::Variant(variant) if name == "value" || name == "name" => {
                            let variant = variant.borrow();
                            if name == "name" {
                                Value::String(variant.variant_name.clone())
                            } else {
                                // The explicit value, or the variant's position.
                                let enum_type = variant.enum_type.borrow();
                                let def = enum_type.variants.get(variant.variant_name.as_str());
                                match def {
                                    Some(def) => {
                                        def.value.clone().unwrap_or(Value::Int(def.index as i64))
                                    }
                                    None => Value::Null,
                                }
                            }
                        }
                        Value::Variant(variant)
                            if resolve_enum_method(&variant.borrow().enum_type, name).is_some() =>
                        {
                            let enum_type = variant.borrow().enum_type.clone();
                            let info = resolve_enum_method(&enum_type, name)
                                .ok_or_else(|| RuntimeError::new("unknown enum method"))?;
                            if !can_access_method(
                                &info,
                                current_class.as_ref(),
                                function.module.as_ref(),
                            ) {
                                return Err(self.throw_runtime_error("method access denied"));
                            }
                            self.bytecode_method_value(
                                method_at,
                                Value::Variant(variant.clone()),
                                &info,
                            )?
                        }
                        other => {
                            let builtin = if self.builtin_extensions.is_empty() {
                                None
                            } else {
                                self.extension_method(method_at, &other, name)?
                            };
                            let native = if builtin.is_some() {
                                None
                            } else {
                                self.builtin_methods.get(
                                    self.builtin_resolver,
                                    other.type_name(),
                                    &name_rc,
                                )
                            };
                            if let Some(method) = builtin {
                                method
                            } else if name == "toString" && native.is_none() {
                                self.bind_to_string(method_at, other)?
                            } else if let Some(func) = native {
                                self.native_method_value(method_at, other, func)?
                            } else {
                                // Specialized len/length checks for common types
                                match &other {
                                    Value::Tuple(t) if name.parse::<usize>().is_ok() => {
                                        let idx = name.parse::<usize>().unwrap_or_default();
                                        let elements = &t.borrow().elements;
                                        match elements.get(idx) {
                                            Some(v) => v.clone(),
                                            None => {
                                                return Err(RuntimeError::new(format!(
                                                    "tuple index {} out of bounds (length {})",
                                                    idx,
                                                    elements.len()
                                                )));
                                            }
                                        }
                                    }
                                    Value::Tuple(t)
                                        if t.borrow()
                                            .names
                                            .as_ref()
                                            .is_some_and(|names| names.contains_key(name)) =>
                                    {
                                        let t = t.borrow();
                                        let idx = t.names.as_ref().and_then(|n| n.get(name));
                                        idx.and_then(|i| t.elements.get(*i))
                                            .cloned()
                                            .unwrap_or(Value::Null)
                                    }
                                    Value::Tuple(t) if name == "len" || name == "length" => {
                                        Value::Int(t.borrow().elements.len() as i64)
                                    }
                                    Value::Range(r) if name == "start" => {
                                        r.start.map(Value::Int).unwrap_or(Value::Null)
                                    }
                                    Value::Range(r) if name == "end" => {
                                        r.end.map(Value::Int).unwrap_or(Value::Null)
                                    }
                                    Value::Range(r) if name == "inclusive" => {
                                        Value::Bool(r.inclusive)
                                    }
                                    Value::String(s) if name == "len" || name == "length" => {
                                        Value::Int(pleno_value::text::char_len(s) as i64)
                                    }
                                    Value::Array(a) if name == "len" || name == "length" => {
                                        Value::Int(a.borrow().len() as i64)
                                    }
                                    Value::Set(s) if name == "len" => {
                                        Value::Int(s.borrow().len() as i64)
                                    }
                                    Value::Function(index) if name == "name" => {
                                        self.function_name_value(*index)
                                    }
                                    Value::Closure(closure) if name == "name" => {
                                        let index = closure.borrow().function;
                                        self.function_name_value(index)
                                    }
                                    _ => {
                                        return Err(
                                            self.handle_property_error(name, other.type_name())
                                        );
                                    }
                                }
                            }
                        }
                    };
                    self.registers[base + a] = NanValue::from_value(&result_val);
                }
                Opcode::GetSuper => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("super property must be string")),
                    };
                    let receiver_nan = self.registers[base + b];
                    let receiver = match receiver_nan.to_value() {
                        Value::Object(object) => object,
                        _ => return Err(RuntimeError::new("super used on non-object value")),
                    };
                    let superclass = {
                        let obj_borrow = receiver.borrow();
                        obj_borrow
                            .class
                            .as_ref()
                            .and_then(|class| class.borrow().superclass.clone())
                            .ok_or_else(|| RuntimeError::new("superclass missing"))?
                    };
                    let info = resolve_method(&superclass, &name)
                        .ok_or_else(|| RuntimeError::new("unknown super method"))?;
                    self.registers[base + a] = NanValue::from_value(&Value::BoundMethod(
                        self.gc.allocate(BoundMethodState {
                            receiver: Value::Object(receiver),
                            function: info.function.clone(),
                            owner: info.owner.clone(),
                            visibility: info.visibility,
                        })?,
                    ));
                }
                Opcode::CallSuper => {
                    // a = dest, b = this register, c = args register
                    let this_nan = self.registers[base + b];
                    let this_obj = match this_nan.to_value() {
                        Value::Object(obj) => obj,
                        _ => return Err(RuntimeError::new("super() called on non-object")),
                    };
                    // `super` refers to the parent of the class that defines the running
                    // constructor, not of the object's class (which may be a deeper subclass).
                    let owner_class = match &frame.current_owner {
                        Some(MethodOwner::Class(class)) => Some(class.clone()),
                        _ => this_obj.borrow().class.clone(),
                    };
                    let superclass = owner_class
                        .and_then(|class| class.borrow().superclass.clone())
                        .ok_or_else(|| RuntimeError::new("superclass missing for super()"))?;
                    let args = match self.registers[base + c].to_value() {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => return Err(RuntimeError::new("super() args must be array")),
                    };

                    // Find and call superclass constructor
                    let constructor = resolve_method(&superclass, "constructor");

                    if let Some(info) = constructor {
                        let handle_idx = match info.function.clone() {
                            Value::Function(idx) => idx,
                            _ => unreachable!("expected function"),
                        };
                        self.call_function(
                            Value::Function(handle_idx),
                            Some(Value::Object(this_obj.clone())),
                            &args,
                            base + a,
                            false,
                            CallOptions {
                                return_override: Some(Value::Object(this_obj.clone())),
                                current_owner: Some(info.owner.clone()),
                                ..Default::default()
                            },
                        )?;
                    } else {
                        // The built-in `Error` has a native constructor; other
                        // classes without a constructor have nothing to run.
                        self.call_native_constructor(
                            &superclass,
                            &Value::Object(this_obj.clone()),
                            &args,
                        )?;
                        self.registers[base + a] = NanValue::null();
                    }
                }
                Opcode::GetIndex => {
                    let collection_nan = self.registers[base + b];
                    let collection = collection_nan.to_value();
                    let index_nan = self.registers[base + c];
                    let index = index_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "[]",
                        &collection,
                        std::slice::from_ref(&index),
                    )? {
                        continue;
                    }
                    let col_type = collection.type_name();
                    let idx_type = index.type_name();
                    let result = match (&collection, &index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = *idx as usize;
                            let array_borrow = array.borrow();
                            if idx >= array_borrow.elements.len() {
                                return Err(self.throw_runtime_error("array index out of bounds"));
                            }
                            Ok(array_borrow.elements[idx].clone())
                        }
                        (Value::Array(array), Value::Range(range)) => {
                            let array_borrow = array.borrow();
                            let len = array_borrow.elements.len();
                            let start = range.start.unwrap_or(0).max(0) as usize;
                            let end = range
                                .end
                                .map(|e| if range.inclusive { e + 1 } else { e })
                                .unwrap_or(len as i64)
                                .max(0) as usize;
                            let start = start.min(len);
                            let end = end.min(len).max(start);
                            let sub_elements = array_borrow.elements[start..end].to_vec();
                            Ok(Value::Array(
                                self.gc.allocate(ArrayState::new(sub_elements))?,
                            ))
                        }
                        (Value::Tuple(tuple), Value::Range(range)) => {
                            let tuple_borrow = tuple.borrow();
                            let len = tuple_borrow.elements.len();
                            let start = (range.start.unwrap_or(0).max(0) as usize).min(len);
                            let end = range
                                .end
                                .map(|e| if range.inclusive { e + 1 } else { e })
                                .unwrap_or(len as i64)
                                .max(0) as usize;
                            let end = end.min(len).max(start);
                            Ok(Value::Tuple(self.gc.allocate(TupleState {
                                elements: tuple_borrow.elements[start..end].to_vec(),
                                names: None,
                            })?))
                        }
                        (Value::String(s), Value::Int(idx)) => {
                            let ch = usize::try_from(*idx)
                                .ok()
                                .and_then(|idx| pleno_value::text::char_at(s, idx));
                            let Some(ch) = ch else {
                                return Err(self.throw_runtime_error("string index out of bounds"));
                            };
                            Ok(Value::String(Rc::new(ch.to_string())))
                        }
                        (Value::String(s), Value::Range(range)) => {
                            // Character positions, clamped like array slices.
                            let len = pleno_value::text::char_len(s);
                            let start =
                                pleno_value::text::clamp_position(range.start.unwrap_or(0), len);
                            let end = match range.end {
                                Some(e) if range.inclusive => {
                                    pleno_value::text::clamp_position(e.saturating_add(1), len)
                                }
                                Some(e) => pleno_value::text::clamp_position(e, len),
                                None => len,
                            };
                            let sub_s = pleno_value::text::char_slice(s, start, end);
                            Ok(Value::String(Rc::new(sub_s.to_string())))
                        }
                        (Value::Tuple(tuple), Value::Int(idx)) => {
                            let idx = *idx as usize;
                            let t = tuple.borrow();
                            if idx >= t.elements.len() {
                                return Err(self.throw_runtime_error("tuple index out of bounds"));
                            }
                            Ok(t.elements[idx].clone())
                        }
                        (Value::Map(map), Value::String(key)) => Ok(map
                            .borrow()
                            .entries
                            .get(key.as_str())
                            .cloned()
                            .unwrap_or(Value::Null)),
                        // `map[0]` reads the key "0", as in JavaScript.
                        (Value::Map(map), Value::Int(key)) => Ok(map
                            .borrow()
                            .entries
                            .get(&key.to_string())
                            .cloned()
                            .unwrap_or(Value::Null)),
                        // A Float reads the key it prints as, so `map[5.0]` is `map[5]`.
                        (Value::Map(map), Value::Float(key)) => Ok(map
                            .borrow()
                            .entries
                            .get(&Value::Float(*key).to_string())
                            .cloned()
                            .unwrap_or(Value::Null)),
                        (Value::Set(set), value) => {
                            let key = ValueKey::try_from_value(value)
                                .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                            Ok(Value::Bool(set.borrow().contains(&key)))
                        }
                        _ => {
                            return Err(RuntimeError::new(format!(
                                "invalid index operation on {} with index {}",
                                col_type, idx_type
                            )));
                        }
                    }?;
                    self.registers[base + a] = NanValue::from_value(&result);
                }
                Opcode::SetIndex => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + a].to_value();
                    let collection = self.registers[base + b].to_value();
                    let index = self.registers[base + c].to_value();

                    if self.try_dispatch_operator(
                        base + a,
                        "[]=",
                        &collection,
                        &[index.clone(), value.clone()],
                    )? {
                        continue;
                    }

                    match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            if array.borrow().is_immutable {
                                return Err(
                                    self.throw_runtime_error("cannot modify immutable array")
                                );
                            }
                            let idx = idx as usize;
                            let mut array = array.borrow_mut();
                            if idx >= array.elements.len() {
                                return Err(self.throw_runtime_error("array index out of bounds"));
                            }
                            array.elements[idx] = value;
                        }
                        (Value::Map(map), Value::String(key)) => {
                            if map.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable map"));
                            }
                            self.gc
                                .track_allocation(std::mem::size_of::<Value>() + key.len())?;
                            map.borrow_mut()
                                .entries
                                .insert(key.as_str().to_string(), value);
                        }
                        _ => return Err(RuntimeError::new("invalid index assignment")),
                    }
                }
                Opcode::Add => {
                    let (left, right) = (self.registers[base + b], self.registers[base + c]);
                    // `s = s + x`: the old string goes away, so append to it.
                    if a == b && left.is_string() && self.append_in_place(base + a, left, right)? {
                        continue;
                    }
                    self.add_registers(base + a, left, right)?;
                }
                Opcode::Sub => {
                    let (left, right) = (self.registers[base + b], self.registers[base + c]);
                    self.sub_registers(base + a, left, right)?;
                }
                Opcode::Mul => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        self.registers[base + a] = crate::value_ops::mul_nan(left_nan, right_nan)?;
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "*",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let res = crate::value_ops::mul_values(left, right)?;
                    self.registers[base + a] = NanValue::from_value(&res);
                }
                Opcode::Div => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        self.registers[base + a] = crate::value_ops::div_nan(left_nan, right_nan)?;
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "/",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let res = crate::value_ops::div_values(left, right)?;
                    self.registers[base + a] = NanValue::from_value(&res);
                }
                Opcode::Mod => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        self.registers[base + a] = crate::value_ops::mod_nan(left_nan, right_nan)?;
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "%",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let res = crate::value_ops::mod_values(left, right)?;
                    self.registers[base + a] = NanValue::from_value(&res);
                }
                Opcode::Neg => {
                    let value_nan = self.registers[base + b];
                    if value_nan.is_f64() || value_nan.as_int().is_some() {
                        self.registers[base + a] = crate::value_ops::neg_nan(value_nan)?;
                        continue;
                    }
                    let value = value_nan.to_value();
                    if self.try_dispatch_operator(base + a, "-", &value, &[])? {
                        continue;
                    }
                    let res = crate::value_ops::neg_value(value)?;
                    self.registers[base + a] = NanValue::from_value(&res);
                }
                Opcode::Not => {
                    let value_nan = self.registers[base + b];
                    self.registers[base + a] =
                        NanValue::from_bool(!crate::value_ops::is_truthy_nan(value_nan));
                }
                Opcode::And => {
                    let left = self.registers[base + b];
                    let right = self.registers[base + c];
                    self.registers[base + a] = NanValue::from_bool(
                        crate::value_ops::is_truthy_nan(left)
                            && crate::value_ops::is_truthy_nan(right),
                    );
                }
                Opcode::Or => {
                    let left = self.registers[base + b];
                    let right = self.registers[base + c];
                    self.registers[base + a] = NanValue::from_bool(
                        crate::value_ops::is_truthy_nan(left)
                            || crate::value_ops::is_truthy_nan(right),
                    );
                }
                Opcode::Eq => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.tag() < 7)
                        && (right_nan.is_f64() || right_nan.tag() < 7)
                    {
                        let res = numbers_or_bits_equal(left_nan, right_nan);
                        self.registers[base + a] = NanValue::from_bool(res);
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "==",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    self.registers[base + a] =
                        NanValue::from_bool(pleno_value::values_equal(&left, &right));
                }
                Opcode::Ne => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.tag() < 7)
                        && (right_nan.is_f64() || right_nan.tag() < 7)
                    {
                        let res = !numbers_or_bits_equal(left_nan, right_nan);
                        self.registers[base + a] = NanValue::from_bool(res);
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "!=",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    // Without its own `!=`, a class's `==` decides: `a != b` is
                    // `!(a == b)`.
                    if let Some(equal) = self.call_equality_operator(&left, &right)? {
                        self.registers[base + a] = NanValue::from_bool(!equal);
                        continue;
                    }
                    self.registers[base + a] =
                        NanValue::from_bool(!pleno_value::values_equal(&left, &right));
                }
                Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    let numeric = match (left_nan.as_inline_int(), right_nan.as_inline_int()) {
                        (Some(left), Some(right)) => Some(match opcode {
                            Opcode::Lt => left < right,
                            Opcode::Le => left <= right,
                            Opcode::Gt => left > right,
                            _ => left >= right,
                        }),
                        _ if (left_nan.is_f64() || left_nan.as_int().is_some())
                            && (right_nan.is_f64() || right_nan.as_int().is_some()) =>
                        {
                            Some(crate::value_ops::compare_nan(opcode, left_nan, right_nan)?)
                        }
                        _ => None,
                    };
                    if let Some(res) = numeric {
                        self.registers[base + a] = NanValue::from_bool(res);
                        // A conditional jump on the result runs here too.
                        fused_jump!(frame, ip, a, res);
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    let op_str = match opcode {
                        Opcode::Lt => "<",
                        Opcode::Le => "<=",
                        Opcode::Gt => ">",
                        Opcode::Ge => ">=",
                        _ => unreachable!(),
                    };
                    if self.try_dispatch_operator(
                        base + a,
                        op_str,
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let result = crate::value_ops::compare_values(opcode, left, right)?;
                    self.registers[base + a] = NanValue::from_bool(result);
                }
                Opcode::Jump => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    frame.ip = apply_offset(frame.ip, offset)?;
                }
                Opcode::JumpIf => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    if crate::value_ops::is_truthy_nan(self.registers[base + a]) {
                        frame.ip = apply_offset(frame.ip, offset)?;
                    }
                }
                Opcode::JumpIfNot => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    if !crate::value_ops::is_truthy_nan(self.registers[base + a]) {
                        frame.ip = apply_offset(frame.ip, offset)?;
                    }
                }
                Opcode::Return => {
                    // A plain return to a caller in this run: copy the register
                    // without converting it to a `Value` and back, and drop the
                    // frame where it is.
                    let plain = !function.is_async && frame.return_override.is_none();
                    let (frame_base, return_reg) = (frame.base, frame.return_reg);
                    if plain && self.frames.len() > depth + 1 {
                        let result = self.registers[base + a];
                        self.frames.truncate(self.frames.len() - 1);
                        self.release_registers(frame_base);
                        self.registers[return_reg] = result;
                        continue;
                    }
                    // A plain return from the frame this run was started for (a
                    // native's callback, say): the same as below, without the
                    // async bookkeeping.
                    if plain && self.frames.len() == depth + 1 {
                        let result = self.registers[base + a].to_value();
                        self.frames.truncate(depth);
                        if self.suspended_frames.is_empty() && self.pending_tasks.is_empty() {
                            self.release_registers(frame_base);
                        }
                        return Ok(Some(ExecutionStatus::Finished(result)));
                    }

                    let frame = self
                        .frames
                        .pop()
                        .ok_or_else(|| RuntimeError::new("no frame"))?;

                    // We need to look up the function to check if it is async
                    let function_is_async = self.functions[frame.function_index].is_async;
                    let function_arity = self.functions[frame.function_index].arity;

                    let result = frame
                        .return_override
                        .clone()
                        .unwrap_or_else(|| self.registers[base + a].to_value());

                    if function_is_async {
                        // Promise ID is the last argument
                        if function_arity > 0 {
                            let promise_id_idx = frame.base + function_arity - 1;
                            if let Some(val_nan) = self.registers.get(promise_id_idx)
                                && let Value::Int(id) = val_nan.to_value()
                                && let Ok(id) = id.try_into()
                            {
                                self.pending_tasks.push(AsyncTask {
                                    promise_id: id,
                                    value: result.clone(),
                                    callee: None,
                                    args: None,
                                    closure_state: None,
                                    is_async: true,
                                    is_resolved: true,
                                    is_rejected: false,
                                });
                            }
                        }
                    }

                    if self.frames.len() > depth
                        || (self.suspended_frames.is_empty() && self.pending_tasks.is_empty())
                    {
                        self.release_registers(frame.base);
                    }

                    if self.frames.len() == depth {
                        return Ok(Some(ExecutionStatus::Finished(result)));
                    }

                    if !function_is_async {
                        self.registers[frame.return_reg] = NanValue::from_value(&result);
                    }
                }
                Opcode::GetGlobal => {
                    if b >= function.chunk.constants.len() {
                        return Err(RuntimeError::new(format!(
                            "global constant index {} out of bounds (len: {})",
                            b,
                            function.chunk.constants.len()
                        )));
                    }

                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value,
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    // The cache indexes the shared globals, so it serves only code
                    // outside modules: a module's own top-level names come first.
                    if function.module_path.is_none() {
                        let func_index = frame.function_index;
                        let cached = self
                            .global_slots
                            .get(func_index)
                            .and_then(|slots| slots.get(b))
                            .copied()
                            .unwrap_or(0) as usize;
                        if let Some((key, value)) = self.globals.get_index(cached.wrapping_sub(1))
                            && key.len() == name.len()
                            && key.bytes().zip(name.bytes()).all(|(x, y)| x == y)
                        {
                            let value = NanValue::from_value(value);
                            self.registers[base + a] = value;
                            // Global reads are followed by a `ForceThunk`, which only
                            // copies a value that is not a thunk.
                            if let Some(force) = next_instruction!(ip)
                                && force.opcode() == Opcode::ForceThunk
                                && force.b() as usize == a
                                && !value.is_thunk()
                            {
                                self.instruction_count += 1;
                                frame.ip = ip + 2;
                                self.registers[base + force.a() as usize] = value;
                            }
                            continue;
                        }
                        if let Some((index, _, value)) = self.globals.get_full(name.as_str()) {
                            self.registers[base + a] = NanValue::from_value(value);
                            if self.global_slots.len() <= func_index {
                                self.global_slots.resize(func_index + 1, Vec::new());
                            }
                            let slots = &mut self.global_slots[func_index];
                            if slots.len() <= b {
                                slots.resize(function.chunk.constants.len().max(b + 1), 0);
                            }
                            slots[b] = index as u32 + 1;
                            continue;
                        }
                    }
                    let name = Rc::clone(name);
                    let mut value = lookup_global_in(
                        &self.globals,
                        &self.module_globals,
                        &self.program_globals,
                        function.module_path.as_deref(),
                        &name,
                    );

                    // An unspecialized type parameter (e.g. `T` in `Box(3)`) accepts any type.
                    let mut is_type_param = false;
                    if value.is_none() {
                        // Check function type parameters
                        for (i, param) in function.type_params.iter().enumerate() {
                            if param.name == *name {
                                is_type_param = true;
                                if i < frame.type_args.len() {
                                    value = Some(frame.type_args[i].clone());
                                }
                                break;
                            }
                        }

                        // Check class type parameters
                        if value.is_none()
                            && let Some(MethodOwner::Class(cls)) = &frame.current_owner
                        {
                            let cls = cls.borrow();
                            for (i, param) in cls.type_params.iter().enumerate() {
                                if param.name == *name {
                                    is_type_param = true;
                                    // First try to get from receiver (this)
                                    if let Value::Object(obj) =
                                        self.registers[frame.base].to_value()
                                        && let Some(type_args) = &obj.borrow().type_args
                                        && i < type_args.len()
                                    {
                                        value = Some(type_args[i].clone());
                                    }

                                    // Fallback to frame type args if receiver didn't work
                                    // (e.g. constructor call where frame args are class args)
                                    if value.is_none()
                                        && function.type_params.is_empty()
                                        && i < frame.type_args.len()
                                    {
                                        value = Some(frame.type_args[i].clone());
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    // `c = 1` marks a name read by the program, which must exist;
                    // the compiler's own type lookups use `c = 0` and get null.
                    // An inferred type parameter with constraints (`T: Named`,
                    // `T: A & B`) stands for them, so `greet(Rock())` is checked too.
                    if value.is_none()
                        && is_type_param
                        && let Some(constraints) = function.type_constraints.get(name.as_str())
                    {
                        value = match constraints.as_slice() {
                            [] => None,
                            [constraint] => self.lookup_type_name(constraint),
                            several => Some(Value::String(Rc::new(several.join(" & ")))),
                        };
                    }
                    let value = match value {
                        Some(value) => value,
                        None if is_type_param => Value::String(Rc::new("Any".to_string())),
                        None if c == 1 => {
                            return Err(
                                self.throw_runtime_error(&format!("undefined variable '{name}'"))
                            );
                        }
                        None => Value::Null,
                    };
                    log::trace!("VM: GetGlobal '{}' => {:?}", name, value);
                    self.registers[base + a] = NanValue::from_value(&value);
                }
                Opcode::SetGlobal => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    let value = self.registers[base + a];
                    log::trace!("VM: SetGlobal '{}' => {:?}", name, value);
                    let val_obj = value.to_value();

                    // A module's top-level names stay in the module, and the main
                    // program's stay out of the modules it imports.
                    match function
                        .module_path
                        .as_deref()
                        .and_then(|path| self.module_globals.get_mut(path.as_str()))
                    {
                        Some(table) => {
                            table.insert(name.clone(), val_obj);
                        }
                        None => {
                            self.program_globals.define(&self.globals, &name);
                            self.globals.insert(name.clone(), val_obj);
                        }
                    }
                    self.globals_epoch += 1;

                    // Data Breakpoint Check
                    if self.debug_context.data_breakpoints.contains_key(&name) {
                        self.debug_context.state = DebugState::Paused(PauseReason::Breakpoint);
                        return Ok(Some(ExecutionStatus::Paused));
                    }
                }
                Opcode::DefineClass => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("class name must be string")),
                    };
                    let module = function.module.as_ref().map(Rc::clone);
                    let methods_map = match &self.registers[base + c].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("class methods must be map")),
                    };

                    // Create the class first
                    let class = self.gc.allocate(Class {
                        name: Rc::new(name),
                        methods: IndexMap::new(),
                        static_methods: IndexMap::new(),
                        operators: IndexMap::new(),
                        constructor: None,
                        native_constructor: None,
                        superclass: None,
                        static_fields: IndexMap::new(),
                        static_field_types: IndexMap::new(),
                        fields: IndexMap::new(),
                        field_types: IndexMap::new(),
                        static_field_visibility: IndexMap::new(),
                        getters: IndexMap::new(),
                        setters: IndexMap::new(),
                        static_getters: IndexMap::new(),
                        static_setters: IndexMap::new(),
                        module: module.clone(),
                        is_abstract: false,
                        abstract_methods: Vec::new(),
                        const_fields: Vec::new(),
                        const_static_fields: Vec::new(),
                        is_pooled: false,
                        implements: Vec::new(),
                        type_params: Vec::new(),
                        type_constraints: IndexMap::new(),
                        annotations: Vec::new(),
                    })?;

                    // Now parse methods with class reference
                    let mut methods = IndexMap::new();
                    let mut constructor = None;
                    for (method_name, value) in methods_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class.clone()),
                            module.as_ref(),
                        )?;
                        if method_name == "constructor" {
                            constructor = Some(info.clone());
                            methods.insert(method_name, info);
                        } else {
                            methods.insert(method_name, info);
                        }
                    }

                    // Update the class with parsed methods
                    class.borrow_mut().methods = methods;
                    class.borrow_mut().constructor = constructor;

                    self.registers[base + a] = NanValue::from_value(&Value::Class(class));
                }
                Opcode::DefineInterface => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(s) => s.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "DefineInterface expects string constant",
                            ));
                        }
                    };
                    let methods_val = self.registers[base + c].to_value();
                    let (methods, getters, setters) = match methods_val {
                        Value::Map(map) => {
                            let mut methods = IndexMap::new();
                            let mut getters = IndexMap::new();
                            let mut setters = IndexMap::new();
                            for (_name, val) in map.borrow().iter() {
                                // Expected structure: [name, type, arity, is_static, is_async]
                                if let Value::Array(meta) = val {
                                    let meta = meta.borrow();
                                    if meta.len() >= 5 {
                                        let name = meta[0].as_str().unwrap_or("").to_string();
                                        let type_str = meta[1].as_str().unwrap_or("");
                                        let arity = meta[2].as_int().unwrap_or(0) as usize;
                                        let is_static = meta[3].as_bool().unwrap_or(false);
                                        let is_async = meta[4].as_bool().unwrap_or(false);

                                        let sig = pleno_value::MethodSignature {
                                            name: name.clone(),
                                            arity,
                                            is_static,
                                            is_async,
                                        };

                                        match type_str {
                                            "method" => {
                                                methods.insert(name, sig);
                                            }
                                            "getter" => {
                                                getters.insert(name, sig);
                                            }
                                            "setter" => {
                                                setters.insert(name, sig);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            (methods, getters, setters)
                        }
                        _ => return Err(RuntimeError::new("DefineInterface expects methods map")),
                    };

                    let module = function.module.as_ref().map(Rc::clone);
                    self.registers[base + a] = NanValue::from_value(&Value::Interface(
                        self.gc.allocate(pleno_value::Interface {
                            name,
                            methods,
                            getters,
                            setters,
                            type_params: Vec::new(),
                            type_constraints: IndexMap::new(),
                            annotations: Vec::new(),
                            module,
                        })?,
                    ));
                }
                Opcode::SetImplements => {
                    let interfaces_val = self.registers[base + b].to_value();
                    let interfaces = match interfaces_val {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetImplements expects array of interfaces",
                            ));
                        }
                    };

                    let class_val = self.registers[base + a].to_value();
                    match class_val {
                        Value::Class(class) => {
                            let mut validation_error = None;
                            // Validation Logic
                            'interface_loop: for interface_val in interfaces {
                                let interface = match &interface_val {
                                    Value::Interface(i) => i.clone(),
                                    Value::BoundInterface(s) => s.borrow().interface.clone(),
                                    _ => {
                                        return Err(RuntimeError::new(
                                            "SetImplements: expected interface",
                                        ));
                                    }
                                };

                                {
                                    let interface_borrow = interface.borrow();
                                    let class_borrow = class.borrow();

                                    // Check methods
                                    for (name, sig) in &interface_borrow.methods {
                                        let method_info = if sig.is_static {
                                            resolve_static_method(&class, name)
                                        } else {
                                            resolve_method(&class, name)
                                        };

                                        if let Some(method_info) = method_info {
                                            let handle_idx = match method_info.function {
                                                Value::Function(idx) => idx,
                                                Value::Closure(ref c) => c.borrow().function,
                                                _ => unreachable!("expected function"),
                                            };
                                            if let Some(func) = self.functions.get(handle_idx) {
                                                let mut expected_arity = sig.arity;
                                                if !sig.is_static {
                                                    expected_arity += 1;
                                                }
                                                if sig.is_async {
                                                    expected_arity += 1;
                                                }

                                                if func.arity != expected_arity {
                                                    validation_error = Some(format!(
                                                        "Class '{}' implements interface '{}' incorrectly: method '{}' requires {} args (including implicit ones), found {}",
                                                        class_borrow.name,
                                                        interface_borrow.name,
                                                        name,
                                                        expected_arity,
                                                        func.arity
                                                    ));
                                                    break 'interface_loop;
                                                }

                                                if func.is_async != sig.is_async {
                                                    validation_error = Some(format!(
                                                        "Class '{}' implements interface '{}' incorrectly: method '{}' async status mismatch",
                                                        class_borrow.name,
                                                        interface_borrow.name,
                                                        name
                                                    ));
                                                    break 'interface_loop;
                                                }
                                            }
                                        } else {
                                            validation_error = Some(format!(
                                                "Class '{}' missing {}method '{}' required by interface '{}'",
                                                class_borrow.name,
                                                if sig.is_static { "static " } else { "" },
                                                name,
                                                interface_borrow.name
                                            ));
                                            break 'interface_loop;
                                        }
                                    }

                                    // Check getters
                                    for (name, sig) in &interface_borrow.getters {
                                        let getter_info = if sig.is_static {
                                            resolve_static_getter(&class, name)
                                        } else {
                                            resolve_getter(&class, name)
                                        };
                                        if getter_info.is_none() {
                                            validation_error = Some(format!(
                                                "Class '{}' missing {}getter '{}' required by interface '{}'",
                                                class_borrow.name,
                                                if sig.is_static { "static " } else { "" },
                                                name,
                                                interface_borrow.name
                                            ));
                                            break 'interface_loop;
                                        }
                                    }

                                    // Check setters
                                    for (name, sig) in &interface_borrow.setters {
                                        let setter_info = if sig.is_static {
                                            resolve_static_setter(&class, name)
                                        } else {
                                            resolve_setter(&class, name)
                                        };
                                        if setter_info.is_none() {
                                            validation_error = Some(format!(
                                                "Class '{}' missing {}setter '{}' required by interface '{}'",
                                                class_borrow.name,
                                                if sig.is_static { "static " } else { "" },
                                                name,
                                                interface_borrow.name
                                            ));
                                            break 'interface_loop;
                                        }
                                    }
                                }

                                class.borrow_mut().implements.push(interface_val.clone());
                            }

                            if let Some(msg) = validation_error {
                                return Err(self.throw_runtime_error(&msg));
                            }
                        }
                        _ => return Err(RuntimeError::new("SetImplements on non-class value")),
                    }
                }
                Opcode::SetFieldType => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetFieldType expects string constant for field name",
                            ));
                        }
                    };
                    let class_val = self.registers[base + a].to_value();
                    match class_val {
                        Value::Class(class) => {
                            // Operand c is a constant index holding the type name.
                            let type_val = function.chunk.constants[c].clone();
                            let type_name = match type_val {
                                Value::String(s) => Some(s.as_str().to_string()),
                                _ => None,
                            };
                            class.borrow_mut().field_types.insert(name, type_name);
                        }
                        _ => return Err(RuntimeError::new("SetFieldType expects class")),
                    }
                }
                Opcode::SetStaticFieldType => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetStaticFieldType expects string constant for field name",
                            ));
                        }
                    };
                    let class_val = self.registers[base + a].to_value();
                    match class_val {
                        Value::Class(class) => {
                            // Operand c is a constant index holding the type name.
                            let type_val = function.chunk.constants[c].clone();
                            let type_name = match type_val {
                                Value::String(s) => Some(s.as_str().to_string()),
                                _ => None,
                            };
                            class
                                .borrow_mut()
                                .static_field_types
                                .insert(name, type_name);
                        }
                        _ => return Err(RuntimeError::new("SetStaticFieldType expects class")),
                    }
                }
                Opcode::SetTypeParams => {
                    let class_val = self.registers[base + a].to_value();
                    let params_val = match &self.registers[base + b].to_value() {
                        Value::Array(arr) => arr
                            .borrow()
                            .elements
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => pleno_value::GenericParamMetadata {
                                    name: s.as_str().to_string(),
                                    variance: pleno_common::Variance::Invariant,
                                },
                                Value::Map(m) => {
                                    let b = m.borrow();
                                    let name = b
                                        .entries
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let variance = b
                                        .entries
                                        .get("variance")
                                        .and_then(|v| v.as_int())
                                        .map(|i| pleno_common::Variance::from_u8(i as u8))
                                        .unwrap_or(pleno_common::Variance::Invariant);
                                    pleno_value::GenericParamMetadata { name, variance }
                                }
                                _ => pleno_value::GenericParamMetadata {
                                    name: "unknown".to_string(),
                                    variance: pleno_common::Variance::Invariant,
                                },
                            })
                            .collect::<Vec<_>>(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetTypeParams expects array of strings",
                            ));
                        }
                    };
                    let constraints_map = match &self.registers[base + c].to_value() {
                        Value::Map(map) => {
                            let mut constraints = IndexMap::new();
                            for (k, v) in &map.borrow().entries {
                                let constraint = match v {
                                    Value::Array(arr) => arr
                                        .borrow()
                                        .elements
                                        .iter()
                                        .filter_map(|ev| match ev {
                                            Value::String(s) => Some(s.as_str().to_string()),
                                            _ => None,
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };
                                constraints.insert(k.clone(), constraint);
                            }
                            constraints
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "SetTypeParams expects map of constraints",
                            ));
                        }
                    };

                    match class_val {
                        Value::Class(class) => {
                            class.borrow_mut().type_params = params_val;
                            class.borrow_mut().type_constraints = constraints_map;
                        }
                        Value::Interface(interface) => {
                            interface.borrow_mut().type_params = params_val;
                            interface.borrow_mut().type_constraints = constraints_map;
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "SetTypeParams expects class or interface",
                            ));
                        }
                    }
                }
                Opcode::SetOperators => {
                    let class_val = self.registers[base + a].to_value();
                    let ops_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => {
                            return Err(RuntimeError::new("SetOperators expects map of operators"));
                        }
                    };

                    if let Value::Class(class) = class_val {
                        let module = class.borrow().module.clone();
                        let mut operators = IndexMap::new();
                        for (op_name, value) in ops_map {
                            let info = parse_method_info(
                                value,
                                MethodOwner::Class(class.clone()),
                                module.as_ref(),
                            )?;
                            operators.insert(op_name, info);
                        }
                        class.borrow_mut().operators = operators;
                    } else {
                        return Err(RuntimeError::new("SetOperators expects class"));
                    }
                }
                Opcode::Specialize => {
                    let class_val = self.registers[base + b].to_value();
                    let args = match &self.registers[base + c].to_value() {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "Specialize expects array of type arguments",
                            ));
                        }
                    };

                    match class_val {
                        Value::Class(class) => {
                            // Verify constraints
                            {
                                let class_borrow = class.borrow();
                                if args.len() != class_borrow.type_params.len() {
                                    return Err(self.throw_runtime_error(&format!(
                                        "Generic type mismatch: expected {} args, found {}",
                                        class_borrow.type_params.len(),
                                        args.len()
                                    )));
                                }

                                for (i, param) in class_borrow.type_params.iter().enumerate() {
                                    let param_name = &param.name;
                                    if let Some(constraints) =
                                        class_borrow.type_constraints.get(param_name)
                                    {
                                        for constraint_name in constraints {
                                            // Resolve constraint type (Class, Interface, or Primitive Name)
                                            let constraint_type = if let Some(val) =
                                                self.lookup_type_name(constraint_name)
                                            {
                                                val.clone()
                                            } else {
                                                // Assume primitive or simple type name if not in globals
                                                Value::String(Rc::new(constraint_name.clone()))
                                            };

                                            if !self
                                                .check_type_compatible(&args[i], &constraint_type)
                                            {
                                                let arg_type_name = match &args[i] {
                                                    Value::Class(c) => c.borrow().name.to_string(),
                                                    Value::Interface(i) => {
                                                        i.borrow().name.to_string()
                                                    }
                                                    Value::String(s) => s.to_string(),
                                                    _ => "Unknown".to_string(),
                                                };
                                                return Err(self.throw_runtime_error(&format!(
                                                    "Generic constraint failed: Type '{}' does not satisfy constraint '{}' for parameter '{}'",
                                                    arg_type_name,
                                                    constraint_name,
                                                    param_name
                                                )));
                                            }
                                        }
                                    }
                                }
                            }

                            self.registers[base + a] = NanValue::from_value(&Value::BoundClass(
                                self.gc.allocate(pleno_value::BoundClassState {
                                    class,
                                    type_args: args,
                                })?,
                            ));
                        }
                        Value::Interface(interface) => {
                            // Verify constraints
                            {
                                let interface_borrow = interface.borrow();
                                if args.len() != interface_borrow.type_params.len() {
                                    return Err(self.throw_runtime_error(&format!(
                                        "Generic type mismatch: expected {} args, found {}",
                                        interface_borrow.type_params.len(),
                                        args.len()
                                    )));
                                }

                                for (i, param) in interface_borrow.type_params.iter().enumerate() {
                                    let param_name = &param.name;
                                    if let Some(constraints) =
                                        interface_borrow.type_constraints.get(param_name)
                                    {
                                        for constraint_name in constraints {
                                            // Resolve constraint type (Class, Interface, or Primitive Name)
                                            let constraint_type = if let Some(val) =
                                                self.lookup_type_name(constraint_name)
                                            {
                                                val.clone()
                                            } else {
                                                // Assume primitive or simple type name if not in globals
                                                Value::String(Rc::new(constraint_name.clone()))
                                            };

                                            if !self
                                                .check_type_compatible(&args[i], &constraint_type)
                                            {
                                                let arg_type_name = match &args[i] {
                                                    Value::Class(c) => c.borrow().name.to_string(),
                                                    Value::Interface(i) => {
                                                        i.borrow().name.to_string()
                                                    }
                                                    Value::String(s) => s.to_string(),
                                                    _ => "Unknown".to_string(),
                                                };
                                                return Err(self.throw_runtime_error(&format!(
                                                    "Generic constraint failed: Type '{}' does not satisfy constraint '{}' for parameter '{}'",
                                                    arg_type_name,
                                                    constraint_name,
                                                    param.name
                                                )));
                                            }
                                        }
                                    }
                                }
                            }

                            self.registers[base + a] =
                                NanValue::from_value(&Value::BoundInterface(self.gc.allocate(
                                    pleno_value::BoundInterfaceState {
                                        interface,
                                        type_args: args,
                                    },
                                )?));
                        }
                        Value::BoundClass(inner) => {
                            self.registers[base + a] = NanValue::from_value(&Value::BoundClass(
                                self.gc.allocate(pleno_value::BoundClassState {
                                    class: inner.borrow().class.clone(),
                                    type_args: args,
                                })?,
                            ));
                        }
                        Value::Function(handle) => {
                            let handle_idx = handle;
                            let func_proto =
                                self.functions.get(handle_idx).cloned().ok_or_else(|| {
                                    RuntimeError::new("unknown function in Specialize")
                                })?;
                            if func_proto.type_params.is_empty() {
                                return Err(RuntimeError::new("function is not generic"));
                            }

                            // Verify constraints
                            for (i, param) in func_proto.type_params.iter().enumerate() {
                                if let Some(constraints) =
                                    func_proto.type_constraints.get(&param.name)
                                {
                                    for constraint_name in constraints {
                                        let constraint_type = if let Some(val) =
                                            self.lookup_type_name(constraint_name)
                                        {
                                            val.clone()
                                        } else {
                                            Value::String(Rc::new(constraint_name.clone()))
                                        };

                                        if !self.check_type_compatible(&args[i], &constraint_type) {
                                            let arg_type_name =
                                                self.get_value_type_name_for_specialize(&args[i]);
                                            return Err(self.throw_runtime_error(&format!(
                                                "Generic constraint failed: Type '{}' does not satisfy constraint '{}' for parameter '{}' in function '{}'",
                                                arg_type_name,
                                                constraint_name,
                                                param.name,
                                                func_proto.name
                                            )));
                                        }
                                    }
                                }
                            }

                            self.registers[base + a] = NanValue::from_value(&Value::BoundFunction(
                                self.gc.allocate(pleno_value::BoundFunctionState {
                                    function: Value::Function(handle),
                                    type_args: args,
                                })?,
                            ));
                        }

                        // `Shape<Int>`, `Promise<Int>(...)`: type arguments of enums and
                        // built-in callables are not checked, so they mean the value itself.
                        unchecked @ (Value::Enum(_) | Value::Map(_) | Value::NativeFunction(_)) => {
                            self.registers[base + a] = NanValue::from_value(&unchecked);
                        }
                        _ => return Err(RuntimeError::new("Specialize on non-class value")),
                    }
                }
                Opcode::Freeze => {
                    let recursive = b == 1;
                    let value = self.registers[base + a].to_value();
                    self.freeze_value(&value, recursive);
                }
                Opcode::SetSuper => {
                    let superclass = match &self.registers[base + b].to_value() {
                        Value::Class(class) => class.clone(),
                        _ => return Err(RuntimeError::new("superclass must be class")),
                    };
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().superclass = Some(superclass);
                    } else {
                        return Err(RuntimeError::new("SetSuper on non-class value"));
                    }
                }
                Opcode::SetAbstract => {
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        // `c = 1`: constant `b` names one of the class's abstract methods.
                        if c == 1 {
                            let method = match &function.chunk.constants[b] {
                                Value::String(value) => value.as_str().to_string(),
                                _ => return Err(RuntimeError::new("method name must be string")),
                            };
                            class.borrow_mut().abstract_methods.push(method);
                        } else {
                            class.borrow_mut().is_abstract = true;
                        }
                    } else {
                        return Err(RuntimeError::new("SetAbstract on non-class value"));
                    }
                }
                Opcode::SetStatic => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let static_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static methods must be map")),
                    };
                    let mut static_methods = IndexMap::new();
                    let (class_ref, module) = match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set static on non-class value")),
                    };
                    for (method_name, value) in static_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class_ref.clone()),
                            module.as_ref(),
                        )?;
                        static_methods.insert(method_name, info);
                    }
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().static_methods = static_methods;
                    } else {
                        return Err(RuntimeError::new("set static on non-class value"));
                    }
                }
                Opcode::SetGetters => {
                    let getters_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("getters must be map")),
                    };
                    let mut getters = IndexMap::new();
                    let (class_ref, module) = match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set getters on non-class value")),
                    };
                    for (prop_name, value) in getters_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class_ref.clone()),
                            module.as_ref(),
                        )?;
                        getters.insert(prop_name, info);
                    }
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().getters = getters;
                    } else {
                        return Err(RuntimeError::new("set getters on non-class value"));
                    }
                }
                Opcode::SetSetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let setters_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("setters must be map")),
                    };
                    let mut setters = IndexMap::new();
                    let (class_ref, module) = match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set setters on non-class value")),
                    };
                    for (prop_name, value) in setters_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class_ref.clone()),
                            module.as_ref(),
                        )?;
                        setters.insert(prop_name, info);
                    }
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().setters = setters;
                    } else {
                        return Err(RuntimeError::new("set setters on non-class value"));
                    }
                }
                Opcode::SetStaticGetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let getters_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static getters must be map")),
                    };
                    let mut static_getters = IndexMap::new();
                    let (class_ref, module) = match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => {
                            return Err(RuntimeError::new("set static getters on non-class value"));
                        }
                    };
                    for (prop_name, value) in getters_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class_ref.clone()),
                            module.as_ref(),
                        )?;
                        static_getters.insert(prop_name, info);
                    }
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().static_getters = static_getters;
                    } else {
                        return Err(RuntimeError::new("set static getters on non-class value"));
                    }
                }
                Opcode::SetStaticSetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let setters_map = match &self.registers[base + b].to_value() {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static setters must be map")),
                    };
                    let mut static_setters = IndexMap::new();
                    let (class_ref, module) = match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => {
                            return Err(RuntimeError::new("set static setters on non-class value"));
                        }
                    };
                    for (prop_name, value) in setters_map {
                        let info = parse_method_info(
                            value,
                            MethodOwner::Class(class_ref.clone()),
                            module.as_ref(),
                        )?;
                        static_setters.insert(prop_name, info);
                    }
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().static_setters = static_setters;
                    } else {
                        return Err(RuntimeError::new("set static setters on non-class value"));
                    }
                }
                Opcode::DefineEnum => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("enum name must be string")),
                    };
                    let module = function.module.as_ref().map(Rc::clone);
                    let enum_type = self.gc.allocate(EnumType {
                        name: Rc::new(name),
                        variants: IndexMap::new(),
                        methods: IndexMap::new(),
                        static_methods: IndexMap::new(),
                        module,
                        annotations: Vec::new(),
                    })?;
                    self.registers[base + a] = NanValue::from_value(&Value::Enum(enum_type));
                }
                Opcode::AddVariant => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let enum_val = self.registers[base + a].to_value();
                    let variant_name = match &function.chunk.constants[b] {
                        Value::String(s) => s.clone(),
                        _ => return Err(RuntimeError::new("AddVariant expects string constant")),
                    };
                    let param_count = instruction.c() as usize;

                    match enum_val {
                        Value::Enum(enum_type) => {
                            let mut et = enum_type.borrow_mut();
                            let index = et.variants.len();
                            et.variants.insert(
                                variant_name.to_string(),
                                EnumVariantDef {
                                    index,
                                    param_count,
                                    annotations: Vec::new(),
                                    value: None,
                                },
                            );
                        }
                        _ => return Err(RuntimeError::new("AddVariant on non-enum value")),
                    }
                }
                Opcode::Destructure => {
                    check_destructure(&self.registers[base + a].to_value(), b as u8, c)?;
                }
                Opcode::ToFloat => {
                    let value = self.registers[base + b];
                    self.registers[base + a] = match value.as_int() {
                        Some(int) => NanValue::from_f64(int as f64),
                        None => value,
                    };
                }
                Opcode::SetVariantValue => {
                    let variant_name = match &function.chunk.constants[b] {
                        Value::String(s) => s.to_string(),
                        _ => return Err(RuntimeError::new("variant name must be string")),
                    };
                    let value = function.chunk.constants[c].clone();
                    match self.registers[base + a].to_value() {
                        Value::Enum(enum_type) => {
                            if let Some(def) =
                                enum_type.borrow_mut().variants.get_mut(&variant_name)
                            {
                                def.value = Some(value);
                            }
                        }
                        _ => return Err(RuntimeError::new("SetVariantValue on non-enum value")),
                    }
                }
                Opcode::ConstructVariant => {
                    let variant_name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("variant name must be string")),
                    };
                    let enum_val = self.registers[base + b].to_value();
                    match enum_val {
                        Value::Enum(enum_type) => {
                            self.registers[base + a] = NanValue::from_value(&Value::Variant(
                                self.gc.allocate(pleno_value::VariantState {
                                    enum_type: enum_type.clone(),
                                    variant_name: Rc::new(variant_name),
                                    data: Vec::new(),
                                })?,
                            ));
                        }
                        _ => return Err(RuntimeError::new("ConstructVariant on non-enum value")),
                    }
                }
                Opcode::MatchVariant => {
                    let variant_name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("variant name must be string")),
                    };
                    let val = self.registers[base + b].to_value();
                    let is_match = match val {
                        Value::Variant(s) => s.borrow().variant_name.as_str() == variant_name,
                        // Option and Result values (`Some`, `None`, `Ok`, `Err`) are maps
                        // tagged with a `variant` key.
                        Value::Map(m) => m
                            .borrow()
                            .get("variant")
                            .and_then(|v| v.as_str())
                            .is_some_and(|v| v == variant_name),
                        _ => false,
                    };
                    self.registers[base + a] = NanValue::from_bool(is_match);
                }
                Opcode::AssertNotNull => {
                    let value = self.registers[base + b].to_value();
                    if matches!(value, Value::Null) {
                        return Err(self.throw_runtime_error("unexpected null value"));
                    }
                    self.registers[base + a] = NanValue::from_value(&value);
                }
                Opcode::GetVariantField => {
                    let index = c;
                    let val = self.registers[base + b].to_value();
                    match val {
                        Value::Variant(s) => {
                            let s = s.borrow();
                            if index < s.data.len() {
                                self.registers[base + a] = NanValue::from_value(&s.data[index]);
                            } else {
                                return Err(RuntimeError::new("variant field index out of bounds"));
                            }
                        }
                        // Map-encoded variants: `Some`/`Ok` keep their payload in `value`,
                        // `Err` in `error`, and multi-field variants in `value0`, `value1`, ...
                        Value::Map(m) => {
                            let m = m.borrow();
                            let indexed = m.get(&format!("value{index}")).cloned();
                            let payload = indexed.or_else(|| {
                                (index == 0)
                                    .then(|| m.get("value").or_else(|| m.get("error")).cloned())
                                    .flatten()
                            });
                            self.registers[base + a] =
                                NanValue::from_value(&payload.unwrap_or(Value::Null));
                        }
                        _ => return Err(RuntimeError::new("GetVariantField on non-variant value")),
                    }
                }
                Opcode::AddMethod => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("method name must be string")),
                    };
                    let func_val = self.registers[base + a].to_value();
                    let target_val = self.registers[base + b].to_value();

                    match target_val {
                        Value::String(type_name) => {
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Builtin(type_name.clone()),
                                None,
                            )?;
                            self.builtin_extensions
                                .entry(type_name.to_string())
                                .or_default()
                                .insert(name, info);
                        }
                        Value::Class(class) => {
                            let module = class.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Class(class.clone()),
                                module.as_ref(),
                            )?;
                            class.borrow_mut().methods.insert(name, info);
                        }
                        Value::Enum(enum_type) => {
                            let module = enum_type.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Enum(enum_type.clone()),
                                module.as_ref(),
                            )?;
                            enum_type.borrow_mut().methods.insert(name, info);
                        }
                        Value::Interface(interface) => {
                            let module = interface.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Interface(interface.clone()),
                                module.as_ref(),
                            )?;
                            let sig = match &info.function {
                                Value::Function(idx) => {
                                    let func = &self.functions[*idx];
                                    pleno_value::MethodSignature {
                                        name: name.clone(),
                                        arity: func.arity,
                                        is_static: false,
                                        is_async: func.is_async,
                                    }
                                }
                                _ => {
                                    return Err(RuntimeError::new(
                                        "interface method must be function",
                                    ));
                                }
                            };
                            interface.borrow_mut().methods.insert(name, sig);
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "AddMethod expects class, enum or interface",
                            ));
                        }
                    }
                }
                Opcode::AddStaticMethod => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let func_val = self.registers[base + a].to_value();
                    let target_val = self.registers[base + b].to_value();
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("method name must be string")),
                    };

                    match target_val {
                        Value::Class(class) => {
                            let module = class.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Class(class.clone()),
                                module.as_ref(),
                            )?;
                            class.borrow_mut().static_methods.insert(name, info);
                        }
                        Value::Enum(enum_type) => {
                            let module = enum_type.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Enum(enum_type.clone()),
                                module.as_ref(),
                            )?;
                            enum_type.borrow_mut().static_methods.insert(name, info);
                        }
                        _ => return Err(RuntimeError::new("AddStaticMethod on invalid target")),
                    }
                }
                Opcode::SetFieldVisibility => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let visibility = visibility_from_code(instruction.c())?;
                    let is_const = instruction.c() & pleno_compiler::CONST_FIELD_FLAG != 0;
                    match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            let mut class = class.borrow_mut();
                            if is_const && !class.const_fields.contains(&name) {
                                class.const_fields.push(name.clone());
                            }
                            class.fields.insert(name, visibility);
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "set field visibility on non-class value",
                            ));
                        }
                    }
                }
                Opcode::SetStaticFieldVisibility => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let visibility = visibility_from_code(instruction.c())?;
                    let is_const = instruction.c() & pleno_compiler::CONST_FIELD_FLAG != 0;
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        let mut class = class.borrow_mut();
                        if is_const && !class.const_static_fields.contains(&name) {
                            class.const_static_fields.push(name.clone());
                        }
                        class.static_field_visibility.insert(name, visibility);
                    } else {
                        return Err(RuntimeError::new(
                            "set static field visibility on non-class value",
                        ));
                    }
                }
                Opcode::SetAnnotations => {
                    let annotations = self.registers[base + b].to_value();
                    match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            class.borrow_mut().annotations = match annotations {
                                Value::Array(arr) => arr
                                    .borrow()
                                    .elements
                                    .iter()
                                    .filter_map(pleno_value::RuntimeAnnotation::from_value)
                                    .collect(),
                                _ => Vec::new(),
                            };
                        }
                        Value::Interface(interface) => {
                            interface.borrow_mut().annotations = match annotations {
                                Value::Array(arr) => arr
                                    .borrow()
                                    .elements
                                    .iter()
                                    .filter_map(pleno_value::RuntimeAnnotation::from_value)
                                    .collect(),
                                _ => Vec::new(),
                            };
                        }
                        Value::Enum(enum_type) => {
                            enum_type.borrow_mut().annotations = match annotations {
                                Value::Array(arr) => arr
                                    .borrow()
                                    .elements
                                    .iter()
                                    .filter_map(pleno_value::RuntimeAnnotation::from_value)
                                    .collect(),
                                _ => Vec::new(),
                            };
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "SetAnnotations expects class, interface or enum",
                            ));
                        }
                    }
                }
                Opcode::Call => {
                    let callee = self.registers[base + b].to_value();
                    if matches!(callee, Value::Null)
                        && let Some(message) = self.describe_missing_method(ip, b)
                    {
                        return Err(RuntimeError::new(message));
                    }
                    let args = match &self.registers[base + c].to_value() {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => return Err(RuntimeError::new("Call expects array of arguments")),
                    };
                    self.call_any_value(callee, args, base + a)?;
                }
                Opcode::CallRegs => {
                    let callee_nan = self.registers[base + b];
                    let args_start = base + b + 1;
                    if args_start + c > self.registers.len() {
                        return Err(RuntimeError::new("call arguments out of range"));
                    }
                    let previous = ip.checked_sub(1).map(|prev| function.chunk.code[prev]);
                    if self.try_call_regs(callee_nan, args_start, c, base + a)? {
                        continue;
                    }
                    let callee = callee_nan.to_value();
                    // The compiler copies the callee into register b just before
                    // the call; describe the register it came from.
                    if matches!(callee, Value::Null)
                        && let Some(copy) = previous
                        && copy.opcode() == Opcode::Move
                        && copy.a() as usize == b
                        && let Some(message) =
                            self.describe_missing_method(ip - 1, copy.b() as usize)
                    {
                        return Err(RuntimeError::new(message));
                    }
                    self.call_value_with_registers(callee, args_start, c, base + a)?;
                }
                Opcode::CallMethod => {
                    // The method, receiver and kind `GetMethod` stored, then the
                    // arguments.
                    let method_at = base + b;
                    let args_start = method_at + 3;
                    if args_start + c > self.registers.len() {
                        return Err(RuntimeError::new("call arguments out of range"));
                    }
                    let callee_nan = self.registers[method_at];
                    let receiver = self.registers[method_at + 1];
                    let kind = self.registers[method_at + 2];
                    match kind.as_bool() {
                        // A native method such as `items.push(x)`: the receiver
                        // goes first, as for a bound native method.
                        Some(true) => {
                            let Value::NativeFunction(native) = callee_nan.to_value() else {
                                return Err(RuntimeError::new(
                                    "CallMethod expects a native method",
                                ));
                            };
                            // The argument vector is kept for the next native
                            // call (a native that calls back takes a new one).
                            let mut args = std::mem::take(&mut self.native_args);
                            args.push(receiver.to_value());
                            args.extend(
                                self.registers[args_start..args_start + c]
                                    .iter()
                                    .map(|arg| arg.to_value()),
                            );
                            let result = native(self, &args);
                            args.clear();
                            self.native_args = args;
                            match result {
                                Ok(result) => {
                                    self.registers[base + a] = NanValue::from_value(&result)
                                }
                                Err(err) => return Err(self.rethrow(err)),
                            }
                        }
                        // Not a method (a field holding a function, a getter's
                        // result, a module member, ...): an ordinary call.
                        Some(false) => {
                            if self.try_call_regs(callee_nan, args_start, c, base + a)? {
                                continue;
                            }
                            let callee = callee_nan.to_value();
                            if matches!(callee, Value::Null)
                                && let Some(message) = self.describe_missing_method(ip, b)
                            {
                                return Err(RuntimeError::new(message));
                            }
                            self.call_value_with_registers(callee, args_start, c, base + a)?;
                        }
                        // A bytecode method, called as its bound method would be.
                        None => {
                            let owner = owner_from_nan(kind)
                                .ok_or_else(|| RuntimeError::new("CallMethod expects a method"))?;
                            if self.try_push_register_frame(
                                callee_nan,
                                Some(receiver),
                                args_start,
                                c,
                                base + a,
                                Some(owner.clone()),
                            )? {
                                continue;
                            }
                            let args: Vec<Value> = self.registers[args_start..args_start + c]
                                .iter()
                                .map(|arg| arg.to_value())
                                .collect();
                            self.call_function(
                                callee_nan.to_value(),
                                Some(receiver.to_value()),
                                &args,
                                base + a,
                                false,
                                CallOptions {
                                    current_owner: Some(owner),
                                    ..Default::default()
                                },
                            )?;
                        }
                    }
                }

                Opcode::CallNamed => {
                    let callee = self.registers[base + b].to_value();
                    let wrapper = match &self.registers[base + c].to_value() {
                        Value::Map(m) => m.borrow().entries.to_owned(),
                        _ => return Err(RuntimeError::new("CallNamed expects map of arguments")),
                    };

                    let positional = match wrapper.get("positional") {
                        Some(Value::Array(arr)) => arr.borrow().elements.clone(),
                        _ => Vec::new(),
                    };

                    let named = match wrapper.get("named") {
                        Some(Value::Map(m)) => m.borrow().entries.clone(),
                        _ => IndexMap::new(),
                    };

                    self.call_with_named(callee, positional, named, base + a)?;
                }
                Opcode::Partial => {
                    let callee = self.registers[base + b].to_value();
                    if !self.is_callable(&callee) {
                        return Err(RuntimeError::new("partial callee must be a function"));
                    }
                    let args_meta = match &self.registers[base + c].to_value() {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => return Err(RuntimeError::new("Partial expects array of arguments")),
                    };
                    // The compiler marks each `_` placeholder slot with `Missing`.
                    let placeholders = args_meta
                        .iter()
                        .enumerate()
                        .filter(|(_, arg)| matches!(arg, Value::Missing))
                        .map(|(idx, _)| idx)
                        .collect();
                    self.registers[base + a] = NanValue::from_value(&Value::Partial(
                        self.gc.allocate(pleno_value::PartialState {
                            callee,
                            args: args_meta,
                            placeholders,
                        })?,
                    ));
                }
                Opcode::Import => {
                    let module_name = match &function.chunk.constants[b] {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("import path must be string")),
                    };
                    let module_value = self.execute_module(&module_name)?;
                    self.registers[base + a] = NanValue::from_value(&module_value);
                }
                Opcode::PushHandler => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    let catch_ip = apply_offset(frame.ip, offset)?;
                    frame.exception_handlers.push(ExceptionHandler {
                        catch_ip,
                        base: frame.base,
                        error_reg: a,
                    });
                }
                Opcode::PopHandler => {
                    frame
                        .exception_handlers
                        .pop()
                        .ok_or_else(|| RuntimeError::new("pop handler underflow"))?;
                }
                Opcode::Throw => {
                    let mut error_value = self.registers[base + a].to_value();
                    // `c == 1`: an error the compiler raises (a failed type
                    // check), thrown as an `Error` like other runtime errors.
                    if c == 1
                        && let Value::String(message) = &error_value
                    {
                        error_value = self.new_error(&message.clone());
                    }
                    match self.unwind_to_handler(error_value) {
                        Ok(_) => unreachable!(),
                        Err(err) => return Err(err),
                    }
                }
                Opcode::SetPooled => {
                    let is_pooled = b == 1;
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class.borrow_mut().is_pooled = is_pooled;
                    } else {
                        return Err(RuntimeError::new("SetPooled on non-class value"));
                    }
                }
                Opcode::Slice => {
                    let collection = self.registers[base + b].to_value();
                    let wrapper = match &self.registers[base + c].to_value() {
                        Value::Map(m) => m.borrow().entries.to_owned(),
                        _ => return Err(RuntimeError::new("Slice expects map of ranges")),
                    };
                    let start = wrapper.get("start").cloned();
                    let end = wrapper.get("end").cloned();
                    let step = wrapper.get("step").cloned();

                    match collection {
                        Value::Array(array) => {
                            let slice = array_slice(&array.borrow().elements, start, end, step)?;
                            self.registers[base + a] = NanValue::from_value(&Value::Array(
                                self.gc.allocate(ArrayState::new(slice))?,
                            ));
                        }
                        Value::Tuple(tuple) => {
                            let slice = array_slice(&tuple.borrow().elements, start, end, step)?;
                            let new_tuple = TupleState::new(slice, None);
                            self.registers[base + a] =
                                NanValue::from_value(&Value::Tuple(self.gc.allocate(new_tuple)?));
                        }
                        Value::String(s) => {
                            let slice = string_slice(s.as_str(), start, end, step)?;
                            self.registers[base + a] =
                                NanValue::from_value(&Value::String(Rc::new(slice)));
                        }
                        _ => return Err(RuntimeError::new("Slice on non-collection")),
                    }
                }
            }
        }
    }

    fn resolve_type_param(&self, name: &str) -> Option<Value> {
        let frame = self.frames.last()?;
        let function = &self.functions[frame.function_index];

        // Check function type parameters
        for (i, param) in function.type_params.iter().enumerate() {
            if param.name == name {
                if i < frame.type_args.len() {
                    return Some(frame.type_args[i].clone());
                }
                // Declared but not specialized: any type is accepted.
                return Some(Value::String(Rc::new("Any".to_string())));
            }
        }

        // Check class type parameters
        if let Some(MethodOwner::Class(cls)) = &frame.current_owner {
            let cls = cls.borrow();
            for (i, param) in cls.type_params.iter().enumerate() {
                if param.name == name {
                    // First try to get from receiver (this)
                    if frame.base < self.registers.len()
                        && let Value::Object(obj) = self.registers[frame.base].to_value()
                        && let Some(type_args) = &obj.borrow().type_args
                        && i < type_args.len()
                    {
                        return Some(type_args[i].clone());
                    }

                    // Fallback to frame type args if receiver didn't work
                    // (e.g. constructor call where frame args are class args)
                    if function.type_params.is_empty() && i < frame.type_args.len() {
                        return Some(frame.type_args[i].clone());
                    }
                    return Some(Value::String(Rc::new("Any".to_string())));
                }
            }
        }
        None
    }

    fn is_instance_nan(&self, value: &NanValue, type_val: &NanValue) -> bool {
        self.is_instance(&value.to_value(), &type_val.to_value())
    }

    pub fn is_instance(&self, value: &Value, type_val: &Value) -> bool {
        match type_val {
            // A variant belongs to the enum that declared it.
            Value::Enum(enum_type) => match value {
                Value::Variant(variant) => {
                    let declared = &variant.borrow().enum_type;
                    let declared = declared.borrow();
                    let expected = enum_type.borrow();
                    declared.name == expected.name && declared.module == expected.module
                }
                _ => false,
            },
            Value::String(s) => {
                let s = s.as_str().trim();
                if s.starts_with('{') && s.ends_with('}') {
                    // Basic structural check: { "key": Type, ... }
                    let content = &s[1..s.len() - 1].trim();
                    if content.is_empty() {
                        return matches!(value, Value::Map(_) | Value::Object(_));
                    }
                    let parts = content.split(',');
                    for part in parts {
                        let sub_parts: Vec<&str> = part.splitn(2, ':').collect();
                        if sub_parts.len() != 2 {
                            continue;
                        }
                        let key = sub_parts[0].trim().replace("\"", "");
                        let expected_ty_name = sub_parts[1].trim();

                        // Check if value has this property and if it matches expected_ty_name
                        // For Map, we check entries. For Object, we check fields/properties.
                        let prop_val = match value {
                            Value::Map(m) => m.borrow().entries.get(&key).cloned(),
                            Value::Object(obj) => obj.borrow().fields.get(&key).cloned(),
                            _ => return false,
                        };

                        match prop_val {
                            Some(val) => {
                                if !self.is_instance(
                                    &val,
                                    &Value::String(Rc::new(expected_ty_name.to_string())),
                                ) {
                                    return false;
                                }
                            }
                            None => return false,
                        }
                    }
                    true
                } else {
                    if let Some(resolved) = self.resolve_type_param(s) {
                        return self.is_instance(value, &resolved);
                    }
                    self.matches_type_name(value, s)
                }
            }
            Value::Map(m) => {
                if let Some(Value::String(s)) = m.borrow().entries.get("__native_type") {
                    let type_name = s.as_str();
                    // An `Int` is accepted where a `Float` is expected.
                    matches!(
                        (type_name, value),
                        ("Int", Value::Int(_)) | ("Float", Value::Float(_) | Value::Int(_))
                    )
                } else {
                    false
                }
            }
            Value::NativeFunction(_) => {
                // Temporary fix: allow any NativeFunction to act as a type check
                // This unblocks native types like Mutex/Channel used in parameters
                true
            }
            Value::Class(target_class) => match value {
                Value::Object(obj) => {
                    if let Some(instance_class) = &obj.borrow().class {
                        let mut current = Some(instance_class.clone());
                        let mut found = false;
                        while let Some(cls) = current {
                            if cls.as_ptr() == target_class.as_ptr() {
                                found = true;
                                break;
                            }
                            current = cls.borrow().superclass.clone();
                        }
                        found
                    } else {
                        false
                    }
                }
                Value::Class(instance_class) => {
                    let mut current = Some(instance_class.clone());
                    let mut found = false;
                    while let Some(cls) = current {
                        if cls.as_ptr() == target_class.as_ptr() {
                            found = true;
                            break;
                        }
                        current = cls.borrow().superclass.clone();
                    }
                    found
                }
                Value::BoundClass(s) => {
                    let mut current = Some(s.borrow().class.clone());
                    let mut found = false;
                    while let Some(cls) = current {
                        if cls.as_ptr() == target_class.as_ptr() {
                            found = true;
                            break;
                        }
                        current = cls.borrow().superclass.clone();
                    }
                    found
                }
                _ => false,
            },
            Value::BoundClass(target) => match value {
                Value::Object(obj) => {
                    let borrowed = obj.borrow();
                    let target = target.borrow();
                    let target_class = &target.class;
                    let target_args = &target.type_args;
                    if let Some(instance_class) = &borrowed.class {
                        let mut current = Some(instance_class.clone());
                        let mut found = false;
                        while let Some(cls) = current {
                            if cls.as_ptr() == target_class.as_ptr() {
                                found = true;
                                break;
                            }
                            current = cls.borrow().superclass.clone();
                        }
                        if !found {
                            return false;
                        }
                        if let Some(instance_args) = &borrowed.type_args {
                            self.type_args_compatible(target_class, instance_args, target_args)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            },
            Value::Interface(target_iface) if target_iface.borrow().is_builtin_comparable() => {
                is_comparable_value(value)
            }
            Value::Interface(target_iface) => match value {
                Value::Object(obj) => {
                    let obj = obj.borrow();
                    let iface = target_iface.borrow();
                    // Check nominal
                    if let Some(cls) = &obj.class {
                        let mut current = Some(cls.clone());
                        while let Some(c) = current {
                            for imp in &c.borrow().implements {
                                let imp_iface = match imp {
                                    Value::Interface(i) => i,
                                    Value::BoundInterface(bi) => &bi.borrow().interface,
                                    _ => continue,
                                };
                                if imp_iface.borrow().name == iface.name {
                                    return true;
                                }
                            }
                            current = c.borrow().superclass.clone();
                        }
                    }
                    // Check structural
                    for name in iface.methods.keys() {
                        let mut found = false;
                        if let Some(cls) = &obj.class {
                            let mut current = Some(cls.clone());
                            while let Some(c) = current {
                                if c.borrow().methods.contains_key(name) {
                                    found = true;
                                    break;
                                }
                                current = c.borrow().superclass.clone();
                            }
                        }
                        if !found {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
            Value::BoundInterface(target) => {
                match value {
                    Value::Object(obj) => obj.borrow().class.as_ref().is_some_and(|class| {
                        self.implements_bound_interface(class, &target.borrow())
                    }),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    #[allow(clippy::cognitive_complexity, clippy::single_match)]
    /// Whether `actual` type arguments satisfy `expected` ones for `class`,
    /// honoring each parameter's variance: `out T` accepts subtypes, `in T`
    /// accepts supertypes, and a plain `T` must match exactly.
    fn type_args_compatible(
        &self,
        class: &pleno_value::gc::GcRc<Class>,
        actual: &[Value],
        expected: &[Value],
    ) -> bool {
        let params = class.borrow().type_params.clone();
        self.params_compatible(&params, actual, expected)
    }

    /// Whether `class` or a superclass implements the specialized interface
    /// `target` with type arguments compatible with the interface's variance.
    fn implements_bound_interface(
        &self,
        class: &pleno_value::gc::GcRc<Class>,
        target: &pleno_value::BoundInterfaceState,
    ) -> bool {
        let iface = target.interface.borrow();
        let mut current = Some(class.clone());
        while let Some(cls) = current {
            for implemented in &cls.borrow().implements {
                if let Value::BoundInterface(bound) = implemented {
                    let bound = bound.borrow();
                    if bound.interface.borrow().name == iface.name
                        && self.params_compatible(
                            &iface.type_params,
                            &bound.type_args,
                            &target.type_args,
                        )
                    {
                        return true;
                    }
                }
            }
            current = cls.borrow().superclass.clone();
        }
        false
    }

    fn params_compatible(
        &self,
        params: &[pleno_value::GenericParamMetadata],
        actual: &[Value],
        expected: &[Value],
    ) -> bool {
        if actual.len() != expected.len() {
            return false;
        }
        actual.iter().zip(expected).enumerate().all(|(i, (a, e))| {
            let variance = params
                .get(i)
                .map(|p| p.variance)
                .unwrap_or(pleno_common::Variance::Invariant);
            match variance {
                pleno_common::Variance::Covariant => self.check_type_compatible(a, e),
                pleno_common::Variance::Contravariant => self.check_type_compatible(e, a),
                pleno_common::Variance::Invariant => {
                    self.check_type_compatible(a, e) && self.check_type_compatible(e, a)
                }
            }
        })
    }

    fn check_type_compatible(&self, val_type: &Value, target_type: &Value) -> bool {
        if val_type == target_type {
            return true;
        }

        match target_type {
            Value::String(s) => {
                if s.as_str() == "Any" {
                    return true;
                }
                if let Some(frame) = self.frames.last() {
                    // First check function's own type_params
                    let function = &self.functions[frame.function_index];
                    for (i, tp_name) in function.type_params.iter().enumerate() {
                        if tp_name.name == s.as_str() && i < frame.type_args.len() {
                            return self.check_type_compatible(val_type, &frame.type_args[i]);
                        }
                    }
                    // Then check owner class's type_params
                    if let Some(MethodOwner::Class(cls)) = &frame.current_owner {
                        let cls_ref = cls.borrow();
                        for (i, tp_name) in cls_ref.type_params.iter().enumerate() {
                            if tp_name.name == s.as_str() && i < frame.type_args.len() {
                                return self.check_type_compatible(val_type, &frame.type_args[i]);
                            }
                        }
                    }
                }
                match val_type {
                    Value::String(vs) => vs.as_str() == s.as_str(),
                    Value::Class(cls) => cls.borrow().name.as_str() == s.as_str(),
                    _ => false,
                }
            }
            Value::Class(target_class) => match val_type {
                Value::Class(instance_class) => {
                    let mut current = Some(instance_class.clone());
                    while let Some(cls) = current {
                        if cls.as_ptr() == target_class.as_ptr() {
                            return true;
                        }
                        current = cls.borrow().superclass.clone();
                    }
                    false
                }
                Value::BoundClass(bc) => {
                    let bc = bc.borrow();
                    let mut current = Some(bc.class.clone());
                    while let Some(cls) = current {
                        if cls.as_ptr() == target_class.as_ptr() {
                            return true;
                        }
                        current = cls.borrow().superclass.clone();
                    }
                    false
                }
                _ => false,
            },
            Value::Map(m) => {
                if let Some(Value::String(s)) = m.borrow().entries.get("__native_type") {
                    match s.as_str() {
                        "Int" => return matches!(val_type, Value::Int(_)),
                        "Float" => return matches!(val_type, Value::Float(_)),
                        "String" => return matches!(val_type, Value::String(_)),
                        "Bool" => return matches!(val_type, Value::Bool(_)),
                        "Char" => return matches!(val_type, Value::Char(_)),
                        _ => {}
                    }
                }
                false
            }
            Value::Interface(target_iface) if target_iface.borrow().is_builtin_comparable() => {
                is_comparable_type(val_type)
            }
            Value::Interface(target_iface) => match val_type {
                Value::Class(cls) => {
                    let iface = target_iface.borrow();
                    let mut current = Some(cls.clone());
                    while let Some(c) = current {
                        for imp in &c.borrow().implements {
                            let imp_iface = match imp {
                                Value::Interface(i) => i.clone(),
                                Value::BoundInterface(s) => s.borrow().interface.clone(),
                                _ => continue,
                            };
                            if imp_iface.borrow().name == iface.name {
                                return true;
                            }
                        }
                        current = c.borrow().superclass.clone();
                    }
                    false
                }
                Value::BoundClass(state) => {
                    let iface = target_iface.borrow();
                    let mut current = Some(state.borrow().class.clone());
                    while let Some(c) = current {
                        for imp in &c.borrow().implements {
                            let imp_iface = match imp {
                                Value::Interface(i) => i.clone(),
                                Value::BoundInterface(s) => s.borrow().interface.clone(),
                                _ => continue,
                            };
                            if imp_iface.borrow().name == iface.name {
                                return true;
                            }
                        }
                        current = c.borrow().superclass.clone();
                    }
                    false
                }
                Value::Map(m) => {
                    let iface = target_iface.borrow();
                    if let Some(Value::Array(impls)) = m.borrow().entries.get("__implements") {
                        for imp in &impls.borrow().elements {
                            match imp {
                                Value::String(s) if s.as_str() == iface.name.as_str() => {
                                    return true;
                                }
                                Value::Interface(i) if i.borrow().name == iface.name => {
                                    return true;
                                }
                                _ => {}
                            }
                        }
                    }
                    false
                }
                _ => false,
            },
            Value::BoundInterface(target) => match val_type {
                Value::BoundInterface(instance) => {
                    let target = target.borrow();
                    let instance = instance.borrow();
                    if instance.interface.as_ptr() != target.interface.as_ptr() {
                        return false;
                    }
                    let params = target.interface.borrow().type_params.clone();
                    self.params_compatible(&params, &instance.type_args, &target.type_args)
                }
                Value::Class(cls) => self.implements_bound_interface(cls, &target.borrow()),
                Value::BoundClass(state) => {
                    self.implements_bound_interface(&state.borrow().class, &target.borrow())
                }
                _ => false,
            },
            Value::BoundClass(target) => match val_type {
                Value::BoundClass(instance) => {
                    let target = target.borrow();
                    let instance = instance.borrow();
                    if instance.class.as_ptr() != target.class.as_ptr() {
                        return false;
                    }
                    self.type_args_compatible(&target.class, &instance.type_args, &target.type_args)
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Runs `left == right` with the `==` operator of `left`'s class, if it
    /// has one, and returns whether the result is truthy.
    fn call_equality_operator(
        &mut self,
        left: &Value,
        right: &Value,
    ) -> Result<Option<bool>, RuntimeError> {
        let Value::Object(obj) = left else {
            return Ok(None);
        };
        let info = obj
            .borrow()
            .class
            .as_ref()
            .and_then(|class| class.borrow().operators.get("==").cloned());
        let Some(info) = info else {
            return Ok(None);
        };
        let method = Value::BoundMethod(self.gc.allocate(BoundMethodState {
            receiver: left.clone(),
            function: info.function.clone(),
            owner: info.owner.clone(),
            visibility: info.visibility.clone(),
        })?);
        let result =
            self.call_ext_with(method, std::slice::from_ref(right), CallOptions::default())?;
        Ok(Some(value_ops::is_truthy(&result)))
    }

    fn try_dispatch_operator(
        &mut self,
        target_reg: usize,
        op: &str,
        left: &Value,
        args: &[Value],
    ) -> Result<bool, RuntimeError> {
        if let Value::Object(obj) = left {
            let op_info = obj
                .borrow()
                .class
                .as_ref()
                .and_then(|c| c.borrow().operators.get(op).cloned());

            if let Some(op_info) = op_info {
                self.call_function(
                    op_info.function.clone(),
                    Some(left.clone()),
                    args,
                    target_reg,
                    false,
                    CallOptions {
                        current_owner: Some(op_info.owner.clone()),
                        ..Default::default()
                    },
                )?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `register = register + right` for a string in `register` that nothing
    /// else refers to: appends to the string where it is instead of copying it
    /// into a new one, so building a string piece by piece takes linear time.
    /// Returns `false` (and changes nothing) when the string may be visible
    /// elsewhere or `right` is not a primitive, for the general `+`.
    fn append_in_place(
        &mut self,
        register: usize,
        left: NanValue,
        right: NanValue,
    ) -> Result<bool, RuntimeError> {
        // Strings are shared with other VMs on this thread (a sandbox) through
        // the string table; only the VM that owns the table may change one.
        // Past a few thousand registers the scan below costs more than a copy.
        if !self.gc.sweeps_boxed_values || self.registers.len() > MAX_APPEND_SCAN || right == left {
            return Ok(false);
        }
        // The text `+` appends: what `add_values` converts the operand to.
        let right = right.to_value();
        let owned;
        let text = match &right {
            Value::String(text) => text.as_str(),
            Value::Int(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Char(_)
            | Value::Null
            | Value::Missing => {
                owned = right.to_string();
                owned.as_str()
            }
            _ => return Ok(false),
        };
        // No `Value` may share the string, and no other register may hold it
        // (an alias such as `let a = s`, an argument, a caller's variable).
        let Some(len) = left.unshared_string_len() else {
            return Ok(false);
        };
        if self
            .registers
            .iter()
            .enumerate()
            .any(|(index, value)| index != register && *value == left)
        {
            return Ok(false);
        }
        // Counted as the copy that `+` would have made.
        self.gc.track_allocation(len + text.len())?;
        Ok(left.append_to_unshared_string(text))
    }

    /// `left + right` into register `dest`, with operator overloading.
    #[inline]
    fn add_registers(
        &mut self,
        dest: usize,
        left_nan: NanValue,
        right_nan: NanValue,
    ) -> Result<(), RuntimeError> {
        if let (Some(left), Some(right)) = (left_nan.as_inline_int(), right_nan.as_inline_int())
            && let Some(sum) = left.checked_add(right)
        {
            self.registers[dest] = NanValue::from_int(sum);
            return Ok(());
        }
        if (left_nan.is_f64() || left_nan.as_int().is_some())
            && (right_nan.is_f64() || right_nan.as_int().is_some())
        {
            self.registers[dest] = crate::value_ops::add_nan(self, left_nan, right_nan)?;
            return Ok(());
        }
        let left = left_nan.to_value();
        let right = right_nan.to_value();
        if self.try_dispatch_operator(dest, "+", &left, std::slice::from_ref(&right))? {
            return Ok(());
        }
        let res = crate::value_ops::add_values(self, left, right)?;
        self.registers[dest] = NanValue::from_value(&res);
        Ok(())
    }

    /// `left - right` into register `dest`, with operator overloading.
    #[inline]
    fn sub_registers(
        &mut self,
        dest: usize,
        left_nan: NanValue,
        right_nan: NanValue,
    ) -> Result<(), RuntimeError> {
        if let (Some(left), Some(right)) = (left_nan.as_inline_int(), right_nan.as_inline_int())
            && let Some(difference) = left.checked_sub(right)
        {
            self.registers[dest] = NanValue::from_int(difference);
            return Ok(());
        }
        if (left_nan.is_f64() || left_nan.as_int().is_some())
            && (right_nan.is_f64() || right_nan.as_int().is_some())
        {
            self.registers[dest] = crate::value_ops::sub_nan(left_nan, right_nan)?;
            return Ok(());
        }
        let left = left_nan.to_value();
        let right = right_nan.to_value();
        if self.try_dispatch_operator(dest, "-", &left, std::slice::from_ref(&right))? {
            return Ok(());
        }
        let res = crate::value_ops::sub_values(left, right)?;
        self.registers[dest] = NanValue::from_value(&res);
        Ok(())
    }

    /// The common call of a bytecode function or closure with positional
    /// arguments already in registers: copies them into the new frame without
    /// converting them to `Value`s. Returns `false` (doing nothing) for calls
    /// that need `call_any_value`: other callees, async functions, variadic or
    /// keyword-only parameters, and argument counts that are errors.
    /// Removes the top frame. The frame of a plain call owns nothing, and is
    /// then removed without running its (empty) destructor.
    #[inline(always)]
    fn pop_frame(&mut self) {
        let Some(frame) = self.frames.last() else {
            return;
        };
        if frame.return_override.is_none()
            && frame.closure_state.is_none()
            && frame.current_owner.is_none()
            && frame.type_args.capacity() == 0
            && frame.exception_handlers.capacity() == 0
        {
            // SAFETY: every field of the last frame that could own memory or a
            // reference count is `None` or a vector that never allocated, so
            // dropping it would do nothing; shortening the vector forgets it.
            unsafe { self.frames.set_len(self.frames.len() - 1) }
        } else {
            drop(self.frames.pop());
        }
    }

    /// When the only frame above `depth` is at a plain `Return` (not async, no
    /// return override), runs it as `run_step` would, ending the run for that
    /// frame, and gives the returned value (to convert before anything can
    /// collect).
    fn finish_plain_return(&mut self, depth: usize) -> Option<NanValue> {
        if self.frames.len() != depth + 1 {
            return None;
        }
        let frame = self.frames.last()?;
        let function = &self.functions[frame.function_index];
        let instruction = *function.chunk.code.get(frame.ip)?;
        if instruction.opcode() != Opcode::Return
            || function.is_async
            || frame.return_override.is_some()
        {
            return None;
        }
        let frame_base = frame.base;
        // Freed registers keep their table slots until the next collection, so
        // the caller can still convert this to a `Value`.
        let result = self.registers[frame_base + instruction.a() as usize];
        self.instruction_count += 1;
        self.pop_frame();
        if self.suspended_frames.is_empty() && self.pending_tasks.is_empty() {
            self.release_registers(frame_base);
        }
        Some(result)
    }

    /// Calls `callee` with the `argc` arguments from register `args_start` on,
    /// storing the result in `return_reg`.
    fn call_value_with_registers(
        &mut self,
        callee: Value,
        args_start: usize,
        argc: usize,
        return_reg: usize,
    ) -> Result<(), RuntimeError> {
        // A built-in method such as `items.push(x)`: one argument vector
        // with the receiver first, as `call_any_value` builds it.
        if let Value::BoundNativeMethod(state) = &callee {
            let (receiver, native) = {
                let state = state.borrow();
                (state.receiver.clone(), state.function)
            };
            let mut args = Vec::with_capacity(argc + 1);
            args.push(receiver);
            args.extend(
                self.registers[args_start..args_start + argc]
                    .iter()
                    .map(|arg| arg.to_value()),
            );
            match native(self, &args) {
                Ok(result) => self.registers[return_reg] = NanValue::from_value(&result),
                Err(err) => return Err(self.rethrow(err)),
            }
            return Ok(());
        }
        let args = self.registers[args_start..args_start + argc]
            .iter()
            .map(|arg| arg.to_value())
            .collect();
        self.call_any_value(callee, args, return_reg)
    }

    fn try_call_regs(
        &mut self,
        callee: NanValue,
        args_start: usize,
        argc: usize,
        return_reg: usize,
    ) -> Result<bool, RuntimeError> {
        self.try_push_register_frame(callee, None, args_start, argc, return_reg, None)
    }

    /// Pushes the frame of a plain call of the bytecode function or closure
    /// `callee` with `receiver` (for a method) and the `argc` arguments from
    /// register `args_start` on, or returns `false` for any other call (an
    /// async function, a function with variadic or keyword-only parameters,
    /// too many arguments, or a callee that is not a function).
    #[inline(always)]
    fn try_push_register_frame(
        &mut self,
        callee: NanValue,
        receiver: Option<NanValue>,
        args_start: usize,
        argc: usize,
        return_reg: usize,
        current_owner: Option<MethodOwner>,
    ) -> Result<bool, RuntimeError> {
        self.check_call_depth()?;
        let (function_index, closure_state) = if let Some(index) = callee.as_function_index() {
            (index, None)
        } else if callee.is_closure() {
            let Value::Closure(state) = callee.to_value() else {
                return Ok(false);
            };
            let index = state.borrow().function;
            (index, Some(state))
        } else {
            return Ok(false);
        };
        let Some(function) = self.functions.get(function_index) else {
            return Ok(false);
        };
        let given = argc + usize::from(receiver.is_some());
        // A deprecated function goes through `call_function`, which warns.
        if function.is_async || function.deprecation.is_some() || !plain_params(function, given) {
            return Ok(false);
        }
        let param_count = function.params.len();
        let register_count = function.chunk.max_registers.max(function.arity);

        let base = self.registers.len();
        // The arguments are below the new frame's registers.
        if args_start + argc > base {
            return Ok(false);
        }
        // `plain_params` holds, so `given <= function.arity <= register_count`.
        let defaults_end = param_count.min(register_count).max(given);
        self.registers.reserve(register_count);
        // SAFETY: `reserve` made room for `register_count` registers from
        // `base` on. Each of them is written exactly once below (the receiver
        // and arguments, then `missing` for parameters left out, then `null`)
        // before `set_len` covers them, and the arguments read lie below `base`
        // (checked above), in initialized registers.
        unsafe {
            let registers = self.registers.as_mut_ptr();
            let mut at = base;
            if let Some(receiver) = receiver {
                registers.add(at).write(receiver);
                at += 1;
            }
            for index in 0..argc {
                registers.add(at).write(*registers.add(args_start + index));
                at += 1;
            }
            // Parameters left out take their default values.
            while at < base + defaults_end {
                registers.add(at).write(NanValue::missing());
                at += 1;
            }
            while at < base + register_count {
                registers.add(at).write(NanValue::null());
                at += 1;
            }
            self.registers.set_len(base + register_count);
        }

        // Written in place: `push` built the frame and then copied it (a
        // `memcpy` call that showed up in profiles of calls).
        self.frames.reserve(1);
        let len = self.frames.len();
        // SAFETY: `reserve` made room for one more frame at index `len`, which
        // `set_len` then covers.
        unsafe {
            self.frames.as_mut_ptr().add(len).write(CallFrame {
                function_index,
                ip: 0,
                base,
                return_reg,
                return_override: None,
                closure_state,
                current_owner,
                type_args: Vec::new(),
                exception_handlers: Vec::new(),
            });
            self.frames.set_len(len + 1);
        }
        Ok(true)
    }

    #[allow(clippy::cognitive_complexity)]
    fn call_function(
        &mut self,
        callee: Value,
        receiver: Option<Value>,
        args: &[Value],
        return_reg_idx: usize,
        args_already_processed: bool,
        opts: CallOptions,
    ) -> Result<(), RuntimeError> {
        self.check_call_depth()?;
        let CallOptions {
            return_override,
            mut closure_state,
            current_owner,
            type_args,
            promise_id: override_promise_id,
        } = opts;

        let function_index = match callee {
            Value::Function(idx) => idx,
            Value::Closure(c) => {
                let idx = c.borrow().function;
                closure_state = Some(c.clone());
                idx
            }
            _ => return Err(RuntimeError::new("expected function or closure")),
        };
        self.warn_if_deprecated(function_index);
        // Copy out what the call needs instead of cloning the whole function
        // (its chunk, constants and parameter list) on every call.
        let function = self
            .functions
            .get(function_index)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
        let function_arity = function.arity;
        let function_is_async = function.is_async;
        let max_registers = function.chunk.max_registers;
        let param_count = function.params.len();
        // Positional arguments that map one to one onto plain parameters go
        // straight into the new registers, without building argument vectors.
        let given = args.len() + usize::from(receiver.is_some());
        let direct = !args_already_processed && !function_is_async && plain_params(function, given);

        let processed_args = if direct {
            Vec::new()
        } else if args_already_processed {
            args.to_vec()
        } else {
            let mut final_args = Vec::with_capacity(args.len() + 2);
            if let Some(recv) = receiver.clone() {
                final_args.push(recv);
            }
            final_args.extend(args.iter().cloned());

            let has_variadic = self.functions[function_index]
                .params
                .last()
                .map(|p| p.is_variadic)
                .unwrap_or(false);

            if !has_variadic && final_args.len() > function_arity {
                return Err(self.throw_runtime_error("too many positional arguments"));
            }

            let mut processed = Vec::with_capacity(param_count);
            let mut arg_idx = 0;

            if function_is_async {
                let promise_id = override_promise_id.unwrap_or_else(|| {
                    let id = self.next_promise_id;
                    self.next_promise_id += 1;
                    id
                });
                final_args.push(Value::Int(promise_id as i64));

                // If it's an override, the promise map ALREADY exists in self.promises.
                // We just need to ensure it's there.
                let promise = if let Some(p) = self.promises.get(&promise_id) {
                    p.clone()
                } else {
                    let mut promise_map = IndexMap::new();
                    promise_map.insert("id".to_string(), Value::Int(promise_id as i64));
                    promise_map.insert(
                        "variant".to_string(),
                        Value::String(Rc::new("Pending".to_string())),
                    );
                    promise_map.insert("value".to_string(), Value::Null);
                    let p = Value::Map(self.gc.allocate(MapState::new(promise_map))?);
                    self.promises.insert(promise_id, p.clone());
                    p
                };
                if return_reg_idx >= self.registers.len() {
                    self.registers.resize(return_reg_idx + 1, NanValue::null());
                }
                self.registers[return_reg_idx] = NanValue::from_value(&promise);
            }

            for param_index in 0..param_count {
                let param = &self.functions[function_index].params[param_index];
                let (is_variadic, is_keyword_only, has_default) =
                    (param.is_variadic, param.is_keyword_only, param.has_default);
                if is_variadic {
                    let rest = final_args[arg_idx..].to_vec();
                    processed.push(Value::Array(self.gc.allocate(ArrayState::new(rest))?));
                    arg_idx = final_args.len();
                    break;
                }

                if is_keyword_only {
                    if arg_idx < final_args.len() {
                        return Err(self.throw_runtime_error("keyword-only argument requires name"));
                    }
                    if has_default {
                        processed.push(Value::Missing);
                    } else {
                        return Err(
                            self.throw_runtime_error("missing required keyword-only argument")
                        );
                    }
                    continue;
                }

                if arg_idx < final_args.len() {
                    processed.push(final_args[arg_idx].clone());
                    arg_idx += 1;
                } else if has_default {
                    processed.push(Value::Missing);
                } else {
                    return Err(self.throw_runtime_error("missing required argument"));
                }
            }

            if arg_idx < final_args.len() {
                return Err(self.throw_runtime_error("too many positional arguments"));
            }
            log::trace!(
                "Calling function. is_async: {}, arity: {}, params: {}",
                function_is_async,
                function_arity,
                param_count
            );

            processed
        };

        let base = self.registers.len();
        let register_count = max_registers.max(function_arity);
        self.registers
            .extend(std::iter::repeat_n(NanValue::null(), register_count));

        if direct {
            let values = receiver.iter().chain(args.iter());
            for (idx, arg) in values.take(register_count).enumerate() {
                self.registers[base + idx] = NanValue::from_value(arg);
            }
            // Parameters left out take their default values.
            for idx in given..param_count.min(register_count) {
                self.registers[base + idx] = NanValue::missing();
            }
        }
        for (idx, arg) in processed_args.iter().enumerate() {
            if idx < register_count {
                self.registers[base + idx] = NanValue::from_value(arg);
            }
        }

        self.frames.push(CallFrame {
            function_index,
            ip: 0,
            base,
            return_reg: return_reg_idx,
            return_override,
            closure_state,
            current_owner,
            type_args,
            exception_handlers: Vec::new(),
        });
        Ok(())
    }

    fn call_with_named(
        &mut self,
        callee: Value,
        positional: Vec<Value>,
        named: IndexMap<String, Value>,
        return_reg: usize,
    ) -> Result<(), RuntimeError> {
        match callee {
            Value::NativeFunction(func) => {
                let prev_named = std::mem::replace(&mut self.current_named_args, named);
                let result = func(self, &positional);
                self.current_named_args = prev_named;
                let result = result?;
                self.registers[return_reg] = NanValue::from_value(&result);
                Ok(())
            }
            Value::BoundNativeMethod(state) => {
                let state = state.borrow();
                let prev_named = std::mem::replace(&mut self.current_named_args, named);
                let mut merged_args = vec![state.receiver.clone()];
                merged_args.extend(positional);
                let result = (state.function)(self, &merged_args);
                self.current_named_args = prev_named;
                let result = result?;
                self.registers[return_reg] = NanValue::from_value(&result);
                Ok(())
            }
            Value::Class(class) => {
                check_instantiable(&class)?;
                let object = self.gc.allocate(Object::new(Some(class.clone()), None))?;
                if let Some(handle) = resolve_constructor(&class) {
                    let handle_idx = match handle.function {
                        Value::Function(idx) => idx,
                        Value::Closure(ref c) => c.borrow().function,
                        _ => unreachable!("constructor must be function or closure"),
                    };
                    let _function = self
                        .functions
                        .get(handle_idx)
                        .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                    let args = self.build_named_args(handle_idx, true, &positional, &named)?;
                    let mut final_args = vec![Value::Object(object.clone())];
                    final_args.extend(args);
                    let override_value = Some(Value::Object(object.clone()));
                    self.call_function(
                        handle.function.clone(),
                        Some(Value::Object(object.clone())),
                        &final_args,
                        return_reg,
                        true,
                        CallOptions {
                            return_override: override_value,
                            current_owner: Some(handle.owner.clone()),
                            ..Default::default()
                        },
                    )
                } else if named.is_empty()
                    && (self.call_native_constructor(
                        &class,
                        &Value::Object(object.clone()),
                        &positional,
                    )? || positional.is_empty())
                {
                    self.registers[return_reg] = NanValue::from_value(&Value::Object(object));
                    Ok(())
                } else {
                    Err(RuntimeError::new("constructor not defined for named args"))
                }
            }
            Value::BoundClass(state) => {
                let state = state.borrow();
                let class = &state.class;
                let type_args = &state.type_args;
                check_instantiable(class)?;
                let object = self
                    .gc
                    .allocate(Object::new(Some(class.clone()), Some(type_args.clone())))?;
                if let Some(handle) = resolve_constructor(class) {
                    let handle_idx = match handle.function {
                        Value::Function(idx) => idx,
                        Value::Closure(ref c) => c.borrow().function,
                        _ => unreachable!("constructor must be function or closure"),
                    };
                    let _function = self
                        .functions
                        .get(handle_idx)
                        .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                    let args = self.build_named_args(handle_idx, true, &positional, &named)?;
                    let mut final_args = vec![Value::Object(object.clone())];
                    final_args.extend(args);
                    let override_value = Some(Value::Object(object.clone()));
                    self.call_function(
                        handle.function.clone(),
                        Some(Value::Object(object.clone())),
                        &final_args,
                        return_reg,
                        true,
                        CallOptions {
                            return_override: override_value,
                            current_owner: Some(handle.owner.clone()),
                            ..Default::default()
                        },
                    )
                } else if named.is_empty()
                    && (self.call_native_constructor(
                        class,
                        &Value::Object(object.clone()),
                        &positional,
                    )? || positional.is_empty())
                {
                    self.registers[return_reg] = NanValue::from_value(&Value::Object(object));
                    Ok(())
                } else {
                    Err(RuntimeError::new("constructor not defined for named args"))
                }
            }
            Value::Function(handle) => {
                let handle_idx = handle;
                let _function = self
                    .functions
                    .get(handle_idx)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(handle_idx, false, &positional, &named)?;
                self.call_function(
                    Value::Function(handle_idx),
                    None,
                    &args,
                    return_reg,
                    true,
                    CallOptions::new(),
                )
            }
            Value::BoundMethod(state) => {
                let state = state.borrow();
                let receiver = &state.receiver;
                let function = &state.function;
                let owner = &state.owner;
                let func_val = function.clone();
                let handle_idx = match func_val.clone() {
                    Value::Function(idx) => idx,
                    Value::Closure(c) => c.borrow().function,
                    _ => unreachable!("expected function"),
                };
                let _function_info = self
                    .functions
                    .get(handle_idx)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(handle_idx, true, &positional, &named)?;
                let mut final_args = vec![receiver.clone()];
                final_args.extend(args);
                self.call_function(
                    func_val,
                    Some(receiver.clone()),
                    &final_args,
                    return_reg,
                    true,
                    CallOptions {
                        current_owner: Some(owner.clone()),
                        ..Default::default()
                    },
                )
            }
            Value::StaticMethod(state) => {
                let state = state.borrow();
                let function = &state.function;
                let owner = &state.owner;
                let func_val = function.clone();
                let handle_idx = match func_val.clone() {
                    Value::Function(idx) => idx,
                    Value::Closure(c) => c.borrow().function,
                    _ => unreachable!("expected function"),
                };
                let args = self.build_named_args(handle_idx, false, &positional, &named)?;
                self.call_function(
                    func_val,
                    None,
                    &args,
                    return_reg,
                    true,
                    CallOptions {
                        current_owner: Some(owner.clone()),
                        ..Default::default()
                    },
                )
            }
            Value::Closure(state) => {
                let borrowed = state.borrow();
                let args = self.build_named_args(borrowed.function, false, &positional, &named)?;
                self.call_function(
                    Value::Function(borrowed.function),
                    None,
                    &args,
                    return_reg,
                    true,
                    CallOptions {
                        closure_state: Some(state.clone()),
                        ..Default::default()
                    },
                )
            }
            _ => Err(RuntimeError::new("call on non-callable value")),
        }
    }

    fn build_named_args(
        &mut self,
        function_index: usize,
        has_receiver: bool,
        positional: &[Value],
        named: &IndexMap<String, Value>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let function = self
            .functions
            .get(function_index)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
        let mut args = Vec::new();
        let start = if has_receiver { 1 } else { 0 };
        let params = function.params.get(start..).unwrap_or(&[]);
        let mut pos_index = 0usize;
        for key in named.keys() {
            if !params.iter().any(|param| param.name == *key) {
                return Err(self.throw_runtime_error("unknown named argument"));
            }
        }
        for (idx, param) in params.iter().enumerate() {
            if param.is_variadic {
                if idx + 1 != params.len() {
                    return Err(self.throw_runtime_error("variadic parameter must be last"));
                }
                if named.contains_key(&param.name) {
                    return Err(self.throw_runtime_error("variadic parameter cannot be named"));
                }
                let rest = positional[pos_index..].to_vec();
                args.push(Value::Array(self.gc.allocate(ArrayState::new(rest))?));
                pos_index = positional.len();
                continue;
            }
            if param.is_keyword_only && pos_index < positional.len() {
                return Err(self.throw_runtime_error("keyword-only argument requires name"));
            }
            if pos_index < positional.len() {
                if named.contains_key(&param.name) {
                    return Err(self.throw_runtime_error("duplicate argument"));
                }
                args.push(positional[pos_index].clone());
                pos_index += 1;
                continue;
            }
            if let Some(value) = named.get(&param.name) {
                args.push(value.clone());
                continue;
            }
            if param.has_default {
                args.push(Value::Missing);
            } else {
                return Err(self.throw_runtime_error("missing required argument"));
            }
        }
        if pos_index < positional.len() {
            return Err(self.throw_runtime_error("too many positional arguments"));
        }

        Ok(args)
    }

    fn unwind_to_handler(&mut self, error_value: Value) -> Result<(), RuntimeError> {
        // Capture where the error was raised before frames are popped, so an
        // uncaught exception still reports its source location.
        let mut uncaught = RuntimeError::new(format!(
            "uncaught exception: {}",
            describe_exception(&error_value)
        ));
        self.enrich_error(&mut uncaught);

        let stop_depth = self.run_depths.last().cloned().unwrap_or(0);
        while self.frames.len() > stop_depth {
            let mut frame = self.frames.pop().unwrap();
            if let Some(handler) = frame.exception_handlers.pop() {
                frame.ip = handler.catch_ip;
                self.registers[handler.base + handler.error_reg] =
                    NanValue::from_value(&error_value);
                self.frames.push(frame);
                return Err(RuntimeError::handled());
            }
            // An exception escaping an async function rejects its promise instead of
            // propagating; the caller already holds that promise.
            let promise_id = self.async_frame_promise_id(&frame);
            self.release_registers(frame.base);
            if let Some(id) = promise_id {
                self.reject_promise(id, error_value);
                return Err(RuntimeError::handled());
            }
        }
        Err(uncaught)
    }

    /// The promise an async function frame settles; its ID is the last argument.
    fn async_frame_promise_id(&self, frame: &CallFrame) -> Option<u64> {
        let function = &self.functions[frame.function_index];
        if !function.is_async || function.arity == 0 {
            return None;
        }
        match self
            .registers
            .get(frame.base + function.arity - 1)?
            .to_value()
        {
            Value::Int(id) => id.try_into().ok(),
            _ => None,
        }
    }

    fn freeze_value(&mut self, value: &Value, recursive: bool) {
        match value {
            Value::Array(array) => {
                let mut arr = array.borrow_mut();
                if !arr.is_immutable {
                    arr.is_immutable = true;
                    if recursive {
                        let elements = arr.elements.clone();
                        drop(arr);
                        for item in elements {
                            self.freeze_value(&item, true);
                        }
                    }
                }
            }
            Value::Map(map) => {
                let mut m = map.borrow_mut();
                if !m.is_immutable {
                    m.is_immutable = true;
                    if recursive {
                        let entries = m.entries.clone();
                        drop(m);
                        for item in entries.values() {
                            self.freeze_value(item, true);
                        }
                    }
                }
            }
            Value::Object(object) => {
                let mut obj = object.borrow_mut();
                if !obj.is_immutable {
                    obj.is_immutable = true;
                    if recursive {
                        let fields = obj.fields.clone();
                        drop(obj);
                        for item in fields.values() {
                            self.freeze_value(item, true);
                        }
                    }
                }
            }
            Value::Set(set) => {
                let mut s = set.borrow_mut();
                if !s.is_immutable {
                    s.is_immutable = true;
                    // Sets usually contain immutable primitives (ValueKey),
                    // but if they ever contain mutable things, we'd recurse here.
                }
            }
            // Tuples themselves are immutable at the top level,
            // but we recurse for deep freezing.
            Value::Tuple(tuple) if recursive => {
                let elements = tuple.borrow().elements.clone();
                for item in elements {
                    self.freeze_value(&item, true);
                }
            }
            _ => {}
        }
    }

    /// An instance of the built-in `Error` class for a runtime error message:
    /// a prefix such as `TypeError: ` becomes its `name`, the rest its `message`.
    /// It is allocated outside the memory limit, so it can always be thrown.
    fn new_error(&mut self, message: &str) -> Value {
        let (name, message) = pleno_value::split_error_message(message);
        let mut object = Object::new(Some(self.error_class.clone()), None);
        object.fields.insert(
            "message".to_string(),
            Value::String(Rc::new(message.to_string())),
        );
        object
            .fields
            .insert("name".to_string(), Value::String(Rc::new(name.to_string())));
        Value::Object(self.gc.gc.allocate(object))
    }

    /// The value a promise is rejected with when a callback or task fails.
    fn rejection_reason(&mut self, error: RuntimeError) -> Value {
        let message = error
            .message
            .strip_prefix("uncaught exception: ")
            .unwrap_or(&error.message)
            .to_string();
        self.new_error(&message)
    }

    /// Prints `warning: f is deprecated: note` the first time a deprecated
    /// function is called, to the program's output when it is captured.
    fn warn_if_deprecated(&mut self, function_index: usize) {
        let Some(function) = self.functions.get(function_index) else {
            return;
        };
        let Some(note) = &function.deprecation else {
            return;
        };
        if !self.deprecation_warnings.insert(function.name.to_string()) {
            return;
        }
        let message = if note.is_empty() {
            format!("warning: {} is deprecated\n", function.name)
        } else {
            format!("warning: {} is deprecated: {}\n", function.name, note)
        };
        match &mut self.output {
            Some(buffer) => buffer.push_str(&message),
            None => eprint!("{message}"),
        }
    }

    /// Fails a call that would go deeper than `MAX_CALL_DEPTH` frames, so
    /// runaway recursion raises a catchable `RecursionError` instead of
    /// growing memory without bound.
    fn check_call_depth(&self) -> Result<(), RuntimeError> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(RuntimeError::new(format!(
                "RecursionError: maximum call depth exceeded ({MAX_CALL_DEPTH} calls)"
            )));
        }
        Ok(())
    }

    /// Ends the program at a sandbox limit. Unlike other errors it does not
    /// unwind to a `catch`: a program that caught it could run on forever.
    fn sandbox_limit_error(&mut self, limit: &str) -> RuntimeError {
        let mut error = RuntimeError::fatal(format!("SandboxError: {limit}"));
        self.enrich_error(&mut error);
        error
    }

    /// Throws `err`, which a native function or callback returned, as a Pleno
    /// exception that `catch` can handle. A fatal error (a sandbox limit) or one
    /// already handled passes through unchanged. An exception that escaped a
    /// callback already reads `uncaught exception: ...`; it is thrown again with
    /// its own message.
    fn rethrow(&mut self, err: RuntimeError) -> RuntimeError {
        if err.fatal || err.handled {
            return err;
        }
        let message = err
            .message
            .strip_prefix("uncaught exception: ")
            .unwrap_or(&err.message)
            .to_string();
        self.throw_runtime_error(&message)
    }

    fn throw_runtime_error(&mut self, message: &str) -> RuntimeError {
        // Provide a small grace buffer for error handling if we hit memory limits
        if self.gc.max_bytes > 0 && self.gc.grace_bytes == 0 {
            self.gc.grace_bytes = 64 * 1024; // 64KB grace for emergency handling
        }

        let error_val = self.new_error(message);
        match self.unwind_to_handler(error_val) {
            Ok(_) => unreachable!("unwind_to_handler should always return Err"),
            Err(err) => err,
        }
    }

    /// Runs the native constructor `class` inherits (the built-in `Error`'s) on
    /// `object`; `false` if it has none.
    fn call_native_constructor(
        &mut self,
        class: &GcRc<Class>,
        object: &Value,
        args: &[Value],
    ) -> Result<bool, RuntimeError> {
        let Some(constructor) = class.borrow().resolve_native_constructor() else {
            return Ok(false);
        };
        let mut full_args = vec![object.clone()];
        full_args.extend_from_slice(args);
        constructor(self, &full_args)?;
        Ok(true)
    }

    fn is_callable(&self, value: &Value) -> bool {
        match value {
            Value::Function(_)
            | Value::Closure(_)
            | Value::NativeFunction(_)
            | Value::BoundNativeMethod { .. }
            | Value::BoundMethod { .. }
            | Value::Class(_)
            | Value::BoundClass { .. }
            | Value::BoundFunction { .. }
            | Value::VariantConstructor { .. }
            | Value::Partial { .. } => true,
            Value::Map(map) => map.borrow().get("__call__").is_some(),
            _ => false,
        }
    }

    fn call_any_value(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        return_reg: usize,
    ) -> Result<(), RuntimeError> {
        log::trace!("VM::call_any_value: calling {:?}", callee);
        match callee {
            Value::Thunk(_) => {
                let forced = self.force_thunk(callee)?;
                return self.call_any_value(forced, args, return_reg);
            }
            Value::Function(handle) => {
                self.call_function(
                    Value::Function(handle),
                    None,
                    &args,
                    return_reg,
                    false,
                    CallOptions::new(),
                )?;
            }
            Value::BoundFunction(state) => {
                let state = state.borrow();
                self.call_function(
                    state.function.clone(),
                    None,
                    &args,
                    return_reg,
                    false,
                    CallOptions {
                        type_args: state.type_args.clone(),
                        ..Default::default()
                    },
                )?;
            }
            Value::Closure(state) => {
                let _borrowed = state.borrow();
                self.call_function(
                    Value::Closure(state.clone()),
                    None,
                    &args,
                    return_reg,
                    false,
                    CallOptions {
                        closure_state: Some(state.clone()),
                        ..Default::default()
                    },
                )?;
            }
            Value::NativeFunction(func) => match func(self, &args) {
                Ok(result) => {
                    if return_reg >= self.registers.len() {
                        self.registers.resize(return_reg + 1, NanValue::null());
                    }
                    self.registers[return_reg] = NanValue::from_value(&result);
                }
                Err(err) => {
                    return Err(self.rethrow(err));
                }
            },
            Value::BoundNativeMethod(state) => {
                let state = state.borrow();
                let mut merged_args = vec![state.receiver.clone()];
                merged_args.extend(args);
                match (state.function)(self, &merged_args) {
                    Ok(result) => {
                        if return_reg >= self.registers.len() {
                            self.registers.resize(return_reg + 1, NanValue::null());
                        }
                        self.registers[return_reg] = NanValue::from_value(&result);
                    }
                    Err(err) => {
                        return Err(self.rethrow(err));
                    }
                }
            }
            Value::BoundMethod(state) => {
                let state = state.borrow();
                self.call_function(
                    state.function.clone(),
                    Some(state.receiver.clone()),
                    &args,
                    return_reg,
                    false,
                    CallOptions {
                        current_owner: Some(state.owner.clone()),
                        ..Default::default()
                    },
                )?;
            }
            Value::StaticMethod(state) => {
                let state = state.borrow();
                self.call_function(
                    state.function.clone(),
                    None,
                    &args,
                    return_reg,
                    true,
                    CallOptions {
                        current_owner: Some(state.owner.clone()),
                        ..Default::default()
                    },
                )?;
            }
            Value::Class(class) => {
                check_instantiable(&class)?;
                let object = self.gc.allocate(Object::new(Some(class.clone()), None))?;
                let val = Value::Object(object.clone());
                if let Some(handle) = resolve_constructor(&class) {
                    self.call_function(
                        handle.function.clone(),
                        Some(val.clone()),
                        &args,
                        return_reg,
                        false,
                        CallOptions {
                            return_override: Some(val),
                            current_owner: Some(handle.owner.clone()),
                            ..Default::default()
                        },
                    )?;
                } else {
                    self.call_native_constructor(&class, &val, &args)?;
                    self.registers[return_reg] = NanValue::from_value(&val);
                }
            }
            Value::BoundClass(state) => {
                let state = state.borrow();
                let class = &state.class;
                let type_args = &state.type_args;
                check_instantiable(class)?;
                let object = self
                    .gc
                    .allocate(Object::new(Some(class.clone()), Some(type_args.clone())))?;
                let val = Value::Object(object.clone());
                if let Some(handle) = resolve_constructor(class) {
                    self.call_function(
                        handle.function.clone(),
                        Some(val.clone()),
                        &args,
                        return_reg,
                        false,
                        CallOptions {
                            return_override: Some(val),
                            current_owner: Some(handle.owner.clone()),
                            type_args: type_args.clone(),
                            ..Default::default()
                        },
                    )?;
                } else {
                    self.call_native_constructor(class, &val, &args)?;
                    self.registers[return_reg] = NanValue::from_value(&val);
                }
            }
            Value::VariantConstructor(state) => {
                let state = state.borrow();
                let mut data = Vec::new();
                data.extend(args);
                let variant = Value::Variant(self.gc.allocate(pleno_value::VariantState {
                    enum_type: state.enum_type.clone(),
                    variant_name: Rc::clone(&state.variant_name),
                    data,
                })?);
                self.registers[return_reg] = NanValue::from_value(&variant);
            }
            Value::Partial(state) => {
                let state = state.borrow();
                let mut merged_args = state.args.clone();
                let mut new_arg_idx = 0;
                for &placeholder_idx in &state.placeholders {
                    if new_arg_idx < args.len() {
                        merged_args[placeholder_idx] = args[new_arg_idx].clone();
                        new_arg_idx += 1;
                    }
                }
                while new_arg_idx < args.len() {
                    merged_args.push(args[new_arg_idx].clone());
                    new_arg_idx += 1;
                }
                self.call_any_value(state.callee.clone(), merged_args, return_reg)?;
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    self.call_any_value(callable, args, return_reg)?;
                } else {
                    return Err(RuntimeError::new("Map is not callable"));
                }
            }
            Value::Object(obj) => {
                let callable = obj.borrow().fields.get("__call__").cloned();
                if let Some(callable) = callable {
                    self.call_any_value(callable, args, return_reg)?;
                } else {
                    return Err(RuntimeError::new("Object is not callable"));
                }
            }
            other => {
                return Err(RuntimeError::new(format!(
                    "{} is not callable",
                    other.type_name()
                )));
            }
        }
        Ok(())
    }
    /// Frees the registers from `base` up, except those of suspended frames:
    /// an async call that is waiting keeps its registers where they are, and
    /// the function that called it may return before it resumes.
    #[inline]
    fn release_registers(&mut self, base: usize) {
        if self.suspended_frames.is_empty() {
            self.registers.truncate(base);
            return;
        }
        let in_use = self
            .suspended_frames
            .values()
            .map(|suspended| {
                let function = &self.functions[suspended.frame.function_index];
                suspended.frame.base + function.chunk.max_registers
            })
            .max()
            .unwrap_or(0);
        self.registers.truncate(base.max(in_use));
    }

    /// Explains a call of `null` that came from reading a property, as in
    /// `map.contains(x)` when maps have no `contains` method: the read found
    /// nothing, so "Null is not callable" would hide the real mistake.
    fn describe_missing_method(&self, ip: usize, callee_register: usize) -> Option<String> {
        let frame = self.frames.last()?;
        let chunk = &self.functions[frame.function_index].chunk;
        // The latest instruction before this call that wrote the callee register.
        let source = chunk.code[..ip]
            .iter()
            .rev()
            .find(|instruction| instruction.a() as usize == callee_register)?;
        if !matches!(source.opcode(), Opcode::GetProperty | Opcode::GetMethod) {
            return None;
        }
        let Value::String(name) = chunk.constants.get(source.c() as usize)? else {
            return None;
        };
        let receiver = self.registers[frame.base + source.b() as usize].to_value();
        Some(match receiver {
            Value::Map(_) => format!("Map has no method '{name}' (and no key '{name}')"),
            Value::Null => format!("cannot call '{name}' on null"),
            other => format!("{} has no method '{name}'", other.type_name()),
        })
    }

    fn execute_module(&mut self, path: &str) -> Result<Value, RuntimeError> {
        if let Some(mod_val) = self.modules.get(path) {
            return Ok(mod_val.clone());
        }

        let base_dir = self
            .frames
            .last()
            .and_then(|frame| self.functions[frame.function_index].module_path.as_ref())
            .map(|path| std::path::Path::new(path.as_ref()))
            .and_then(|path| path.parent())
            .map(|path| path.to_path_buf())
            .or_else(|| {
                self.main_path
                    .as_ref()
                    .and_then(|main| main.parent())
                    .map(|dir| dir.to_path_buf())
            });
        let resolved = resolve_module_path(path, base_dir)?;
        let resolved_key = resolved.to_string_lossy().to_string();
        if let Some(value) = self.modules.get(&resolved_key).cloned() {
            return Ok(value);
        }
        let source = fs::read_to_string(&resolved).map_err(|err| {
            RuntimeError::new(format!("failed to read module {resolved_key}: {err}"))
        })?;
        let tokens = pleno_lexer::lex(&source).map_err(|err| RuntimeError::new(err.message))?;
        let mut parser = pleno_parser::Parser::new(tokens, &source);
        let mut program = parser
            .parse_program()
            .map_err(|err| RuntimeError::new(err.message))?;
        let mut compiler = pleno_compiler::Compiler::new(source);
        let chunk = compiler
            .compile_module(&mut program)
            .map_err(|err| RuntimeError::new(err.diagnostic.message))?;
        let module_name = chunk
            .module_name
            .clone()
            .unwrap_or_else(|| resolved_key.clone());
        let module_rc = Rc::new(module_name);
        let function = Function {
            name: Rc::new(resolved_key.clone()),
            chunk,
            arity: 0,
            params: Vec::new(),
            module: Some(Rc::clone(&module_rc)),
            module_path: Some(Rc::new(resolved_key.clone())),
            is_async: false,
            is_pure: false,
            type_params: Vec::new(),
            type_constraints: IndexMap::new(),
            annotations: Vec::new(),
            plain_args: (0, 0),
            deprecation: None,
        };
        let func_index = self.functions.len();
        self.functions.push(function);
        self.module_globals.entry(resolved_key.clone()).or_default();
        let base = self.registers.len();
        let register_count = self.functions[func_index].chunk.max_registers;
        self.registers
            .extend(std::iter::repeat_n(NanValue::null(), register_count));
        let depth = self.frames.len();
        self.frames.push(CallFrame {
            function_index: func_index,
            ip: 0,
            base,
            return_reg: 0,
            return_override: None,
            closure_state: None,
            current_owner: None,
            type_args: Vec::new(),
            exception_handlers: Vec::new(),
        });
        let result = match self.run_until(depth)? {
            ExecutionStatus::Finished(v) => v,
            ExecutionStatus::Paused => return Err(RuntimeError::new("Paused during module load")),
        };
        self.registers.truncate(base);
        self.modules.insert(resolved_key, result.clone());
        Ok(result)
    }
    fn get_value_type_name_for_specialize(&self, val: &Value) -> String {
        match val {
            Value::Class(c) => c.borrow().name.to_string(),
            Value::BoundClass(s) => s.borrow().class.borrow().name.to_string(),
            Value::Interface(i) => i.borrow().name.to_string(),
            Value::BoundInterface(s) => s.borrow().interface.borrow().name.to_string(),
            Value::Map(m) => {
                if let Some(Value::String(s)) = m.borrow().get("__native_type") {
                    s.to_string()
                } else {
                    "Map".to_string()
                }
            }
            Value::String(s) => s.to_string(),
            Value::Char(_) => "Char".to_string(),
            Value::Int(_) => "Int".to_string(),
            Value::Float(_) => "Float".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            _ => "Unknown".to_string(),
        }
    }
}

/// A method's owner in a register, for `CallMethod` (see `Opcode::GetMethod`).
fn owner_to_nan(owner: &MethodOwner) -> NanValue {
    NanValue::from_value(&match owner {
        MethodOwner::Class(class) => Value::Class(class.clone()),
        MethodOwner::Enum(enum_type) => Value::Enum(enum_type.clone()),
        MethodOwner::Interface(interface) => Value::Interface(interface.clone()),
        MethodOwner::Builtin(name) => Value::String(name.clone()),
    })
}

/// The owner `owner_to_nan` stored, or `None` for `true` and `false`.
fn owner_from_nan(value: NanValue) -> Option<MethodOwner> {
    match value.to_value() {
        Value::Class(class) => Some(MethodOwner::Class(class)),
        Value::Enum(enum_type) => Some(MethodOwner::Enum(enum_type)),
        Value::Interface(interface) => Some(MethodOwner::Interface(interface)),
        Value::String(name) => Some(MethodOwner::Builtin(name)),
        _ => None,
    }
}

/// The innermost and outermost frames a deep backtrace keeps (see
/// `generate_backtrace`).
const BACKTRACE_INNER: usize = 50;
const BACKTRACE_OUTER: usize = 10;

/// Whether `<` and `>` work on `value`: numbers, strings and characters, and
/// objects whose class (or a parent) defines `operator <` or `operator >`, or
/// implements an interface named `Comparable`.
fn is_comparable_value(value: &Value) -> bool {
    match value {
        Value::Int(_) | Value::Float(_) | Value::String(_) | Value::Char(_) => true,
        Value::Object(obj) => obj.borrow().class.as_ref().is_some_and(is_comparable_class),
        _ => false,
    }
}

/// Whether a type argument names a comparable type (see `is_comparable_value`).
fn is_comparable_type(type_value: &Value) -> bool {
    match type_value {
        Value::Map(map) => matches!(
            map.borrow().entries.get("__native_type"),
            Some(Value::String(name)) if matches!(name.as_str(), "Int" | "Float")
        ),
        Value::String(name) => matches!(name.as_str(), "String" | "Char" | "Int" | "Float"),
        Value::Class(class) => is_comparable_class(class),
        Value::BoundClass(bound) => is_comparable_class(&bound.borrow().class),
        _ => false,
    }
}

fn is_comparable_class(class: &GcRc<Class>) -> bool {
    let mut current = Some(class.clone());
    while let Some(class) = current {
        let class_ref = class.borrow();
        let implements = class_ref.implements.iter().any(|imp| match imp {
            Value::Interface(i) => i.borrow().name.as_str() == "Comparable",
            Value::BoundInterface(b) => b.borrow().interface.borrow().name.as_str() == "Comparable",
            _ => false,
        });
        if implements
            || class_ref.operators.contains_key("<")
            || class_ref.operators.contains_key(">")
        {
            return true;
        }
        current = class_ref.superclass.clone();
    }
    false
}

/// The note of a `@deprecated` annotation: `Some("")` without an argument,
/// `None` when the function is not deprecated.
fn deprecation_note(annotations: &[pleno_value::RuntimeAnnotation]) -> Option<Rc<String>> {
    let annotation = annotations.iter().find(|a| a.name == "deprecated")?;
    Some(Rc::new(match annotation.args.first() {
        Some(Value::String(note)) => note.to_string(),
        _ => String::new(),
    }))
}

/// The deepest a chain of calls can go. Frames live on the heap, so this only
/// bounds memory: about 200 bytes and a few registers per call.
const MAX_CALL_DEPTH: usize = 100_000;

/// How many runs can be nested inside each other by default: calls from
/// native code back into Pleno (`map` callbacks, operators, test bodies). Each
/// uses about 8 KB of Rust stack in a release build, more in a debug build or
/// under a sanitizer; see `VM::max_nested_runs`.
pub const DEFAULT_MAX_NESTED_RUNS: usize = 200;

/// Whether `path` is inside one of the `allowed` folders (or is one of the
/// allowed files). Both sides are compared as normalized absolute paths, one
/// component at a time, so `data/../secret` is not inside `data` and `data2`
/// is not inside `data`.
fn path_allowed(path: &str, allowed: &[String]) -> bool {
    let Some(path) = normalized_path(path) else {
        return false;
    };
    allowed
        .iter()
        .filter_map(|base| normalized_path(base))
        .any(|base| path_starts_with(&path, &base))
}

/// `path` made absolute against the working directory, with `.` and `..`
/// removed and symbolic links resolved in the part of it that exists.
fn normalized_path(path: &str) -> Option<std::path::PathBuf> {
    use std::path::{Component, Path, PathBuf};
    let path = Path::new(path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut lexical = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                lexical.pop();
            }
            other => lexical.push(other.as_os_str()),
        }
    }
    // Resolve links in the longest part that exists; a file about to be
    // written does not exist yet.
    let mut existing = lexical.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => {
                missing.push(name.to_os_string());
                existing.pop();
            }
            None => break,
        }
    }
    let mut resolved = fs::canonicalize(&existing)
        .map(strip_verbatim_prefix)
        .unwrap_or(existing);
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Some(resolved)
}

/// `C:\dir` for Windows' `\\?\C:\dir`, so a canonical path compares equal to
/// one that was not canonicalized.
fn strip_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => std::path::PathBuf::from(rest),
        _ => path,
    }
}

/// `path` starts with `base`, component by component; on Windows without
/// regard to case, as its file systems compare names.
fn path_starts_with(path: &std::path::Path, base: &std::path::Path) -> bool {
    if cfg!(windows) {
        let lower = |p: &std::path::Path| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .collect::<Vec<_>>()
        };
        let (path, base) = (lower(path), lower(base));
        path.len() >= base.len() && path[..base.len()] == base[..]
    } else {
        path.starts_with(base)
    }
}

/// Registers that `append_in_place` scans at most for copies of a string.
const MAX_APPEND_SCAN: usize = 4096;

/// The main program's top-level names. They share `VM::globals` with the
/// built-in globals, so code outside modules reads both the same way; code in
/// a module sees the built-ins only.
#[derive(Debug, Default)]
pub struct ProgramGlobals {
    /// Every name the main program defined.
    names: std::collections::HashSet<String>,
    /// The built-in values those definitions replaced (a program's own
    /// `print`, say), which modules still see.
    replaced: IndexMap<String, Value>,
}

impl ProgramGlobals {
    /// Records that the main program defines `name`, before it is stored.
    fn define(&mut self, globals: &IndexMap<String, Value>, name: &str) {
        if self.names.insert(name.to_string())
            && let Some(builtin) = globals.get(name)
        {
            self.replaced.insert(name.to_string(), builtin.clone());
        }
    }
}

/// The value of global `name` as code in the module at `module_path` sees it:
/// the module's own top-level name if it has one, otherwise a built-in global.
/// Code outside modules sees the shared globals, its own names included.
fn lookup_global_in(
    globals: &IndexMap<String, Value>,
    module_globals: &IndexMap<String, IndexMap<String, Value>>,
    program: &ProgramGlobals,
    module_path: Option<&String>,
    name: &str,
) -> Option<Value> {
    let Some(table) = module_path.and_then(|path| module_globals.get(path.as_str())) else {
        return globals.get(name).cloned();
    };
    if let Some(value) = table.get(name) {
        return Some(value.clone());
    }
    if program.names.contains(name) {
        return program.replaced.get(name).cloned();
    }
    globals.get(name).cloned()
}

/// Fails for an abstract class, and for a class that leaves an abstract
/// method of an ancestor undefined.
fn check_instantiable(class: &GcRc<Class>) -> Result<(), RuntimeError> {
    let class_ref = class.borrow();
    if class_ref.is_abstract {
        return Err(RuntimeError::new(format!(
            "cannot instantiate abstract class '{}'",
            class_ref.name
        )));
    }
    let missing = access::missing_abstract_methods(class);
    if missing.is_empty() {
        return Ok(());
    }
    let what = if missing.len() == 1 {
        "abstract method"
    } else {
        "abstract methods"
    };
    Err(RuntimeError::new(format!(
        "cannot create {}: it does not implement {} {}",
        class_ref.name,
        what,
        missing.join(", ")
    )))
}

fn resolve_module_path(
    path: &str,
    base_dir: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf, RuntimeError> {
    if let Some(name) = path.strip_prefix("std:") {
        let current_dir = std::env::current_dir()
            .map_err(|err| RuntimeError::new(format!("failed to get current dir: {err}")))?;
        let std_path = current_dir
            .join("sources/std")
            .join(format!("{}.pln", name));
        if std_path.exists() {
            return Ok(std_path);
        }
        return Err(RuntimeError::new(format!("std module not found: {}", name)));
    }

    let raw_path = std::path::Path::new(path);
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }

    let current_dir = std::env::current_dir()
        .map_err(|err| RuntimeError::new(format!("failed to get current dir: {err}")))?;

    // Package Resolution (non-relative paths)
    if !path.starts_with("./") && !path.starts_with("../") && !path.starts_with("/") {
        let start_dir = base_dir.clone().unwrap_or_else(|| current_dir.clone());
        let mut current = start_dir.as_path();
        loop {
            let pkg_root = current.join("packages").join(path);
            if pkg_root.is_dir() {
                // Check pleno.toml
                let manifest_path = pkg_root.join("pleno.toml");
                if manifest_path.exists()
                    && let Ok(content) = fs::read_to_string(&manifest_path)
                    && let Ok(manifest) = content.parse::<toml::Table>()
                    && let Some(main) = manifest
                        .get("package")
                        .and_then(|package| package.get("main"))
                        .or_else(|| manifest.get("main"))
                        .and_then(|v| v.as_str())
                {
                    let main_path = pkg_root.join(main);
                    if main_path.exists() {
                        return Ok(main_path);
                    }
                } // Fallback to lib.pln
                let lib_path = pkg_root.join("lib.pln");
                if lib_path.exists() {
                    return Ok(lib_path);
                }
            } else {
                // Check packages/<path>.pln
                let pkg_file = current.join("packages").join(format!("{}.pln", path));
                if pkg_file.exists() {
                    return Ok(pkg_file);
                }
            }

            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }
    }

    // Relative or regular file resolution
    let mut candidates = Vec::new();
    let base = base_dir.unwrap_or_else(|| current_dir.clone());
    candidates.push(base.join(path));
    if !path.ends_with(".pln") {
        candidates.push(base.join(format!("{}.pln", path)));
    }

    // Legacy source fallback
    if !path.contains('/') && !path.contains('\\') {
        candidates.push(current_dir.join("sources").join(format!("{}.pln", path)));
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(RuntimeError::new(format!("module not found: {}", path)))
}

// fn resolve_builtin_method(vm: &VM, ty: &str, name: &str) -> Option<NativeFunction> {
//     vm.builtin_resolver.and_then(|resolver| resolver(ty, name))
// }

/// New strings, big integers and ranges between collections, at least: see
/// `boxed_values_added`. Scaled up with the number of objects, so that the
/// collections cost about as much as the values that cause them.
const BOXED_VALUES_PER_COLLECTION: usize = 1 << 14;

/// Whether `given` positional arguments fill the parameters of `function`
/// one to one: no variadic or keyword-only parameters, not too many
/// arguments, and defaults for the parameters left out.
fn plain_params(function: &Function, given: usize) -> bool {
    let (min, max) = function.plain_args;
    min <= given && given <= max
}

/// The range of argument counts for which `plain_params` holds: at least one
/// for each parameter up to the last without a default, at most one for each
/// parameter (and no more than the arity); empty with a variadic or
/// keyword-only parameter.
fn plain_arg_range(params: &[ParamSpec], arity: usize) -> (usize, usize) {
    if params
        .iter()
        .any(|param| param.is_variadic || param.is_keyword_only)
    {
        return (1, 0);
    }
    let required = params
        .iter()
        .rposition(|param| !param.has_default)
        .map_or(0, |index| index + 1);
    (required, arity.min(params.len()))
}

fn value_to_key(value: &Value) -> Result<ValueKey, RuntimeError> {
    ValueKey::try_from_value(value).ok_or_else(|| RuntimeError::new("unhashable type"))
}

fn key_to_value(key: &ValueKey) -> Value {
    key.to_value()
}

/// The id of a pending promise, or None for any other value.
fn pending_promise_id(value: &Value) -> Option<u64> {
    let Value::Map(map) = value else {
        return None;
    };
    let map = map.borrow();
    match (map.entries.get("variant"), map.entries.get("id")) {
        (Some(Value::String(variant)), Some(Value::Int(id))) if variant.as_str() == "Pending" => {
            Some(*id as u64)
        }
        _ => None,
    }
}

/// How an uncaught exception is reported: the `message` of an error map or
/// object (after the class name for custom exceptions), or else the value.
/// `==` for NaN-boxed scalars: numbers compare by value across Int and Float
/// (`5 == 5.0`), anything else by its bits.
fn numbers_or_bits_equal(left: NanValue, right: NanValue) -> bool {
    match (left.is_f64(), right.is_f64()) {
        (true, true) => left.as_f64() == right.as_f64(),
        (true, false) => right
            .as_int()
            .is_some_and(|int| pleno_value::int_equals_float(int, left.as_f64())),
        (false, true) => left
            .as_int()
            .is_some_and(|int| pleno_value::int_equals_float(int, right.as_f64())),
        (false, false) => match (left.as_int(), right.as_int()) {
            // Large Ints are boxed, so equal ones can differ in their bits.
            (Some(a), Some(b)) => a == b,
            _ => left == right,
        },
    }
}

fn describe_exception(value: &Value) -> String {
    match value {
        Value::Map(map) => {
            if let Some(Value::String(message)) = map.borrow().entries.get("message") {
                return message.to_string();
            }
        }
        Value::Object(object) => {
            let object = object.borrow();
            if let Some(report) = object.error_report() {
                return report;
            }
            if let Some(Value::String(message)) = object.fields.get("message") {
                return match &object.class {
                    Some(class) => format!("{}: {}", class.borrow().name, message),
                    None => message.to_string(),
                };
            }
        }
        _ => {}
    }
    value.to_string()
}

/// Splits `text` on `sep` outside of `<>`, `()` and `[]`; `None` if `sep` never
/// occurs at the top level.
/// The shape check of a destructuring `let`: `let (a, b) = value;` needs a
/// tuple with two elements, `let [a, ...rest] = value;` an array with at least one.
fn check_destructure(value: &Value, mode: u8, count: usize) -> Result<(), RuntimeError> {
    let elements = |n: usize| {
        if n == 1 {
            "1 element".to_string()
        } else {
            format!("{n} elements")
        }
    };
    let type_name = || match value {
        Value::Object(object) => object
            .borrow()
            .class
            .as_ref()
            .map(|class| class.borrow().name.to_string())
            .unwrap_or_else(|| "Object".to_string()),
        other => other.type_name().to_string(),
    };
    match mode {
        pleno_bytecode::DESTRUCTURE_TUPLE => {
            let Value::Tuple(tuple) = value else {
                return Err(RuntimeError::new(format!(
                    "cannot destructure {} as a tuple",
                    type_name()
                )));
            };
            let len = tuple.borrow().elements.len();
            if len != count {
                return Err(RuntimeError::new(format!(
                    "tuple has {}, pattern expects {count}",
                    elements(len)
                )));
            }
        }
        _ => {
            let Value::Array(array) = value else {
                return Err(RuntimeError::new(format!(
                    "cannot destructure {} as an array",
                    type_name()
                )));
            };
            let len = array.borrow().elements.len();
            if mode == pleno_bytecode::DESTRUCTURE_ARRAY_AT_LEAST {
                if len < count {
                    return Err(RuntimeError::new(format!(
                        "array has {}, pattern expects at least {count}",
                        elements(len)
                    )));
                }
            } else if len < count {
                return Err(RuntimeError::new(format!(
                    "array has {}, pattern expects {count}",
                    elements(len)
                )));
            } else if len > count {
                return Err(RuntimeError::new(format!(
                    "array has {}, pattern expects {count} (add '...rest' or '...' to the \
                     pattern to allow more)",
                    elements(len)
                )));
            }
        }
    }
    Ok(())
}

/// The element types of a tuple type such as `(Int, String)`, or `None` for
/// anything else (a function type `(Int) => Int`, a grouping, a plain name).
fn tuple_type_elements(ty: &str) -> Option<Vec<String>> {
    let inner = ty.strip_prefix('(')?;
    // The '(' that opens the type must close at the very end.
    let mut depth = 1i32;
    let mut prev = ' ';
    for (index, ch) in inner.char_indices() {
        match ch {
            '(' | '[' | '<' => depth += 1,
            '>' if prev != '=' => depth -= 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            if index + ch.len_utf8() != inner.len() {
                return None;
            }
            let parts = split_top_level(&inner[..index], ',')?;
            return (parts.len() >= 2).then_some(parts);
        }
        prev = ch;
    }
    None
}

/// `value` as stored under the annotation `ty` (a string) after passing its
/// check: an `Int` becomes a `Float` where the annotation calls for one.
fn widen_to_annotation(value: Value, ty: &Value) -> Value {
    match (&value, ty) {
        (Value::Int(int), Value::String(ty)) if pleno_bytecode::annotation_widens_int(ty) => {
            Value::Float(*int as f64)
        }
        _ => value,
    }
}

fn split_top_level(text: &str, sep: char) -> Option<Vec<String>> {
    // Most annotations have no separator: answer without allocating.
    if !text.contains(sep) {
        return None;
    }
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut prev = ' ';
    for ch in text.chars() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            // `=>` in a function type is not a closing bracket.
            '>' if prev != '=' => depth -= 1,
            ')' | ']' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
                prev = ch;
                continue;
            }
            _ => {}
        }
        current.push(ch);
        prev = ch;
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(current.trim().to_string());
    Some(parts)
}

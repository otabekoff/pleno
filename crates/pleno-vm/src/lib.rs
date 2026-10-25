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
    pub gc: pleno_value::gc::GCManager,
    pub registers: Vec<NanValue>,
    pub frames: Vec<CallFrame>,
    pub globals: IndexMap<String, Value>,
    pub modules: IndexMap<String, Value>,
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
    pub run_depths: Vec<usize>,
    pub current_named_args: IndexMap<String, Value>,
    pub instruction_count: u64,
    pub event_loop: event_loop::EventLoop,
    pub debug_context: DebugContext,
    pub output: Option<String>,
    /// The built-in `Error` class, which runtime errors are instances of even
    /// when a program shadows the global `Error`.
    pub error_class: GcRc<Class>,
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
        let return_reg_idx = self.registers.len();
        // Execute until the callee frame returns
        let depth = self.frames.len();
        match callee {
            Value::Function(handle) => {
                self.call_function(
                    Value::Function(handle),
                    None,
                    &args,
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
                    &args,
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
                    &args,
                    return_reg_idx,
                    false,
                    call_opts,
                )?;
            }
            Value::NativeFunction(f) => {
                return f(self, &args);
            }
            Value::BoundNativeMethod(m) => {
                let m = m.borrow();
                let mut final_args = vec![m.receiver.clone()];
                final_args.extend(args);
                let result = (m.function)(self, &final_args)?;
                return Ok(result);
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call_ext(callable, args, opts);
                } else {
                    return Err(RuntimeError::new("Map is not callable"));
                }
            }
            Value::Object(obj) => {
                let callable = obj.borrow().fields.get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call_ext(callable, args, opts);
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
        let maybe_promise = self.registers.get(return_reg_idx).cloned();

        let sync_result = match self.run_until(depth)? {
            ExecutionStatus::Finished(v) => v,
            ExecutionStatus::Paused => {
                return Err(RuntimeError::new("Paused during call_function"));
            }
        };

        // If run_until returned Null (could be suspension), and we have a promise map, return the map.
        if let Some(p) = maybe_promise
            && sync_result == Value::Null
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
                return config
                    .allowed_read_paths
                    .iter()
                    .any(|p| path.starts_with(p));
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
                return config
                    .allowed_write_paths
                    .iter()
                    .any(|p| path.starts_with(p));
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
        if let Some(val) = self.globals.get(name) {
            return Ok(val.clone());
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
            gc,
            registers: vec![NanValue::null(); 256],
            frames: Vec::new(),
            globals,
            modules,
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
            run_depths: Vec::new(),
            current_named_args: IndexMap::new(),
            instruction_count: 0,
            event_loop: event_loop::EventLoop::new(),
            debug_context: DebugContext::new(),
            output: None,
            error_class,
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
            current_module: None,
            current_module_path: path,
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
                Err(err) if err.message.contains("uncaught exception") => return Err(err),
                Err(err) => {
                    // Let `catch` blocks see runtime errors, as normal execution does.
                    let unwound = self.throw_runtime_error(&err.message);
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

    fn generate_backtrace(&self) -> Vec<pleno_value::StackFrame> {
        let mut trace = Vec::new();
        for frame in self.frames.iter().rev() {
            if let Some(func) = self.functions.get(frame.function_index) {
                let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 };
                let span = func.chunk.spans.get(ip).copied();
                trace.push(pleno_value::StackFrame {
                    function_name: (*func.name).clone(),
                    span,
                });
            }
        }
        trace
    }

    /// `value.toString()` for built-in values: the same text `print` shows.
    fn bind_to_string(&mut self, receiver: Value) -> Result<Value, RuntimeError> {
        fn to_string(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
            let text = args.first().map(|v| v.to_string()).unwrap_or_default();
            Ok(Value::String(Rc::new(text)))
        }
        Ok(Value::BoundNativeMethod(self.gc.allocate(
            BoundNativeMethodState {
                receiver,
                function: to_string,
            },
        )?))
    }

    /// A method added to `receiver`'s built-in type with `extend`, bound to it.
    fn extension_method(
        &mut self,
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
        Ok(Some(Value::BoundMethod(self.gc.allocate(
            BoundMethodState {
                receiver: receiver.clone(),
                function: info.function,
                owner: info.owner,
                visibility: info.visibility,
            },
        )?)))
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
        match self.globals.get(ty).cloned() {
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
                    if err.message.contains("uncaught exception") {
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
    /// Runs instructions until the frame at `depth` returns, the program pauses
    /// or an instruction asks the caller to check its state (`Ok(None)`).
    fn run_step(&mut self, depth: usize) -> Result<Option<ExecutionStatus>, RuntimeError> {
        let mut first = true;
        loop {
            // The debugger's stepping commands look at each instruction.
            if !std::mem::take(&mut first) && self.debug_context.stepping {
                return Ok(None);
            }
            // Between instructions every live value is in a register, a frame or
            // another root, so this is where the collector may run.
            if self.gc.current_allocation >= self.gc.gc.allocation_threshold {
                let roots = VmRoots {
                    registers: &self.registers,
                    frames: &self.frames,
                    globals: &self.globals,
                    modules: &self.modules,
                    builtin_extensions: &self.builtin_extensions,
                    pending_tasks: &self.pending_tasks,
                    suspended_frames: &self.suspended_frames,
                    promises: &self.promises,
                    multi_waiters: &self.multi_waiters,
                };
                self.gc.maybe_collect(&[&roots]);
            }

            if self.frames.len() == depth {
                return Ok(Some(ExecutionStatus::Finished(Value::Null)));
            }

            let (func_index, ip) = {
                let frame = self
                    .frames
                    .last()
                    .ok_or_else(|| RuntimeError::new("no frame"))?;
                (frame.function_index, frame.ip)
            };

            if self
                .debug_context
                .should_pause(func_index, ip, &self.functions)
            {
                if let DebugState::Paused(_) = self.debug_context.state {
                    return Ok(Some(ExecutionStatus::Paused));
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
                        if let Some(bp) =
                            self.debug_context
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
                            return Ok(Some(ExecutionStatus::Paused));
                        }
                    }
                }
            }

            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| RuntimeError::new("no frame"))?;

            let function = &self.functions[frame.function_index];
            if ip >= function.chunk.code.len() {
                return Err(self.throw_runtime_error("program counter out of bounds"));
            }
            let instruction = function.chunk.code[ip];
            let opcode = instruction.opcode();

            log::trace!("VM::run_step: ip={:04} {:?}", ip, opcode);
            frame.ip += 1;

            self.instruction_count += 1;
            if self.instruction_count.is_multiple_of(100)
                && let Some(config) = &self.sandbox_config
            {
                if config.max_instructions > 0 && self.instruction_count > config.max_instructions {
                    return Err(
                        self.throw_runtime_error("SandboxError: instruction limit exceeded")
                    );
                }
                if config.max_memory > 0
                    && self.gc.allocated_bytes > config.max_memory + self.gc.grace_bytes
                {
                    return Err(self.throw_runtime_error("SandboxError: memory limit exceeded"));
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
                    let name = if c == 1 {
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
                                return Err(self.throw_runtime_error(&err.message));
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
                                return Err(self.throw_runtime_error(&err.message));
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
                    self.registers[base + a] = self.registers[base + b];
                }
                Opcode::LoadConst => {
                    let value = &function.chunk.constants[b];
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
                        module: proto
                            .chunk
                            .module_name
                            .as_ref()
                            .map(|name| Rc::new(name.clone())),
                        module_path: frame.current_module_path.as_ref().cloned(),
                        is_async: proto.is_async,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
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
                        module: proto
                            .chunk
                            .module_name
                            .as_ref()
                            .map(|name| Rc::new(name.clone())),
                        module_path: frame.current_module_path.as_ref().cloned(),
                        is_async: true,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
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
                        module: proto
                            .chunk
                            .module_name
                            .as_ref()
                            .map(|name| Rc::new(name.clone())),
                        module_path: frame.current_module_path.as_ref().cloned(),
                        is_async: proto.is_async,
                        is_pure: proto.is_pure,
                        type_params: proto.type_params.clone(),
                        type_constraints: proto.type_constraints.clone(),
                        annotations: proto.annotations.clone(),
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
                    self.registers[base + a] = NanValue::from_value(&Value::Range(Range {
                        start,
                        end,
                        inclusive: false,
                    }));
                }
                Opcode::BuildRangeInclusive => {
                    let start = expect_int_or_null(&self.registers[base + b].to_value())?;
                    let end = expect_int_or_null(&self.registers[base + c].to_value())?;
                    self.registers[base + a] = NanValue::from_value(&Value::Range(Range {
                        start,
                        end,
                        inclusive: true,
                    }));
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
                    let value = self.registers[base + a].to_value();
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
                                Value::Int(s.len() as i64)
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
                Opcode::AddInt => {
                    let val = match self.registers[base + b].as_int() {
                        Some(i) => i,
                        _ => return Err(RuntimeError::new("AddInt expects integer")),
                    };
                    let result = val
                        .checked_add(c as i64)
                        .ok_or_else(|| RuntimeError::new("integer overflow in addition"))?;
                    self.registers[base + a] = NanValue::from_int(result);
                }
                Opcode::SubInt => {
                    let val = match self.registers[base + b].as_int() {
                        Some(i) => i,
                        _ => return Err(RuntimeError::new("SubInt expects integer")),
                    };
                    let result = val
                        .checked_sub(c as i64)
                        .ok_or_else(|| RuntimeError::new("integer overflow in subtraction"))?;
                    self.registers[base + a] = NanValue::from_int(result);
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
                    let current_module = frame.current_module.as_ref().cloned();
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
                                if let Some(visibility) = class.borrow().fields.get(&name) {
                                    let class_borrowed = class.borrow();
                                    if !can_access_field(
                                        visibility,
                                        &class_borrowed.name,
                                        class_borrowed.module.as_ref(),
                                        current_class.as_ref(),
                                        current_module.as_ref(),
                                    ) {
                                        return Err(self.throw_runtime_error("field access denied"));
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
                            if let Some(visibility) =
                                class.borrow().static_field_visibility.get(&name)
                            {
                                let class_borrowed = class.borrow();
                                if !can_access_field(
                                    visibility,
                                    &class_borrowed.name,
                                    class_borrowed.module.as_ref(),
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static field access denied")
                                    );
                                }
                            }

                            let ty_name = class.borrow().static_field_types.get(&name).cloned();
                            if let Some(Some(ty_name)) = ty_name {
                                let ty_val = Value::String(Rc::new(ty_name));
                                if !self.is_instance(&value.to_value(), &ty_val) {
                                    let actual_type_name = value.type_name();
                                    let msg = format!(
                                        "TypeError: static field '{}' expected {}, got {}",
                                        name,
                                        ty_val.as_str().unwrap_or_default(),
                                        actual_type_name
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                            }

                            class
                                .borrow_mut()
                                .static_fields
                                .insert(name, value.to_value());
                        }
                        _ => return Err(RuntimeError::new("field set on non-object")),
                    }
                }
                Opcode::GetProperty => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
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
                                        .builtin_resolver
                                        .and_then(|r| r("Result", &name))
                                        .is_some()
                                {
                                    drop(map_ref);
                                    let func = self
                                        .builtin_resolver
                                        .and_then(|r| r("Result", &name))
                                        .unwrap();
                                    self.registers[base + a] =
                                        NanValue::from_value(&Value::BoundNativeMethod(
                                            self.gc.allocate(BoundNativeMethodState {
                                                receiver: Value::Map(map.clone()),
                                                function: func,
                                            })?,
                                        ));
                                    continue;
                                } else if matches!(
                                    variant.as_str(),
                                    "Pending" | "Resolved" | "Rejected"
                                ) && map_ref.get("id").is_some()
                                    && let Some(func) =
                                        self.builtin_resolver.and_then(|r| r("Promise", &name))
                                {
                                    drop(map_ref);
                                    self.registers[base + a] =
                                        NanValue::from_value(&Value::BoundNativeMethod(
                                            self.gc.allocate(BoundNativeMethodState {
                                                receiver: Value::Map(map.clone()),
                                                function: func,
                                            })?,
                                        ));
                                    continue;
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && self
                                        .builtin_resolver
                                        .and_then(|r| r("Option", &name))
                                        .is_some()
                                {
                                    drop(map_ref);
                                    let func = self
                                        .builtin_resolver
                                        .and_then(|r| r("Option", &name))
                                        .unwrap();
                                    self.registers[base + a] =
                                        NanValue::from_value(&Value::BoundNativeMethod(
                                            self.gc.allocate(BoundNativeMethodState {
                                                receiver: Value::Map(map.clone()),
                                                function: func,
                                            })?,
                                        ));
                                    continue;
                                }
                            }
                            let val = map_ref.entries.get(&name).cloned();
                            drop(map_ref);

                            if let Some(val) = val {
                                val
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Map", &name))
                            {
                                Value::BoundNativeMethod(self.gc.allocate(
                                    BoundNativeMethodState {
                                        receiver: Value::Map(map.clone()),
                                        function: func,
                                    },
                                )?)
                            } else if let Some(method) =
                                self.extension_method(&Value::Map(map.clone()), &name)?
                            {
                                method
                            } else if name == "toString" {
                                self.bind_to_string(Value::Map(map.clone()))?
                            } else {
                                // A missing key reads as null, like `map.get(key)`.
                                Value::Null
                            }
                        }
                        Value::Enum(enum_type) => {
                            let et = enum_type.borrow();
                            if let Some(def) = et.variants.get(&name) {
                                let val = if def.param_count == 0 {
                                    Value::Variant(self.gc.allocate(pleno_value::VariantState {
                                        enum_type: enum_type.clone(),
                                        variant_name: Rc::new(name),
                                        data: Vec::new(),
                                    })?)
                                } else {
                                    Value::VariantConstructor(self.gc.allocate(
                                        pleno_value::VariantConstructorState {
                                            enum_type: enum_type.clone(),
                                            variant_name: Rc::new(name),
                                        },
                                    )?)
                                };
                                self.registers[base + a] = NanValue::from_value(&val);
                                continue;
                            } else if let Some(info) = resolve_enum_static_method(&enum_type, &name)
                            {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
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
                            let field_value = object.borrow().fields.get(&name).cloned();
                            if let Some(class) = class.as_ref() {
                                if let Some(info) = resolve_getter(class, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
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
                                if let Some(v) = class.borrow().fields.get(&name) {
                                    if !can_access_field(
                                        v,
                                        &class.borrow().name,
                                        class.borrow().module.as_ref(),
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(self.throw_runtime_error("field access denied"));
                                    }
                                    field_value.unwrap_or(Value::Null)
                                } else if let Some(v) = field_value {
                                    v
                                } else if let Some(info) = resolve_method(class, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod(self.gc.allocate(BoundMethodState {
                                        receiver: Value::Object(object.clone()),
                                        function: info.function.clone(),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    })?)
                                } else if name == "toString" && class.borrow().extends_error() {
                                    // `Error: message`, as errors print.
                                    self.bind_to_string(Value::Object(object.clone()))?
                                } else {
                                    return Err(self.handle_property_error(&name, "Object"));
                                }
                            } else {
                                field_value.unwrap_or(Value::Null)
                            }
                        }
                        Value::Class(class) => {
                            if let Some(info) = resolve_static_getter(&class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
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
                            if let Some(v) = class.borrow().static_field_visibility.get(&name) {
                                if !can_access_field(
                                    v,
                                    &class.borrow().name,
                                    class.borrow().module.as_ref(),
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static field access denied")
                                    );
                                }
                                class
                                    .borrow()
                                    .static_fields
                                    .get(&name)
                                    .cloned()
                                    .unwrap_or(Value::Null)
                            } else if let Some(v) = class.borrow().static_fields.get(&name).cloned()
                            {
                                v
                            } else if let Some(info) = resolve_static_method(&class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
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
                                return Err(self.handle_property_error(&name, "Class"));
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
                            if resolve_enum_method(&variant.borrow().enum_type, &name)
                                .is_some() =>
                        {
                            let enum_type = variant.borrow().enum_type.clone();
                            let info = resolve_enum_method(&enum_type, &name)
                                .ok_or_else(|| RuntimeError::new("unknown enum method"))?;
                            if !can_access_method(
                                &info,
                                current_class.as_ref(),
                                frame.current_module.as_ref(),
                            ) {
                                return Err(self.throw_runtime_error("method access denied"));
                            }
                            Value::BoundMethod(self.gc.allocate(BoundMethodState {
                                receiver: Value::Variant(variant.clone()),
                                function: info.function.clone(),
                                owner: info.owner.clone(),
                                visibility: info.visibility,
                            })?)
                        }
                        other => {
                            if let Some(method) = self.extension_method(&other, &name)? {
                                method
                            } else if name == "toString"
                                && self
                                    .builtin_resolver
                                    .and_then(|r| r(other.type_name(), &name))
                                    .is_none()
                            {
                                self.bind_to_string(other)?
                            } else if let Some(func) = self
                                .builtin_resolver
                                .and_then(|r| r(other.type_name(), &name))
                            {
                                Value::BoundNativeMethod(self.gc.allocate(
                                    BoundNativeMethodState {
                                        receiver: other,
                                        function: func,
                                    },
                                )?)
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
                                            .is_some_and(|names| names.contains_key(&name)) =>
                                    {
                                        let t = t.borrow();
                                        let idx = t.names.as_ref().and_then(|n| n.get(&name));
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
                                        Value::Int(s.len() as i64)
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
                                            self.handle_property_error(&name, other.type_name())
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
                            let idx = *idx as usize;
                            if idx >= s.len() {
                                return Err(self.throw_runtime_error("string index out of bounds"));
                            }
                            let char = s.chars().nth(idx).unwrap_or('\0');
                            Ok(Value::String(Rc::new(char.to_string())))
                        }
                        (Value::String(s), Value::Range(range)) => {
                            let len = s.len();
                            let start = range.start.unwrap_or(0).max(0) as usize;
                            let end = range
                                .end
                                .map(|e| if range.inclusive { e + 1 } else { e })
                                .unwrap_or(len as i64)
                                .max(0) as usize;
                            let start = start.min(len);
                            let end = end.min(len).max(start);
                            // Rough byte slicing for now, should ideally be char-based but strings in pleno might be bytes/utf8
                            let sub_s = &s[start..end];
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
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        self.registers[base + a] =
                            crate::value_ops::add_nan(self, left_nan, right_nan)?;
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "+",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let res = crate::value_ops::add_values(self, left, right)?;
                    self.registers[base + a] = NanValue::from_value(&res);
                }
                Opcode::Sub => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        self.registers[base + a] = crate::value_ops::sub_nan(left_nan, right_nan)?;
                        continue;
                    }
                    let left = left_nan.to_value();
                    let right = right_nan.to_value();
                    if self.try_dispatch_operator(
                        base + a,
                        "-",
                        &left,
                        std::slice::from_ref(&right),
                    )? {
                        continue;
                    }
                    let res = crate::value_ops::sub_values(left, right)?;
                    self.registers[base + a] = NanValue::from_value(&res);
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
                    self.registers[base + a] =
                        NanValue::from_bool(!pleno_value::values_equal(&left, &right));
                }
                Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                    let left_nan = self.registers[base + b];
                    let right_nan = self.registers[base + c];
                    if (left_nan.is_f64() || left_nan.as_int().is_some())
                        && (right_nan.is_f64() || right_nan.as_int().is_some())
                    {
                        let res = crate::value_ops::compare_nan(opcode, left_nan, right_nan)?;
                        self.registers[base + a] = NanValue::from_bool(res);
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
                    let frame = self
                        .frames
                        .pop()
                        .ok_or_else(|| RuntimeError::new("no frame"))?;

                    // We need to look up the function to check if it is async
                    let function_is_async = self.functions[frame.function_index].is_async;
                    let function_arity = self.functions[frame.function_index].arity;

                    // A plain return to a caller in this run: copy the register
                    // without converting it to a `Value` and back.
                    if !function_is_async
                        && frame.return_override.is_none()
                        && self.frames.len() > depth
                    {
                        let result = self.registers[base + a];
                        self.release_registers(frame.base);
                        self.registers[frame.return_reg] = result;
                        continue;
                    }

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
                        Value::String(value) => Rc::clone(value),
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    let mut value = self.globals.get(name.as_str()).cloned();

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

                    // Data Breakpoint Check
                    if self.debug_context.data_breakpoints.contains_key(&name) {
                        self.globals.insert(name, val_obj);
                        self.debug_context.state = DebugState::Paused(PauseReason::Breakpoint);
                        return Ok(Some(ExecutionStatus::Paused));
                    }

                    self.globals.insert(name, val_obj);
                }
                Opcode::DefineClass => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("class name must be string")),
                    };
                    let module = frame.current_module.as_ref().map(Rc::clone);
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

                    let module = frame.current_module.as_ref().map(Rc::clone);
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
                                                self.globals.get(constraint_name)
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
                                                self.globals.get(constraint_name)
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
                                        let constraint_type =
                                            if let Some(val) = self.globals.get(constraint_name) {
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
                        class.borrow_mut().is_abstract = true;
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
                    let module = frame.current_module.as_ref().map(Rc::clone);
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
                    match self.registers[base + a].to_value() {
                        Value::Class(class) => {
                            class.borrow_mut().fields.insert(name, visibility);
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
                    if let Value::Class(class) = self.registers[base + a].to_value() {
                        class
                            .borrow_mut()
                            .static_field_visibility
                            .insert(name, visibility);
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
                    let args = self.registers[args_start..args_start + c]
                        .iter()
                        .map(|arg| arg.to_value())
                        .collect();
                    self.call_any_value(callee, args, base + a)?;
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
                    matches!(
                        (type_name, value),
                        ("Int", Value::Int(_)) | ("Float", Value::Float(_))
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

    /// The common call of a bytecode function or closure with positional
    /// arguments already in registers: copies them into the new frame without
    /// converting them to `Value`s. Returns `false` (doing nothing) for calls
    /// that need `call_any_value`: other callees, async functions, variadic or
    /// keyword-only parameters, and argument counts that are errors.
    fn try_call_regs(
        &mut self,
        callee: NanValue,
        args_start: usize,
        argc: usize,
        return_reg: usize,
    ) -> Result<bool, RuntimeError> {
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
        if function.is_async
            || argc > function.arity
            || argc > function.params.len()
            || function.params.iter().enumerate().any(|(index, param)| {
                param.is_variadic || param.is_keyword_only || (index >= argc && !param.has_default)
            })
        {
            return Ok(false);
        }
        let param_count = function.params.len();
        let register_count = function.chunk.max_registers.max(function.arity);
        let current_module = function.module.clone();
        let current_module_path = function.module_path.clone();

        let base = self.registers.len();
        self.registers
            .extend(std::iter::repeat_n(NanValue::null(), register_count));
        let copied = argc.min(register_count);
        self.registers
            .copy_within(args_start..args_start + copied, base);
        // Parameters left out take their default values.
        for index in argc..param_count.min(register_count) {
            self.registers[base + index] = NanValue::missing();
        }

        self.frames.push(CallFrame {
            function_index,
            ip: 0,
            base,
            return_reg,
            return_override: None,
            closure_state,
            current_owner: None,
            current_module,
            current_module_path,
            type_args: Vec::new(),
            exception_handlers: Vec::new(),
        });
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
        // Copy out what the call needs instead of cloning the whole function
        // (its chunk, constants and parameter list) on every call.
        let function = self
            .functions
            .get(function_index)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
        let current_module = function.module.clone();
        let current_module_path = function.module_path.clone();
        let function_arity = function.arity;
        let function_is_async = function.is_async;
        let max_registers = function.chunk.max_registers;
        let param_count = function.params.len();

        let processed_args = if args_already_processed {
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
            current_module,
            current_module_path,
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
                    return Err(self.throw_runtime_error(&err.message));
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
                        return Err(self.throw_runtime_error(&err.message));
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
                if class.borrow().is_abstract {
                    return Err(RuntimeError::new(format!(
                        "cannot instantiate abstract class '{}'",
                        class.borrow().name
                    )));
                }
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
                if class.borrow().is_abstract {
                    return Err(RuntimeError::new(format!(
                        "cannot instantiate abstract class '{}'",
                        class.borrow().name
                    )));
                }
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
    fn release_registers(&mut self, base: usize) {
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
        if source.opcode() != Opcode::GetProperty {
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
            .and_then(|frame| frame.current_module_path.as_ref())
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
        };
        let func_index = self.functions.len();
        self.functions.push(function);
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
            current_module: Some(Rc::clone(&module_rc)),
            current_module_path: Some(Rc::new(resolved_key.clone())),
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

fn split_top_level(text: &str, sep: char) -> Option<Vec<String>> {
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

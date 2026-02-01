use indexmap::{IndexMap, IndexSet};
use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    ArrayState, Class, ClosureState, EnumType, EnumVariantDef, MapState, MethodOwner, MultiWaiter,
    MultiWaiterMode, NativeFunction, Object, Range, RuntimeError, SetState, ThunkState, TupleState,
    VMProvider, Value, ValueKey,
    gc::{GcObject, GcRc},
};
// use std::collections::HashSet;
use std::fs;
use std::rc::Rc;

mod access;
pub mod event_loop;
mod models;
mod utils;
mod value_ops;

pub use access::*;
pub use event_loop::*;
pub use models::*;
pub use utils::*;
pub use value_ops::*;

// Core VM implementation

// Removed local SandboxConfig, utilizing pleno_value::SandboxConfig

pub struct VM {
    pub sandbox_config: Option<pleno_value::SandboxConfig>,
    pub gc: pleno_value::gc::GCManager,
    pub registers: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub globals: IndexMap<String, Value>,
    pub modules: IndexMap<String, Value>,
    pub functions: Vec<Function>,
    pub pending_tasks: Vec<AsyncTask>,
    pub suspended_frames: IndexMap<u64, SuspendedFrame>,
    pub promises: IndexMap<u64, Value>,
    pub next_promise_id: u64,
    pub multi_waiters: Vec<MultiWaiter>,
    pub builtin_resolver: Option<fn(&str, &str) -> Option<NativeFunction>>,
    pub run_depths: Vec<usize>,
    pub current_named_args: IndexMap<String, Value>,
    pub instruction_count: u64,
    pub event_loop: event_loop::EventLoop,
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
    pub type_params: Vec<String>,
    pub type_constraints: IndexMap<String, Vec<String>>,
    pub annotations: Vec<pleno_value::RuntimeAnnotation>,
}

impl VMProvider for VM {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
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
                    None,
                    None,
                    None,
                    false,
                    Vec::new(),
                )?;
            }
            Value::Closure(state) => {
                let borrowed = state.borrow();
                self.call_function(
                    Value::Function(borrowed.function),
                    None,
                    &args,
                    return_reg_idx,
                    None,
                    Some(state.clone()),
                    None,
                    false,
                    Vec::new(),
                )?;
            }
            Value::BoundMethod {
                receiver,
                function,
                owner,
                ..
            } => {
                self.call_function(
                    *function.clone(),
                    Some(*receiver.clone()),
                    &args,
                    return_reg_idx,
                    None,
                    None,
                    Some(owner),
                    false,
                    Vec::new(),
                )?;
            }
            Value::NativeFunction(f) => {
                return f(self, &args);
            }
            Value::BoundNativeMethod { receiver, function } => {
                let mut final_args = vec![*receiver.clone()];
                final_args.extend(args);
                let result = function(self, &final_args)?;
                return Ok(result);
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call(callable, args);
                } else {
                    return Err(RuntimeError::new("value is not callable"));
                }
            }
            Value::Object(obj) => {
                let callable = obj.borrow().fields.get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call(callable, args);
                } else {
                    return Err(RuntimeError::new("value is not callable"));
                }
            }
            _ => return Err(RuntimeError::new("value is not callable")),
        }

        let result = self.run_until(depth)?;
        Ok(result)
    }

    fn get_gc(&mut self) -> &mut pleno_value::gc::GCManager {
        &mut self.gc
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
        self.pending_tasks.push(AsyncTask {
            promise_id: id,
            value,
            callee: None,
            args: None,
            is_async: true,
            is_resolved: true,
        });
    }

    fn spawn(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let (id, promise) = self.create_promise()?;
        self.pending_tasks.push(AsyncTask {
            promise_id: id,
            value: Value::Null,
            callee: Some(callee),
            args: Some(args),
            is_async: true,
            is_resolved: false,
        });
        Ok(promise)
    }

    fn register_multi_waiter(&mut self, waiter: MultiWaiter) {
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

    fn add_timer(&mut self, delay_ms: u64, callback: Value, args: Vec<Value>) -> u64 {
        self.event_loop.add_timer(delay_ms, callback, args)
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
    pub fn new() -> Self {
        let gc = pleno_value::gc::GCManager::new(1000);
        let globals = IndexMap::new();
        let modules = IndexMap::new();

        Self {
            sandbox_config: None,
            gc,
            registers: vec![Value::Null; 256],
            frames: Vec::new(),
            globals,
            modules,
            functions: Vec::new(),
            pending_tasks: Vec::new(),
            suspended_frames: IndexMap::new(),
            promises: IndexMap::new(),
            next_promise_id: 1,
            multi_waiters: Vec::new(),
            builtin_resolver: None,
            run_depths: Vec::new(),
            current_named_args: IndexMap::new(),
            instruction_count: 0,
            event_loop: event_loop::EventLoop::new(),
        }
    }

    pub fn apply_config(&mut self, config: pleno_value::SandboxConfig) {
        self.gc.max_bytes = config.max_memory;
        self.sandbox_config = Some(config);
    }

    pub fn execute(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        let register_count = chunk.max_registers;

        let function = Function {
            name: Rc::new("<script>".to_string()),
            chunk,
            arity: 0,
            module: None,
            module_path: None,
            is_async: false,
            is_pure: false,
            type_params: Vec::new(),
            type_constraints: IndexMap::new(),
            params: Vec::new(),
            annotations: Vec::new(),
        };

        let func_index = self.functions.len();
        self.functions.push(function);

        self.registers.clear();

        self.registers
            .extend(std::iter::repeat_n(Value::Null, register_count));

        self.frames.push(CallFrame {
            function_index: func_index,
            ip: 0,
            base: 0,
            return_reg: 0,
            return_override: None,
            closure_state: None,
            current_owner: None,
            current_module: None,
            current_module_path: None,
            type_args: Vec::new(),
            exception_handlers: Vec::new(),
        });

        let mut last_result = Value::Null;
        let mut script_finished = false;

        loop {
            // 1. Run synchronous code until we hit a suspension point or finish
            if !self.frames.is_empty() {
                let result = match self.run_until(0) {
                    Ok(val) => val,
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
            if !self.event_loop.is_empty() {
                // Determine wait time (for now we might spin or sleep briefly,
                // but efficiently we should sleep until next timer)
                if let Some(wait_time) = self.event_loop.next_timeout() {
                    // In a real loop we'd sleep, but here we just tick if time passed?
                    // Or sleep thread?
                    // For testing, just sleeping is fine.
                    if !wait_time.is_zero() {
                        std::thread::sleep(wait_time);
                    }
                }

                if let Some(task) = self.event_loop.tick() {
                    // Convert EventLoop Task to VM pending task or call directly
                    // It's safer to push to pending_tasks to run on next loop iteration
                    self.pending_tasks.push(AsyncTask {
                        promise_id: task.promise_id.unwrap_or(0), // 0 or ignored if not a promise
                        value: Value::Null,                       // Or pass args?
                        callee: Some(task.callback),
                        args: Some(task.args),
                        is_async: true,
                        is_resolved: false,
                    });
                    continue;
                }
            }

            // 5. If no frames, no pending tasks, and event loop empty -> Done
            break;
        }
        Ok(last_result)
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

    fn handle_property_error(&self, name: &str) -> RuntimeError {
        if name == "len" || name == "length" {
            RuntimeError::new("array length on non-array value")
        } else if name == "append" {
            RuntimeError::new("array append on non-array value")
        } else if name == "add" {
            RuntimeError::new("set add on non-set value")
        } else if name == "has" {
            RuntimeError::new("set has on non-set value")
        } else if name == "keys" {
            RuntimeError::new("map keys on non-map value")
        } else if name == "prop"
            || name == "field"
            || (name.len() == 1 && name.chars().next().is_some_and(|c| c.is_ascii_lowercase()))
        {
            // Heuristic for tests or general field access
            RuntimeError::new("field access on non-object")
        } else {
            RuntimeError::new("property access on non-object")
        }
    }

    fn process_tasks(&mut self) -> Result<(), RuntimeError> {
        let tasks: Vec<_> = self.pending_tasks.drain(..).collect();
        for task in tasks {
            if !task.is_resolved {
                if let Some(callee) = task.callee {
                    let args = task.args.unwrap_or_default();
                    match self.call(callee, args) {
                        Ok(res) => self.resolve_promise(task.promise_id, res),
                        Err(e) => {
                            self.resolve_promise(
                                task.promise_id,
                                Value::String(Rc::new(e.message)),
                            );
                        }
                    }
                }
                continue;
            }
            // Update Promise map if it exists
            if let Some(Value::Map(map)) = self.promises.swap_remove(&task.promise_id) {
                {
                    let mut b = map.borrow_mut();
                    b.insert(
                        "variant".to_string(),
                        Value::String(Rc::new("Resolved".to_string())),
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
                    for cb_info_val in elements {
                        if let Value::Array(info) = cb_info_val {
                            let info_borrow = info.borrow();
                            let info_elements = &info_borrow.elements;
                            if info_elements.len() >= 2 {
                                let callback = info_elements[0].clone();
                                if let Value::Int(child_id) = info_elements[1] {
                                    // Execute callback(value) and resolve child promise
                                    match self.call(callback, vec![task.value.clone()]) {
                                        Ok(res) => self.resolve_promise(child_id as u64, res),
                                        Err(e) => {
                                            // Basic error propagation to child
                                            self.resolve_promise(
                                                child_id as u64,
                                                Value::String(Rc::new(e.message)),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(suspended) = self.suspended_frames.swap_remove(&task.promise_id) {
                let frame = suspended.frame;
                self.registers[frame.base + suspended.await_reg] = task.value.clone();
                self.frames.push(frame);
            }

            // Check multi-waiters
            let mut i = 0;
            let mut resolutions = Vec::new();
            while i < self.multi_waiters.len() {
                let mut waiter = self.multi_waiters[i].clone();
                let mut resolved = false;

                if let Some(pos) = waiter
                    .dependencies
                    .iter()
                    .position(|&id| id == task.promise_id)
                {
                    match waiter.mode {
                        MultiWaiterMode::All => {
                            waiter.results[pos] = task.value.clone();
                            waiter.resolved_count += 1;
                            if waiter.resolved_count == waiter.dependencies.len() {
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
        }
        Ok(())
    }

    fn run_until(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        self.run_depths.push(depth);
        let res = self.run_until_internal(depth);
        self.run_depths.pop();
        res
    }

    fn run_until_internal(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        loop {
            match self.run_step(depth) {
                Ok(Some(v)) => return Ok(v),
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

    fn run_step(&mut self, depth: usize) -> Result<Option<Value>, RuntimeError> {
        let roots = VmRoots {
            registers: &self.registers,
            frames: &self.frames,
            globals: &self.globals,
            modules: &self.modules,
            pending_tasks: &self.pending_tasks,
            suspended_frames: &self.suspended_frames,
            promises: &self.promises,
            multi_waiters: &self.multi_waiters,
        };
        self.gc.maybe_collect(&[&roots]);

        if self.frames.len() == depth {
            return Ok(Some(Value::Null));
        }

        loop {
            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| RuntimeError::new("no frame"))?;
            let instruction = self.functions[frame.function_index].chunk.code[frame.ip];
            let _opcode = instruction.opcode();
            // println!("VM::run_step: ip={:04} {:?}", frame.ip, _opcode);

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

            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| RuntimeError::new("no frame"))?;
            let function = self
                .functions
                .get(frame.function_index)
                .ok_or_else(|| RuntimeError::new("missing function"))?;
            if frame.ip >= function.chunk.code.len() {
                return Err(RuntimeError::new("instruction pointer out of bounds"));
            }
            let instruction = function.chunk.code[frame.ip];
            let opcode = instruction.opcode();
            frame.ip += 1;

            let a = instruction.a() as usize;
            let b = instruction.b() as usize;
            let c = instruction.c() as usize;
            let base = frame.base;

            match opcode {
                Opcode::GetIterator => {
                    let source = self.registers[base + b].clone();
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
                                self.registers[base + a] = iter;
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
                                None,
                                None,
                                Some(method_info.owner.clone()),
                                false,
                                Vec::new(),
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
                    let iter = self.registers[base + b].clone();
                    let name = "next".to_string();
                    let func_reg = base + a;

                    // Look up next method
                    if let Some(native) = self
                        .builtin_resolver
                        .and_then(|r| r(iter.type_name(), &name))
                    {
                        match native(self, &[iter]) {
                            Ok(val) => {
                                self.registers[base + a] = val;
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
                                None,
                                None,
                                Some(method_info.owner.clone()),
                                false,
                                Vec::new(),
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
                    self.registers[base + a] = self.registers[base + b].clone();
                }
                Opcode::LoadConst => {
                    let value = function.chunk.constants[b].clone();
                    self.registers[base + a] = value;
                }
                Opcode::LoadMissing => {
                    self.registers[base + a] = Value::Missing;
                }
                Opcode::LoadNull => {
                    self.registers[base + a] = Value::Null;
                }
                Opcode::LoadTrue => {
                    self.registers[base + a] = Value::Bool(true);
                }
                Opcode::LoadFalse => {
                    self.registers[base + a] = Value::Bool(false);
                }
                Opcode::LoadInt => {
                    let value = (b as i8) as i64;
                    self.registers[base + a] = Value::Int(value);
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
                    self.registers[base + a] = Value::Closure(state);
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
                    self.registers[base + a] = Value::Function(func_index);
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
                    self.registers[base + a] = Value::Function(func_index);
                }
                Opcode::Capture => {
                    let value = self.registers[base + b].clone();
                    if let Value::Closure(state) = &self.registers[base + a] {
                        state.borrow_mut().captures[c] = value;
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
                        self.registers[base + a] = state.captures[idx].clone();
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
                    let value = self.registers[base + a].clone();
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
                    let value = self.registers[base + b].clone();
                    let cell = self.gc.allocate(pleno_value::CellState::new(value))?;
                    self.registers[base + a] = Value::Cell(cell);
                }
                Opcode::LoadCell => {
                    let val = if let Value::Cell(cell) = &self.registers[base + b] {
                        cell.borrow().value.clone()
                    } else {
                        return Err(RuntimeError::new("LoadCell from non-cell value"));
                    };
                    self.registers[base + a] = val;
                }
                Opcode::SetCell => {
                    let value = self.registers[base + b].clone();
                    if let Value::Cell(cell) = &self.registers[base + a] {
                        cell.borrow_mut().value = value;
                    } else {
                        return Err(RuntimeError::new("SetCell on non-cell value"));
                    }
                }
                Opcode::MakeThunk => {
                    let callable = self.registers[base + b].clone();
                    let thunk = self.gc.allocate(ThunkState::new(callable))?;
                    self.registers[base + a] = Value::Thunk(thunk);
                }
                Opcode::ForceThunk => {
                    let value = self.registers[base + b].clone();
                    let forced = self.force_thunk(value)?;
                    self.registers[base + a] = forced;
                }
                Opcode::BuildRange => {
                    let start = expect_int_or_null(&self.registers[base + b])?;
                    let end = expect_int_or_null(&self.registers[base + c])?;
                    self.registers[base + a] = Value::Range(Range {
                        start,
                        end,
                        inclusive: false,
                    });
                }
                Opcode::BuildRangeInclusive => {
                    let start = expect_int_or_null(&self.registers[base + b])?;
                    let end = expect_int_or_null(&self.registers[base + c])?;
                    self.registers[base + a] = Value::Range(Range {
                        start,
                        end,
                        inclusive: true,
                    });
                }
                Opcode::NewMap => {
                    self.registers[base + a] =
                        Value::Map(self.gc.allocate(MapState::new(IndexMap::new()))?);
                }
                Opcode::NewArray => {
                    self.registers[base + a] =
                        Value::Array(self.gc.allocate(ArrayState::new(Vec::new()))?);
                }
                Opcode::NewSet => {
                    self.registers[base + a] =
                        Value::Set(self.gc.allocate(SetState::new(IndexSet::new()))?);
                }
                Opcode::SetAdd => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
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
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
                        Value::Set(set) => {
                            let key = value_to_key(&value)?;
                            let has = set.borrow().contains(&key);
                            self.registers[base + a] = Value::Bool(has);
                        }
                        _ => return Err(RuntimeError::new("set has on non-set value")),
                    }
                }
                Opcode::SetLen => match &self.registers[base + b] {
                    Value::Set(set) => {
                        let len = set.borrow().len() as i64;
                        self.registers[base + a] = Value::Int(len);
                    }
                    _ => return Err(RuntimeError::new("set len on non-set value")),
                },
                Opcode::SetValues => match &self.registers[base + b] {
                    Value::Set(set) => {
                        let values: Vec<Value> = set.borrow().iter().map(key_to_value).collect();
                        self.registers[base + a] =
                            Value::Array(self.gc.allocate(ArrayState::new(values))?);
                    }
                    _ => return Err(RuntimeError::new("set values on non-set value")),
                },
                Opcode::ArrayAppend => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
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
                    self.registers[base + a] = Value::Tuple(self.gc.allocate(tuple)?);
                }
                Opcode::TupleAppend => {
                    let val = self.registers[base + b].clone();
                    match &self.registers[base + a] {
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
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
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
                    let key_val = self.registers[base + b].clone();
                    let name = match key_val {
                        Value::String(s) => s.as_str().to_string(),
                        _ => return Err(RuntimeError::new("map remove key must be string")),
                    };
                    match &self.registers[base + a] {
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
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
                    self.registers[base + a] =
                        Value::Object(self.gc.allocate(Object::new(None, None))?);
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
                    let value = self.registers[base + a].clone();
                    match &self.registers[base + b] {
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
                                    let type_val = if let Some(val) = self.globals.get(&ty_name) {
                                        val.clone()
                                    } else {
                                        Value::String(Rc::new(ty_name.clone()))
                                    };

                                    if !self.is_instance(&value, &type_val) {
                                        let actual_type_name = value.type_name().to_string();
                                        let msg = format!(
                                            "Type error: field '{}' expected {}, got {}",
                                            name, ty_name, actual_type_name
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
                    let value = match &self.registers[base + b] {
                        Value::Object(object) => object
                            .borrow()
                            .fields
                            .get(&name)
                            .cloned()
                            .unwrap_or(Value::Null),
                        Value::Array(array) => {
                            if name == "length" {
                                Value::Int(array.borrow().len() as i64)
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
                    self.registers[base + a] = value;
                }
                Opcode::MapKeys => {
                    let keys = match &self.registers[base + b] {
                        Value::Map(map) => map
                            .borrow()
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
                        _ => return Err(RuntimeError::new("map keys on non-map value")),
                    };
                    self.registers[base + a] =
                        Value::Array(self.gc.allocate(ArrayState::new(keys))?);
                }
                Opcode::IterValues => {
                    let value = match &self.registers[base + b] {
                        Value::Array(array) => Ok(Value::Array(array.clone())),
                        Value::Map(map) => {
                            let values = map
                                .borrow()
                                .keys()
                                .cloned()
                                .map(|key| Value::String(Rc::new(key)))
                                .collect::<Vec<_>>();
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(values))?))
                        }
                        Value::Set(set) => {
                            let values = set
                                .borrow()
                                .elements
                                .iter()
                                .map(|key| key.to_value())
                                .collect::<Vec<_>>();
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(values))?))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    }?;
                    self.registers[base + a] = value;
                }
                Opcode::IterEntries => {
                    let value = match &self.registers[base + b] {
                        Value::Array(array) => {
                            let entries = array
                                .borrow()
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
                                        let val = val.to_value();
                                        Ok(Value::Array(self.gc.allocate(ArrayState::new(
                                            vec![Value::Int(idx as i64), val],
                                        ))?))
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                            Ok(Value::Array(self.gc.allocate(ArrayState::new(entries))?))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    }?;
                    self.registers[base + a] = value;
                }
                Opcode::ArrayLen => {
                    let len = match &self.registers[base + b] {
                        Value::Array(array) => array.borrow().elements.len() as i64,
                        Value::Tuple(tuple) => tuple.borrow().elements.len() as i64,
                        _ => {
                            return Err(RuntimeError::new("array length on non-array/tuple value"));
                        }
                    };
                    self.registers[base + a] = Value::Int(len);
                }
                Opcode::Await => {
                    let value = self.registers[base + b].clone();
                    // println!("Opcode::Await on value: {:?}", value);
                    match value {
                        Value::Map(map) => {
                            let variant = map.borrow().get("variant").cloned();
                            match variant {
                                Some(Value::String(s)) if s.as_str() == "Pending" => {
                                    let id = map
                                        .borrow()
                                        .get("id")
                                        .and_then(|v| match v {
                                            Value::Int(i) => Some(*i as u64),
                                            _ => None,
                                        })
                                        .unwrap_or(self.next_promise_id);

                                    let frame = self.frames.pop().unwrap();
                                    self.suspended_frames.insert(
                                        id,
                                        SuspendedFrame {
                                            frame,
                                            await_reg: a,
                                        },
                                    );
                                    return Ok(Some(Value::Null)); // Suspend
                                }
                                Some(Value::String(s)) if s.as_str() == "Resolved" => {
                                    let val =
                                        map.borrow().get("value").cloned().unwrap_or(Value::Null);
                                    self.registers[base + a] = val;
                                }
                                _ => {
                                    self.registers[base + a] = Value::Map(map);
                                }
                            }
                        }
                        _ => {
                            self.registers[base + a] = value;
                        }
                    }
                }
                Opcode::Is => {
                    let value = &self.registers[base + b];
                    let type_val = &self.registers[base + c];
                    let result = self.is_instance(value, type_val);
                    self.registers[base + a] = Value::Bool(result);
                }
                Opcode::TypeOf => {
                    let value = &self.registers[base + b];
                    let type_name = match value {
                        Value::Int(_) => "Int",
                        Value::Float(_) => "Float",
                        Value::Bool(_) => "Bool",
                        Value::String(_) => "String",
                        Value::Array(_) => "Array",
                        Value::Map(_) => "Map",
                        Value::Set(_) => "Set",
                        Value::Object(_) => "Object",
                        Value::Null => "Null",
                        Value::ArrayOp { .. } => "Array",
                        Value::Tuple(_) => "Tuple",
                        Value::Mutex(_) => "Mutex",
                        Value::Channel(_) => "Channel",
                        Value::File(_) => "File",
                        Value::Enum(_) => "Enum",
                        Value::Variant { .. } => "Variant",
                        _ => "Unknown",
                    };
                    self.registers[base + a] = Value::String(Rc::new(type_name.to_string()));
                }
                Opcode::AddInt => {
                    let val = match &self.registers[base + b] {
                        Value::Int(i) => *i,
                        _ => return Err(RuntimeError::new("AddInt expects integer")),
                    };
                    self.registers[base + a] = Value::Int(val + c as i64);
                }
                Opcode::SubInt => {
                    let val = match &self.registers[base + b] {
                        Value::Int(i) => *i,
                        _ => return Err(RuntimeError::new("SubInt expects integer")),
                    };
                    self.registers[base + a] = Value::Int(val - c as i64);
                }
                Opcode::MulInt => {
                    let val = match &self.registers[base + b] {
                        Value::Int(i) => *i,
                        _ => return Err(RuntimeError::new("MulInt expects integer")),
                    };
                    self.registers[base + a] = Value::Int(val * c as i64);
                }
                Opcode::DivInt => {
                    let val = match &self.registers[base + b] {
                        Value::Int(i) => *i,
                        _ => return Err(RuntimeError::new("DivInt expects integer")),
                    };
                    if c == 0 {
                        return Err(RuntimeError::new("division by zero"));
                    }
                    self.registers[base + a] = Value::Int(val / c as i64);
                }
                Opcode::ModInt => {
                    let val = match &self.registers[base + b] {
                        Value::Int(i) => *i,
                        _ => return Err(RuntimeError::new("ModInt expects integer")),
                    };
                    if c == 0 {
                        return Err(RuntimeError::new("division by zero"));
                    }
                    self.registers[base + a] = Value::Int(val % c as i64);
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
                    let value = self.registers[base + a].clone();
                    let current_owner = frame.current_owner.clone();
                    let current_class = match &current_owner {
                        Some(MethodOwner::Class(c)) => Some(c.clone()),
                        _ => None,
                    };
                    let current_module = frame.current_module.as_ref().cloned();
                    let target = self.registers[base + b].clone();
                    match target {
                        Value::Map(map) => {
                            if map.borrow().is_immutable {
                                return Err(self.throw_runtime_error("cannot modify immutable map"));
                            }
                            self.gc
                                .track_allocation(std::mem::size_of::<Value>() + name.len())?;
                            map.borrow_mut().insert(name, value);
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
                                        &[value],
                                        base + a,
                                        None,
                                        None,
                                        Some(info.owner.clone()),
                                        false,
                                        Vec::new(),
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
                                    let type_val = if let Some(val) = self.globals.get(&ty_name) {
                                        val.clone()
                                    } else {
                                        Value::String(Rc::new(ty_name.clone()))
                                    };

                                    if !self.is_instance(&value, &type_val) {
                                        let actual_type_name = value.type_name().to_string();
                                        let msg = format!(
                                            "Type error: field '{}' expected {}, got {}",
                                            name, ty_name, actual_type_name
                                        );
                                        return Err(self.throw_runtime_error(&msg));
                                    }
                                }
                            }
                            object.borrow_mut().fields.insert(name, value);
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
                                    &[value],
                                    base + a,
                                    None,
                                    None,
                                    Some(info.owner.clone()),
                                    false,
                                    Vec::new(),
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
                                let type_val = if let Some(val) = self.globals.get(&ty_name) {
                                    val.clone()
                                } else {
                                    Value::String(Rc::new(ty_name.clone()))
                                };

                                if !self.is_instance(&value, &type_val) {
                                    let actual_type_name = value.type_name().to_string();
                                    let msg = format!(
                                        "Type error: static field '{}' expected {}, got {}",
                                        name, ty_name, actual_type_name
                                    );
                                    return Err(self.throw_runtime_error(&msg));
                                }
                            }

                            class.borrow_mut().static_fields.insert(name, value);
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
                    let value = self.registers[base + b].clone();
                    let value = match value {
                        Value::Map(map) => {
                            let map_ref = map.borrow();
                            if let Some(Value::String(variant)) = map_ref.get("variant") {
                                if (variant.as_str() == "Ok" || variant.as_str() == "Err")
                                    && self
                                        .builtin_resolver
                                        .and_then(|r| r("Result", &name))
                                        .is_some()
                                {
                                    // Result type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: self
                                            .builtin_resolver
                                            .and_then(|r| r("Result", &name))
                                            .unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && self
                                        .builtin_resolver
                                        .and_then(|r| r("Option", &name))
                                        .is_some()
                                {
                                    // Option type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: self
                                            .builtin_resolver
                                            .and_then(|r| r("Option", &name))
                                            .unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Pending"
                                    || variant.as_str() == "Resolved")
                                    && self
                                        .builtin_resolver
                                        .and_then(|r| r("Promise", &name))
                                        .is_some()
                                {
                                    // Promise type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: self
                                            .builtin_resolver
                                            .and_then(|r| r("Promise", &name))
                                            .unwrap(),
                                    };
                                    continue;
                                } else if variant.as_str() == "Regex"
                                    && self
                                        .builtin_resolver
                                        .and_then(|r| r("Regex", &name))
                                        .is_some()
                                {
                                    // Regex type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: self
                                            .builtin_resolver
                                            .and_then(|r| r("Regex", &name))
                                            .unwrap(),
                                    };
                                    continue;
                                }
                            }
                            // Check for Map built-in methods if not found as a key
                            let val = map_ref.get(&name).cloned();
                            drop(map_ref);

                            if let Some(val) = val {
                                val
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Map", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Map(map.clone())),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Map")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Enum(enum_type) => {
                            let et = enum_type.borrow();
                            if let Some(def) = et.variants.get(&name) {
                                if def.param_count == 0 {
                                    self.registers[base + a] = Value::Variant {
                                        enum_type: enum_type.clone(),
                                        variant_name: Rc::new(name),
                                        data: Vec::new(),
                                    };
                                } else {
                                    self.registers[base + a] = Value::VariantConstructor {
                                        enum_type: enum_type.clone(),
                                        variant_name: Rc::new(name),
                                    };
                                }
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
                                self.registers[base + a] = Value::StaticMethod {
                                    function: Box::new(info.function.clone()),
                                    owner: info.owner.clone(),
                                    visibility: info.visibility,
                                };
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
                                        None,
                                        None,
                                        Some(info.owner.clone()),
                                        false,
                                        Vec::new(),
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
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(self.throw_runtime_error("field access denied"));
                                    }
                                    field_value.unwrap_or(Value::Null)
                                } else if let Some(value) = field_value {
                                    value
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
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Object(object.clone())),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
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
                                    None,
                                    None,
                                    Some(info.owner.clone()),
                                    false,
                                    Vec::new(),
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
                            } else if let Some(value) =
                                class.borrow().static_fields.get(&name).cloned()
                            {
                                value
                            } else if let Some(info) = {
                                let class_ref = &class;
                                resolve_static_method(class_ref, &name)
                            } {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                ) {
                                    return Err(
                                        self.throw_runtime_error("static method access denied")
                                    );
                                }
                                Value::StaticMethod {
                                    function: Box::new(info.function.clone()),
                                    owner: info.owner.clone(),
                                    visibility: info.visibility,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Set(set) => {
                            if name == "len" {
                                Value::Int(set.borrow().len() as i64)
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Set", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Set(set.clone())),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Set")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Set(set.clone())),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Array(array) => {
                            if name == "len" || name == "length" {
                                Value::Int(array.borrow().len() as i64)
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Array", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Array(array.clone())),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Array")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Array(array.clone())),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::String(s) => {
                            if name == "len" || name == "length" {
                                Value::Int(s.len() as i64)
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("String", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::String(s.clone())),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_String")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(RuntimeError::new("method access denied"));
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::String(s.clone())),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::ArrayOp { kind: _, array } => {
                            if name == "len" || name == "length" {
                                Value::Int(array.borrow().len() as i64)
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Array", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Array(array.clone())),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Mutex(m) => {
                            if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Mutex", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Mutex(m.clone())),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Channel(c) => {
                            if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Channel", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Channel(c.clone())),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Range(r) => match name.as_str() {
                            "start" => r.start.map(Value::Int).unwrap_or(Value::Null),
                            "end" => r.end.map(Value::Int).unwrap_or(Value::Null),
                            "inclusive" => Value::Bool(r.inclusive),
                            _ => {
                                if let Some(func) =
                                    self.builtin_resolver.and_then(|r| r("Range", &name))
                                {
                                    Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Range(r.clone())),
                                        function: func,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            }
                        },
                        Value::Char(c) => {
                            if let Some(func) = self.builtin_resolver.and_then(|r| r("Char", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Char(c)),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Tuple(tuple) => {
                            let t = tuple.borrow();
                            if let Ok(idx) = name.parse::<usize>() {
                                t.elements
                                    .get(idx)
                                    .cloned()
                                    .ok_or_else(|| self.handle_property_error(&name))?
                            } else if let Some(names) = &t.names {
                                if let Some(&idx) = names.get(&name) {
                                    t.elements
                                        .get(idx)
                                        .cloned()
                                        .ok_or_else(|| self.handle_property_error(&name))?
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::File(f) => {
                            if let Some(func) = self.builtin_resolver.and_then(|r| r("File", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::File(f.clone())),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Variant {
                            enum_type,
                            variant_name,
                            data,
                        } => {
                            if let Some(info) = resolve_enum_method(&enum_type, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                ) {
                                    return Err(self.throw_runtime_error("method access denied"));
                                }
                                self.registers[base + a] = Value::BoundMethod {
                                    receiver: Box::new(Value::Variant {
                                        enum_type: enum_type.clone(),
                                        variant_name: variant_name.clone(),
                                        data: data.clone(),
                                    }),
                                    function: Box::new(info.function.clone()),
                                    owner: info.owner.clone(),
                                    visibility: info.visibility,
                                };
                                continue;
                            } else if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Variant", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Variant {
                                        enum_type: enum_type.clone(),
                                        variant_name: variant_name.clone(),
                                        data: data.clone(),
                                    }),
                                    function: func,
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Int(i) => {
                            if let Some(func) = self.builtin_resolver.and_then(|r| r("Int", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Int(i)),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Int")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Int(i)),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Float(f) => {
                            if let Some(func) =
                                self.builtin_resolver.and_then(|r| r("Float", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Float(f)),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Float")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Float(f)),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        Value::Bool(b) => {
                            if let Some(func) = self.builtin_resolver.and_then(|r| r("Bool", &name))
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Bool(b)),
                                    function: func,
                                }
                            } else if let Some(Value::Class(cls)) =
                                self.globals.get("__Extension_Bool")
                            {
                                if let Some(info) = resolve_method(cls, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                    ) {
                                        return Err(
                                            self.throw_runtime_error("method access denied")
                                        );
                                    }
                                    Value::BoundMethod {
                                        receiver: Box::new(Value::Bool(b)),
                                        function: Box::new(info.function.clone()),
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    return Err(self.handle_property_error(&name));
                                }
                            } else {
                                return Err(self.handle_property_error(&name));
                            }
                        }
                        _ => {
                            return Err(self.handle_property_error(&name));
                        }
                    };
                    self.registers[base + a] = value;
                }
                Opcode::GetSuper => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("super property must be string")),
                    };
                    let receiver = match &self.registers[base + b] {
                        Value::Object(object) => object.clone(),
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
                    self.registers[base + a] = Value::BoundMethod {
                        receiver: Box::new(Value::Object(receiver)),
                        function: Box::new(info.function.clone()),
                        owner: info.owner.clone(),
                        visibility: info.visibility,
                    };
                }
                Opcode::CallSuper => {
                    // a = dest, b = this register, c = args register
                    let this_obj = match &self.registers[base + b] {
                        Value::Object(obj) => obj.clone(),
                        _ => return Err(RuntimeError::new("super() called on non-object")),
                    };
                    let superclass = {
                        let obj_borrow = this_obj.borrow();
                        obj_borrow
                            .class
                            .as_ref()
                            .and_then(|class| class.borrow().superclass.clone())
                            .ok_or_else(|| RuntimeError::new("superclass missing for super()"))?
                    };
                    let args = match &self.registers[base + c] {
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
                            Some(Value::Object(this_obj.clone())),
                            None,
                            Some(info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                    } else {
                        // No constructor, just return null
                        self.registers[base + a] = Value::Null;
                    }
                }
                Opcode::GetIndex => {
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    let result = match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
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
                        (Value::String(s), Value::Int(idx)) => {
                            let idx = idx as usize;
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
                            let idx = idx as usize;
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
                        (Value::Set(set), value) => {
                            let key = ValueKey::try_from_value(&value)
                                .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                            Ok(Value::Bool(set.borrow().contains(&key)))
                        }
                        _ => return Err(RuntimeError::new("invalid index operation")),
                    }?;
                    self.registers[base + a] = result;
                }
                Opcode::SetIndex => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(
                            self.throw_runtime_error("pure function cannot have side effects")
                        );
                    }
                    let value = self.registers[base + a].clone();
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
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
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("+").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(pleno_value::Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = add_values(self, left, right)?;
                }
                Opcode::Sub => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("-").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = sub_values(left, right)?;
                }
                Opcode::Mul => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("*").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = mul_values(left, right)?;
                }
                Opcode::Div => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("/").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = div_values(left, right)?;
                }
                Opcode::Mod => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("%").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = mod_values(left, right)?;
                }
                Opcode::Neg => {
                    let value = self.registers[base + b].clone();
                    self.registers[base + a] = neg_value(value)?;
                }
                Opcode::Not => {
                    let value = self.registers[base + b].clone();
                    self.registers[base + a] = Value::Bool(!is_truthy(&value));
                }
                Opcode::And => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] = Value::Bool(is_truthy(&left) && is_truthy(&right));
                }
                Opcode::Or => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] = Value::Bool(is_truthy(&left) || is_truthy(&right));
                }
                Opcode::Eq => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("==").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = Value::Bool(left == right);
                }
                Opcode::Ne => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    // Check for operator overloading on Object
                    if let Some(op_info) = match &left {
                        Value::Object(obj) => obj
                            .borrow()
                            .class
                            .as_ref()
                            .and_then(|c| c.borrow().operators.get("!=").cloned()),
                        _ => None,
                    } {
                        self.call_function(
                            op_info.function.clone(),
                            Some(Value::Object(match &left {
                                Value::Object(o) => o.clone(),
                                _ => unreachable!(),
                            })),
                            &[right],
                            base + a,
                            None,
                            None,
                            Some(op_info.owner.clone()),
                            false,
                            Vec::new(),
                        )?;
                        continue;
                    }
                    self.registers[base + a] = Value::Bool(left != right);
                }
                Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    let result = compare_values(opcode, left, right)?;
                    self.registers[base + a] = Value::Bool(result);
                }
                Opcode::Jump => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    frame.ip = apply_offset(frame.ip, offset)?;
                }
                Opcode::JumpIf => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    let value = self.registers[base + a].clone();
                    if is_truthy(&value) {
                        frame.ip = apply_offset(frame.ip, offset)?;
                    }
                }
                Opcode::JumpIfNot => {
                    let offset = decode_offset(instruction.b(), instruction.c());
                    let value = self.registers[base + a].clone();
                    if !is_truthy(&value) {
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

                    let result = frame
                        .return_override
                        .clone()
                        .unwrap_or_else(|| self.registers[base + a].clone());

                    if function_is_async {
                        // Promise ID is the last argument
                        if function_arity > 0 {
                            let promise_id_idx = frame.base + function_arity - 1;
                            if let Some(Value::Int(id)) = self.registers.get(promise_id_idx)
                                && let Ok(id) = (*id).try_into()
                            {
                                self.pending_tasks.push(AsyncTask {
                                    promise_id: id,
                                    value: result.clone(),
                                    callee: None,
                                    args: None,
                                    is_async: true,
                                    is_resolved: true,
                                });
                            }
                        }
                    }

                    if self.frames.len() > depth
                        || (self.suspended_frames.is_empty() && self.pending_tasks.is_empty())
                    {
                        self.registers.truncate(frame.base);
                    }

                    if self.frames.len() == depth {
                        return Ok(Some(result));
                    }

                    if !function_is_async {
                        self.registers[frame.return_reg] = result;
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
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    let mut value = self.globals.get(&name).cloned();

                    if value.is_none() {
                        // Check function type parameters
                        for (i, param) in function.type_params.iter().enumerate() {
                            if param == &name {
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
                                if param == &name {
                                    // First try to get from receiver (this)
                                    let receiver_val = &self.registers[frame.base];
                                    if let Value::Object(obj) = receiver_val
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

                    let value = value.unwrap_or(Value::Null);
                    // println!("VM: GetGlobal '{}' => {:?}", name, value);
                    self.registers[base + a] = value;
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
                    let value = self.registers[base + a].clone();
                    // println!("VM: SetGlobal '{}' => {:?}", name, value);
                    self.globals.insert(name, value);
                }
                Opcode::DefineClass => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("class name must be string")),
                    };
                    let module = frame.current_module.as_ref().map(Rc::clone);
                    let methods_map = match &self.registers[base + c] {
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

                    self.registers[base + a] = Value::Class(class);
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
                    let methods_val = self.registers[base + c].clone();
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

                    self.registers[base + a] =
                        Value::Interface(self.gc.allocate(pleno_value::Interface {
                            name,
                            methods,
                            getters,
                            setters,
                            type_params: Vec::new(),
                            type_constraints: IndexMap::new(),
                        })?);
                }
                Opcode::SetImplements => {
                    let interfaces_val = self.registers[base + b].clone();
                    let interfaces = match interfaces_val {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetImplements expects array of interfaces",
                            ));
                        }
                    };

                    let class_val = self.registers[base + a].clone();
                    match class_val {
                        Value::Class(class) => {
                            let mut validation_error = None;
                            // Validation Logic
                            'interface_loop: for interface_val in interfaces {
                                let interface = match &interface_val {
                                    Value::Interface(i) => i.clone(),
                                    Value::BoundInterface { interface, .. } => interface.clone(),
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
                    let class_val = self.registers[base + a].clone();
                    let field_name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let type_name = match &function.chunk.constants[c] {
                        Value::String(value) => Some(value.as_str().to_string()),
                        Value::Null => None,
                        _ => return Err(RuntimeError::new("type name must be string or null")),
                    };

                    if let Value::Class(class) = class_val {
                        class.borrow_mut().field_types.insert(field_name, type_name);
                    } else {
                        return Err(RuntimeError::new("SetFieldType on non-class value"));
                    }
                }
                Opcode::SetStaticFieldType => {
                    let class_val = self.registers[base + a].clone();
                    let field_name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let type_name = match &function.chunk.constants[c] {
                        Value::String(value) => Some(value.as_str().to_string()),
                        Value::Null => None,
                        _ => return Err(RuntimeError::new("type name must be string or null")),
                    };

                    if let Value::Class(class) = class_val {
                        class
                            .borrow_mut()
                            .static_field_types
                            .insert(field_name, type_name);
                    } else {
                        return Err(RuntimeError::new("SetStaticFieldType on non-class value"));
                    }
                }
                Opcode::SetTypeParams => {
                    let class_val = self.registers[base + a].clone();
                    let params_val = match &self.registers[base + b] {
                        Value::Array(arr) => arr.borrow().elements.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "SetTypeParams expects array of parameter names",
                            ));
                        }
                    };
                    let params = params_val
                        .into_iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>();

                    let constraints_map = match &self.registers[base + c] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => IndexMap::new(), // If reg c is not map (old bytecode?), or valid empty map
                    };

                    let mut type_constraints = IndexMap::new();
                    for (param, constraints_val) in constraints_map {
                        if let Value::Array(arr) = constraints_val {
                            let constraints = arr
                                .borrow()
                                .elements
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect::<Vec<_>>();
                            type_constraints.insert(param, constraints);
                        }
                    }

                    match class_val {
                        Value::Class(class) => {
                            class.borrow_mut().type_params = params;
                            class.borrow_mut().type_constraints = type_constraints;
                        }
                        Value::Interface(interface) => {
                            interface.borrow_mut().type_params = params;
                            interface.borrow_mut().type_constraints = type_constraints;
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "SetTypeParams on non-class/interface value",
                            ));
                        }
                    }
                }
                Opcode::SetOperators => {
                    let class_val = self.registers[base + a].clone();
                    let ops_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("SetOperators expects map")),
                    };

                    match class_val {
                        Value::Class(class) => {
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
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "SetOperators expects class in target register",
                            ));
                        }
                    }
                }
                Opcode::Specialize => {
                    let class_val = self.registers[base + b].clone();
                    let args = match &self.registers[base + c] {
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

                                for (i, param_name) in class_borrow.type_params.iter().enumerate() {
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

                            self.registers[base + a] = Value::BoundClass {
                                class,
                                type_args: args,
                            };
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

                                for (i, param_name) in
                                    interface_borrow.type_params.iter().enumerate()
                                {
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
                                                    param_name
                                                )));
                                            }
                                        }
                                    }
                                }
                            }

                            self.registers[base + a] = Value::BoundInterface {
                                interface,
                                type_args: args,
                            };
                        }
                        Value::BoundClass { class, .. } => {
                            self.registers[base + a] = Value::BoundClass {
                                class,
                                type_args: args,
                            };
                        }
                        Value::Function(handle) => {
                            let handle_idx = handle;
                            let func_proto = self.functions.get(handle_idx).cloned().unwrap();
                            if args.len() != func_proto.type_params.len() {
                                return Err(self.throw_runtime_error(&format!(
                                    "Generic function type mismatch: expected {} args, found {}",
                                    func_proto.type_params.len(),
                                    args.len()
                                )));
                            }

                            // Verify constraints
                            for (i, param_name) in func_proto.type_params.iter().enumerate() {
                                if let Some(constraints) =
                                    func_proto.type_constraints.get(param_name)
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
                                                param_name,
                                                func_proto.name
                                            )));
                                        }
                                    }
                                }
                            }

                            self.registers[base + a] = Value::BoundFunction {
                                function: Box::new(Value::Function(handle)),
                                type_args: args,
                            };
                        }

                        _ => return Err(RuntimeError::new("Specialize on non-class value")),
                    }
                }
                Opcode::Freeze => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (freeze)",
                        ));
                    }
                    let recursive = b != 0;
                    let value = self.registers[base + a].clone();
                    self.freeze_value(&value, recursive);
                }
                Opcode::SetSuper => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let superclass = match &self.registers[base + b] {
                        Value::Class(class) => class.clone(),
                        _ => return Err(RuntimeError::new("superclass must be class")),
                    };
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().superclass = Some(superclass);
                        }
                        _ => return Err(RuntimeError::new("set super on non-class value")),
                    }
                }
                Opcode::SetAbstract => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().is_abstract = true;
                        }
                        _ => return Err(RuntimeError::new("set abstract on non-class value")),
                    }
                }
                Opcode::SetStatic => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let static_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static methods must be map")),
                    };
                    let mut static_methods = IndexMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().static_methods = static_methods;
                        }
                        _ => return Err(RuntimeError::new("set static on non-class value")),
                    }
                }
                Opcode::SetGetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let getters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("getters must be map")),
                    };
                    let mut getters = IndexMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().getters = getters;
                        }
                        _ => return Err(RuntimeError::new("set getters on non-class value")),
                    }
                }
                Opcode::SetSetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let setters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("setters must be map")),
                    };
                    let mut setters = IndexMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().setters = setters;
                        }
                        _ => return Err(RuntimeError::new("set setters on non-class value")),
                    }
                }
                Opcode::SetStaticGetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let getters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static getters must be map")),
                    };
                    let mut static_getters = IndexMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().static_getters = static_getters;
                        }
                        _ => {
                            return Err(RuntimeError::new("set static getters on non-class value"));
                        }
                    }
                }
                Opcode::SetStaticSetters => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let setters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static setters must be map")),
                    };
                    let mut static_setters = IndexMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().static_setters = static_setters;
                        }
                        _ => {
                            return Err(RuntimeError::new("set static setters on non-class value"));
                        }
                    }
                }
                Opcode::DefineEnum => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(s) => s.clone(),
                        _ => return Err(RuntimeError::new("DefineEnum expects string constant")),
                    };
                    let module = frame.current_module.as_ref().map(Rc::clone);
                    let enum_type = self.gc.allocate(EnumType {
                        name,
                        variants: IndexMap::new(),
                        methods: IndexMap::new(),
                        static_methods: IndexMap::new(),
                        module,
                        annotations: Vec::new(),
                    })?;
                    self.registers[base + a] = Value::Enum(enum_type);
                }
                Opcode::AddVariant => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let enum_val = self.registers[base + a].clone();
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
                                },
                            );
                        }
                        _ => return Err(RuntimeError::new("AddVariant on non-enum value")),
                    }
                }
                Opcode::ConstructVariant => {
                    let enum_val = self.registers[base + b].clone();
                    let variant_name = match &function.chunk.constants[c] {
                        Value::String(s) => s.clone(),
                        _ => {
                            return Err(RuntimeError::new(
                                "ConstructVariant expects variant name string",
                            ));
                        }
                    };

                    match enum_val {
                        Value::Enum(enum_type) => {
                            self.registers[base + a] = Value::Variant {
                                enum_type: enum_type.clone(),
                                variant_name: variant_name.clone(),
                                data: Vec::new(),
                            };
                        }
                        _ => return Err(RuntimeError::new("ConstructVariant on non-enum value")),
                    }
                }
                Opcode::MatchVariant => {
                    let val = self.registers[base + b].clone();
                    let name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str(),
                        _ => return Err(RuntimeError::new("MatchVariant expects string constant")),
                    };

                    let is_match = match val {
                        Value::Variant { variant_name, .. } => variant_name.as_str() == name,
                        _ => false,
                    };
                    self.registers[base + a] = Value::Bool(is_match);
                }
                Opcode::AssertNotNull => {
                    let value = self.registers[base + b].clone();
                    if matches!(value, Value::Null) {
                        return Err(self.throw_runtime_error("unexpected null value"));
                    }
                    self.registers[base + a] = value;
                }
                Opcode::GetVariantField => {
                    let val = self.registers[base + b].clone();
                    let index = instruction.c() as usize;

                    match val {
                        Value::Variant { data, .. } => {
                            if index < data.len() {
                                self.registers[base + a] = data[index].clone();
                            } else {
                                return Err(RuntimeError::new("Variant index out of bounds"));
                            }
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
                    let func_val = self.registers[base + a].clone();
                    let target_val = self.registers[base + b].clone();
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
                        Value::String(s) => {
                            // Primitive extension via shadow class
                            let shadow_name = format!("__Extension_{}", s.as_str());
                            let class =
                                if let Some(Value::Class(cls)) = self.globals.get(&shadow_name) {
                                    cls.clone()
                                } else {
                                    let cls = self.gc.allocate(Class {
                                        name: Rc::new(shadow_name.clone()),
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
                                        module: None,
                                        is_abstract: false,
                                        is_pooled: false,
                                        implements: Vec::new(),
                                        type_params: Vec::new(),
                                        type_constraints: IndexMap::new(),
                                        annotations: Vec::new(),
                                    })?;
                                    self.globals.insert(shadow_name, Value::Class(cls.clone()));
                                    cls
                                };
                            let module = class.borrow().module.clone();
                            let info = parse_method_info(
                                func_val,
                                MethodOwner::Class(class.clone()),
                                module.as_ref(),
                            )?;
                            class.borrow_mut().methods.insert(name, info);
                        }
                        _ => return Err(RuntimeError::new("AddMethod on invalid target")),
                    }
                }
                Opcode::AddStaticMethod => {
                    if self.functions[frame.function_index].is_pure {
                        return Err(self.throw_runtime_error(
                            "pure function cannot have side effects (class mutation)",
                        ));
                    }
                    let func_val = self.registers[base + a].clone();
                    let target_val = self.registers[base + b].clone();
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
                    match &mut self.registers[base + a] {
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
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            class
                                .borrow_mut()
                                .static_field_visibility
                                .insert(name, visibility);
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "set static field visibility on non-class value",
                            ));
                        }
                    }
                }
                Opcode::SetAnnotations => {
                    let annotations = self.registers[base + b].clone();
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            if let Value::Array(arr) = annotations {
                                let mut annos = Vec::new();
                                for val in &arr.borrow().elements {
                                    if let Value::Object(obj) = val {
                                        let b = obj.borrow();
                                        if let Some(Value::String(name)) = b.fields.get("name")
                                            && let Some(Value::Array(args)) = b.fields.get("args")
                                        {
                                            annos.push(pleno_value::RuntimeAnnotation {
                                                name: name.to_string(),
                                                args: args.borrow().elements.clone(),
                                            });
                                        }
                                    } else if let Value::Map(map) = val {
                                        let b = map.borrow();
                                        if let Some(Value::String(name)) = b.entries.get("name")
                                            && let Some(Value::Array(args)) = b.entries.get("args")
                                        {
                                            annos.push(pleno_value::RuntimeAnnotation {
                                                name: name.to_string(),
                                                args: args.borrow().elements.clone(),
                                            });
                                        }
                                    }
                                }
                                class.borrow_mut().annotations = annos;
                            }
                        }
                        Value::Enum(enum_type) => {
                            if let Value::Array(arr) = annotations {
                                let mut annos = Vec::new();
                                for val in &arr.borrow().elements {
                                    if let Value::Object(obj) = val {
                                        let b = obj.borrow();
                                        if let Some(Value::String(name)) = b.fields.get("name")
                                            && let Some(Value::Array(args)) = b.fields.get("args")
                                        {
                                            annos.push(pleno_value::RuntimeAnnotation {
                                                name: name.to_string(),
                                                args: args.borrow().elements.clone(),
                                            });
                                        }
                                    } else if let Value::Map(map) = val {
                                        let b = map.borrow();
                                        if let Some(Value::String(name)) = b.entries.get("name")
                                            && let Some(Value::Array(args)) = b.entries.get("args")
                                        {
                                            annos.push(pleno_value::RuntimeAnnotation {
                                                name: name.to_string(),
                                                args: args.borrow().elements.clone(),
                                            });
                                        }
                                    }
                                }
                                enum_type.borrow_mut().annotations = annos;
                            }
                        }
                        value => {
                            return Err(RuntimeError::new(format!(
                                "SetAnnotations expects Class or Enum, got {}",
                                value.type_name()
                            )));
                        }
                    }
                }
                Opcode::Call => {
                    let callee = self.registers[base + b].clone();
                    let args = match &self.registers[base + c] {
                        Value::Array(array) => array.borrow().elements.clone(),
                        _ => return Err(RuntimeError::new("call expects args array")),
                    };
                    self.call_any_value(callee, args, base + a)?;
                }

                Opcode::CallNamed => {
                    let callee = self.registers[base + b].clone();
                    let wrapper = match &self.registers[base + c] {
                        Value::Array(array) => array.borrow().clone(),
                        _ => return Err(RuntimeError::new("call named expects args array")),
                    };
                    if wrapper.len() != 2 {
                        return Err(RuntimeError::new("call named expects [positional, named]"));
                    }
                    let positional = match &wrapper[0] {
                        Value::Array(array) => array.borrow().elements.clone(),
                        _ => return Err(RuntimeError::new("call named positional must be array")),
                    };
                    let named = match &wrapper[1] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("call named named must be map")),
                    };
                    match callee {
                        Value::Partial { .. } => {
                            return Err(RuntimeError::new("partial named calls not supported yet"));
                        }
                        _ => {
                            self.call_with_named(callee, positional, named, base + a)?;
                        }
                    }
                }
                Opcode::Partial => {
                    let callee = self.registers[base + b].clone();
                    if !self.is_callable(&callee) {
                        return Err(RuntimeError::new("partial callee must be a function"));
                    }
                    let args_meta = match &self.registers[base + c] {
                        Value::Array(arr) => arr.borrow().clone(),
                        _ => return Err(RuntimeError::new("partial metadata must be an array")),
                    };
                    let mut args = Vec::new();
                    let mut placeholders = Vec::new();
                    for (idx, val) in args_meta.into_iter().enumerate() {
                        if val == Value::Missing {
                            placeholders.push(idx);
                        }
                        args.push(val);
                    }
                    self.registers[base + a] = Value::Partial {
                        callee: Box::new(callee),
                        args,
                        placeholders,
                    };
                }
                Opcode::Import => {
                    let path = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("import path must be string")),
                    };
                    let module_value = self.execute_module(&path)?;
                    self.registers[base + a] = module_value;
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
                    let error_value = self.registers[base + a].clone();
                    self.unwind_to_handler(error_value)?;
                    continue;
                }
                Opcode::SetPooled => {
                    let is_pooled = b != 0;
                    match &self.registers[base + a] {
                        Value::Class(class) => {
                            class.borrow_mut().is_pooled = is_pooled;
                        }
                        _ => return Err(RuntimeError::new("SetPooled expects class")),
                    }
                }
                Opcode::Slice => {
                    let collection = self.registers[base + b].clone();
                    // According to Opcode::Slice emission in compiler:
                    // self.emit(Opcode::Slice, dest, val_reg, wrapper_reg);
                    // wrapper_reg has [start, end]
                    let wrapper = match &self.registers[base + c] {
                        Value::Array(arr) => arr.borrow().clone(),
                        _ => return Err(RuntimeError::new("slice expects wrapper array")),
                    };

                    if wrapper.len() != 2 {
                        return Err(RuntimeError::new("slice wrapper must have 2 elements"));
                    }
                    let start = expect_int_or_null(&wrapper[0])?;
                    let end = expect_int_or_null(&wrapper[1])?;

                    match collection {
                        Value::Array(array) => {
                            let array = array.borrow();
                            let len = array.elements.len() as i64;
                            let start = resolve_index(start, len, 0);
                            let end = resolve_index(end, len, len);
                            let slice = array.elements[start..end].to_vec();
                            self.registers[base + a] =
                                Value::Array(self.gc.allocate(ArrayState::new(slice))?);
                        }
                        Value::Tuple(tuple) => {
                            let t = tuple.borrow();
                            let len = t.elements.len() as i64;
                            let start = resolve_index(start, len, 0);
                            let end = resolve_index(end, len, len);
                            let slice = t.elements[start..end].to_vec();
                            let new_tuple = TupleState::new(slice, None);
                            self.registers[base + a] = Value::Tuple(self.gc.allocate(new_tuple)?);
                        }
                        Value::String(s) => {
                            let len = s.len() as i64;
                            let start = resolve_index(start, len, 0);
                            let end = resolve_index(end, len, len);
                            let slice = s[start..end].to_string();
                            self.registers[base + a] = Value::String(Rc::new(slice));
                        }
                        _ => return Err(RuntimeError::new("slice expects array or string")),
                    }
                }
            }
            return Ok(None);
        }
    }

    fn resolve_type_param(&self, name: &str) -> Option<Value> {
        let frame = self.frames.last()?;
        let function = &self.functions[frame.function_index];

        // Check function type parameters
        for (i, param) in function.type_params.iter().enumerate() {
            if param == name {
                if i < frame.type_args.len() {
                    return Some(frame.type_args[i].clone());
                }
                break;
            }
        }

        // Check class type parameters
        if let Some(MethodOwner::Class(cls)) = &frame.current_owner {
            let cls = cls.borrow();
            for (i, param) in cls.type_params.iter().enumerate() {
                if param == name {
                    // First try to get from receiver (this)
                    if frame.base < self.registers.len() {
                        let receiver_val = &self.registers[frame.base];
                        if let Value::Object(obj) = receiver_val
                            && let Some(type_args) = &obj.borrow().type_args
                            && i < type_args.len()
                        {
                            return Some(type_args[i].clone());
                        }
                    }

                    // Fallback to frame type args if receiver didn't work
                    // (e.g. constructor call where frame args are class args)
                    if function.type_params.is_empty() && i < frame.type_args.len() {
                        return Some(frame.type_args[i].clone());
                    }
                    break;
                }
            }
        }
        None
    }

    pub fn is_instance(&self, value: &Value, type_val: &Value) -> bool {
        match type_val {
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
                    matches!(
                        (s, value),
                        ("Int", Value::Int(_))
                            | ("Float", Value::Float(_))
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
                            | ("Null", Value::Null)
                            | ("Char", Value::Char(_))
                            | ("Any", _)
                    )
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
                _ => false,
            },
            Value::BoundClass {
                class: target_class,
                type_args: target_args,
            } => match value {
                Value::Object(obj) => {
                    let borrowed = obj.borrow();
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
                            if instance_args.len() != target_args.len() {
                                return false;
                            }
                            for (a, b) in instance_args.iter().zip(target_args.iter()) {
                                if !self.check_type_compatible(a, b) {
                                    return false;
                                }
                            }
                            true
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
                                    Value::BoundInterface { interface, .. } => interface,
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
            Value::BoundInterface {
                interface: target_iface,
                type_args: target_args,
            } => match value {
                Value::Object(obj) => {
                    let obj = obj.borrow();
                    let iface = target_iface.borrow();

                    if let Some(cls) = &obj.class {
                        let mut current = Some(cls.clone());
                        while let Some(c) = current {
                            for imp in &c.borrow().implements {
                                match imp {
                                    Value::BoundInterface {
                                        interface,
                                        type_args,
                                    } if interface.borrow().name == iface.name => {
                                        if type_args.len() != target_args.len() {
                                            continue;
                                        }
                                        let mut args_match = true;
                                        for (a, b) in type_args.iter().zip(target_args.iter()) {
                                            if !self.check_type_compatible(a, b) {
                                                args_match = false;
                                                break;
                                            }
                                        }
                                        if args_match {
                                            return true;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            current = c.borrow().superclass.clone();
                        }
                    }
                    false
                }
                _ => false,
            },
            _ => false,
        }
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
                        if tp_name == s.as_str() && i < frame.type_args.len() {
                            return self.check_type_compatible(val_type, &frame.type_args[i]);
                        }
                    }
                    // Then check owner class's type_params
                    if let Some(MethodOwner::Class(cls)) = &frame.current_owner {
                        let cls_ref = cls.borrow();
                        for (i, tp_name) in cls_ref.type_params.iter().enumerate() {
                            if tp_name == s.as_str() && i < frame.type_args.len() {
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
                Value::BoundClass { class, .. } => {
                    let mut current = Some(class.clone());
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
                Value::Class(cls) | Value::BoundClass { class: cls, .. } => {
                    let iface = target_iface.borrow();
                    let mut current = Some(cls.clone());
                    while let Some(c) = current {
                        for imp in &c.borrow().implements {
                            let imp_iface = match imp {
                                Value::Interface(i) => i.clone(),
                                Value::BoundInterface { interface, .. } => interface.clone(),
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
            Value::BoundInterface {
                interface: target_iface,
                type_args: target_args,
            } => match val_type {
                Value::BoundInterface {
                    interface: instance_iface,
                    type_args: instance_args,
                } => {
                    if instance_iface.as_ptr() != target_iface.as_ptr() {
                        return false;
                    }
                    if instance_args.len() != target_args.len() {
                        return false;
                    }
                    for (a, b) in instance_args.iter().zip(target_args.iter()) {
                        if !self.check_type_compatible(a, b) {
                            return false;
                        }
                    }
                    true
                }
                Value::Class(cls) | Value::BoundClass { class: cls, .. } => {
                    let iface = target_iface.borrow();
                    let mut current = Some(cls.clone());
                    while let Some(c) = current {
                        for imp in &c.borrow().implements {
                            match imp {
                                Value::BoundInterface {
                                    interface,
                                    type_args,
                                } if interface.borrow().name == iface.name => {
                                    if type_args.len() != target_args.len() {
                                        continue;
                                    }
                                    let mut match_args = true;
                                    for (a, b) in type_args.iter().zip(target_args.iter()) {
                                        if !self.check_type_compatible(a, b) {
                                            match_args = false;
                                            break;
                                        }
                                    }
                                    if match_args {
                                        return true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        current = c.borrow().superclass.clone();
                    }
                    false
                }
                _ => false,
            },
            Value::BoundClass {
                class: target_class,
                type_args: target_args,
            } => match val_type {
                Value::BoundClass {
                    class: instance_class,
                    type_args: instance_args,
                } => {
                    if instance_class.as_ptr() != target_class.as_ptr() {
                        return false;
                    }
                    if instance_args.len() != target_args.len() {
                        return false;
                    }
                    for (a, b) in instance_args.iter().zip(target_args.iter()) {
                        if !self.check_type_compatible(a, b) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call_function(
        &mut self,
        callee: Value,
        receiver: Option<Value>,
        args: &[Value],
        return_reg_idx: usize,
        return_override: Option<Value>,
        mut closure_state: Option<GcRc<pleno_value::ClosureState>>,
        current_owner: Option<MethodOwner>,
        args_already_processed: bool,
        type_args: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let function_index = match callee {
            Value::Function(idx) => idx,
            Value::Closure(c) => {
                let idx = c.borrow().function;
                closure_state = Some(c.clone());
                idx
            }
            _ => return Err(RuntimeError::new("expected function or closure")),
        };
        let function = self
            .functions
            .get(function_index)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?
            .clone();
        let current_module = function.module.clone();
        let current_module_path = function.module_path.clone();

        let processed_args = if args_already_processed {
            args.to_vec()
        } else {
            let mut final_args = Vec::new();
            if let Some(recv) = receiver.clone() {
                final_args.push(recv);
            }
            final_args.extend(args.iter().cloned());

            let has_variadic = function
                .params
                .last()
                .map(|p| p.is_variadic)
                .unwrap_or(false);

            if !has_variadic && final_args.len() > function.arity {
                return Err(self.throw_runtime_error("too many positional arguments"));
            }

            let mut processed = Vec::new();
            let mut arg_idx = 0;

            if function.is_async {
                // ... (rest of async logic)
                let promise_id = self.next_promise_id;
                self.next_promise_id += 1;
                final_args.push(Value::Int(promise_id as i64));
                let mut promise_map = IndexMap::new();
                promise_map.insert("id".to_string(), Value::Int(promise_id as i64));
                promise_map.insert(
                    "variant".to_string(),
                    Value::String(Rc::new("Pending".to_string())),
                );
                promise_map.insert("value".to_string(), Value::Null);
                let promise = Value::Map(self.gc.allocate(MapState::new(promise_map))?);
                self.promises.insert(promise_id, promise.clone());
                if return_reg_idx >= self.registers.len() {
                    self.registers.resize(return_reg_idx + 1, Value::Null);
                }
                self.registers[return_reg_idx] = promise;
            }

            for param in function.params.iter() {
                if param.is_variadic {
                    let rest = final_args[arg_idx..].to_vec();
                    processed.push(Value::Array(self.gc.allocate(ArrayState::new(rest))?));
                    arg_idx = final_args.len();
                    break;
                }

                if param.is_keyword_only {
                    if arg_idx < final_args.len() {
                        return Err(self.throw_runtime_error("keyword-only argument requires name"));
                    }
                    if param.has_default {
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
                } else if param.has_default {
                    processed.push(Value::Missing);
                } else {
                    return Err(self.throw_runtime_error("missing required argument"));
                }
            }

            if arg_idx < final_args.len() {
                return Err(self.throw_runtime_error("too many positional arguments"));
            }
            // println!("Calling function. is_async: {}, arity: {}, params: {}", function.is_async, function.arity, function.params.len());

            processed
        };

        let base = self.registers.len();
        let register_count = function.chunk.max_registers.max(function.arity);
        self.registers
            .extend(std::iter::repeat_n(Value::Null, register_count));

        for (idx, arg) in processed_args.iter().enumerate() {
            if idx < register_count {
                self.registers[base + idx] = arg.clone();
            }
        }

        self.frames.push(CallFrame {
            function_index, // Use function_index directly
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
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::BoundNativeMethod { receiver, function } => {
                let prev_named = std::mem::replace(&mut self.current_named_args, named);
                let mut merged_args = vec![*receiver.clone()];
                merged_args.extend(positional);
                let result = function(self, &merged_args);
                self.current_named_args = prev_named;
                let result = result?;
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::Class(class) => {
                let object = self.gc.allocate(Object::new(Some(class.clone()), None))?;
                if let Some(handle) = class.borrow().constructor.clone() {
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
                        override_value,
                        None,
                        Some(MethodOwner::Class(class.clone())),
                        true,
                        Vec::new(),
                    )
                } else if positional.is_empty() && named.is_empty() {
                    self.registers[return_reg] = Value::Object(object);
                    Ok(())
                } else {
                    Err(RuntimeError::new("constructor not defined for named args"))
                }
            }
            Value::BoundClass { class, type_args } => {
                let object = self
                    .gc
                    .allocate(Object::new(Some(class.clone()), Some(type_args.clone())))?;
                if let Some(handle) = class.borrow().constructor.clone() {
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
                        override_value,
                        None,
                        Some(MethodOwner::Class(class.clone())),
                        true,
                        Vec::new(),
                    )
                } else if positional.is_empty() && named.is_empty() {
                    self.registers[return_reg] = Value::Object(object);
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
                    None,
                    None,
                    None,
                    true,
                    Vec::new(),
                )
            }
            Value::BoundMethod {
                receiver,
                function,
                owner,
                ..
            } => {
                let func_val = *function.clone();
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
                let mut final_args = vec![*receiver.clone()];
                final_args.extend(args);
                self.call_function(
                    func_val,
                    Some(*receiver.clone()),
                    &final_args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    true,
                    Vec::new(),
                )
            }
            Value::StaticMethod {
                function, owner, ..
            } => {
                let func_val = *function.clone();
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
                    None,
                    None,
                    Some(owner),
                    true,
                    Vec::new(),
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
                    None,
                    Some(state.clone()),
                    None,
                    true,
                    Vec::new(),
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
        let stop_depth = self.run_depths.last().cloned().unwrap_or(0);
        while self.frames.len() > stop_depth {
            let mut frame = self.frames.pop().unwrap();
            if let Some(handler) = frame.exception_handlers.pop() {
                frame.ip = handler.catch_ip;
                self.registers[handler.base + handler.error_reg] = error_value;
                self.frames.push(frame);
                return Err(RuntimeError::handled());
            }
            self.registers.truncate(frame.base);
        }
        Err(RuntimeError::new(format!(
            "uncaught exception: {}",
            error_value
        )))
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
            Value::Tuple(tuple) => {
                // Tuples themselves are immutable at the top level,
                // but we recurse for deep freezing.
                if recursive {
                    let elements = tuple.borrow().elements.clone();
                    for item in elements {
                        self.freeze_value(&item, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn throw_runtime_error(&mut self, message: &str) -> RuntimeError {
        // Provide a small grace buffer for error handling if we hit memory limits
        if self.gc.max_bytes > 0 && self.gc.grace_bytes == 0 {
            self.gc.grace_bytes = 64 * 1024; // 64KB grace for emergency handling
        }

        let mut map = IndexMap::new();
        map.insert(
            "message".to_string(),
            Value::String(Rc::new(message.to_string())),
        );
        map.insert(
            "peakMemory".to_string(),
            Value::Int(self.gc.peak_bytes as i64),
        );
        map.insert(
            "instructions".to_string(),
            Value::Int(self.instruction_count as i64),
        );
        let error_val = match self.gc.allocate(MapState::new(map.clone())) {
            Ok(v) => Value::Map(v),
            Err(_) => {
                // If allocation still fails, try allocating WITHOUT tracking
                // to ensure the exception can be thrown and caught.
                Value::Map(self.gc.gc.allocate(MapState::new(map)))
            }
        };
        match self.unwind_to_handler(error_val) {
            Ok(_) => unreachable!("unwind_to_handler should always return Err"),
            Err(err) => err,
        }
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
        // println!("VM::call_any_value: calling {:?}", callee);
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
                    None,
                    None,
                    None,
                    false,
                    Vec::new(),
                )?;
            }
            Value::BoundFunction {
                function,
                type_args,
                ..
            } => {
                // Type args are currently ignored during call as Pleno uses erasure or they are just for checking
                // If reification is needed, we'd pass them to call_function
                self.call_function(
                    *function.clone(),
                    None,
                    &args,
                    return_reg,
                    None,
                    None,
                    None,
                    false,
                    type_args.clone(),
                )?;
            }
            Value::Closure(state) => {
                let _borrowed = state.borrow();
                self.call_function(
                    Value::Closure(state.clone()),
                    None,
                    &args,
                    return_reg,
                    None,
                    Some(state.clone()),
                    None,
                    false,
                    Vec::new(),
                )?;
            }
            Value::NativeFunction(func) => match func(self, &args) {
                Ok(result) => {
                    if return_reg >= self.registers.len() {
                        self.registers.resize(return_reg + 1, Value::Null);
                    }
                    self.registers[return_reg] = result;
                }
                Err(err) => {
                    return Err(self.throw_runtime_error(&err.message));
                }
            },
            Value::BoundNativeMethod { receiver, function } => {
                let mut merged_args = vec![*receiver.clone()];
                merged_args.extend(args);
                match function(self, &merged_args) {
                    Ok(result) => {
                        if return_reg >= self.registers.len() {
                            self.registers.resize(return_reg + 1, Value::Null);
                        }
                        self.registers[return_reg] = result;
                    }
                    Err(err) => {
                        return Err(self.throw_runtime_error(&err.message));
                    }
                }
            }
            Value::BoundMethod {
                receiver,
                function,
                owner,
                ..
            } => {
                self.call_function(
                    *function.clone(),
                    Some(*receiver.clone()),
                    &args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    false,
                    Vec::new(),
                )?;
            }
            Value::StaticMethod {
                function, owner, ..
            } => {
                self.call_function(
                    *function.clone(),
                    None,
                    &args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    true,
                    Vec::new(),
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
                if let Some(handle) = class.borrow().constructor.clone() {
                    self.call_function(
                        handle.function.clone(),
                        Some(val.clone()),
                        &args,
                        return_reg,
                        Some(val),
                        None,
                        Some(MethodOwner::Class(class.clone())),
                        false,
                        Vec::new(),
                    )?;
                } else {
                    self.registers[return_reg] = val;
                }
            }
            Value::BoundClass { class, type_args } => {
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
                if let Some(handle) = class.borrow().constructor.clone() {
                    self.call_function(
                        handle.function.clone(),
                        Some(val.clone()),
                        &args,
                        return_reg,
                        Some(val),
                        None,
                        Some(MethodOwner::Class(class.clone())),
                        false,
                        type_args.clone(),
                    )?;
                } else {
                    self.registers[return_reg] = val;
                }
            }
            Value::VariantConstructor {
                enum_type,
                variant_name,
            } => {
                let mut data = Vec::new();
                data.extend(args);
                let variant = Value::Variant {
                    enum_type: enum_type.clone(),
                    variant_name: Rc::clone(&variant_name),
                    data,
                };
                self.registers[return_reg] = variant;
            }
            Value::Partial {
                callee,
                args: saved_args,
                placeholders,
            } => {
                let mut merged_args = saved_args.clone();
                let mut new_arg_idx = 0;
                for &placeholder_idx in &placeholders {
                    if new_arg_idx < args.len() {
                        merged_args[placeholder_idx] = args[new_arg_idx].clone();
                        new_arg_idx += 1;
                    }
                }
                while new_arg_idx < args.len() {
                    merged_args.push(args[new_arg_idx].clone());
                    new_arg_idx += 1;
                }
                self.call_any_value(*callee.clone(), merged_args, return_reg)?;
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    self.call_any_value(callable, args, return_reg)?;
                } else {
                    return Err(RuntimeError::new("value is not callable"));
                }
            }
            Value::Object(obj) => {
                let callable = obj.borrow().fields.get("__call__").cloned();
                if let Some(callable) = callable {
                    self.call_any_value(callable, args, return_reg)?;
                } else {
                    return Err(RuntimeError::new("value is not callable"));
                }
            }
            _ => return Err(RuntimeError::new("value is not callable")),
        }
        Ok(())
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
            .map(|path| path.to_path_buf());
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
        let program = parser
            .parse_program()
            .map_err(|err| RuntimeError::new(err.message))?;
        let mut compiler = pleno_compiler::Compiler::new(source);
        let chunk = compiler
            .compile_module(&program)
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
            .extend(std::iter::repeat_n(Value::Null, register_count));
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
        let result = self.run_until(depth)?;
        self.registers.truncate(base);
        self.modules.insert(resolved_key, result.clone());
        Ok(result)
    }
    fn get_value_type_name_for_specialize(&self, val: &Value) -> String {
        match val {
            Value::Class(c) => c.borrow().name.to_string(),
            Value::BoundClass { class, .. } => class.borrow().name.to_string(),
            Value::Interface(i) => i.borrow().name.to_string(),
            Value::BoundInterface { interface, .. } => interface.borrow().name.to_string(),
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
                    && let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content)
                    && let Some(main) = manifest.get("main").and_then(|v| v.as_str())
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

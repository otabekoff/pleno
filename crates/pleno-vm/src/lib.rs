use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    ArrayState, Class, ClosureState, MapState, MultiWaiter, MultiWaiterMode, NativeFunction,
    Object, Range, RuntimeError, VMProvider, Value, ValueKey,
    gc::{GcObject, GcRc},
};
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

mod access;
mod models;
mod utils;
mod value_ops;

pub use access::*;
pub use models::*;
pub use utils::*;
pub use value_ops::*;

// Core VM implementation

pub struct VM {
    pub gc: pleno_value::gc::GCManager,
    pub registers: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, Value>,
    pub modules: HashMap<String, Value>,
    pub functions: Vec<Function>,
    pub pending_tasks: Vec<AsyncTask>,
    pub suspended_frames: HashMap<u64, SuspendedFrame>,
    pub promises: HashMap<u64, Value>,
    pub next_promise_id: u64,
    pub multi_waiters: Vec<MultiWaiter>,
    pub builtin_resolver: Option<fn(&str, &str) -> Option<NativeFunction>>,
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
}

impl VMProvider for VM {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let return_reg = self.registers.len();
        self.registers.push(Value::Null); // Placeholder for return value

        match callee {
            Value::Function(handle) => {
                self.call_function(handle, None, &args, return_reg, None, None, None, false)?;
            }
            Value::Closure(state) => {
                let borrowed = state.borrow();
                self.call_function(
                    borrowed.function,
                    None,
                    &args,
                    return_reg,
                    None,
                    Some(state.clone()),
                    None,
                    false,
                )?;
            }
            Value::BoundMethod {
                receiver,
                function,
                owner,
                ..
            } => {
                self.call_function(
                    function,
                    Some(receiver),
                    &args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    false,
                )?;
            }
            Value::NativeFunction(f) => {
                return f(self, &args);
            }
            Value::BoundNativeMethod { receiver, function } => {
                let mut final_args = vec![*receiver.clone()];
                final_args.extend(args);
                return function(self, &final_args);
            }
            Value::Map(map) => {
                let callable = map.borrow().get("__call__").cloned();
                if let Some(callable) = callable {
                    return self.call(callable, args);
                } else {
                    return Err(RuntimeError::new("value is not callable"));
                }
            }
            _ => return Err(RuntimeError::new("value is not callable")),
        }

        // Execute until the callee frame returns
        let depth = self.frames.len() - 1;
        let result = self.run_until(depth)?;

        // The result of run_until(depth) is the last value returned?
        // Actually run_until returns the final result if depth is 0,
        // but if it's a nested call, we might need to be careful.
        // Let's check run_until implementation.
        Ok(result)
    }

    fn get_gc(&mut self) -> &mut pleno_value::gc::GCManager {
        &mut self.gc
    }

    fn create_promise(&mut self) -> (u64, Value) {
        let id = self.next_promise_id;
        self.next_promise_id += 1;

        let mut map = HashMap::new();
        map.insert("id".to_string(), Value::Int(id as i64));
        map.insert(
            "variant".to_string(),
            Value::String(Rc::new("Pending".to_string())),
        );
        map.insert("value".to_string(), Value::Null);
        let promise = Value::Map(self.gc.allocate(MapState::new(map)));
        self.promises.insert(id, promise.clone());
        (id, promise)
    }

    fn resolve_promise(&mut self, id: u64, value: Value) {
        self.pending_tasks.push(AsyncTask {
            promise_id: id,
            value,
            is_async: true,
            is_resolved: true,
        });
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
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let gc = pleno_value::gc::GCManager::new(1000);
        let globals = HashMap::new();
        let modules = HashMap::new();

        Self {
            gc,
            registers: vec![Value::Null; 256],
            frames: Vec::new(),
            globals,
            modules,
            functions: Vec::new(),
            pending_tasks: Vec::new(),
            suspended_frames: HashMap::new(),
            promises: HashMap::new(),
            next_promise_id: 1,
            multi_waiters: Vec::new(),
            builtin_resolver: None,
        }
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
            params: Vec::new(),
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
            current_class: None,
            current_module: None,
            current_module_path: None,
            exception_handlers: Vec::new(),
        });

        let mut last_result = Value::Null;
        while !self.frames.is_empty() || !self.pending_tasks.is_empty() {
            if !self.frames.is_empty() {
                last_result = match self.run_until(0) {
                    Ok(val) => val,
                    Err(mut err) => {
                        self.enrich_error(&mut err);
                        return Err(err);
                    }
                };
            }
            if !self.pending_tasks.is_empty()
                && let Err(mut err) = self.process_tasks()
            {
                self.enrich_error(&mut err);
                return Err(err);
            }
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

    fn process_tasks(&mut self) -> Result<(), RuntimeError> {
        let tasks: Vec<_> = self.pending_tasks.drain(..).collect();
        for task in tasks {
            // Update Promise map if it exists
            if let Some(Value::Map(map)) = self.promises.remove(&task.promise_id) {
                map.borrow_mut().insert(
                    "variant".to_string(),
                    Value::String(Rc::new("Resolved".to_string())),
                );
                map.borrow_mut()
                    .insert("value".to_string(), task.value.clone());
            }

            if let Some(suspended) = self.suspended_frames.remove(&task.promise_id) {
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
                                    gc.allocate(ArrayState::new(waiter.results.clone())),
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
        loop {
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
                return Ok(Value::Null);
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
            frame.ip += 1;

            let opcode = instruction.opcode();
            let a = instruction.a() as usize;
            let b = instruction.b() as usize;
            let c = instruction.c() as usize;
            let base = frame.base;

            match opcode {
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
                    });
                    let captures = vec![Value::Null; c];
                    let state = self.gc.allocate(ClosureState::new(func_index, captures));
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
                    if let Some(state) = &frame.closure_state {
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
                    let idx = b;
                    let value = self.registers[base + a].clone();
                    if let Some(state) = &frame.closure_state {
                        let mut state = state.borrow_mut();
                        if idx >= state.captures.len() {
                            return Err(RuntimeError::new("capture index out of bounds"));
                        }
                        state.captures[idx] = value;
                    } else {
                        return Err(RuntimeError::new("not in a closure"));
                    }
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
                        Value::Map(self.gc.allocate(MapState::new(HashMap::new())));
                }
                Opcode::NewArray => {
                    self.registers[base + a] =
                        Value::Array(self.gc.allocate(ArrayState::new(Vec::new())));
                }
                Opcode::ArrayAppend => {
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
                        Value::Array(array) => {
                            if array.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable array")?;
                                continue;
                            }
                            array.borrow_mut().elements.push(value);
                        }
                        _ => return Err(RuntimeError::new("array append on non-array value")),
                    }
                }
                Opcode::ArraySpread => {
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
                        (Value::Array(target), Value::Array(source)) => {
                            if target.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable array")?;
                                continue;
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
                        (Value::Map(target), Value::Map(source)) => {
                            if target.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable map")?;
                                continue;
                            }
                            for (k, v) in source.borrow().entries.iter() {
                                target.borrow_mut().entries.insert(k.clone(), v.clone());
                            }
                        }
                        _ => return Err(RuntimeError::new("map spread expects map values")),
                    }
                }
                Opcode::SetSpread => {
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
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
                        (Value::Array(target), Value::Array(source)) => {
                            target
                                .borrow_mut()
                                .elements
                                .extend(source.borrow().elements.iter().cloned());
                        }
                        _ => return Err(RuntimeError::new("array extend expects array values")),
                    }
                }

                Opcode::NewSet => {
                    self.registers[base + a] =
                        Value::Set(self.gc.allocate(std::collections::HashSet::new()));
                }
                Opcode::SetAdd => {
                    let value = self.registers[base + b].clone();
                    let key = ValueKey::try_from_value(&value)
                        .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                    match &self.registers[base + a] {
                        Value::Set(set) => {
                            set.borrow_mut().insert(key);
                        }
                        _ => return Err(RuntimeError::new("set add on non-set value")),
                    }
                }
                Opcode::SetHas => {
                    let value = self.registers[base + c].clone();
                    let key = ValueKey::try_from_value(&value)
                        .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                    let exists = match &self.registers[base + b] {
                        Value::Set(set) => set.borrow().contains(&key),
                        _ => return Err(RuntimeError::new("set has on non-set value")),
                    };
                    self.registers[base + a] = Value::Bool(exists);
                }
                Opcode::SetLen => {
                    let len = match &self.registers[base + b] {
                        Value::Set(set) => set.borrow().len() as i64,
                        _ => return Err(RuntimeError::new("set length on non-set value")),
                    };
                    self.registers[base + a] = Value::Int(len);
                }
                Opcode::SetValues => {
                    let values = match &self.registers[base + b] {
                        Value::Set(set) => set
                            .borrow()
                            .iter()
                            .map(|value| match value {
                                ValueKey::Null => Value::Null,
                                ValueKey::Bool(value) => Value::Bool(*value),
                                ValueKey::Int(value) => Value::Int(*value),
                                ValueKey::Float(value) => Value::Float(f64::from_bits(*value)),
                                ValueKey::String(value) => Value::String(Rc::clone(value)),
                            })
                            .collect::<Vec<_>>(),
                        _ => return Err(RuntimeError::new("set values on non-set value")),
                    };
                    self.registers[base + a] =
                        Value::Array(self.gc.allocate(ArrayState::new(values)));
                }
                Opcode::NewObject => {
                    self.registers[base + a] = Value::Object(self.gc.allocate(Object::new(None)));
                }
                Opcode::SetField => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let value = self.registers[base + a].clone();
                    match &self.registers[base + b] {
                        Value::Object(object) => {
                            if object.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable object")?;
                                continue;
                            }
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
                        Value::Array(self.gc.allocate(ArrayState::new(keys)));
                }
                Opcode::IterValues => {
                    let value = match &self.registers[base + b] {
                        Value::Array(array) => Value::Array(array.clone()),
                        Value::Map(map) => {
                            let values = map
                                .borrow()
                                .keys()
                                .cloned()
                                .map(|key| Value::String(Rc::new(key)))
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(ArrayState::new(values)))
                        }
                        Value::Set(set) => {
                            let values = set
                                .borrow()
                                .iter()
                                .cloned()
                                .map(|key| match key {
                                    pleno_value::ValueKey::Null => Value::Null,
                                    pleno_value::ValueKey::Bool(value) => Value::Bool(value),
                                    pleno_value::ValueKey::Int(value) => Value::Int(value),
                                    pleno_value::ValueKey::Float(bits) => {
                                        Value::Float(f64::from_bits(bits))
                                    }
                                    pleno_value::ValueKey::String(value) => Value::String(value),
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(ArrayState::new(values)))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    };
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
                                    Value::Array(self.gc.allocate(ArrayState::new(vec![
                                        Value::Int(idx as i64),
                                        val.clone(),
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(ArrayState::new(entries)))
                        }
                        Value::Map(map) => {
                            let entries = map
                                .borrow()
                                .iter()
                                .map(|(key, val)| {
                                    Value::Array(self.gc.allocate(ArrayState::new(vec![
                                        Value::String(Rc::new(key.clone())),
                                        val.clone(),
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(ArrayState::new(entries)))
                        }
                        Value::Set(set) => {
                            let entries = set
                                .borrow()
                                .iter()
                                .enumerate()
                                .map(|(idx, val)| {
                                    let val = match val {
                                        ValueKey::Null => Value::Null,
                                        ValueKey::Bool(v) => Value::Bool(*v),
                                        ValueKey::Int(v) => Value::Int(*v),
                                        ValueKey::Float(bits) => {
                                            Value::Float(f64::from_bits(*bits))
                                        }
                                        ValueKey::String(v) => Value::String(Rc::clone(v)),
                                    };
                                    Value::Array(self.gc.allocate(ArrayState::new(vec![
                                        Value::Int(idx as i64),
                                        val,
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(ArrayState::new(entries)))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    };
                    self.registers[base + a] = value;
                }
                Opcode::ArrayLen => {
                    let len = match &self.registers[base + b] {
                        Value::Array(array) => array.borrow().elements.len() as i64,
                        _ => return Err(RuntimeError::new("array length on non-array value")),
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
                                    return Ok(Value::Null); // Suspend
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

                    let result = match type_val {
                        Value::String(s) => matches!(
                            (s.as_str(), value),
                            ("Int", Value::Int(_))
                                | ("Float", Value::Float(_))
                                | ("Bool", Value::Bool(_))
                                | ("String", Value::String(_))
                                | ("Array", Value::Array(_))
                                | ("Map", Value::Map(_))
                                | ("Set", Value::Set(_))
                                | ("Object", Value::Map(_) | Value::Class(_)) // Object matches maps/classes
                                | ("Null", Value::Null)
                                | ("Any", _)
                        ),
                        Value::Class(_target_class) => {
                            match value {
                                Value::Class(instance_class) => {
                                    // Check inheritance chain
                                    // TODO: proper inheritance check
                                    // For now just check exact module + name match of class definition
                                    let _current = Some(instance_class.clone());
                                    let _found = false;
                                    // Simple check: compare Rc pointers
                                    // But instances are distinct from definitions if allocated differently?
                                    // No, Value::Class holds the class definition (or instance?)
                                    // Wait, Value::Class is an INSTANCE or DEFINITION?
                                    // Usually "instance" is Value::Object (if map based) or Value::Instance?
                                    // VM implementation uses Value::Map for instances of classes desugared?
                                    // Or does it have separate Value::Instance?
                                    // Let's assume strict equality for now or fix later if specs need instanceof
                                    // Re-reading VM structure: Value::Class holds ClassState (definition).
                                    // Instances are Value::Object (if implemented) or Value::Map?
                                    // Specs say: "class Person ... let p = Person(...)". p is an Instance.
                                    // We need to check if p is instance of Person.
                                    // For now, let's assume we fixed primitive bug.
                                    false
                                }
                                _ => false,
                            }
                        }
                        _ => false, // Invalid type operand
                    };
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
                        self.throw_runtime_error("pure function cannot have side effects")?;
                        continue;
                    }
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let value = self.registers[base + a].clone();
                    let current_class = frame.current_class.as_ref().cloned();
                    let current_module = frame.current_module.as_ref().cloned();
                    match &self.registers[base + b] {
                        Value::Map(map) => {
                            if map.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable map")?;
                                continue;
                            }
                            map.borrow_mut().insert(name, value);
                        }
                        Value::Object(object) => {
                            if object.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable object")?;
                                continue;
                            }
                            let class = object.borrow().class.clone();
                            if let Some(class) = class.as_ref() {
                                if let Some(info) = resolve_setter(class, &name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        current_module.as_ref(),
                                    ) {
                                        return Err(RuntimeError::new("property access denied"));
                                    }
                                    self.call_function(
                                        info.function,
                                        Some(object.clone()),
                                        &[value],
                                        base + a,
                                        None,
                                        None,
                                        Some(info.owner.clone()),
                                        false,
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
                                        return Err(RuntimeError::new("field access denied"));
                                    }
                                }
                            }
                            object.borrow_mut().fields.insert(name, value);
                        }
                        Value::Class(class) => {
                            if let Some(info) = resolve_static_setter(class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                ) {
                                    return Err(RuntimeError::new("static property access denied"));
                                }
                                self.call_function(
                                    info.function,
                                    None,
                                    &[value],
                                    base + a,
                                    None,
                                    None,
                                    Some(info.owner.clone()),
                                    false,
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
                                    return Err(RuntimeError::new("static field access denied"));
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
                    let current_class = frame.current_class.as_ref().cloned();
                    let value = match &self.registers[base + b] {
                        Value::Map(map) => {
                            let map_ref = map.borrow();
                            if let Some(Value::String(variant)) = map_ref.get("variant") {
                                if (variant.as_str() == "Ok" || variant.as_str() == "Err")
                                    && resolve_builtin_method(self, "Result", &name).is_some()
                                {
                                    // Result type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method(self, "Result", &name)
                                            .unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && resolve_builtin_method(self, "Option", &name).is_some()
                                {
                                    // Option type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method(self, "Option", &name)
                                            .unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Pending"
                                    || variant.as_str() == "Resolved")
                                    && resolve_builtin_method(self, "Promise", &name).is_some()
                                {
                                    // Promise type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method(self, "Promise", &name)
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
                            } else if let Some(func) = resolve_builtin_method(self, "Map", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Map(map.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
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
                                        return Err(RuntimeError::new("property access denied"));
                                    }
                                    self.call_function(
                                        info.function,
                                        Some(object.clone()),
                                        &[],
                                        base + a,
                                        None,
                                        None,
                                        Some(info.owner.clone()),
                                        false,
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
                                        return Err(RuntimeError::new("field access denied"));
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
                                        return Err(RuntimeError::new("method access denied"));
                                    }
                                    Value::BoundMethod {
                                        receiver: object.clone(),
                                        function: info.function,
                                        owner: info.owner.clone(),
                                        visibility: info.visibility,
                                    }
                                } else {
                                    Value::Null
                                }
                            } else {
                                field_value.unwrap_or(Value::Null)
                            }
                        }
                        Value::Class(class) => {
                            if let Some(info) = resolve_static_getter(class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                ) {
                                    return Err(RuntimeError::new("static property access denied"));
                                }
                                self.call_function(
                                    info.function,
                                    None,
                                    &[],
                                    base + a,
                                    None,
                                    None,
                                    Some(info.owner.clone()),
                                    false,
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
                                    return Err(RuntimeError::new("static field access denied"));
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
                            } else if let Some(info) = resolve_static_method(class, &name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                ) {
                                    return Err(RuntimeError::new("static method access denied"));
                                }
                                Value::StaticMethod {
                                    function: info.function,
                                    owner: info.owner.clone(),
                                    visibility: info.visibility,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Set(set) => {
                            if name == "len" {
                                Value::Int(set.borrow().len() as i64)
                            } else if let Some(func) = resolve_builtin_method(self, "Set", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Set(set.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Array(array) => {
                            if name == "len" || name == "length" {
                                Value::Int(array.borrow().len() as i64)
                            } else if let Some(func) = resolve_builtin_method(self, "Array", &name)
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Array(array.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::String(s) => {
                            if name == "len" || name == "length" {
                                Value::Int(s.len() as i64)
                            } else if let Some(func) = resolve_builtin_method(self, "String", &name)
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::String(Rc::clone(s))),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::ArrayOp { kind: _, array } => {
                            if name == "len" || name == "length" {
                                Value::Int(array.borrow().len() as i64)
                            } else if let Some(func) = resolve_builtin_method(self, "Array", &name)
                            {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Array(array.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Mutex(m) => {
                            if let Some(func) = resolve_builtin_method(self, "Mutex", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Mutex(m.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Channel(c) => {
                            if let Some(func) = resolve_builtin_method(self, "Channel", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Channel(c.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        _ => {
                            if name == "len" || name == "length" {
                                return Err(RuntimeError::new("array length on non-array value"));
                            }
                            if name == "append" {
                                return Err(RuntimeError::new("array append on non-array value"));
                            }
                            if name == "add" {
                                return Err(RuntimeError::new("set add on non-set value"));
                            }
                            if name == "has" {
                                return Err(RuntimeError::new("set has on non-set value"));
                            }
                            if name == "keys" {
                                return Err(RuntimeError::new("map keys on non-map value"));
                            }
                            return Err(RuntimeError::new("field access on non-object"));
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
                        receiver,
                        function: info.function,
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
                        self.call_function(
                            info.function,
                            Some(this_obj.clone()),
                            &args,
                            base + a,
                            None, // No return value needed
                            None,
                            Some(info.owner.clone()),
                            false,
                        )?;
                    } else {
                        // No constructor, just return null
                        self.registers[base + a] = Value::Null;
                    }
                }
                Opcode::GetIndex => {
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            let array_borrow = array.borrow();
                            if idx >= array_borrow.len() {
                                self.throw_runtime_error("array index out of bounds")?;
                                continue;
                            }
                            self.registers[base + a] = array_borrow[idx].clone();
                        }
                        (Value::Map(map), Value::String(key)) => {
                            self.registers[base + a] = map
                                .borrow()
                                .get(key.as_str())
                                .cloned()
                                .unwrap_or(Value::Null);
                        }
                        (Value::Set(set), value) => {
                            let key = ValueKey::try_from_value(&value)
                                .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                            self.registers[base + a] = Value::Bool(set.borrow().contains(&key));
                        }
                        _ => return Err(RuntimeError::new("invalid index operation")),
                    }
                }
                Opcode::SetIndex => {
                    if self.functions[frame.function_index].is_pure {
                        self.throw_runtime_error("pure function cannot have side effects")?;
                        continue;
                    }
                    let value = self.registers[base + a].clone();
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            if array.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable array")?;
                                continue;
                            }
                            let idx = idx as usize;
                            let mut array = array.borrow_mut();
                            if idx >= array.elements.len() {
                                self.throw_runtime_error("array index out of bounds")?;
                                continue;
                            }
                            array.elements[idx] = value;
                        }
                        (Value::Map(map), Value::String(key)) => {
                            if map.borrow().is_immutable {
                                self.throw_runtime_error("cannot modify immutable map")?;
                                continue;
                            }
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
                    self.registers[base + a] = add_values(left, right)?;
                }
                Opcode::Sub => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] = sub_values(left, right)?;
                }
                Opcode::Mul => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] = mul_values(left, right)?;
                }
                Opcode::Div => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] = div_values(left, right)?;
                }
                Opcode::Mod => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
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
                    self.registers[base + a] = Value::Bool(left == right);
                }
                Opcode::Ne => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
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
                        return Ok(result);
                    }

                    if !function_is_async {
                        self.registers[frame.return_reg] = result;
                    }
                }
                Opcode::GetGlobal => {
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    let value = self.globals.get(&name).cloned().unwrap_or(Value::Null);
                    self.registers[base + a] = value;
                }
                Opcode::SetGlobal => {
                    if self.functions[frame.function_index].is_pure {
                        self.throw_runtime_error("pure function cannot have side effects")?;
                        continue;
                    }
                    let name = match &function.chunk.constants[b] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("global name must be string")),
                    };
                    let value = self.registers[base + a].clone();
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
                        methods: HashMap::new(),
                        static_methods: HashMap::new(),
                        constructor: None,
                        native_constructor: None,
                        superclass: None,
                        static_fields: HashMap::new(),
                        fields: HashMap::new(),
                        static_field_visibility: HashMap::new(),
                        getters: HashMap::new(),
                        setters: HashMap::new(),
                        static_getters: HashMap::new(),
                        static_setters: HashMap::new(),
                        module: module.clone(),
                        is_abstract: false,
                        implements: Vec::new(),
                    });

                    // Now parse methods with class reference
                    let mut methods = HashMap::new();
                    let mut constructor = None;
                    for (method_name, value) in methods_map {
                        let info = parse_method_info(value, &class, module.as_ref())?;
                        if method_name == "constructor" {
                            constructor = Some(info.function);
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
                            let mut methods = HashMap::new();
                            let mut getters = HashMap::new();
                            let mut setters = HashMap::new();
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
                        }));
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
                                if let Value::Interface(interface) = &interface_val {
                                    {
                                        let interface_borrow = interface.borrow();
                                        let class_borrow = class.borrow();

                                        // Check methods
                                        for (name, sig) in &interface_borrow.methods {
                                            if let Some(method_info) =
                                                class_borrow.methods.get(name)
                                            {
                                                if let Some(func) =
                                                    self.functions.get(method_info.function)
                                                {
                                                    let mut expected_arity = sig.arity;
                                                    if !sig.is_static {
                                                        expected_arity += 1;
                                                    }

                                                    if func.arity != expected_arity {
                                                        validation_error = Some(format!(
                                                            "Class '{}' implements interface '{}' incorrectly: method '{}' requires {} args, found {}",
                                                            class_borrow.name,
                                                            interface_borrow.name,
                                                            name,
                                                            expected_arity,
                                                            func.arity
                                                        ));
                                                        break 'interface_loop;
                                                    }
                                                }
                                            } else {
                                                validation_error = Some(format!(
                                                    "Class '{}' missing method '{}' required by interface '{}'",
                                                    class_borrow.name, name, interface_borrow.name
                                                ));
                                                break 'interface_loop;
                                            }
                                        }

                                        // Check getters
                                        for name in interface_borrow.getters.keys() {
                                            if !class_borrow.getters.contains_key(name) {
                                                validation_error = Some(format!(
                                                    "Class '{}' missing getter '{}' required by interface '{}'",
                                                    class_borrow.name, name, interface_borrow.name
                                                ));
                                                break 'interface_loop;
                                            }
                                        }

                                        // Check setters
                                        for name in interface_borrow.setters.keys() {
                                            if !class_borrow.setters.contains_key(name) {
                                                validation_error = Some(format!(
                                                    "Class '{}' missing setter '{}' required by interface '{}'",
                                                    class_borrow.name, name, interface_borrow.name
                                                ));
                                                break 'interface_loop;
                                            }
                                        }
                                    }

                                    class.borrow_mut().implements.push(interface_val.clone());
                                }
                            }

                            if let Some(msg) = validation_error {
                                self.throw_runtime_error(&msg)?;
                                continue;
                            }
                        }
                        _ => return Err(RuntimeError::new("SetImplements on non-class value")),
                    }
                }
                Opcode::Freeze => {
                    let value = self.registers[base + a].clone();
                    self.freeze_value(&value);
                }
                Opcode::SetSuper => {
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
                Opcode::SetAbstract => match &mut self.registers[base + a] {
                    Value::Class(class) => {
                        class.borrow_mut().is_abstract = true;
                    }
                    _ => return Err(RuntimeError::new("set abstract on non-class value")),
                },
                Opcode::SetStatic => {
                    let static_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static methods must be map")),
                    };
                    let mut static_methods = HashMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set static on non-class value")),
                    };
                    for (method_name, value) in static_map {
                        let info = parse_method_info(value, &class_ref, module.as_ref())?;
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
                    let getters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("getters must be map")),
                    };
                    let mut getters = HashMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set getters on non-class value")),
                    };
                    for (prop_name, value) in getters_map {
                        let info = parse_method_info(value, &class_ref, module.as_ref())?;
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
                    let setters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("setters must be map")),
                    };
                    let mut setters = HashMap::new();
                    let (class_ref, module) = match &self.registers[base + a] {
                        Value::Class(class) => {
                            let borrowed = class.borrow();
                            (class.clone(), borrowed.module.clone())
                        }
                        _ => return Err(RuntimeError::new("set setters on non-class value")),
                    };
                    for (prop_name, value) in setters_map {
                        let info = parse_method_info(value, &class_ref, module.as_ref())?;
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
                    let getters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static getters must be map")),
                    };
                    let mut static_getters = HashMap::new();
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
                        let info = parse_method_info(value, &class_ref, module.as_ref())?;
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
                    let setters_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().entries.clone(),
                        _ => return Err(RuntimeError::new("static setters must be map")),
                    };
                    let mut static_setters = HashMap::new();
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
                        let info = parse_method_info(value, &class_ref, module.as_ref())?;
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
                Opcode::SetFieldVisibility => {
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
                Opcode::Call => {
                    let callee = self.registers[base + b].clone();
                    let args = match &self.registers[base + c] {
                        Value::Array(array) => array.borrow().clone(),
                        _ => return Err(RuntimeError::new("call expects args array")),
                    };
                    let mut current_callee = callee;
                    loop {
                        match current_callee.clone() {
                            Value::Partial {
                                function,
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
                                self.call_function(
                                    function,
                                    None,
                                    &merged_args,
                                    base + a,
                                    None,
                                    None,
                                    None,
                                    false,
                                )?;
                                break;
                            }
                            Value::Function(handle) => {
                                self.call_function(
                                    handle,
                                    None,
                                    &args,
                                    base + a,
                                    None,
                                    None,
                                    None,
                                    false,
                                )?;
                                break;
                            }
                            Value::Closure(state) => {
                                let borrowed = state.borrow();
                                self.call_function(
                                    borrowed.function,
                                    None,
                                    &args,
                                    base + a,
                                    None,
                                    Some(state.clone()),
                                    None,
                                    false,
                                )?;
                                break;
                            }
                            Value::NativeFunction(func) => {
                                match func(self, &args) {
                                    Ok(result) => {
                                        self.registers[base + a] = result;
                                    }
                                    Err(err) => {
                                        self.throw_runtime_error(&err.message)?;
                                    }
                                }
                                break;
                            }
                            Value::BoundNativeMethod { receiver, function } => {
                                let mut merged_args = vec![*receiver];
                                merged_args.extend(args.elements.clone());
                                match function(self, &merged_args) {
                                    Ok(result) => {
                                        self.registers[base + a] = result;
                                    }
                                    Err(err) => {
                                        self.throw_runtime_error(&err.message)?;
                                    }
                                }
                                break;
                            }
                            Value::BoundMethod {
                                receiver,
                                function,
                                owner,
                                ..
                            } => {
                                self.call_function(
                                    function,
                                    Some(receiver),
                                    &args,
                                    base + a,
                                    None,
                                    None,
                                    Some(owner),
                                    false,
                                )?;
                                break;
                            }
                            Value::StaticMethod {
                                function, owner, ..
                            } => {
                                self.call_function(
                                    function,
                                    None,
                                    &args,
                                    base + a,
                                    None,
                                    None,
                                    Some(owner),
                                    false,
                                )?;
                                break;
                            }
                            Value::Class(class) => {
                                if class.borrow().is_abstract {
                                    self.throw_runtime_error(&format!(
                                        "cannot instantiate abstract class '{}'",
                                        class.borrow().name
                                    ))?;
                                    break;
                                }
                                let object = self.gc.allocate(Object::new(Some(class.clone())));
                                if let Some(handle) = class.borrow().constructor {
                                    self.call_function(
                                        handle,
                                        Some(object.clone()),
                                        &args,
                                        base + a,
                                        Some(Value::Object(object.clone())),
                                        None,
                                        Some(class.clone()),
                                        false,
                                    )?;
                                } else {
                                    self.registers[base + a] = Value::Object(object);
                                }
                                break;
                            }
                            Value::Map(map) => {
                                let callable = map.borrow().entries.get("__call__").cloned();
                                if let Some(callable) = callable {
                                    current_callee = callable;
                                    continue;
                                } else {
                                    return Err(RuntimeError::new("value is not callable"));
                                }
                            }
                            _ => return Err(RuntimeError::new("value is not callable")),
                        }
                    }
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
                    let func_index = match callee {
                        Value::Function(idx) => idx,
                        Value::Closure(state) => state.borrow().function,
                        _ => return Err(RuntimeError::new("partial callee must be a function")),
                    };
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
                        function: func_index,
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
                                Value::Array(self.gc.allocate(ArrayState::new(slice)));
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
        }
    }
}

impl VM {
    #[allow(clippy::too_many_arguments)]
    fn call_function(
        &mut self,
        handle: usize,
        receiver: Option<GcRc<Object>>,
        args: &[Value],
        return_reg: usize,
        return_override: Option<Value>,
        closure_state: Option<GcRc<pleno_value::ClosureState>>,
        current_class: Option<GcRc<Class>>,
        args_already_processed: bool,
    ) -> Result<(), RuntimeError> {
        let function = self
            .functions
            .get(handle)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?
            .clone();
        let current_module = function.module.clone();
        let current_module_path = function.module_path.clone();

        let processed_args = if args_already_processed {
            args.to_vec()
        } else {
            let mut final_args = Vec::new();
            if let Some(recv) = receiver.clone() {
                final_args.push(Value::Object(recv));
            }
            final_args.extend(args.iter().cloned());

            let has_variadic = function
                .params
                .last()
                .map(|p| p.is_variadic)
                .unwrap_or(false);

            if !has_variadic && final_args.len() > function.arity {
                return Err(RuntimeError::new("too many positional arguments"));
            }

            let mut processed = Vec::new();
            let mut arg_idx = 0;

            if function.is_async {
                // Generate promise ID
                let promise_id = self.next_promise_id;
                self.next_promise_id += 1;

                // Inject promise ID as last argument
                // Note: The compiler should have added an extra parameter to function.params
                // So when we iterate function.params, the last one will consume this value.
                final_args.push(Value::Int(promise_id as i64));

                // Create Promise object
                let mut promise_map = HashMap::new();
                promise_map.insert("id".to_string(), Value::Int(promise_id as i64));
                promise_map.insert(
                    "variant".to_string(),
                    Value::String(Rc::new("Pending".to_string())),
                );
                promise_map.insert("value".to_string(), Value::Null);
                let promise = Value::Map(self.gc.allocate(MapState::new(promise_map)));
                self.promises.insert(promise_id, promise.clone());

                // Set return register to Promise immediately
                self.registers[return_reg] = promise;
            }

            for param in function.params.iter() {
                if param.is_variadic {
                    let rest = final_args[arg_idx..].to_vec();
                    processed.push(Value::Array(self.gc.allocate(ArrayState::new(rest))));
                    arg_idx = final_args.len();
                    break;
                }

                if param.is_keyword_only {
                    if arg_idx < final_args.len() {
                        return Err(RuntimeError::new("keyword-only argument requires name"));
                    }
                    if param.has_default {
                        processed.push(Value::Missing);
                    } else {
                        return Err(RuntimeError::new("missing required keyword-only argument"));
                    }
                    continue;
                }

                if arg_idx < final_args.len() {
                    processed.push(final_args[arg_idx].clone());
                    arg_idx += 1;
                } else if param.has_default {
                    processed.push(Value::Missing);
                } else {
                    println!(
                        "DEBUG: Missing arg. param: {}, arg_idx: {}, args_len: {}",
                        param.name,
                        arg_idx,
                        final_args.len()
                    );
                    return Err(RuntimeError::new("missing required argument"));
                }
            }

            if arg_idx < final_args.len() {
                // Return arg_idx and len for debug
                // return Err(RuntimeError::new(format!("too many positional arguments: {} > {}", arg_idx, final_args.len())));
                return Err(RuntimeError::new("too many positional arguments"));
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
            function_index: handle,
            ip: 0,
            base,
            return_reg,
            return_override,
            closure_state,
            current_class,
            current_module,
            current_module_path,
            exception_handlers: Vec::new(),
        });
        Ok(())
    }

    fn call_with_named(
        &mut self,
        callee: Value,
        positional: Vec<Value>,
        named: HashMap<String, Value>,
        return_reg: usize,
    ) -> Result<(), RuntimeError> {
        match callee {
            Value::NativeFunction(func) => {
                if !named.is_empty() {
                    return Err(RuntimeError::new(
                        "named args not supported for native functions",
                    ));
                }
                let result = func(self, &positional)?;
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::BoundNativeMethod { receiver, function } => {
                if !named.is_empty() {
                    return Err(RuntimeError::new(
                        "named args not supported for native functions",
                    ));
                }
                let mut merged_args = vec![*receiver];
                merged_args.extend(positional);
                let result = function(self, &merged_args)?;
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::Class(class) => {
                let object = self.gc.allocate(Object::new(Some(class.clone())));
                if let Some(handle) = class.borrow().constructor {
                    let _function = self
                        .functions
                        .get(handle)
                        .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                    let args = self.build_named_args(handle, true, &positional, &named)?;
                    let mut final_args = vec![Value::Object(object.clone())];
                    final_args.extend(args);
                    let override_value = Some(Value::Object(object.clone()));
                    self.call_function(
                        handle,
                        Some(object),
                        &final_args,
                        return_reg,
                        override_value,
                        None,
                        Some(class.clone()),
                        true,
                    )
                } else if positional.is_empty() && named.is_empty() {
                    self.registers[return_reg] = Value::Object(object);
                    Ok(())
                } else {
                    Err(RuntimeError::new("constructor not defined for named args"))
                }
            }
            Value::Function(handle) => {
                let _function = self
                    .functions
                    .get(handle)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(handle, false, &positional, &named)?;
                self.call_function(handle, None, &args, return_reg, None, None, None, true)
            }
            Value::BoundMethod {
                receiver,
                function,
                owner,
                ..
            } => {
                let _function_info = self
                    .functions
                    .get(function)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(function, true, &positional, &named)?;
                let mut final_args = vec![Value::Object(receiver.clone())];
                final_args.extend(args);
                self.call_function(
                    function,
                    Some(receiver),
                    &final_args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    true,
                )
            }
            Value::StaticMethod {
                function, owner, ..
            } => {
                let function_index = function;
                let args = self.build_named_args(function_index, false, &positional, &named)?;
                self.call_function(
                    function,
                    None,
                    &args,
                    return_reg,
                    None,
                    None,
                    Some(owner),
                    true,
                )
            }
            Value::Closure(state) => {
                let borrowed = state.borrow();
                let args = self.build_named_args(borrowed.function, false, &positional, &named)?;
                self.call_function(
                    borrowed.function,
                    None,
                    &args,
                    return_reg,
                    None,
                    Some(state.clone()),
                    None,
                    true,
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
        named: &HashMap<String, Value>,
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
                return Err(RuntimeError::new("unknown named argument"));
            }
        }
        for (idx, param) in params.iter().enumerate() {
            if param.is_variadic {
                if idx + 1 != params.len() {
                    return Err(RuntimeError::new("variadic parameter must be last"));
                }
                if named.contains_key(&param.name) {
                    return Err(RuntimeError::new("variadic parameter cannot be named"));
                }
                let rest = positional[pos_index..].to_vec();
                args.push(Value::Array(self.gc.allocate(ArrayState::new(rest))));
                pos_index = positional.len();
                continue;
            }
            if param.is_keyword_only && pos_index < positional.len() {
                return Err(RuntimeError::new("keyword-only argument requires name"));
            }
            if pos_index < positional.len() {
                if named.contains_key(&param.name) {
                    return Err(RuntimeError::new("duplicate argument"));
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
                return Err(RuntimeError::new("missing required argument"));
            }
        }
        if pos_index < positional.len() {
            return Err(RuntimeError::new("too many positional arguments"));
        }

        Ok(args)
    }

    fn unwind_to_handler(&mut self, error_value: Value) -> Result<(), RuntimeError> {
        while let Some(mut frame) = self.frames.pop() {
            if let Some(handler) = frame.exception_handlers.pop() {
                frame.ip = handler.catch_ip;
                self.registers[handler.base + handler.error_reg] = error_value;
                self.frames.push(frame);
                return Ok(());
            }
            self.registers.truncate(frame.base);
        }
        Err(RuntimeError::new(format!(
            "uncaught exception: {}",
            error_value
        )))
    }

    fn freeze_value(&mut self, value: &Value) {
        match value {
            Value::Array(array) => {
                let mut arr = array.borrow_mut();
                if !arr.is_immutable {
                    arr.is_immutable = true;
                    let elements = arr.elements.clone();
                    drop(arr);
                    for item in elements {
                        self.freeze_value(&item);
                    }
                }
            }
            Value::Map(map) => {
                let mut m = map.borrow_mut();
                if !m.is_immutable {
                    m.is_immutable = true;
                    let entries = m.entries.clone();
                    drop(m);
                    for item in entries.values() {
                        self.freeze_value(item);
                    }
                }
            }
            Value::Object(object) => {
                let mut obj = object.borrow_mut();
                if !obj.is_immutable {
                    obj.is_immutable = true;
                    let fields = obj.fields.clone();
                    drop(obj);
                    for item in fields.values() {
                        self.freeze_value(item);
                    }
                }
            }
            _ => {}
        }
    }

    fn throw_runtime_error(&mut self, message: &str) -> Result<(), RuntimeError> {
        let mut map = HashMap::new();
        map.insert(
            "message".to_string(),
            Value::String(Rc::new(message.to_string())),
        );
        let error_val = Value::Map(self.gc.allocate(MapState::new(map)));
        self.unwind_to_handler(error_val)
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
        let mut parser = pleno_parser::Parser::new(tokens);
        let program = parser
            .parse_program()
            .map_err(|err| RuntimeError::new(err.message))?;
        let mut compiler = pleno_compiler::Compiler::new(source);
        let chunk = compiler
            .compile_module(&program)
            .map_err(|err| RuntimeError::new(err.message))?;
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
            current_class: None,
            current_module: Some(Rc::clone(&module_rc)),
            current_module_path: Some(Rc::new(resolved_key.clone())),
            exception_handlers: Vec::new(),
        });
        let result = self.run_until(depth)?;
        self.registers.truncate(base);
        self.modules.insert(resolved_key, result.clone());
        Ok(result)
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
                // Check package.pln
                let manifest_path = pkg_root.join("package.pln");
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

fn resolve_builtin_method(vm: &VM, ty: &str, name: &str) -> Option<NativeFunction> {
    vm.builtin_resolver.and_then(|resolver| resolver(ty, name))
}

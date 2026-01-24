use chrono::{DateTime as ChronoDateTime, TimeZone, Utc};
use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    Class, MethodInfo, MethodVisibility, MultiWaiter, MultiWaiterMode, NativeFunction, Object,
    Range, RuntimeError, VMProvider, Value, ValueKey,
    gc::{GcObject, GcRc, MarkAndSweepGC},
};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::rc::Rc;

mod concurrency;
use concurrency::*;

#[derive(Debug, Clone)]
pub struct AsyncTask {
    pub promise_id: u64,
    pub value: Value,
    pub is_async: bool,
    pub is_resolved: bool,
}

#[derive(Debug, Clone)]
pub struct SuspendedFrame {
    pub frame: CallFrame,
    pub await_reg: usize,
}

#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    pub catch_ip: usize,
    pub base: usize,
    pub error_reg: usize,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_index: usize,
    pub ip: usize,
    pub base: usize,
    pub return_reg: usize,
    pub return_override: Option<Value>,
    pub captures: Vec<Value>,
    pub current_class: Option<GcRc<Class>>,
    pub current_module: Option<Rc<String>>,
    pub current_module_path: Option<Rc<String>>,
    pub exception_handlers: Vec<ExceptionHandler>,
}

pub struct VM {
    pub gc: pleno_value::gc::GCManager,
    registers: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    modules: HashMap<String, Value>,
    functions: Vec<Function>,
    pub pending_tasks: Vec<AsyncTask>,
    pub suspended_frames: HashMap<u64, SuspendedFrame>,
    pub promises: HashMap<u64, Value>,
    pub next_promise_id: u64,
    pub multi_waiters: Vec<MultiWaiter>,
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
}

impl VMProvider for VM {
    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let return_reg = self.registers.len();
        self.registers.push(Value::Null); // Placeholder for return value

        match callee {
            Value::Function(handle) => {
                self.call_function(
                    handle,
                    None,
                    &args,
                    return_reg,
                    None,
                    Vec::new(),
                    None,
                    false,
                )?;
            }
            Value::Closure { function, captures } => {
                self.call_function(
                    function, None, &args, return_reg, None, captures, None, false,
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
                    Vec::new(),
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
        let promise = Value::Map(self.gc.allocate(map));
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
}

impl GcObject for AsyncTask {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.value.mark(gc);
    }
}

impl GcObject for SuspendedFrame {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.frame.mark(gc);
    }
}

impl GcObject for CallFrame {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        if let Some(val) = &self.return_override {
            val.mark(gc);
        }
        for capture in &self.captures {
            capture.mark(gc);
        }
        if let Some(class) = &self.current_class {
            gc.mark_object(&class.to_gc_object());
        }
    }
}

struct VmRoots<'a> {
    registers: &'a [Value],
    frames: &'a [CallFrame],
    globals: &'a HashMap<String, Value>,
    modules: &'a HashMap<String, Value>,
    pending_tasks: &'a [AsyncTask],
    suspended_frames: &'a HashMap<u64, SuspendedFrame>,
    promises: &'a HashMap<u64, Value>,
    multi_waiters: &'a [MultiWaiter],
}

impl<'a> GcObject for VmRoots<'a> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for reg in self.registers {
            reg.mark(gc);
        }
        for frame in self.frames {
            frame.mark(gc);
        }
        for val in self.globals.values() {
            val.mark(gc);
        }
        for val in self.modules.values() {
            val.mark(gc);
        }
        for task in self.pending_tasks {
            task.mark(gc);
        }
        for promise in self.promises.values() {
            promise.mark(gc);
        }
        for frame in self.suspended_frames.values() {
            frame.mark(gc);
        }
        for waiter in self.multi_waiters {
            waiter.mark(gc);
        }
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> Self {
        let mut gc = pleno_value::gc::GCManager::new(1000);
        let mut globals = HashMap::new();
        globals.insert("print".to_string(), Value::NativeFunction(native_print));
        globals.insert("println".to_string(), Value::NativeFunction(native_print));
        globals.insert("id".to_string(), Value::NativeFunction(native_id));
        // Result type constructors
        globals.insert("Ok".to_string(), Value::NativeFunction(native_result_ok));
        globals.insert("Err".to_string(), Value::NativeFunction(native_result_err));
        // Option type constructors
        globals.insert(
            "Some".to_string(),
            Value::NativeFunction(native_option_some),
        );
        let mut none_map = HashMap::new();
        none_map.insert(
            "variant".to_string(),
            Value::String(Rc::new("None".to_string())),
        );
        globals.insert("None".to_string(), Value::Map(gc.allocate(none_map)));
        globals.insert(
            "Promise".to_string(),
            Value::NativeFunction(native_promise_constructor),
        );

        // Math native functions
        globals.insert(
            "__native_math_abs".to_string(),
            Value::NativeFunction(native_math_abs),
        );
        globals.insert(
            "__native_math_pow".to_string(),
            Value::NativeFunction(native_math_pow),
        );
        globals.insert(
            "__native_math_sqrt".to_string(),
            Value::NativeFunction(native_math_sqrt),
        );
        globals.insert(
            "__native_math_random".to_string(),
            Value::NativeFunction(native_math_random),
        );
        globals.insert(
            "__native_math_random_int".to_string(),
            Value::NativeFunction(native_math_random_int),
        );
        globals.insert(
            "__native_math_sin".to_string(),
            Value::NativeFunction(native_math_sin),
        );
        globals.insert(
            "__native_math_cos".to_string(),
            Value::NativeFunction(native_math_cos),
        );
        globals.insert(
            "__native_math_tan".to_string(),
            Value::NativeFunction(native_math_tan),
        );
        globals.insert(
            "__native_math_log".to_string(),
            Value::NativeFunction(native_math_log),
        );
        globals.insert(
            "__native_math_exp".to_string(),
            Value::NativeFunction(native_math_exp),
        );
        globals.insert(
            "__native_math_floor".to_string(),
            Value::NativeFunction(native_math_floor),
        );
        globals.insert(
            "__native_math_ceil".to_string(),
            Value::NativeFunction(native_math_ceil),
        );
        globals.insert(
            "__native_math_round".to_string(),
            Value::NativeFunction(native_math_round),
        );

        // Regex native functions
        globals.insert(
            "__native_regex_is_match".to_string(),
            Value::NativeFunction(native_regex_is_match),
        );
        globals.insert(
            "__native_regex_match".to_string(),
            Value::NativeFunction(native_regex_match),
        );
        globals.insert(
            "__native_regex_find_all".to_string(),
            Value::NativeFunction(native_regex_find_all),
        );
        globals.insert(
            "__native_regex_replace".to_string(),
            Value::NativeFunction(native_regex_replace),
        );

        // JSON native functions
        globals.insert(
            "__native_json_parse".to_string(),
            Value::NativeFunction(native_json_parse),
        );
        globals.insert(
            "__native_json_stringify".to_string(),
            Value::NativeFunction(native_json_stringify),
        );
        globals.insert(
            "__native_json_stringify_pretty".to_string(),
            Value::NativeFunction(native_json_stringify_pretty),
        );

        // I/O native functions
        globals.insert(
            "__native_file_read".to_string(),
            Value::NativeFunction(native_file_read),
        );
        globals.insert(
            "__native_file_write".to_string(),
            Value::NativeFunction(native_file_write),
        );
        globals.insert(
            "__native_file_exists".to_string(),
            Value::NativeFunction(native_file_exists),
        );
        globals.insert(
            "__native_directory_create".to_string(),
            Value::NativeFunction(native_directory_create),
        );
        globals.insert(
            "__native_directory_exists".to_string(),
            Value::NativeFunction(native_directory_exists),
        );

        // OS native functions
        globals.insert(
            "__native_os_exit".to_string(),
            Value::NativeFunction(native_os_exit),
        );
        globals.insert(
            "__native_os_getenv".to_string(),
            Value::NativeFunction(native_os_getenv),
        );
        globals.insert(
            "__native_os_args".to_string(),
            Value::NativeFunction(native_os_args),
        );
        globals.insert(
            "__native_os_cwd".to_string(),
            Value::NativeFunction(native_os_cwd),
        );
        globals.insert(
            "__native_os_platform".to_string(),
            Value::NativeFunction(native_os_platform),
        );
        globals.insert(
            "__native_http_get".to_string(),
            Value::NativeFunction(native_http_get),
        );
        globals.insert(
            "__native_http_post".to_string(),
            Value::NativeFunction(native_http_post),
        );
        globals.insert(
            "__native_http_put".to_string(),
            Value::NativeFunction(native_http_put),
        );
        globals.insert(
            "__native_http_delete".to_string(),
            Value::NativeFunction(native_http_delete),
        );

        // Populate Math object
        let mut math_map = HashMap::new();
        math_map.insert("abs".to_string(), Value::NativeFunction(native_math_abs));
        math_map.insert("pow".to_string(), Value::NativeFunction(native_math_pow));
        math_map.insert("sqrt".to_string(), Value::NativeFunction(native_math_sqrt));
        math_map.insert(
            "random".to_string(),
            Value::NativeFunction(native_math_random),
        );
        math_map.insert(
            "randomInt".to_string(),
            Value::NativeFunction(native_math_random_int),
        );
        math_map.insert("sin".to_string(), Value::NativeFunction(native_math_sin));
        math_map.insert("cos".to_string(), Value::NativeFunction(native_math_cos));
        math_map.insert("tan".to_string(), Value::NativeFunction(native_math_tan));
        math_map.insert("log".to_string(), Value::NativeFunction(native_math_log));
        math_map.insert("exp".to_string(), Value::NativeFunction(native_math_exp));
        math_map.insert(
            "floor".to_string(),
            Value::NativeFunction(native_math_floor),
        );
        math_map.insert("ceil".to_string(), Value::NativeFunction(native_math_ceil));
        math_map.insert(
            "round".to_string(),
            Value::NativeFunction(native_math_round),
        );
        globals.insert("Math".to_string(), Value::Map(gc.allocate(math_map)));

        // Populate JSON object
        let mut json_map = HashMap::new();
        json_map.insert(
            "parse".to_string(),
            Value::NativeFunction(native_json_parse),
        );
        json_map.insert(
            "stringify".to_string(),
            Value::NativeFunction(native_json_stringify),
        );
        json_map.insert(
            "stringifyPretty".to_string(),
            Value::NativeFunction(native_json_stringify_pretty),
        );
        globals.insert("JSON".to_string(), Value::Map(gc.allocate(json_map)));

        // Populate OS object
        let mut os_map = HashMap::new();
        os_map.insert("exit".to_string(), Value::NativeFunction(native_os_exit));
        os_map.insert(
            "getenv".to_string(),
            Value::NativeFunction(native_os_getenv),
        );
        os_map.insert("args".to_string(), Value::NativeFunction(native_os_args));
        os_map.insert("cwd".to_string(), Value::NativeFunction(native_os_cwd));
        os_map.insert(
            "platform".to_string(),
            Value::NativeFunction(native_os_platform),
        );
        globals.insert("OS".to_string(), Value::Map(gc.allocate(os_map)));

        // Time native functions
        globals.insert(
            "__native_datetime_now".to_string(),
            Value::NativeFunction(native_datetime_now),
        );
        globals.insert(
            "__native_datetime_format".to_string(),
            Value::NativeFunction(native_datetime_format),
        );
        globals.insert(
            "__native_datetime_parse".to_string(),
            Value::NativeFunction(native_datetime_parse),
        );

        // Async primitives
        globals.insert(
            "spawn".to_string(),
            Value::NativeFunction(native_async_spawn),
        );
        let mut promise_map = HashMap::new();
        promise_map.insert(
            "__call__".to_string(),
            Value::NativeFunction(native_promise_constructor),
        );
        promise_map.insert("all".to_string(), Value::NativeFunction(native_promise_all));
        promise_map.insert(
            "race".to_string(),
            Value::NativeFunction(native_promise_race),
        );
        globals.insert("Promise".to_string(), Value::Map(gc.allocate(promise_map)));

        // Debug shared store for concurrency tests
        globals.insert(
            "__TestShared".to_string(),
            Value::Map(gc.allocate(HashMap::new())),
        );

        Self {
            gc,
            registers: vec![Value::Null; 256],
            frames: Vec::new(),
            globals,
            modules: HashMap::new(),
            functions: Vec::new(),
            pending_tasks: Vec::new(),
            suspended_frames: HashMap::new(),
            promises: HashMap::new(),
            next_promise_id: 1,
            multi_waiters: Vec::new(),
        }
    }

    pub fn execute(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        let register_count = chunk.max_registers;
        let function = Function {
            name: Rc::new("<script>".to_string()),
            chunk,
            arity: 0,
            params: Vec::new(),
            module: None,
            module_path: None,
            is_async: false,
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
            captures: Vec::new(),
            current_class: None,
            current_module: None,
            current_module_path: None,
            exception_handlers: Vec::new(),
        });

        let mut last_result = Value::Null;
        while !self.frames.is_empty() || !self.pending_tasks.is_empty() {
            if !self.frames.is_empty() {
                last_result = self.run_until(0)?;
            }
            if !self.pending_tasks.is_empty() {
                self.process_tasks()?;
            }
        }
        Ok(last_result)
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
                                let results_val = Value::Array(gc.allocate(waiter.results.clone()));
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
                        name: Rc::new("<fn>".to_string()),
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
                    });
                    let captures = vec![Value::Null; c];
                    self.registers[base + a] = Value::Closure {
                        function: func_index,
                        captures,
                    };
                }
                Opcode::AsyncFunction => {
                    let proto = function
                        .chunk
                        .functions
                        .get(b)
                        .ok_or_else(|| RuntimeError::new("unknown function prototype"))?;
                    let func_index = self.functions.len();
                    self.functions.push(Function {
                        name: Rc::new("<async fn>".to_string()),
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
                        name: Rc::new("<fn>".to_string()),
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
                    });
                    self.registers[base + a] = Value::Function(func_index);
                }
                Opcode::Capture => {
                    let value = self.registers[base + b].clone();
                    let closure = match &mut self.registers[base + a] {
                        Value::Closure { captures, .. } => captures,
                        _ => return Err(RuntimeError::new("capture on non-closure")),
                    };
                    let idx = c;
                    if idx >= closure.len() {
                        return Err(RuntimeError::new("capture index out of bounds"));
                    }
                    closure[idx] = value;
                }
                Opcode::LoadCapture => {
                    let idx = b;
                    if idx >= frame.captures.len() {
                        return Err(RuntimeError::new("capture index out of bounds"));
                    }
                    self.registers[base + a] = frame.captures[idx].clone();
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
                    self.registers[base + a] = Value::Map(self.gc.allocate(HashMap::new()));
                }
                Opcode::NewArray => {
                    self.registers[base + a] = Value::Array(self.gc.allocate(Vec::new()));
                }
                Opcode::ArrayAppend => {
                    let value = self.registers[base + b].clone();
                    match &self.registers[base + a] {
                        Value::Array(array) => {
                            array.borrow_mut().push(value);
                        }
                        _ => return Err(RuntimeError::new("array append on non-array value")),
                    }
                }
                Opcode::ArraySpread => {
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
                        (Value::Array(target), Value::Array(source)) => {
                            target.borrow_mut().extend(source.borrow().iter().cloned());
                        }
                        _ => return Err(RuntimeError::new("array spread expects array values")),
                    }
                }
                Opcode::MapSpread => {
                    let value = self.registers[base + b].clone();
                    match (&self.registers[base + a], value) {
                        (Value::Map(target), Value::Map(source)) => {
                            for (k, v) in source.borrow().iter() {
                                target.borrow_mut().insert(k.clone(), v.clone());
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
                    self.registers[base + a] = Value::Array(self.gc.allocate(values));
                }
                Opcode::NewObject => {
                    self.registers[base + a] = Value::Object(self.gc.allocate(Object {
                        class: None,
                        fields: HashMap::new(),
                    }));
                }
                Opcode::SetField => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("field name must be string")),
                    };
                    let value = self.registers[base + a].clone();
                    match &self.registers[base + b] {
                        Value::Object(object) => {
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
                    self.registers[base + a] = Value::Array(self.gc.allocate(keys));
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
                            Value::Array(self.gc.allocate(values))
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
                            Value::Array(self.gc.allocate(values))
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
                                    Value::Array(
                                        self.gc.allocate(vec![Value::Int(idx as i64), val.clone()]),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(entries))
                        }
                        Value::Map(map) => {
                            let entries = map
                                .borrow()
                                .iter()
                                .map(|(key, val)| {
                                    Value::Array(self.gc.allocate(vec![
                                        Value::String(Rc::new(key.clone())),
                                        val.clone(),
                                    ]))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(entries))
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
                                    Value::Array(
                                        self.gc.allocate(vec![Value::Int(idx as i64), val]),
                                    )
                                })
                                .collect::<Vec<_>>();
                            Value::Array(self.gc.allocate(entries))
                        }
                        _ => return Err(RuntimeError::new("iter over non-collection value")),
                    };
                    self.registers[base + a] = value;
                }
                Opcode::ArrayLen => {
                    let len = match &self.registers[base + b] {
                        Value::Array(array) => array.borrow().len() as i64,
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
                    let type_name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str(),
                        _ => return Err(RuntimeError::new("type name must be string")),
                    };
                    let result = matches!(
                        (type_name, value),
                        ("Int", Value::Int(_))
                            | ("Float", Value::Float(_))
                            | ("Bool", Value::Bool(_))
                            | ("String", Value::String(_))
                            | ("Array", Value::Array(_))
                            | ("Map", Value::Map(_))
                            | ("Set", Value::Set(_))
                            | ("Object", Value::Object(_))
                            | ("Null", Value::Null)
                    );
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
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let value = self.registers[base + a].clone();
                    let current_class = frame.current_class.as_ref().cloned();
                    let current_module = frame.current_module.as_ref().cloned();
                    match &self.registers[base + b] {
                        Value::Map(map) => {
                            map.borrow_mut().insert(name, value);
                        }
                        Value::Object(object) => {
                            let class = object.borrow().class.clone();
                            if let Some(class) = class.as_ref() {
                                let setter_name = format!("set:{name}");
                                if let Some(info) = resolve_method(class, &setter_name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        current_module.as_ref(),
                                        &self.globals,
                                    ) {
                                        return Err(RuntimeError::new("method access denied"));
                                    }
                                    self.call_function(
                                        info.function,
                                        Some(object.clone()),
                                        &[value],
                                        base + a,
                                        None,
                                        Vec::new(),
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
                                        &self.globals,
                                    ) {
                                        return Err(RuntimeError::new("field access denied"));
                                    }
                                }
                            }
                            object.borrow_mut().fields.insert(name, value);
                        }
                        Value::Class(class) => {
                            let setter_name = format!("set:{name}");
                            if let Some(info) = resolve_static_method(class, &setter_name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                    &self.globals,
                                ) {
                                    return Err(RuntimeError::new("static method access denied"));
                                }
                                self.call_function(
                                    info.function,
                                    None,
                                    &[value],
                                    base + a,
                                    None,
                                    Vec::new(),
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
                                    &self.globals,
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
                                    && resolve_builtin_method("Result", &name).is_some()
                                {
                                    // Result type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method("Result", &name).unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && resolve_builtin_method("Option", &name).is_some()
                                {
                                    // Option type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method("Option", &name).unwrap(),
                                    };
                                    continue;
                                } else if (variant.as_str() == "Pending"
                                    || variant.as_str() == "Resolved")
                                    && resolve_builtin_method("Promise", &name).is_some()
                                {
                                    // Promise type with method
                                    drop(map_ref);
                                    self.registers[base + a] = Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method("Promise", &name).unwrap(),
                                    };
                                    continue;
                                }
                            }
                            // Check for Map built-in methods if not found as a key
                            let val = map_ref.get(&name).cloned();
                            drop(map_ref);

                            if let Some(val) = val {
                                val
                            } else if let Some(func) = resolve_builtin_method("Map", &name) {
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
                                let getter_name = format!("get:{name}");
                                if let Some(info) = resolve_method(class, &getter_name) {
                                    if !can_access_method(
                                        &info,
                                        current_class.as_ref(),
                                        frame.current_module.as_ref(),
                                        &self.globals,
                                    ) {
                                        return Err(RuntimeError::new("method access denied"));
                                    }
                                    self.call_function(
                                        info.function,
                                        Some(object.clone()),
                                        &[],
                                        base + a,
                                        None,
                                        Vec::new(),
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
                                        &self.globals,
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
                                        &self.globals,
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
                            let getter_name = format!("get:{name}");
                            if let Some(info) = resolve_static_method(class, &getter_name) {
                                if !can_access_method(
                                    &info,
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                    &self.globals,
                                ) {
                                    return Err(RuntimeError::new("static method access denied"));
                                }
                                self.call_function(
                                    info.function,
                                    None,
                                    &[],
                                    base + a,
                                    None,
                                    Vec::new(),
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
                                    &self.globals,
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
                                    &self.globals,
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
                            } else if let Some(func) = resolve_builtin_method("Set", &name) {
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
                            } else if let Some(func) = resolve_builtin_method("Array", &name) {
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
                            } else if let Some(func) = resolve_builtin_method("String", &name) {
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
                            } else if let Some(func) = resolve_builtin_method("Array", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Array(array.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Mutex(m) => {
                            if let Some(func) = resolve_builtin_method("Mutex", &name) {
                                Value::BoundNativeMethod {
                                    receiver: Box::new(Value::Mutex(m.clone())),
                                    function: func,
                                }
                            } else {
                                Value::Null
                            }
                        }
                        Value::Channel(c) => {
                            if let Some(func) = resolve_builtin_method("Channel", &name) {
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
                Opcode::GetIndex => {
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    let value = match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            let array_borrow = array.borrow();
                            if idx >= array_borrow.len() {
                                return Err(RuntimeError::new("array index out of bounds"));
                            }
                            array_borrow[idx].clone()
                        }
                        (Value::Map(map), Value::String(key)) => map
                            .borrow()
                            .get(key.as_str())
                            .cloned()
                            .unwrap_or(Value::Null),
                        (Value::Set(set), value) => {
                            let key = ValueKey::try_from_value(&value)
                                .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                            Value::Bool(set.borrow().contains(&key))
                        }
                        _ => return Err(RuntimeError::new("invalid index operation")),
                    };
                    self.registers[base + a] = value;
                }
                Opcode::SetIndex => {
                    let value = self.registers[base + a].clone();
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            let mut array = array.borrow_mut();
                            if idx >= array.len() {
                                return Err(RuntimeError::new("array index out of bounds"));
                            }
                            array[idx] = value;
                        }
                        (Value::Map(map), Value::String(key)) => {
                            map.borrow_mut().insert(key.as_str().to_string(), value);
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
                        Value::Map(map) => map.borrow().clone(),
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
                        module: module.clone(),
                    });

                    // Now parse methods with class reference
                    let mut methods = HashMap::new();
                    let mut constructor = None;
                    for (method_name, value) in methods_map {
                        let info = parse_method_info(value, &class, module.as_ref())?;
                        if method_name == "constructor" {
                            constructor = Some(info.function);
                        } else {
                            methods.insert(method_name, info);
                        }
                    }

                    // Update the class with parsed methods
                    class.borrow_mut().methods = methods;
                    class.borrow_mut().constructor = constructor;

                    self.registers[base + a] = Value::Class(class);
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
                Opcode::SetStatic => {
                    let static_map = match &self.registers[base + b] {
                        Value::Map(map) => map.borrow().clone(),
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
                        match current_callee {
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
                                    Vec::new(),
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
                                    Vec::new(),
                                    None,
                                    false,
                                )?;
                                break;
                            }
                            Value::Closure { function, captures } => {
                                self.call_function(
                                    function,
                                    None,
                                    &args,
                                    base + a,
                                    None,
                                    captures,
                                    None,
                                    false,
                                )?;
                                break;
                            }
                            Value::NativeFunction(func) => {
                                let result = func(self, &args)?;
                                self.registers[base + a] = result;
                                break;
                            }
                            Value::BoundNativeMethod { receiver, function } => {
                                let mut merged_args = vec![*receiver];
                                merged_args.extend(args);
                                let result = function(self, &merged_args)?;
                                self.registers[base + a] = result;
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
                                    Vec::new(),
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
                                    Vec::new(),
                                    Some(owner),
                                    false,
                                )?;
                                break;
                            }
                            Value::Class(class) => {
                                let object = self.gc.allocate(Object {
                                    class: Some(class.clone()),
                                    fields: HashMap::new(),
                                });
                                if let Some(handle) = class.borrow().constructor {
                                    self.call_function(
                                        handle,
                                        Some(object.clone()),
                                        &args,
                                        base + a,
                                        Some(Value::Object(object.clone())),
                                        Vec::new(),
                                        Some(class.clone()),
                                        false,
                                    )?;
                                } else {
                                    self.registers[base + a] = Value::Object(object);
                                }
                                break;
                            }
                            Value::Map(map) => {
                                let callable = map.borrow().get("__call__").cloned();
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
                        Value::Array(array) => array.borrow().clone(),
                        _ => return Err(RuntimeError::new("call named positional must be array")),
                    };
                    let named = match &wrapper[1] {
                        Value::Map(map) => map.borrow().clone(),
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
                        Value::Closure { function, .. } => function,
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
                            let len = array.len() as i64;
                            let start = resolve_index(start, len, 0);
                            let end = resolve_index(end, len, len);
                            let slice = array[start..end].to_vec();
                            self.registers[base + a] = Value::Array(self.gc.allocate(slice));
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
        captures: Vec<Value>,
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
                let promise = Value::Map(self.gc.allocate(promise_map));
                self.promises.insert(promise_id, promise.clone());

                // Set return register to Promise immediately
                self.registers[return_reg] = promise;
            }

            for param in function.params.iter() {
                if param.is_variadic {
                    let rest = final_args[arg_idx..].to_vec();
                    processed.push(Value::Array(self.gc.allocate(rest)));
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
            captures,
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
                let object = self.gc.allocate(Object {
                    class: Some(class.clone()),
                    fields: HashMap::new(),
                });
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
                        Vec::new(),
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
                self.call_function(
                    handle,
                    None,
                    &args,
                    return_reg,
                    None,
                    Vec::new(),
                    None,
                    true,
                )
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
                    Vec::new(),
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
                    Vec::new(),
                    Some(owner),
                    true,
                )
            }
            Value::Closure { function, captures } => {
                let function_index = function;
                let args = self.build_named_args(function_index, false, &positional, &named)?;
                self.call_function(
                    function, None, &args, return_reg, None, captures, None, true,
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
                args.push(Value::Array(self.gc.allocate(rest)));
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

    fn execute_module(&mut self, path: &str) -> Result<Value, RuntimeError> {
        if path == "std:sync" {
            if let Some(mod_val) = self.modules.get("std:sync") {
                return Ok(mod_val.clone());
            }

            let mut map = HashMap::new();

            let mutex_fn = Value::NativeFunction(native_mutex_new);
            let channel_fn = Value::NativeFunction(native_channel_new);

            map.insert("Mutex".to_string(), mutex_fn);
            map.insert("Channel".to_string(), channel_fn);

            let module = Value::Map(self.gc.allocate(map));
            self.modules.insert("std:sync".to_string(), module.clone());
            return Ok(module);
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
        let mut compiler = pleno_compiler::Compiler::new();
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
            captures: Vec::new(),
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

    if let Some(name) = path.strip_prefix("registry:") {
        let current_dir = std::env::current_dir()
            .map_err(|err| RuntimeError::new(format!("failed to get current dir: {err}")))?;
        let pkg_path = current_dir
            .join("sources/packages")
            .join(format!("{}.pln", name));
        if pkg_path.exists() {
            return Ok(pkg_path);
        }
        let pkg_index = current_dir
            .join("sources/packages")
            .join(name)
            .join("index.pln");
        if pkg_index.exists() {
            return Ok(pkg_index);
        }
        return Err(RuntimeError::new(format!(
            "registry package not found: {}",
            name
        )));
    }

    let raw_path = std::path::Path::new(path);
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }
    let mut candidates = Vec::new();
    let current_dir = std::env::current_dir()
        .map_err(|err| RuntimeError::new(format!("failed to get current dir: {err}")))?;
    if let Some(base) = base_dir.as_ref() {
        candidates.push(base.join(raw_path));
    }
    candidates.push(current_dir.join(raw_path));
    if raw_path.extension().is_none() {
        if let Some(base) = base_dir.as_ref() {
            candidates.push(base.join(format!("{path}.pln")));
        }
        candidates.push(current_dir.join(format!("{path}.pln")));
    }
    if !path.contains('/') && !path.contains('\\') {
        candidates.push(current_dir.join("sources").join(format!("{path}.pln")));
    }
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(RuntimeError::new(format!("module not found: {path}")))
}

fn native_os_exit(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let code = match args.first() {
        Some(Value::Int(code)) => *code as i32,
        _ => 0,
    };
    std::process::exit(code);
}

fn native_os_getenv(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let name = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("getenv expects string argument")),
    };
    match std::env::var(name) {
        Ok(val) => Ok(Value::String(Rc::new(val))),
        Err(_) => Ok(Value::Null),
    }
}

fn native_os_args(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let args: Vec<Value> = std::env::args()
        .map(|arg| Value::String(Rc::new(arg)))
        .collect();
    Ok(Value::Array(gc.allocate(args)))
}

fn native_os_cwd(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let cwd = std::env::current_dir()
        .map_err(|e| RuntimeError::new(format!("failed to get cwd: {}", e)))?;
    Ok(Value::String(Rc::new(cwd.to_string_lossy().into_owned())))
}

fn native_os_platform(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    Ok(Value::String(Rc::new(std::env::consts::OS.to_string())))
}

fn native_http_get(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("get() expects URL string")),
    };
    let response = reqwest::blocking::get(url)
        .map_err(|err| RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("failed to read response body: {err}")))?;
    Ok(Value::String(Rc::new(body)))
}

fn native_http_post(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("post() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .body(body.to_string())
        .send()
        .map_err(|err| RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("failed to read response body: {err}")))?;
    Ok(Value::String(Rc::new(body)))
}

fn native_http_put(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("put() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .put(url)
        .body(body.to_string())
        .send()
        .map_err(|err| RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("failed to read response body: {err}")))?;
    Ok(Value::String(Rc::new(body)))
}

fn native_http_delete(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("delete() expects URL string")),
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .delete(url)
        .send()
        .map_err(|err| RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("failed to read response body: {err}")))?;
    Ok(Value::String(Rc::new(body)))
}

fn native_print(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{}", value));
    }
    println!("{out}");
    let _ = std::io::stdout().flush();
    Ok(Value::Null)
}

fn native_id(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.first().cloned().unwrap_or(Value::Null))
}

fn resolve_method(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().methods.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_method(superclass, name))
}

fn resolve_static_method(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().static_methods.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_static_method(superclass, name))
}

fn parse_method_info(
    value: Value,
    owner: &GcRc<Class>,
    module: Option<&Rc<String>>,
) -> Result<MethodInfo, RuntimeError> {
    match value {
        Value::Function(handle) => Ok(MethodInfo {
            function: handle,
            visibility: MethodVisibility::Public,
            owner: owner.clone(),
            module: module.cloned(),
        }),
        Value::Array(array) => {
            let items = array.borrow();
            if items.len() != 2 {
                return Err(RuntimeError::new("method metadata must have 2 items"));
            }
            let handle = match &items[0] {
                Value::Function(handle) => *handle,
                _ => return Err(RuntimeError::new("method entry must be function")),
            };
            let visibility = match &items[1] {
                Value::String(name) if name.as_str() == "public" => MethodVisibility::Public,
                Value::String(name) if name.as_str() == "private" => MethodVisibility::Private,
                Value::String(name) if name.as_str() == "protected" => MethodVisibility::Protected,
                Value::String(name) if name.as_str() == "internal" => MethodVisibility::Internal,
                _ => return Err(RuntimeError::new("invalid method visibility")),
            };
            Ok(MethodInfo {
                function: handle,
                visibility,
                owner: owner.clone(),
                module: module.cloned(),
            })
        }
        _ => Err(RuntimeError::new(
            "method entry must be function or metadata array",
        )),
    }
}

fn can_access_method(
    method: &MethodInfo,
    current_class: Option<&GcRc<Class>>,
    current_module: Option<&Rc<String>>,
    globals: &HashMap<String, Value>,
) -> bool {
    match method.visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class.map(|c| c == &method.owner).unwrap_or(false),
        MethodVisibility::Protected => current_class
            .map(|c| is_subclass(c, &method.owner, globals))
            .unwrap_or(false),
        MethodVisibility::Internal => method
            .module
            .as_ref()
            .and_then(|module| current_module.map(|current| module.as_str() == current.as_str()))
            .unwrap_or(false),
    }
}

fn can_access_field(
    visibility: &MethodVisibility,
    owner: &Rc<String>,
    owner_module: Option<&Rc<String>>,
    current_class: Option<&GcRc<Class>>,
    current_module: Option<&Rc<String>>,
    _globals: &HashMap<String, Value>,
) -> bool {
    match visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class
            .map(|c| c.borrow().name.as_str() == owner.as_str())
            .unwrap_or(false),
        MethodVisibility::Protected => current_class
            .map(|c| {
                let class_name = c.borrow().name.to_string();
                class_name == owner.as_str() || {
                    // Check if current class is a subclass by walking the superclass chain
                    let mut current = c.borrow().superclass.clone();
                    while let Some(sc) = current {
                        if sc.borrow().name.as_str() == owner.as_str() {
                            return true;
                        }
                        current = sc.borrow().superclass.clone();
                    }
                    false
                }
            })
            .unwrap_or(false),
        MethodVisibility::Internal => owner_module
            .as_ref()
            .and_then(|module| current_module.map(|current| module.as_str() == current.as_str()))
            .unwrap_or(false),
    }
}

fn visibility_from_code(code: u8) -> Result<MethodVisibility, RuntimeError> {
    match code {
        0 => Ok(MethodVisibility::Public),
        1 => Ok(MethodVisibility::Private),
        2 => Ok(MethodVisibility::Protected),
        3 => Ok(MethodVisibility::Internal),
        _ => Err(RuntimeError::new("invalid visibility code")),
    }
}

fn is_subclass(
    class: &GcRc<Class>,
    superclass: &GcRc<Class>,
    _globals: &HashMap<String, Value>,
) -> bool {
    is_subclass_of(class, superclass)
}

fn is_subclass_of(class: &GcRc<Class>, target: &GcRc<Class>) -> bool {
    if class == target {
        return true;
    }
    let mut current = class.borrow().superclass.clone();
    while let Some(super_class) = current {
        if &super_class == target {
            return true;
        }
        current = super_class.borrow().superclass.clone();
    }
    false
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Missing => false,
        Value::Bool(value) => *value,
        _ => true,
    }
}

fn add_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (&a, &b) {
        (Value::Int(a_val), Value::Int(b_val)) => Ok(Value::Int(a_val + b_val)),
        (Value::Float(a_val), Value::Float(b_val)) => Ok(Value::Float(a_val + b_val)),
        (Value::Int(a_val), Value::Float(b_val)) => Ok(Value::Float(*a_val as f64 + b_val)),
        (Value::Float(a_val), Value::Int(b_val)) => Ok(Value::Float(a_val + *b_val as f64)),
        (Value::String(a_str), Value::String(b_str)) => {
            let mut res = a_str.as_str().to_string();
            res.push_str(b_str.as_str());
            Ok(Value::String(Rc::new(res)))
        }
        (Value::String(a_str), _) => {
            let mut res = a_str.as_str().to_string();
            res.push_str(&b.to_string());
            Ok(Value::String(Rc::new(res)))
        }
        (_, Value::String(b_str)) => {
            let mut res = a.to_string();
            res.push_str(b_str.as_str());
            Ok(Value::String(Rc::new(res)))
        }
        _ => Err(RuntimeError::new("invalid operands for +")),
    }
}

fn sub_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),
        _ => Err(RuntimeError::new("invalid operands for -")),
    }
}

fn mul_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),
        _ => Err(RuntimeError::new("invalid operands for *")),
    }
}

fn div_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
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

fn mod_values(a: Value, b: Value) -> Result<Value, RuntimeError> {
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

fn neg_value(value: Value) -> Result<Value, RuntimeError> {
    match value {
        Value::Int(value) => Ok(Value::Int(-value)),
        Value::Float(value) => Ok(Value::Float(-value)),
        _ => Err(RuntimeError::new("invalid operand for unary -")),
    }
}

fn expect_int_or_null(value: &Value) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Int(value) => Ok(Some(*value)),
        Value::Null => Ok(None),
        _ => Err(RuntimeError::new("expected integer or null")),
    }
}

fn resolve_index(index: Option<i64>, len: i64, default_val: i64) -> usize {
    let idx = index.unwrap_or(default_val);
    if idx < 0 {
        (len + idx).max(0) as usize
    } else {
        idx.min(len) as usize
    }
}

fn compare_values(opcode: Opcode, left: Value, right: Value) -> Result<bool, RuntimeError> {
    let (left, right) = match (left, right) {
        (Value::Int(a), Value::Int(b)) => (a as f64, b as f64),
        (Value::Float(a), Value::Float(b)) => (a, b),
        (Value::Int(a), Value::Float(b)) => (a as f64, b),
        (Value::Float(a), Value::Int(b)) => (a, b as f64),
        _ => return Err(RuntimeError::new("invalid operands for comparison")),
    };

    let result = match opcode {
        Opcode::Lt => left < right,
        Opcode::Le => left <= right,
        Opcode::Gt => left > right,
        Opcode::Ge => left >= right,
        _ => return Err(RuntimeError::new("invalid comparison opcode")),
    };
    Ok(result)
}

fn decode_offset(b: u8, c: u8) -> i16 {
    let hi = (b as i16) << 8;
    let lo = c as i16;
    hi | lo
}

fn apply_offset(ip: usize, offset: i16) -> Result<usize, RuntimeError> {
    let next = ip as isize + offset as isize;
    if next < 0 {
        return Err(RuntimeError::new("jump offset out of bounds"));
    }
    Ok(next as usize)
}

fn resolve_builtin_method(ty: &str, name: &str) -> Option<NativeFunction> {
    match (ty, name) {
        ("String", "toUpperCase") | ("String", "upper") => Some(native_string_upper),
        ("String", "toLowerCase") | ("String", "lower") => Some(native_string_lower),
        ("String", "trim") => Some(native_string_trim),
        ("String", "split") => Some(native_string_split),
        ("String", "contains") => Some(native_string_contains),
        ("String", "startsWith") => Some(native_string_starts_with),
        ("String", "endsWith") => Some(native_string_ends_with),
        ("String", "replace") => Some(native_string_replace),
        ("String", "substring") => Some(native_string_substring),
        ("String", "charAt") => Some(native_string_char_at),
        ("String", "indexOf") => Some(native_string_index_of),
        ("String", "repeat") => Some(native_string_repeat),
        ("Array", "push") => Some(native_array_push),
        ("Array", "pop") => Some(native_array_pop),
        ("Array", "shift") => Some(native_array_shift),
        ("Array", "unshift") => Some(native_array_unshift),
        ("Array", "insert") => Some(native_array_insert),
        ("Array", "remove") => Some(native_array_remove_at),
        ("Array", "clear") => Some(native_array_clear),
        ("Array", "slice") => Some(native_array_slice),
        ("Array", "join") => Some(native_array_join),
        ("Array", "find") => Some(native_array_find),
        ("Array", "map") => Some(native_array_map),
        ("Array", "filter") => Some(native_array_filter),
        ("Array", "reduce") => Some(native_array_reduce),
        ("Array", "forEach") => Some(native_array_for_each),
        ("Array", "reverse") => Some(native_array_reverse),
        ("Array", "contains") => Some(native_array_contains),
        ("Array", "indexOf") => Some(native_array_index_of),
        ("Array", "sort") => Some(native_array_sort),
        ("Array", "isEmpty") => Some(native_array_is_empty),
        ("Map", "put") => Some(native_map_put),
        ("Map", "get") => Some(native_map_get),
        ("Map", "remove") => Some(native_map_remove),
        ("Map", "containsKey") => Some(native_map_contains_key),
        ("Map", "keys") => Some(native_map_keys),
        ("Map", "values") => Some(native_map_values),
        ("Map", "entries") => Some(native_map_entries),
        ("Map", "size") => Some(native_map_size),
        ("Map", "isEmpty") => Some(native_map_is_empty),
        ("Map", "clear") => Some(native_map_clear),
        ("Set", "add") => Some(native_set_add),
        ("Set", "remove") => Some(native_set_remove),
        ("Set", "contains") => Some(native_set_contains),
        ("Set", "size") => Some(native_set_size),
        ("Set", "isEmpty") => Some(native_set_is_empty),
        ("Set", "clear") => Some(native_set_clear),
        ("Set", "union") => Some(native_set_union),
        ("Set", "intersection") => Some(native_set_intersection),
        ("Set", "difference") => Some(native_set_difference),
        // Result methods
        ("Result", "unwrap") => Some(native_result_unwrap),
        ("Result", "unwrapOr") => Some(native_result_unwrap_or),
        ("Result", "expect") => Some(native_result_expect),
        ("Result", "isOk") => Some(native_result_is_ok),
        ("Mutex", "lock") => Some(native_mutex_lock),
        ("Mutex", "unlock") => Some(native_mutex_unlock),
        ("Channel", "send") => Some(native_channel_send),
        ("Channel", "recv") => Some(native_channel_recv),
        ("Result", "isErr") => Some(native_result_is_err),
        // Option methods
        ("Option", "unwrap") => Some(native_option_unwrap),
        ("Option", "unwrapOr") => Some(native_option_unwrap_or),
        ("Option", "expect") => Some(native_option_expect),
        ("Option", "isSome") => Some(native_option_is_some),
        ("Option", "isNone") => Some(native_option_is_none),
        // Promise methods
        ("Promise", "then") => Some(native_promise_then),
        _ => None,
    }
}

fn native_string_upper(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_uppercase())))
}

fn native_string_lower(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_lowercase())))
}

fn native_string_trim(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.trim().to_string())))
}

fn native_string_split(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("split() expects 1 argument (separator)"));
    }
    let separator = match &args[1] {
        Value::String(sep) => sep.as_str(),
        _ => return Err(RuntimeError::new("split() separator must be string")),
    };
    let parts: Vec<Value> = s
        .split(separator)
        .map(|part| Value::String(Rc::new(part.to_string())))
        .collect();
    Ok(Value::Array(gc.allocate(parts)))
}

fn native_string_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let substr = match &args[1] {
        Value::String(sub) => sub.as_str(),
        _ => return Err(RuntimeError::new("contains() argument must be string")),
    };
    Ok(Value::Bool(s.contains(substr)))
}

fn native_string_starts_with(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("startsWith() expects 1 argument"));
    }
    let prefix = match &args[1] {
        Value::String(pre) => pre.as_str(),
        _ => return Err(RuntimeError::new("startsWith() argument must be string")),
    };
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn native_string_ends_with(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("endsWith() expects 1 argument"));
    }
    let suffix = match &args[1] {
        Value::String(suf) => suf.as_str(),
        _ => return Err(RuntimeError::new("endsWith() argument must be string")),
    };
    Ok(Value::Bool(s.ends_with(suffix)))
}

fn native_string_replace(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "replace() expects 2 arguments (old, new)",
        ));
    }
    let old_str = match &args[1] {
        Value::String(old) => old.as_str(),
        _ => return Err(RuntimeError::new("replace() old argument must be string")),
    };
    let new_str = match &args[2] {
        Value::String(new) => new.as_str(),
        _ => return Err(RuntimeError::new("replace() new argument must be string")),
    };
    Ok(Value::String(Rc::new(s.replace(old_str, new_str))))
}

fn native_array_push(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("push() expects 1 argument"));
    }
    let val = args[1].clone();
    arr.borrow_mut().push(val);
    Ok(Value::Null)
}

fn native_array_pop(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    Ok(arr.borrow_mut().pop().unwrap_or(Value::Null))
}

fn native_array_clear(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow_mut().clear();
    Ok(Value::Null)
}

fn native_set_add(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("add() expects 1 argument"));
    }
    let key = pleno_value::ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    set.borrow_mut().insert(key);
    Ok(Value::Null)
}

fn native_set_remove(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("remove() expects 1 argument"));
    }
    let key = pleno_value::ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    set.borrow_mut().remove(&key);
    Ok(Value::Null)
}

fn native_set_clear(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow_mut().clear();
    Ok(Value::Null)
}

fn native_set_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let key = pleno_value::ValueKey::try_from_value(&args[1])
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    Ok(Value::Bool(set.borrow().contains(&key)))
}

fn native_array_slice(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let array = arr.borrow();
    let len = array.len();

    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) | None => 0,
        _ => 0,
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) | None => len,
        _ => len,
    };

    let start = start.min(len);
    let end = end.min(len);

    let slice: Vec<Value> = array[start..end].to_vec();
    Ok(Value::Array(gc.allocate(slice)))
}

fn native_array_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("map() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let res = vm.call(callback.clone(), vec![item])?;
        results.push(res);
    }
    Ok(Value::Array(vm.get_gc().allocate(results)))
}

fn native_array_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("filter() expects 1 argument (callback)"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    let mut results = Vec::new();
    for item in items {
        let res = vm.call(callback.clone(), vec![item.clone()])?;
        if is_truthy(&res) {
            results.push(item);
        }
    }
    Ok(Value::Array(vm.get_gc().allocate(results)))
}

fn native_array_reduce(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "reduce() expects at least 1 argument (callback)",
        ));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    if items.is_empty() {
        if args.len() >= 3 {
            return Ok(args[2].clone());
        } else {
            return Err(RuntimeError::new(
                "reduce() of empty array with no initial value",
            ));
        }
    }
    let (mut accumulator, start_idx) = if args.len() >= 3 {
        (args[2].clone(), 0)
    } else {
        (items[0].clone(), 1)
    };
    for item in items.iter().skip(start_idx) {
        accumulator = vm.call(callback.clone(), vec![accumulator, item.clone()])?;
    }
    Ok(accumulator)
}

fn native_array_reverse(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut array = arr.borrow_mut();
    array.reverse();
    Ok(Value::Null)
}

fn native_array_sort(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut items = arr.borrow().clone();
    items.sort_by_key(|a| a.to_string());
    Ok(Value::Array(vm.get_gc().allocate(items)))
}

fn native_promise_constructor(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Promise() requires a callback"));
    }
    let mut promise_map = HashMap::new();
    promise_map.insert(
        "variant".to_string(),
        Value::String(std::rc::Rc::new("Pending".to_string())),
    );
    promise_map.insert("value".to_string(), Value::Null);
    Ok(Value::Map(gc.allocate(promise_map)))
}

fn native_promise_then(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("then() requires callback"));
    }
    Ok(args[0].clone())
}

fn native_array_contains(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("contains() expects 1 argument"));
    }
    let array = arr.borrow();
    let search_value = &args[1];
    Ok(Value::Bool(array.contains(search_value)))
}

fn native_array_index_of(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("indexOf() expects 1 argument"));
    }
    let array = arr.borrow();
    let search_value = &args[1];
    for (i, item) in array.iter().enumerate() {
        if item == search_value {
            return Ok(Value::Int(i as i64));
        }
    }
    Ok(Value::Int(-1)) // Not found
}

// Result type constructors and methods
fn native_result_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Ok() requires 1 argument"));
    }
    let mut result = HashMap::new();
    result.insert(
        "variant".to_string(),
        Value::String(Rc::new("Ok".to_string())),
    );
    result.insert("value".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(result)))
}

fn native_result_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Err() requires 1 argument"));
    }
    let mut result = HashMap::new();
    result.insert(
        "variant".to_string(),
        Value::String(Rc::new("Err".to_string())),
    );
    result.insert("error".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(result)))
}

// Option type constructors
fn native_option_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("Some() requires 1 argument"));
    }
    let mut option = HashMap::new();
    option.insert(
        "variant".to_string(),
        Value::String(Rc::new("Some".to_string())),
    );
    option.insert("value".to_string(), args[0].clone());
    Ok(Value::Map(gc.allocate(option)))
}

// Result methods
fn native_result_unwrap(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrap() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => map_ref
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeError::new("Result::Ok missing value")),
        Some(Value::String(variant)) if variant.as_str() == "Err" => {
            Err(RuntimeError::new("called unwrap() on Result::Err"))
        }
        _ => Err(RuntimeError::new("invalid Result value")),
    }
}

fn native_result_unwrap_or(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("unwrapOr() requires a default value"));
    }
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrapOr() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => Ok(map_ref
            .get("value")
            .cloned()
            .unwrap_or_else(|| args[1].clone())),
        _ => Ok(args[1].clone()), // Return default for Err
    }
}

fn native_result_expect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("expect() requires a message"));
    }
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("expect() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Ok" => {
            map_ref.get("value").cloned().ok_or_else(|| {
                let msg = match &args[1] {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "Result::Err".to_string(),
                };
                RuntimeError::new(&msg)
            })
        }
        _ => {
            let msg = match &args[1] {
                Value::String(s) => s.as_str().to_string(),
                _ => "Result::Err".to_string(),
            };
            Err(RuntimeError::new(&msg))
        }
    }
}

fn native_result_is_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isOk() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Ok")),
        _ => Ok(Value::Bool(false)),
    }
}

fn native_result_is_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let result = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isErr() called on non-Result value")),
    };
    let map_ref = result.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Err")),
        _ => Ok(Value::Bool(false)),
    }
}

// Option methods
fn native_option_unwrap(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrap() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => map_ref
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeError::new("Option::Some missing value")),
        Some(Value::String(variant)) if variant.as_str() == "None" => {
            Err(RuntimeError::new("called unwrap() on Option::None"))
        }
        _ => Err(RuntimeError::new("invalid Option value")),
    }
}

fn native_option_unwrap_or(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("unwrapOr() requires a default value"));
    }
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("unwrapOr() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => Ok(map_ref
            .get("value")
            .cloned()
            .unwrap_or_else(|| args[1].clone())),
        _ => Ok(args[1].clone()), // Return default for None
    }
}

fn native_option_expect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("expect() requires a message"));
    }
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("expect() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) if variant.as_str() == "Some" => {
            map_ref.get("value").cloned().ok_or_else(|| {
                let msg = match &args[1] {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "Option::None".to_string(),
                };
                RuntimeError::new(&msg)
            })
        }
        _ => {
            let msg = match &args[1] {
                Value::String(s) => s.as_str().to_string(),
                _ => "Option::None".to_string(),
            };
            Err(RuntimeError::new(&msg))
        }
    }
}

fn native_option_is_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isSome() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "Some")),
        _ => Ok(Value::Bool(false)),
    }
}

fn native_option_is_none(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let option = match &args[0] {
        Value::Map(map) => map,
        _ => return Err(RuntimeError::new("isNone() called on non-Option value")),
    };
    let map_ref = option.borrow();
    match map_ref.get("variant") {
        Some(Value::String(variant)) => Ok(Value::Bool(variant.as_str() == "None")),
        _ => Ok(Value::Bool(false)),
    }
}

// Math native functions
fn native_math_abs(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Err(RuntimeError::new("abs() expects a number")),
    }
}

fn native_math_pow(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    let base = match args.first() {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        _ => return Err(RuntimeError::new("pow() base must be a number")),
    };
    let exp = match args.get(1) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        _ => return Err(RuntimeError::new("pow() exponent must be a number")),
    };
    Ok(Value::Float(base.powf(exp)))
}

fn native_math_sqrt(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        _ => Err(RuntimeError::new("sqrt() expects a number")),
    }
}

fn native_math_random(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    use rand::Rng;
    Ok(Value::Float(rand::thread_rng().r#gen()))
}

fn native_math_random_int(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    use rand::Rng;
    let min = match args.first() {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::new("randomInt() min must be an integer")),
    };
    let max = match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => return Err(RuntimeError::new("randomInt() max must be an integer")),
    };
    if min >= max {
        return Err(RuntimeError::new("randomInt() min must be less than max"));
    }
    Ok(Value::Int(rand::thread_rng().gen_range(min..max)))
}

fn native_math_sin(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sin())),
        _ => Err(RuntimeError::new("sin() expects a number")),
    }
}

fn native_math_cos(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cos())),
        _ => Err(RuntimeError::new("cos() expects a number")),
    }
}

fn native_math_tan(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tan())),
        _ => Err(RuntimeError::new("tan() expects a number")),
    }
}

fn native_math_log(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ln())),
        Some(Value::Float(n)) => Ok(Value::Float(n.ln())),
        _ => Err(RuntimeError::new("log() expects a number")),
    }
}

fn native_math_exp(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).exp())),
        Some(Value::Float(n)) => Ok(Value::Float(n.exp())),
        _ => Err(RuntimeError::new("exp() expects a number")),
    }
}

fn native_math_floor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.floor() as i64)),
        _ => Err(RuntimeError::new("floor() expects a number")),
    }
}

fn native_math_ceil(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.ceil() as i64)),
        _ => Err(RuntimeError::new("ceil() expects a number")),
    }
}

fn native_math_round(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.round() as i64)),
        _ => Err(RuntimeError::new("round() expects a number")),
    }
}

// Regex native functions
fn native_regex_is_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("isMatch() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(text)))
}

fn native_regex_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("match() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    match re.captures(text) {
        Some(caps) => {
            let mut groups = Vec::new();
            for i in 0..caps.len() {
                match caps.get(i) {
                    Some(m) => groups.push(Value::String(Rc::new(m.as_str().to_string()))),
                    None => groups.push(Value::Null),
                }
            }
            Ok(Value::Array(gc.allocate(groups)))
        }
        None => Ok(Value::Null),
    }
}

fn native_regex_find_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("findAll() requires pattern and text"));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    let mut matches = Vec::new();

    for caps in re.captures_iter(text) {
        let mut groups = Vec::new();
        for i in 0..caps.len() {
            match caps.get(i) {
                Some(m) => groups.push(Value::String(Rc::new(m.as_str().to_string()))),
                None => groups.push(Value::Null),
            }
        }
        matches.push(Value::Array(gc.allocate(groups)));
    }
    Ok(Value::Array(gc.allocate(matches)))
}

fn native_regex_replace(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "replace() requires pattern, text, and replacement",
        ));
    }
    let pattern = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("pattern must be string")),
    };
    let text = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("text must be string")),
    };
    let replacement = match &args[2] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("replacement must be string")),
    };

    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    let result = re.replace_all(text, replacement);
    Ok(Value::String(Rc::new(result.to_string())))
}

// JSON native functions
fn value_to_json(value: &Value) -> Result<serde_json::Value, RuntimeError> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Int(i) => Ok(serde_json::json!(i)),
        Value::Float(f) => Ok(serde_json::json!(f)),
        Value::String(s) => Ok(serde_json::Value::String(s.as_str().to_string())),
        Value::Array(arr) => {
            let mut vec = Vec::new();
            for item in arr.borrow().iter() {
                vec.push(value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(vec))
        }
        Value::Map(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map.borrow().iter() {
                obj.insert(k.clone(), value_to_json(v)?);
            }
            Ok(serde_json::Value::Object(obj))
        }
        _ => Err(RuntimeError::new("unsupported type for JSON serialization")),
    }
}

fn json_to_value(json: serde_json::Value, gc: &mut pleno_value::gc::GCManager) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                Value::Int(n.as_i64().unwrap())
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::String(Rc::new(s)),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value> = arr.into_iter().map(|v| json_to_value(v, gc)).collect();
            Value::Array(gc.allocate(items))
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_value(v, gc));
            }
            Value::Map(gc.allocate(map))
        }
    }
}

fn native_json_parse(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("JSON.parse() requires string argument"));
    }
    let json_str = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("JSON.parse() expects string")),
    };

    let json_val: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| RuntimeError::new(format!("JSON parse error: {}", e)))?;

    Ok(json_to_value(json_val, gc))
}

fn native_json_stringify(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new(
            "JSON.stringify() requires value argument",
        ));
    }
    let json_val = value_to_json(&args[0])?;
    let s = serde_json::to_string(&json_val)
        .map_err(|e| RuntimeError::new(format!("JSON stringify error: {}", e)))?;
    Ok(Value::String(Rc::new(s)))
}

fn native_json_stringify_pretty(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new(
            "JSON.stringify() requires value argument",
        ));
    }
    let json_val = value_to_json(&args[0])?;
    let s = serde_json::to_string_pretty(&json_val)
        .map_err(|e| RuntimeError::new(format!("JSON stringify error: {}", e)))?;
    Ok(Value::String(Rc::new(s)))
}

// I/O native functions
fn native_file_read(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("read() requires path"));
    }
    let path = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("path must be string")),
    };
    let content = fs::read_to_string(path)
        .map_err(|e| RuntimeError::new(format!("failed to read file: {}", e)))?;
    Ok(Value::String(Rc::new(content)))
}

fn native_file_write(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.len() < 2 {
        return Err(RuntimeError::new("write() requires path and content"));
    }
    let path = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("path must be string")),
    };
    let content = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("content must be string")),
    };
    fs::write(path, content)
        .map_err(|e| RuntimeError::new(format!("failed to write file: {}", e)))?;
    Ok(Value::Null)
}

fn native_file_exists(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("exists() requires path"));
    }
    let path = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("path must be string")),
    };
    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

fn native_directory_create(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("create() requires path"));
    }
    let path = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("path must be string")),
    };
    std::fs::create_dir_all(path)
        .map_err(|e| RuntimeError::new(format!("failed to create directory: {}", e)))?;
    Ok(Value::Null)
}

fn native_directory_exists(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let _gc = vm.get_gc();
    if args.is_empty() {
        return Err(RuntimeError::new("exists() requires path"));
    }
    let path = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("path must be string")),
    };
    let p = std::path::Path::new(path);
    Ok(Value::Bool(p.exists() && p.is_dir()))
}

// Time native functions
fn native_datetime_now(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(Utc::now().timestamp()))
}

fn native_datetime_format(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let timestamp = match args.first() {
        Some(Value::Int(t)) => *t,
        _ => return Err(RuntimeError::new("format() expects integer timestamp")),
    };
    let fmt = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("format() expects format string")),
    };

    let dt = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or_else(|| RuntimeError::new("invalid timestamp"))?;
    Ok(Value::String(Rc::new(dt.format(fmt).to_string())))
}

fn native_datetime_parse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let date_str = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("parse() expects date string")),
    };
    let fmt = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("parse() expects format string")),
    };

    let dt = ChronoDateTime::parse_from_str(date_str, fmt)
        .map_err(|e| RuntimeError::new(format!("date parse error: {}", e)))?;
    Ok(Value::Int(dt.with_timezone(&Utc).timestamp()))
}

// Map native methods
fn native_map_keys(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("keys() expects Map")),
    };
    let keys: Vec<Value> = map
        .borrow()
        .keys()
        .map(|k| Value::String(Rc::new(k.clone())))
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(keys)))
}

fn native_map_values(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("values() expects Map")),
    };
    let values: Vec<Value> = map.borrow().values().cloned().collect();
    Ok(Value::Array(vm.get_gc().allocate(values)))
}

fn native_map_entries(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("entries() expects Map")),
    };
    let mut entries = Vec::new();
    for (k, v) in map.borrow().iter() {
        let entry = vec![Value::String(Rc::new(k.clone())), v.clone()];
        entries.push(Value::Array(vm.get_gc().allocate(entry)));
    }
    Ok(Value::Array(vm.get_gc().allocate(entries)))
}

fn native_map_contains_key(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("has() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("has() expects string key")),
    };
    Ok(Value::Bool(map.borrow().contains_key(key)))
}

fn native_map_remove(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("delete() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("delete() expects string key")),
    };
    let removed = map.borrow_mut().remove(key).is_some();
    Ok(Value::Bool(removed))
}

fn native_map_clear(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("clear() expects Map")),
    };
    map.borrow_mut().clear();
    Ok(Value::Null)
}

// Set methods (aliases)

// String aliases and new methods

fn native_string_substring(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("substring() expects String")),
    };
    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("substring() expects start index")),
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) | None => s.len(),
        _ => return Err(RuntimeError::new("substring() expects end index or null")),
    };

    if start > s.len() || end > s.len() || start > end {
        return Err(RuntimeError::new("substring() out of bounds"));
    }

    Ok(Value::String(Rc::new(s[start..end].to_string())))
}

// Array aliases and new methods

fn native_array_shift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("shift() expects Array")),
    };
    let mut array = arr.borrow_mut();
    if array.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(array.remove(0))
    }
}

fn native_array_unshift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("unshift() expects Array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("unshift() expects value"));
    }
    arr.borrow_mut().insert(0, args[1].clone());
    Ok(Value::Null)
}

fn native_array_join(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("join() expects Array")),
    };
    let sep = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        Some(Value::Null) | None => ",",
        _ => return Err(RuntimeError::new("join() expects string separator")),
    };
    let array = arr.borrow();
    let s: Vec<String> = array.iter().map(|v| v.to_string()).collect();
    Ok(Value::String(Rc::new(s.join(sep))))
}

fn native_array_find(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("find() expects Array")),
    };
    if args.len() < 2 {
        return Err(RuntimeError::new("find() expects callback"));
    }
    let callback = args[1].clone();
    let items = arr.borrow().clone();
    for item in items {
        let res = vm.call(callback.clone(), vec![item.clone()])?;
        if let Value::Bool(true) = res {
            return Ok(item);
        }
    }
    Ok(Value::Null)
}

fn native_async_spawn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("spawn() requires a function"));
    }
    let callee = args[0].clone();
    let call_args = args[1..].to_vec();
    // In this VM, calling an async function already starts it and returns a promise.
    // If it's a synchronous function, it will run to completion here.
    vm.call(callee, call_args)
}

fn native_promise_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let promises = match args.first() {
        Some(Value::Array(arr)) => arr.borrow().clone(),
        _ => return Err(RuntimeError::new("Promise.all() expects array of promises")),
    };

    if promises.is_empty() {
        let gc = vm.get_gc();
        let mut map = HashMap::new();
        map.insert(
            "variant".to_string(),
            Value::String(Rc::new("Resolved".to_string())),
        );
        map.insert("value".to_string(), Value::Array(gc.allocate(vec![])));
        return Ok(Value::Map(gc.allocate(map)));
    }

    let (master_id, master_promise) = vm.create_promise();
    let mut dependencies = Vec::new();
    let mut results = vec![Value::Null; promises.len()];
    let mut resolved_count = 0;

    for (i, p) in promises.iter().enumerate() {
        if let Value::Map(map) = p {
            let map_borrow = map.borrow();
            let variant = map_borrow.get("variant").and_then(|v| v.as_str());
            let id = map_borrow
                .get("id")
                .and_then(|v| v.as_int())
                .map(|i| i as u64);

            if variant == Some("Resolved") {
                results[i] = map_borrow.get("value").cloned().unwrap_or(Value::Null);
                resolved_count += 1;
            } else if let Some(id) = id {
                dependencies.push(id);
            } else {
                results[i] = p.clone();
                resolved_count += 1;
            }
        } else {
            results[i] = p.clone();
            resolved_count += 1;
        }
    }

    if resolved_count == promises.len() {
        let gc = vm.get_gc();
        let results_val = Value::Array(gc.allocate(results));
        vm.resolve_promise(master_id, results_val);
    } else {
        vm.register_multi_waiter(MultiWaiter {
            promise_id: master_id,
            dependencies,
            resolved_count,
            results,
            mode: MultiWaiterMode::All,
        });
    }

    Ok(master_promise)
}

fn native_promise_race(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let promises = match args.first() {
        Some(Value::Array(arr)) => arr.borrow().clone(),
        _ => {
            return Err(RuntimeError::new(
                "Promise.race() expects array of promises",
            ));
        }
    };

    if promises.is_empty() {
        return Err(RuntimeError::new("Promise.race() called on empty array"));
    }

    for p in &promises {
        let mut is_promise = false;
        if let Value::Map(map) = p {
            let map_borrow = map.borrow();
            let variant = map_borrow.get("variant").and_then(|v| v.as_str());
            if variant == Some("Resolved") {
                let val = map_borrow.get("value").cloned().unwrap_or(Value::Null);
                let (master_id, master_promise) = vm.create_promise();
                vm.resolve_promise(master_id, val);
                return Ok(master_promise);
            } else if variant == Some("Pending") {
                is_promise = true;
            }
        }

        if !is_promise {
            let (master_id, master_promise) = vm.create_promise();
            vm.resolve_promise(master_id, p.clone());
            return Ok(master_promise);
        }
    }

    let (master_id, master_promise) = vm.create_promise();
    let mut dependencies = Vec::new();

    for p in &promises {
        if let Value::Map(map) = p
            && let Some(id) = map.borrow().get("id").and_then(|v| v.as_int())
        {
            dependencies.push(id as u64);
        }
    }

    if dependencies.is_empty() {
        let val = promises[0].clone();
        vm.resolve_promise(master_id, val);
    } else {
        vm.register_multi_waiter(MultiWaiter {
            promise_id: master_id,
            dependencies,
            resolved_count: 0,
            results: Vec::new(),
            mode: MultiWaiterMode::Race,
        });
    }

    Ok(master_promise)
}

fn native_string_char_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("charAt() expects String")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("charAt() expects index")),
    };
    if index >= s.len() {
        return Ok(Value::Null);
    }
    Ok(Value::String(Rc::new(s[index..index + 1].to_string())))
}

fn native_string_index_of(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("indexOf() expects String")),
    };
    let search = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("indexOf() expects string to search")),
    };
    match s.find(search) {
        Some(i) => Ok(Value::Int(i as i64)),
        None => Ok(Value::Int(-1)),
    }
}

fn native_string_repeat(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("repeat() expects String")),
    };
    let count = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("repeat() expects count")),
    };
    Ok(Value::String(Rc::new(s.repeat(count))))
}

fn native_array_insert(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("insert() expects Array")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("insert() expects index")),
    };
    let value = match args.get(2) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("insert() expects value")),
    };
    let mut array = arr.borrow_mut();
    if index > array.len() {
        return Err(RuntimeError::new("insert() index out of bounds"));
    }
    array.insert(index, value);
    Ok(Value::Null)
}

fn native_array_remove_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("remove() expects Array")),
    };
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("remove() expects index")),
    };
    let mut array = arr.borrow_mut();
    if index >= array.len() {
        return Err(RuntimeError::new("remove() index out of bounds"));
    }
    Ok(array.remove(index))
}

fn native_array_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("isEmpty() expects Array")),
    };
    Ok(Value::Bool(arr.borrow().is_empty()))
}

fn native_array_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr.clone(),
        _ => return Err(RuntimeError::new("forEach() expects Array")),
    };
    let callback = match args.get(1) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("forEach() expects callback")),
    };
    let items = arr.borrow().clone();
    for item in items {
        vm.call(callback.clone(), vec![item])?;
    }
    Ok(Value::Null)
}

fn native_map_put(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("put() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => return Err(RuntimeError::new("put() expects string key")),
    };
    let value = match args.get(2) {
        Some(v) => v.clone(),
        None => return Err(RuntimeError::new("put() expects value")),
    };
    map.borrow_mut().insert(key, value);
    Ok(Value::Null)
}

fn native_map_get(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("get() expects Map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("get() expects string key")),
    };
    Ok(map.borrow().get(key).cloned().unwrap_or(Value::Null))
}

fn native_map_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("size() expects Map")),
    };
    Ok(Value::Int(map.borrow().len() as i64))
}

fn native_map_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("isEmpty() expects Map")),
    };
    Ok(Value::Bool(map.borrow().is_empty()))
}

fn native_set_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("size() expects Set")),
    };
    Ok(Value::Int(set.borrow().len() as i64))
}

fn native_set_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("isEmpty() expects Set")),
    };
    Ok(Value::Bool(set.borrow().is_empty()))
}

fn native_set_union(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("union() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("union() expects second Set")),
    };
    let mut result = set1.borrow().clone();
    for item in set2.borrow().iter() {
        result.insert(item.clone());
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

fn native_set_intersection(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("intersection() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("intersection() expects second Set")),
    };
    let mut result = std::collections::HashSet::new();
    let s2 = set2.borrow();
    for item in set1.borrow().iter() {
        if s2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

fn native_set_difference(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("difference() expects Set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s,
        _ => return Err(RuntimeError::new("difference() expects second Set")),
    };
    let mut result = std::collections::HashSet::new();
    let s2 = set2.borrow();
    for item in set1.borrow().iter() {
        if !s2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(result)))
}

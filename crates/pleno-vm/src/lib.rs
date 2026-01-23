use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    ArrayOpKind, Class, MethodInfo, MethodVisibility, NativeFunction, Object, Range, RuntimeError,
    Value, ValueKey,
    gc::{GcObject, GcRc},
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct AsyncTask {
    pub promise_id: u64,
    pub value: Value,
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
    pub next_promise_id: u64,
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

impl VM {
    pub fn new() -> Self {
        let mut gc = pleno_value::gc::GCManager::new(1000);
        let mut globals = HashMap::new();
        globals.insert("print".to_string(), Value::NativeFunction(native_print));
        globals.insert("id".to_string(), Value::NativeFunction(native_id));
        // Result type constructors
        globals.insert("Ok".to_string(), Value::NativeFunction(native_result_ok));
        globals.insert("Err".to_string(), Value::NativeFunction(native_result_err));
        // Option type constructors
        globals.insert(
            "Some".to_string(),
            Value::NativeFunction(native_option_some),
        );
        globals.insert(
            "None".to_string(),
            Value::NativeFunction(native_option_none),
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

        Self {
            gc,
            registers: vec![Value::Null; 256],
            frames: Vec::new(),
            globals,
            modules: HashMap::new(),
            functions: Vec::new(),
            pending_tasks: Vec::new(),
            suspended_frames: HashMap::new(),
            next_promise_id: 1,
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
            .extend(std::iter::repeat(Value::Null).take(register_count));

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
            if let Some(suspended) = self.suspended_frames.remove(&task.promise_id) {
                let mut frame = suspended.frame;
                self.registers[frame.base + suspended.await_reg] = task.value;
                self.frames.push(frame);
            }
        }
        Ok(())
    }

    fn run_until(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        loop {
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
                    let idx = c as usize;
                    if idx >= closure.len() {
                        return Err(RuntimeError::new("capture index out of bounds"));
                    }
                    closure[idx] = value;
                }
                Opcode::LoadCapture => {
                    let idx = b as usize;
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
                                    if id == self.next_promise_id {
                                        self.next_promise_id += 1;
                                    }

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
                                _ => self.registers[base + a] = Value::Map(map),
                            }
                        }
                        _ => self.registers[base + a] = value,
                    }
                }
                Opcode::Is => {
                    let value = &self.registers[base + b];
                    let type_name = match &function.chunk.constants[c] {
                        Value::String(s) => s.as_str(),
                        _ => return Err(RuntimeError::new("type name must be string")),
                    };
                    let result = match (type_name, value) {
                        ("Int", Value::Int(_)) => true,
                        ("Float", Value::Float(_)) => true,
                        ("Bool", Value::Bool(_)) => true,
                        ("String", Value::String(_)) => true,
                        ("Array", Value::Array(_)) => true,
                        ("Map", Value::Map(_)) => true,
                        ("Set", Value::Set(_)) => true,
                        ("Object", Value::Object(_)) => true,
                        ("Null", Value::Null) => true,
                        _ => false,
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
                        _ => return Err(RuntimeError::new("property set on non-map/object value")),
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
                            // Check if this is a Result or Option type first
                            let map_ref = map.borrow();
                            if let Some(Value::String(variant)) = map_ref.get("variant") {
                                if (variant.as_str() == "Ok" || variant.as_str() == "Err")
                                    && resolve_builtin_method("Result", &name).is_some()
                                {
                                    // Result type with method
                                    drop(map_ref);
                                    return Ok(Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method("Result", &name).unwrap(),
                                    });
                                } else if (variant.as_str() == "Some" || variant.as_str() == "None")
                                    && resolve_builtin_method("Option", &name).is_some()
                                {
                                    // Option type with method
                                    drop(map_ref);
                                    return Ok(Value::BoundNativeMethod {
                                        receiver: Box::new(Value::Map(map.clone())),
                                        function: resolve_builtin_method("Option", &name).unwrap(),
                                    });
                                }
                            }
                            drop(map_ref);
                            // Regular map access
                            map.borrow().get(&name).cloned().unwrap_or(Value::Null)
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
                            if name == "len" {
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
                            if name == "len" {
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
                        _ => return Err(RuntimeError::new("property access on non-object value")),
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
                    let superclass = receiver
                        .borrow()
                        .class
                        .as_ref()
                        .and_then(|class| class.borrow().superclass.as_ref())
                        .map(|sc| sc.clone())
                        .ok_or_else(|| RuntimeError::new("superclass missing"))?;
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
                            array.borrow().get(idx).cloned().unwrap_or(Value::Null)
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
                    let result = frame
                        .return_override
                        .unwrap_or_else(|| self.registers[base + a].clone());
                    self.registers.truncate(frame.base);
                    if self.frames.len() == depth {
                        return Ok(result);
                    }
                    self.registers[frame.return_reg] = result;
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
                    match callee {
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
                        }
                        Value::NativeFunction(func) => {
                            let result = func(&mut self.gc, &args)?;
                            self.registers[base + a] = result;
                        }
                        Value::BoundNativeMethod { receiver, function } => {
                            let mut merged_args = vec![*receiver];
                            merged_args.extend(args);
                            let result = function(&mut self.gc, &merged_args)?;
                            self.registers[base + a] = result;
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
                        }
                        _ => return Err(RuntimeError::new("value is not callable")),
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
                Opcode::TypeOf => {
                    let value = &self.registers[base + b];
                    let type_name = match value {
                        Value::Null => "Null",
                        Value::Missing => "Missing",
                        Value::Bool(_) => "Bool",
                        Value::Int(_) => "Int",
                        Value::Float(_) => "Float",
                        Value::String(_) => "String",
                        Value::Range(_) => "Range",
                        Value::Class(_) => "Class",
                        Value::Function(_) => "Function",
                        Value::BoundMethod { .. } => "Function",
                        Value::StaticMethod { .. } => "Function",
                        Value::Closure { .. } => "Function",
                        Value::NativeFunction(_) => "Function",
                        Value::BoundNativeMethod { .. } => "Function",
                        Value::Map(_) => "Map",
                        Value::Array(_) => "Array",
                        Value::Set(_) => "Set",
                        Value::Object(_) => "Object",
                        Value::Partial { .. } => "Function",
                    };
                    self.registers[base + a] = Value::String(Rc::new(type_name.to_string()));
                }
                Opcode::Is => {
                    let value = &self.registers[base + b];
                    let type_val = &self.registers[base + c];
                    let result = match type_val {
                        Value::String(s) => {
                            let type_name = s.as_str();
                            match (type_name, value) {
                                ("Null", Value::Null) => true,
                                ("Bool", Value::Bool(_)) => true,
                                ("Int", Value::Int(_)) => true,
                                ("Float", Value::Float(_)) => true,
                                ("String", Value::String(_)) => true,
                                ("Array", Value::Array(_)) => true,
                                ("Map", Value::Map(_)) => true,
                                ("Set", Value::Set(_)) => true,
                                ("Object", Value::Object(_)) => true,
                                ("Function", Value::Function(_)) => true,
                                ("Function", Value::Closure { .. }) => true,
                                ("Function", Value::NativeFunction(_)) => true,
                                ("Function", Value::BoundNativeMethod { .. }) => true,
                                ("Function", Value::BoundMethod { .. }) => true,
                                ("Function", Value::StaticMethod { .. }) => true,
                                ("Function", Value::Partial { .. }) => true,
                                _ => false,
                            }
                        }
                        Value::Class(target_class) => match value {
                            Value::Object(obj) => {
                                if let Some(class) = &obj.borrow().class {
                                    is_subclass_of(class, target_class)
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    self.registers[base + a] = Value::Bool(result);
                }
                _ => {
                    return Err(RuntimeError::new("opcode not implemented"));
                }
            }
        }
    }
}

impl VM {
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
                return Err(RuntimeError::new("argument count mismatch"));
            }

            let mut processed = Vec::new();
            let mut arg_idx = 0;

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
                    return Err(RuntimeError::new("missing required argument"));
                }
            }

            if arg_idx < final_args.len() {
                return Err(RuntimeError::new("too many positional arguments"));
            }
            processed
        };

        let base = self.registers.len();
        let register_count = function.chunk.max_registers.max(function.arity);
        self.registers
            .extend(std::iter::repeat(Value::Null).take(register_count));

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
                let result = func(&mut self.gc, &positional)?;
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
                let result = function(&mut self.gc, &merged_args)?;
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::Class(class) => {
                let object = self.gc.allocate(Object {
                    class: Some(class.clone()),
                    fields: HashMap::new(),
                });
                if let Some(handle) = class.borrow().constructor {
                    let function = self
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
                let function = self
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
                let function_info = self
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
        for key in named.keys() {
            if !params.iter().any(|param| param.name == *key) {
                return Err(RuntimeError::new("unknown named argument"));
            }
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
            .extend(std::iter::repeat(Value::Null).take(register_count));
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

fn native_print(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{}", value));
    }
    println!("{out}");
    Ok(Value::Null)
}

fn native_id(_gc: &mut pleno_value::gc::GCManager, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.get(0).cloned().unwrap_or(Value::Null))
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
    globals: &HashMap<String, Value>,
) -> bool {
    match visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class
            .map(|c| c.borrow().name.as_str() == owner.as_str())
            .unwrap_or(false),
        MethodVisibility::Protected => current_class
            .map(|c| is_subclass(c, owner.as_ref(), globals))
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
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
        (Value::String(a), b) => Ok(Value::String(Rc::new(format!("{}{}", a, b)))),
        (a, Value::String(b)) => Ok(Value::String(Rc::new(format!("{}{}", a, b)))),
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
        ("String", "upper") => Some(native_string_upper),
        ("String", "lower") => Some(native_string_lower),
        ("String", "trim") => Some(native_string_trim),
        ("String", "split") => Some(native_string_split),
        ("String", "contains") => Some(native_string_contains),
        ("String", "startsWith") => Some(native_string_starts_with),
        ("String", "endsWith") => Some(native_string_ends_with),
        ("String", "replace") => Some(native_string_replace),
        ("Array", "push") => Some(native_array_push),
        ("Array", "pop") => Some(native_array_pop),
        ("Array", "clear") => Some(native_array_clear),
        ("Array", "slice") => Some(native_array_slice),
        ("Array", "map") => Some(native_array_map),
        ("Array", "filter") => Some(native_array_filter),
        ("Array", "reduce") => Some(native_array_reduce),
        ("Array", "reverse") => Some(native_array_reverse),
        ("Array", "contains") => Some(native_array_contains),
        ("Array", "indexOf") => Some(native_array_index_of),
        ("Set", "add") => Some(native_set_add),
        ("Set", "remove") => Some(native_set_remove),
        ("Set", "clear") => Some(native_set_clear),
        ("Set", "contains") => Some(native_set_contains),
        // Result methods
        ("Result", "unwrap") => Some(native_result_unwrap),
        ("Result", "unwrapOr") => Some(native_result_unwrap_or),
        ("Result", "expect") => Some(native_result_expect),
        ("Result", "isOk") => Some(native_result_is_ok),
        ("Result", "isErr") => Some(native_result_is_err),
        // Option methods
        ("Option", "unwrap") => Some(native_option_unwrap),
        ("Option", "unwrapOr") => Some(native_option_unwrap_or),
        ("Option", "expect") => Some(native_option_expect),
        ("Option", "isSome") => Some(native_option_is_some),
        ("Option", "isNone") => Some(native_option_is_none),
        _ => None,
    }
}

fn native_string_upper(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_uppercase())))
}

fn native_string_lower(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_lowercase())))
}

fn native_string_trim(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.trim().to_string())))
}

fn native_string_split(
    gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_string_contains(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_string_ends_with(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_string_replace(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_array_push(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_array_pop(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    Ok(arr.borrow_mut().pop().unwrap_or(Value::Null))
}

fn native_array_clear(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow_mut().clear();
    Ok(Value::Null)
}

fn native_set_add(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_set_remove(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_set_clear(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(set) => set,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow_mut().clear();
    Ok(Value::Null)
}

fn native_set_contains(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_array_slice(
    gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let array = arr.borrow();
    let len = array.len();

    // Parse start and end from args
    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) => 0,
        _ => 0,
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        Some(Value::Null) => len,
        _ => len,
    };

    // Handle negative indices
    let start = if start > len { len } else { start };
    let end = if end > len { len } else { end };

    // Create new array with slice
    let slice: Vec<Value> = array[start..end].to_vec();
    Ok(Value::Array(gc.allocate(slice)))
}

fn native_array_map(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // TODO: Implement map() - requires calling function for each element
    Err(RuntimeError::new(
        "map() is not fully implemented yet - requires function calling in native code",
    ))
}

fn native_array_filter(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // TODO: Implement filter() - requires calling function for each element
    Err(RuntimeError::new(
        "filter() is not fully implemented yet - requires function calling in native code",
    ))
}

fn native_array_reduce(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // TODO: Implement reduce() - requires calling function for each element
    Err(RuntimeError::new(
        "reduce() is not fully implemented yet - requires function calling in native code",
    ))
}

fn native_array_reverse(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut array = arr.borrow_mut();
    array.reverse();
    Ok(Value::Null)
}

fn native_array_contains(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_array_index_of(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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
fn native_result_ok(
    gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_result_err(
    gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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
fn native_option_some(
    gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_option_none(
    gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    let mut option = HashMap::new();
    option.insert(
        "variant".to_string(),
        Value::String(Rc::new("None".to_string())),
    );
    Ok(Value::Map(gc.allocate(option)))
}

// Result methods
fn native_result_unwrap(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_result_unwrap_or(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_result_expect(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_result_is_ok(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_result_is_err(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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
fn native_option_unwrap(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_option_unwrap_or(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_option_expect(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_option_is_some(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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

fn native_option_is_none(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
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
fn native_math_abs(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        _ => Err(RuntimeError::new("abs() expects a number")),
    }
}

fn native_math_pow(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let base = match args.get(0) {
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

fn native_math_sqrt(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        _ => Err(RuntimeError::new("sqrt() expects a number")),
    }
}

fn native_math_random(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    use rand::Rng;
    Ok(Value::Float(rand::thread_rng().r#gen()))
}

fn native_math_random_int(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    use rand::Rng;
    let min = match args.get(0) {
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

fn native_math_sin(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sin())),
        _ => Err(RuntimeError::new("sin() expects a number")),
    }
}

fn native_math_cos(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cos())),
        _ => Err(RuntimeError::new("cos() expects a number")),
    }
}

fn native_math_tan(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tan())),
        _ => Err(RuntimeError::new("tan() expects a number")),
    }
}

fn native_math_log(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ln())),
        Some(Value::Float(n)) => Ok(Value::Float(n.ln())),
        _ => Err(RuntimeError::new("log() expects a number")),
    }
}

fn native_math_exp(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).exp())),
        Some(Value::Float(n)) => Ok(Value::Float(n.exp())),
        _ => Err(RuntimeError::new("exp() expects a number")),
    }
}

fn native_math_floor(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.floor() as i64)),
        _ => Err(RuntimeError::new("floor() expects a number")),
    }
}

fn native_math_ceil(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.ceil() as i64)),
        _ => Err(RuntimeError::new("ceil() expects a number")),
    }
}

fn native_math_round(
    _gc: &mut pleno_value::gc::GCManager,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    match args.get(0) {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.round() as i64)),
        _ => Err(RuntimeError::new("round() expects a number")),
    }
}

// Regex native functions
fn native_regex_is_match(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Bool(false))
}

fn native_regex_match(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_regex_find_all(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_regex_replace(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

// JSON native functions
fn native_json_parse(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_json_stringify(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::String(Rc::new("null".to_string())))
}

fn native_json_stringify_pretty(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::String(Rc::new("null".to_string())))
}

// I/O native functions
fn native_file_read(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_file_write(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_file_exists(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Bool(false))
}

fn native_directory_create(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Null)
}

fn native_directory_exists(
    _gc: &mut pleno_value::gc::GCManager,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    // Stub
    Ok(Value::Bool(false))
}

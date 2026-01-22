use pleno_bytecode::{Chunk, Opcode};
use pleno_value::{Class, Object, Range, RuntimeError, Value, ValueKey};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_index: usize,
    pub ip: usize,
    pub base: usize,
    pub return_reg: usize,
    pub return_override: Option<Value>,
    pub captures: Vec<Value>,
}

pub struct VM {
    registers: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Rc<String>,
    pub chunk: Chunk,
    pub arity: usize,
}

impl VM {
    pub fn new() -> Self {
        let mut globals = HashMap::new();
        globals.insert("print".to_string(), Value::NativeFunction(native_print));
        globals.insert("id".to_string(), Value::NativeFunction(native_id));
        Self {
            registers: vec![Value::Null; 256],
            frames: Vec::new(),
            globals,
            functions: Vec::new(),
        }
    }

    pub fn execute(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        let function = Function {
            name: Rc::new("<script>".to_string()),
            chunk,
            arity: 0,
        };
        let func_index = self.functions.len();
        self.functions.push(function);
        self.frames.push(CallFrame {
            function_index: func_index,
            ip: 0,
            base: 0,
            return_reg: 0,
            return_override: None,
            captures: Vec::new(),
        });
        self.run()
    }

    fn run(&mut self) -> Result<Value, RuntimeError> {
        loop {
            let frame = self.frames.last_mut().ok_or_else(|| RuntimeError::new("no frame"))?;
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
                    });
                    let captures = vec![Value::Null; c];
                    self.registers[base + a] = Value::Closure {
                        function: func_index,
                        captures,
                    };
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
                    self.registers[base + a] =
                        Value::Map(Rc::new(std::cell::RefCell::new(HashMap::new())));
                }
                Opcode::NewArray => {
                    self.registers[base + a] =
                        Value::Array(Rc::new(std::cell::RefCell::new(Vec::new())));
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
                    self.registers[base + a] = Value::Set(Rc::new(std::cell::RefCell::new(
                        std::collections::HashSet::new(),
                    )));
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
                    self.registers[base + a] = Value::Array(Rc::new(std::cell::RefCell::new(values)));
                }
                Opcode::NewObject => {
                    self.registers[base + a] = Value::Object(Rc::new(std::cell::RefCell::new(
                        Object {
                            class: None,
                            fields: HashMap::new(),
                        },
                    )));
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
                    self.registers[base + a] = Value::Array(Rc::new(std::cell::RefCell::new(keys)));
                }
                Opcode::IterValues => {
                    let value = match &self.registers[base + b] {
                        Value::Array(array) => Value::Array(Rc::clone(array)),
                        Value::Map(map) => {
                            let values = map
                                .borrow()
                                .keys()
                                .cloned()
                                .map(|key| Value::String(Rc::new(key)))
                                .collect::<Vec<_>>();
                            Value::Array(Rc::new(std::cell::RefCell::new(values)))
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
                            Value::Array(Rc::new(std::cell::RefCell::new(values)))
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
                Opcode::SetProperty => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let value = self.registers[base + a].clone();
                    match &self.registers[base + b] {
                        Value::Map(map) => {
                            map.borrow_mut().insert(name, value);
                        }
                        Value::Object(object) => {
                            object.borrow_mut().fields.insert(name, value);
                        }
                        _ => return Err(RuntimeError::new("property set on non-map/object value")),
                    }
                }
                Opcode::GetProperty => {
                    let name = match &function.chunk.constants[c] {
                        Value::String(value) => value.as_str().to_string(),
                        _ => return Err(RuntimeError::new("property name must be string")),
                    };
                    let value = match &self.registers[base + b] {
                        Value::Map(map) => map
                            .borrow()
                            .get(&name)
                            .cloned()
                            .unwrap_or(Value::Null),
                        Value::Object(object) => object
                            .borrow()
                            .fields
                            .get(&name)
                            .cloned()
                            .or_else(|| {
                                object
                                    .borrow()
                                    .class
                                    .as_ref()
                                    .and_then(|class| resolve_method(class, &name))
                                    .map(|function| Value::BoundMethod {
                                        receiver: Rc::clone(object),
                                        function,
                                    })
                            })
                            .unwrap_or(Value::Null),
                        Value::Class(class) => class
                            .static_methods
                            .get(&name)
                            .copied()
                            .map(Value::Function)
                            .or_else(|| {
                                class
                                    .superclass
                                    .as_ref()
                                    .and_then(|superclass| superclass.static_methods.get(&name))
                                    .copied()
                                    .map(Value::Function)
                            })
                            .unwrap_or(Value::Null),
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
                        Value::Object(object) => Rc::clone(object),
                        _ => return Err(RuntimeError::new("super used on non-object value")),
                    };
                    let superclass = receiver
                        .borrow()
                        .class
                        .as_ref()
                        .and_then(|class| class.superclass.as_ref())
                        .map(Rc::clone)
                        .ok_or_else(|| RuntimeError::new("superclass missing"))?;
                    let handle = resolve_method(&superclass, &name)
                        .ok_or_else(|| RuntimeError::new("unknown super method"))?;
                    self.registers[base + a] = Value::BoundMethod {
                        receiver,
                        function: handle,
                    };
                }
                Opcode::GetIndex => {
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    let value = match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            array
                                .borrow()
                                .get(idx)
                                .cloned()
                                .unwrap_or(Value::Null)
                        }
                        (Value::Map(map), Value::String(key)) => {
                            map.borrow().get(key.as_str()).cloned().unwrap_or(Value::Null)
                        }
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
                    self.registers[base + a] =
                        Value::Bool(is_truthy(&left) && is_truthy(&right));
                }
                Opcode::Or => {
                    let left = self.registers[base + b].clone();
                    let right = self.registers[base + c].clone();
                    self.registers[base + a] =
                        Value::Bool(is_truthy(&left) || is_truthy(&right));
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
                    let frame = self.frames.pop().ok_or_else(|| RuntimeError::new("no frame"))?;
                    let result = frame
                        .return_override
                        .unwrap_or_else(|| self.registers[base + a].clone());
                    self.registers.truncate(frame.base);
                    if self.frames.is_empty() {
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
                    let methods_map = match &self.registers[base + c] {
                        Value::Map(map) => map.borrow().clone(),
                        _ => return Err(RuntimeError::new("class methods must be map")),
                    };
                    let mut methods = HashMap::new();
                    let mut constructor = None;
                    for (method_name, value) in methods_map {
                        let handle = match value {
                            Value::Function(handle) => handle,
                            _ => {
                                return Err(RuntimeError::new(
                                    "class method must be a function",
                                ))
                            }
                        };
                        if method_name == "constructor" {
                            constructor = Some(handle);
                        } else {
                            methods.insert(method_name, handle);
                        }
                    }
                    let class = Class {
                        name: Rc::new(name),
                        methods,
                        static_methods: HashMap::new(),
                        constructor,
                        superclass: None,
                    };
                    self.registers[base + a] = Value::Class(Rc::new(class));
                }
                Opcode::SetSuper => {
                    let superclass = match &self.registers[base + b] {
                        Value::Class(class) => Rc::clone(class),
                        _ => return Err(RuntimeError::new("superclass must be class")),
                    };
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            let class = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods: class.static_methods.clone(),
                                constructor: class.constructor,
                                superclass: Some(superclass),
                            };
                            self.registers[base + a] = Value::Class(Rc::new(class));
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
                    for (method_name, value) in static_map {
                        let handle = match value {
                            Value::Function(handle) => handle,
                            _ => {
                                return Err(RuntimeError::new(
                                    "static method must be a function",
                                ))
                            }
                        };
                        static_methods.insert(method_name, handle);
                    }
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            let class = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods,
                                constructor: class.constructor,
                                superclass: class.superclass.clone(),
                            };
                            self.registers[base + a] = Value::Class(Rc::new(class));
                        }
                        _ => return Err(RuntimeError::new("set static on non-class value")),
                    }
                }
                Opcode::Call => {
                    let callee = self.registers[base + b].clone();
                    let args = match &self.registers[base + c] {
                        Value::Array(array) => array.borrow().clone(),
                        _ => return Err(RuntimeError::new("call expects args array")),
                    };
                    match callee {
                        Value::NativeFunction(func) => {
                            let result = func(&args)?;
                            self.registers[base + a] = result;
                        }
                        Value::Class(class) => {
                            let object = Rc::new(std::cell::RefCell::new(Object {
                                class: Some(Rc::clone(&class)),
                                fields: HashMap::new(),
                            }));
                            if let Some(handle) = class.constructor {
                                let override_value = Some(Value::Object(Rc::clone(&object)));
                                self.call_function(
                                    handle,
                                    Some(object),
                                    &args,
                                    base + a,
                                    override_value,
                                    Vec::new(),
                                )?;
                            } else {
                                self.registers[base + a] = Value::Object(object);
                            }
                        }
                        Value::Function(handle) => {
                            self.call_function(handle, None, &args, base + a, None, Vec::new())?;
                        }
                        Value::BoundMethod { receiver, function } => {
                            self.call_function(function, Some(receiver), &args, base + a, None, Vec::new())?;
                        }
                        Value::Closure { function, captures } => {
                            self.call_function(function, None, &args, base + a, None, captures)?;
                        }
                        _ => return Err(RuntimeError::new("call on non-callable value")),
                    }
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
        receiver: Option<Rc<std::cell::RefCell<Object>>>,
        args: &[Value],
        return_reg: usize,
        return_override: Option<Value>,
        captures: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let function = self
            .functions
            .get(handle)
            .ok_or_else(|| RuntimeError::new("unknown function handle"))?
            .clone();
        let mut arg_offset = 0usize;
        let total_args = if receiver.is_some() { args.len() + 1 } else { args.len() };
        if total_args != function.arity {
            return Err(RuntimeError::new("argument count mismatch"));
        }
        let base = self.registers.len();
        let register_count = function.chunk.max_registers.max(function.arity);
        self.registers
            .extend(std::iter::repeat(Value::Null).take(register_count));
        if let Some(receiver) = receiver {
            self.registers[base] = Value::Object(receiver);
            arg_offset = 1;
        }
        for (idx, arg) in args.iter().enumerate() {
            self.registers[base + idx + arg_offset] = arg.clone();
        }
        self.frames.push(CallFrame {
            function_index: handle,
            ip: 0,
            base,
            return_reg,
            return_override,
            captures,
        });
        Ok(())
    }
}

fn native_print(args: &[Value]) -> Result<Value, RuntimeError> {
    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{value:?}"));
    }
    println!("{out}");
    Ok(Value::Null)
}

fn native_id(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.get(0).cloned().unwrap_or(Value::Null))
}

fn resolve_method(class: &Class, name: &str) -> Option<usize> {
    if let Some(handle) = class.methods.get(name) {
        return Some(*handle);
    }
    class
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_method(superclass, name))
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
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
        (Value::String(a), Value::String(b)) => Ok(Value::String(Rc::new(format!("{}{}", a, b)))),
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

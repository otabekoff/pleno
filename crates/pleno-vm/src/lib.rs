use pleno_bytecode::{Chunk, Opcode, ParamSpec};
use pleno_value::{
    Class, MethodInfo, MethodVisibility, Object, Range, RuntimeError, Value, ValueKey,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

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
    pub current_class: Option<Rc<String>>,
    pub current_module: Option<Rc<String>>,
    pub current_module_path: Option<Rc<String>>,
    pub exception_handlers: Vec<ExceptionHandler>,
}

pub struct VM {
    registers: Vec<Value>,
    frames: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    modules: HashMap<String, Value>,
    functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Rc<String>,
    pub chunk: Chunk,
    pub arity: usize,
    pub params: Vec<ParamSpec>,
    pub module: Option<Rc<String>>,
    pub module_path: Option<Rc<String>>,
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
            modules: HashMap::new(),
            functions: Vec::new(),
        }
    }

    pub fn execute(&mut self, chunk: Chunk) -> Result<Value, RuntimeError> {
        let function = Function {
            name: Rc::new("<script>".to_string()),
            chunk,
            arity: 0,
            params: Vec::new(),
            module: None,
            module_path: None,
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
            current_class: None,
            current_module: None,
            current_module_path: None,
            exception_handlers: Vec::new(),
        });
        self.run_until(0)
    }

    fn run_until(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        loop {
            if self.frames.len() == depth {
                return Ok(Value::Null);
            }
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
                        params: proto.params.clone(),
                        module: proto
                            .chunk
                            .module_name
                            .as_ref()
                            .map(|name| Rc::new(name.clone())),
                        module_path: frame.current_module_path.as_ref().cloned(),
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
                Opcode::IterEntries => {
                    let value = match &self.registers[base + b] {
                        Value::Array(array) => {
                            let entries = array
                                .borrow()
                                .iter()
                                .enumerate()
                                .map(|(idx, val)| {
                                    Value::Array(Rc::new(std::cell::RefCell::new(vec![
                                        Value::Int(idx as i64),
                                        val.clone(),
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(Rc::new(std::cell::RefCell::new(entries)))
                        }
                        Value::Map(map) => {
                            let entries = map
                                .borrow()
                                .iter()
                                .map(|(key, val)| {
                                    Value::Array(Rc::new(std::cell::RefCell::new(vec![
                                        Value::String(Rc::new(key.clone())),
                                        val.clone(),
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(Rc::new(std::cell::RefCell::new(entries)))
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
                                        ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
                                        ValueKey::String(v) => Value::String(Rc::clone(v)),
                                    };
                                    Value::Array(Rc::new(std::cell::RefCell::new(vec![
                                        Value::Int(idx as i64),
                                        val,
                                    ])))
                                })
                                .collect::<Vec<_>>();
                            Value::Array(Rc::new(std::cell::RefCell::new(entries)))
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
                                        Some(Rc::clone(object)),
                                        &[value],
                                        base + a,
                                        None,
                                        Vec::new(),
                                        Some(Rc::clone(&info.owner)),
                                        false,
                                    )?;
                                    continue;
                                }
                                if let Some(visibility) = class.fields.get(&name) {
                                    if !can_access_field(
                                        visibility,
                                        &class.name,
                                        class.module.as_ref(),
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
                                    Some(Rc::clone(&info.owner)),
                                    false,
                                )?;
                                continue;
                            }
                            if let Some(visibility) = class.static_field_visibility.get(&name) {
                                if !can_access_field(
                                    visibility,
                                    &class.name,
                                    class.module.as_ref(),
                                    current_class.as_ref(),
                                    current_module.as_ref(),
                                    &self.globals,
                                ) {
                                    return Err(RuntimeError::new("static field access denied"));
                                }
                            }
                            let class_name = class.name.as_ref().to_string();
                            let updated = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods: class.static_methods.clone(),
                                constructor: class.constructor,
                                superclass: class.superclass.clone(),
                                static_fields: {
                                    let mut fields = class.static_fields.clone();
                                    fields.insert(name, value);
                                    fields
                                },
                                fields: class.fields.clone(),
                                static_field_visibility: class.static_field_visibility.clone(),
                                module: class.module.clone(),
                            };
                            let updated = Value::Class(Rc::new(updated));
                            self.registers[base + b] = updated.clone();
                            self.globals.insert(class_name, updated);
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
                        Value::Map(map) => map
                            .borrow()
                            .get(&name)
                            .cloned()
                            .unwrap_or(Value::Null),
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
                                        Some(Rc::clone(object)),
                                        &[],
                                        base + a,
                                        None,
                                        Vec::new(),
                                        Some(Rc::clone(&info.owner)),
                                        false,
                                    )?;
                                    continue;
                                }
                                if let Some(visibility) = class.fields.get(&name) {
                                    if !can_access_field(
                                        visibility,
                                        &class.name,
                                        class.module.as_ref(),
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
                                        receiver: Rc::clone(object),
                                        function: info.function,
                                        owner: Rc::clone(&info.owner),
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
                                    Some(Rc::clone(&info.owner)),
                                )?;
                                continue;
                            }
                            if let Some(visibility) = class.static_field_visibility.get(&name) {
                                if !can_access_field(
                                    visibility,
                                    &class.name,
                                    class.module.as_ref(),
                                    current_class.as_ref(),
                                    frame.current_module.as_ref(),
                                    &self.globals,
                                ) {
                                    return Err(RuntimeError::new("static field access denied"));
                                }
                                class
                                    .static_fields
                                    .get(&name)
                                    .cloned()
                                    .unwrap_or(Value::Null)
                            } else if let Some(value) = class.static_fields.get(&name).cloned() {
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
                                    owner: Rc::clone(&info.owner),
                                    visibility: info.visibility,
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
                    let info = resolve_method(&superclass, &name)
                        .ok_or_else(|| RuntimeError::new("unknown super method"))?;
                    self.registers[base + a] = Value::BoundMethod {
                        receiver,
                        function: info.function,
                        owner: Rc::clone(&info.owner),
                        visibility: info.visibility,
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
                    let mut methods = HashMap::new();
                    let mut constructor = None;
                    let owner = Rc::new(name.clone());
                    for (method_name, value) in methods_map {
                        let info = parse_method_info(value, &owner, module.as_ref())?;
                        if method_name == "constructor" {
                            constructor = Some(info.function);
                        } else {
                            methods.insert(method_name, info);
                        }
                    }
                    let class = Class {
                        name: Rc::new(name),
                        methods,
                        static_methods: HashMap::new(),
                        constructor,
                        superclass: None,
                        static_fields: HashMap::new(),
                        fields: HashMap::new(),
                        static_field_visibility: HashMap::new(),
                        module: module.clone(),
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
                                static_fields: class.static_fields.clone(),
                                fields: class.fields.clone(),
                                static_field_visibility: class.static_field_visibility.clone(),
                                module: class.module.clone(),
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
                    let (owner, module) = match &self.registers[base + a] {
                        Value::Class(class) => (Rc::clone(&class.name), class.module.clone()),
                        _ => return Err(RuntimeError::new("set static on non-class value")),
                    };
                    for (method_name, value) in static_map {
                        let info = parse_method_info(value, &owner, module.as_ref())?;
                        static_methods.insert(method_name, info);
                    }
                    match &mut self.registers[base + a] {
                        Value::Class(class) => {
                            let class = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods,
                                constructor: class.constructor,
                                superclass: class.superclass.clone(),
                                static_fields: class.static_fields.clone(),
                                fields: class.fields.clone(),
                                static_field_visibility: class.static_field_visibility.clone(),
                                module: class.module.clone(),
                            };
                            self.registers[base + a] = Value::Class(Rc::new(class));
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
                            let mut fields = class.fields.clone();
                            fields.insert(name, visibility);
                            let class = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods: class.static_methods.clone(),
                                constructor: class.constructor,
                                superclass: class.superclass.clone(),
                                static_fields: class.static_fields.clone(),
                                fields,
                                static_field_visibility: class.static_field_visibility.clone(),
                                module: class.module.clone(),
                            };
                            self.registers[base + a] = Value::Class(Rc::new(class));
                        }
                        _ => return Err(RuntimeError::new("set field visibility on non-class value")),
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
                            let mut fields = class.static_field_visibility.clone();
                            fields.insert(name, visibility);
                            let class = Class {
                                name: Rc::clone(&class.name),
                                methods: class.methods.clone(),
                                static_methods: class.static_methods.clone(),
                                constructor: class.constructor,
                                superclass: class.superclass.clone(),
                                static_fields: class.static_fields.clone(),
                                fields: class.fields.clone(),
                                static_field_visibility: fields,
                                module: class.module.clone(),
                            };
                            self.registers[base + a] = Value::Class(Rc::new(class));
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                "set static field visibility on non-class value",
                            ))
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
                            )?;
                        }
                        Value::Function(handle) => {
                            self.call_function(handle, None, &args, base + a, None, Vec::new(), None)?;
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
                            )?;
                        }
                        Value::NativeFunction(func) => {
                            let result = func(&args)?;
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
                            )?;
                        }
                        Value::Class(class) => {
                            let object = Rc::new(std::cell::RefCell::new(Object {
                                class: Some(Rc::clone(&class)),
                                fields: HashMap::new(),
                            }));
                            if let Some(handle) = class.constructor {
                                self.call_function(
                                    handle,
                                    Some(Rc::clone(&object)),
                                    &args,
                                    base + a,
                                    Some(Value::Object(Rc::clone(&object))),
                                    Vec::new(),
                                    Some(Rc::clone(&class.name)),
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
                            return Err(RuntimeError::new(
                                "partial named calls not supported yet",
                            ));
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
                                ("Function", Value::BoundMethod { .. }) => true,
                                ("Function", Value::StaticMethod { .. }) => true,
                                ("Function", Value::Partial { .. }) => true,
                                _ => false,
                            }
                        }
                        Value::Class(target_class) => {
                            match value {
                                Value::Object(obj) => {
                                    if let Some(class) = &obj.borrow().class {
                                        is_subclass_of(class, target_class)
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        }
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
        receiver: Option<Rc<std::cell::RefCell<Object>>>,
        args: &[Value],
        return_reg: usize,
        return_override: Option<Value>,
        captures: Vec<Value>,
        current_class: Option<Rc<String>>,
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

            let has_variadic = function.params.last().map(|p| p.is_variadic).unwrap_or(false);
            
            if !has_variadic && final_args.len() > function.arity {
                return Err(RuntimeError::new("argument count mismatch"));
            }

            let mut processed = Vec::new();
            let mut arg_idx = 0;
            
            for param in function.params.iter() {
                if param.is_variadic {
                    let rest = final_args[arg_idx..].to_vec();
                    processed.push(Value::Array(Rc::new(RefCell::new(rest))));
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
                    return Err(RuntimeError::new("named args not supported for native functions"));
                }
                let result = func(&positional)?;
                self.registers[return_reg] = result;
                Ok(())
            }
            Value::Class(class) => {
                let object = Rc::new(std::cell::RefCell::new(Object {
                    class: Some(Rc::clone(&class)),
                    fields: HashMap::new(),
                }));
                if let Some(handle) = class.constructor {
                    let function = self
                        .functions
                        .get(handle)
                        .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                    let args = self.build_named_args(function, true, &positional, &named)?;
                    let override_value = Some(Value::Object(Rc::clone(&object)));
                    self.call_function(
                        handle,
                        Some(object),
                        &args,
                        return_reg,
                        override_value,
                        Vec::new(),
                        Some(Rc::clone(&class.name)),
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
                let args = self.build_named_args(function, false, &positional, &named)?;
                self.call_function(handle, None, &args, return_reg, None, Vec::new(), None)
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
                let args = self.build_named_args(function_info, true, &positional, &named)?;
                self.call_function(
                    function,
                    Some(receiver),
                    &args,
                    return_reg,
                    None,
                    Vec::new(),
                    Some(owner),
                )
            }
            Value::StaticMethod {
                function,
                owner,
                ..
            } => {
                let function_info = self
                    .functions
                    .get(function)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(function_info, false, &positional, &named)?;
                self.call_function(
                    function,
                    None,
                    &args,
                    return_reg,
                    None,
                    Vec::new(),
                    Some(owner),
                )
            }
            Value::Closure { function, captures } => {
                let function_info = self
                    .functions
                    .get(function)
                    .ok_or_else(|| RuntimeError::new("unknown function handle"))?;
                let args = self.build_named_args(function_info, false, &positional, &named)?;
                self.call_function(function, None, &args, return_reg, None, captures, None)
            }
            _ => Err(RuntimeError::new("call on non-callable value")),
        }
    }

    fn build_named_args(
        &self,
        function: &Function,
        has_receiver: bool,
        positional: &[Value],
        named: &HashMap<String, Value>,
    ) -> Result<Vec<Value>, RuntimeError> {
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
                args.push(Value::Array(Rc::new(RefCell::new(rest))));
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
        Err(RuntimeError::new(format!("uncaught exception: {}", error_value)))
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
        let source = fs::read_to_string(&resolved)
            .map_err(|err| RuntimeError::new(format!("failed to read module {resolved_key}: {err}")))?;
        let tokens =
            pleno_lexer::lex(&source).map_err(|err| RuntimeError::new(err.message))?;
        let mut parser = pleno_parser::Parser::new(tokens);
        let program =
            parser.parse_program().map_err(|err| RuntimeError::new(err.message))?;
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
        let current_dir = std::env::current_dir().map_err(|err| {
            RuntimeError::new(format!("failed to get current dir: {err}"))
        })?;
        let std_path = current_dir.join("sources/std").join(format!("{}.pln", name));
        if std_path.exists() {
            return Ok(std_path);
        }
        return Err(RuntimeError::new(format!("std module not found: {}", name)));
    }

    if let Some(name) = path.strip_prefix("registry:") {
        let current_dir = std::env::current_dir().map_err(|err| {
            RuntimeError::new(format!("failed to get current dir: {err}"))
        })?;
        let pkg_path = current_dir.join("sources/packages").join(format!("{}.pln", name));
        if pkg_path.exists() {
            return Ok(pkg_path);
        }
        let pkg_index = current_dir.join("sources/packages").join(name).join("index.pln");
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
    let current_dir = std::env::current_dir().map_err(|err| {
        RuntimeError::new(format!("failed to get current dir: {err}"))
    })?;
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
    Err(RuntimeError::new(format!(
        "module not found: {path}"
    )))
}

fn native_print(args: &[Value]) -> Result<Value, RuntimeError> {
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

fn native_id(args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.get(0).cloned().unwrap_or(Value::Null))
}

fn resolve_method(class: &Class, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.methods.get(name) {
        return Some(info.clone());
    }
    class
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_method(superclass, name))
}

fn resolve_static_method(class: &Class, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.static_methods.get(name) {
        return Some(info.clone());
    }
    class
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_static_method(superclass, name))
}

fn parse_method_info(
    value: Value,
    owner: &Rc<String>,
    module: Option<&Rc<String>>,
) -> Result<MethodInfo, RuntimeError> {
    match value {
        Value::Function(handle) => Ok(MethodInfo {
            function: handle,
            visibility: MethodVisibility::Public,
            owner: Rc::clone(owner),
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
                owner: Rc::clone(owner),
                module: module.cloned(),
            })
        }
        _ => Err(RuntimeError::new("method entry must be function or metadata array")),
    }
}

fn can_access_method(
    method: &MethodInfo,
    current_class: Option<&Rc<String>>,
    current_module: Option<&Rc<String>>,
    globals: &HashMap<String, Value>,
) -> bool {
    match method.visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => {
            current_class.map(|name| name.as_str() == method.owner.as_str()).unwrap_or(false)
        }
        MethodVisibility::Protected => current_class
            .and_then(|name| is_subclass(name, method.owner.as_ref(), globals))
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
    current_class: Option<&Rc<String>>,
    current_module: Option<&Rc<String>>,
    globals: &HashMap<String, Value>,
) -> bool {
    match visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class
            .map(|name| name.as_str() == owner.as_str())
            .unwrap_or(false),
        MethodVisibility::Protected => current_class
            .and_then(|name| is_subclass(name, owner.as_ref(), globals))
            .unwrap_or(false),
        MethodVisibility::Internal => owner_module
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
    class_name: &str,
    owner_name: &str,
    globals: &HashMap<String, Value>,
) -> Option<bool> {
    if class_name == owner_name {
        return Some(true);
    }
    let class = globals.get(class_name).and_then(|value| match value {
        Value::Class(class) => Some(Rc::clone(class)),
        _ => None,
    })?;
    let mut current = class.superclass.as_ref().map(Rc::clone);
    while let Some(next) = current {
        if next.name.as_str() == owner_name {
            return Some(true);
        }
        current = next.superclass.as_ref().map(Rc::clone);
    }
    Some(false)
}

fn is_subclass_of(class: &Rc<Class>, target: &Rc<Class>) -> bool {
    if Rc::ptr_eq(class, target) {
        return true;
    }
    let mut current = class.superclass.clone();
    while let Some(super_class) = current {
        if Rc::ptr_eq(&super_class, target) {
            return true;
        }
        current = super_class.superclass.clone();
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

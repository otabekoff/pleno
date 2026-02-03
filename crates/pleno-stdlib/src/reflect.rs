use indexmap::{IndexMap, IndexSet};
use pleno_value::{
    ArrayState, BoundMethodState, Class, MethodOwner, RuntimeError, VMProvider, Value,
};
use std::rc::Rc;

pub fn reflect_get_type_name(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let value = args.first().cloned().unwrap_or(Value::Null);
    let type_name = match &value {
        Value::Object(obj) => obj
            .borrow()
            .class
            .as_ref()
            .map(|c| c.borrow().name.to_string())
            .unwrap_or_else(|| "Object".to_string()),
        _ => value.type_name().to_string(),
    };
    Ok(Value::String(Rc::new(type_name)))
}

pub fn reflect_get_fields(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let value = args.first().cloned().unwrap_or(Value::Null);
    let mut names: IndexSet<String> = IndexSet::new();

    match value {
        Value::Object(obj) => {
            let obj_ref = obj.borrow();
            // Instance fields that currently have values
            for name in obj_ref.fields.keys() {
                names.insert(name.clone());
            }
            // Declared fields on the class (if any)
            if let Some(class) = &obj_ref.class {
                for name in class.borrow().fields.keys() {
                    names.insert(name.clone());
                }
            }
        }
        _ => {
            // For non-objects, return empty array
        }
    }

    let mut list: Vec<Value> = names
        .into_iter()
        .map(|n| Value::String(Rc::new(n)))
        .collect();
    list.sort_by(|a, b| {
        a.as_str()
            .unwrap_or_default()
            .cmp(b.as_str().unwrap_or_default())
    });

    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(list))?))
}

pub fn reflect_get_methods(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let value = args.first().cloned().unwrap_or(Value::Null);
    let mut names: IndexSet<String> = IndexSet::new();

    // Methods live on Class
    let class_opt: Option<Class> = match value {
        Value::Class(c) => Some(c.borrow().clone()),
        Value::Object(obj) => obj.borrow().class.as_ref().map(|c| c.borrow().clone()),
        _ => None,
    };

    if let Some(class) = class_opt {
        for name in class.methods.keys() {
            names.insert(name.clone());
        }
        for name in class.static_methods.keys() {
            names.insert(name.clone());
        }
    }

    let mut list: Vec<Value> = names
        .into_iter()
        .map(|n| Value::String(Rc::new(n)))
        .collect();
    list.sort_by(|a, b| {
        a.as_str()
            .unwrap_or_default()
            .cmp(b.as_str().unwrap_or_default())
    });

    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(list))?))
}

pub fn reflect_get_field(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "Reflect.getField(value, name) expects 2 arguments",
        ));
    }
    let target = args[0].clone();
    let name = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Reflect.getField expects field name as string",
            ));
        }
    };

    match target {
        Value::Object(obj) => {
            let obj_ref = obj.borrow();
            Ok(obj_ref.fields.get(name).cloned().unwrap_or(Value::Null))
        }
        _ => Err(RuntimeError::new(
            "Reflect.getField expects an object instance",
        )),
    }
}

pub fn reflect_set_field(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "Reflect.setField(value, name, newValue) expects 3 arguments",
        ));
    }
    let target = args[0].clone();
    let name = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Reflect.setField expects field name as string",
            ));
        }
    };
    let new_value = args[2].clone();

    match target {
        Value::Object(obj) => {
            let mut obj_ref = obj.borrow_mut();
            obj_ref.ensure_mutable()?;
            obj_ref.fields.insert(name.to_string(), new_value);
            Ok(Value::Null)
        }
        _ => Err(RuntimeError::new(
            "Reflect.setField expects an object instance",
        )),
    }
}

pub fn reflect_is_instance_of(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "Reflect.isInstanceOf(value, Type) expects 2 arguments",
        ));
    }
    let value = &args[0];
    let ty = &args[1];

    Ok(Value::Bool(vm.check_is_instance(value, ty)))
}

pub fn reflect_call_method(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 3 {
        return Err(RuntimeError::new(
            "Reflect.callMethod(target, name, argsArray) expects 3 arguments",
        ));
    }
    let target = args[0].clone();
    let name = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Reflect.callMethod expects method name as string",
            ));
        }
    };
    let arg_list = match &args[2] {
        Value::Array(arr) => arr.borrow().elements.clone(),
        _ => {
            return Err(RuntimeError::new(
                "Reflect.callMethod expects third argument to be an Array",
            ));
        }
    };

    // Methods live on the object's class; dynamic dispatch is handled by BoundMethod
    match target {
        Value::Object(obj) => {
            let obj_ref = obj.borrow();
            if let Some(class) = &obj_ref.class {
                let class_ref = class.borrow();
                if let Some(method) = class_ref.methods.get(name) {
                    let bound = Value::BoundMethod(vm.get_gc().allocate(BoundMethodState {
                        receiver: Value::Object(obj.clone()),
                        function: method.function.clone(),
                        owner: MethodOwner::Class(class.clone()),
                        visibility: method.visibility.clone(),
                    })?);
                    vm.call(bound, arg_list)
                } else {
                    Err(RuntimeError::new("method not found on target"))
                }
            } else {
                Err(RuntimeError::new("object has no associated class"))
            }
        }
        _ => Err(RuntimeError::new(
            "Reflect.callMethod currently only supports object instances",
        )),
    }
}

pub fn reflect_get_annotations(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new(
            "Reflect.getAnnotations(target) expects 1 argument",
        ));
    }
    let target = &args[0];

    let annotations = match target {
        Value::Class(class) => class.borrow().annotations.clone(),
        Value::Enum(enum_type) => enum_type.borrow().annotations.clone(),
        Value::Object(obj) => {
            // Return class annotations for object instance
            obj.borrow()
                .class
                .as_ref()
                .map(|c| c.borrow().annotations.clone())
                .unwrap_or_default()
        }
        // For functions, annotations are stored in the FunctionProto which requires VM internal access
        // This can be extended later with a VMProvider method
        _ => Vec::new(),
    };

    let mut list = Vec::new();
    let gc = vm.get_gc();
    for anno in annotations {
        // Convert RuntimeAnnotation to Map {name: ..., args: ...}
        let mut map = IndexMap::new();
        map.insert("name".to_string(), Value::String(Rc::new(anno.name)));

        // Convert args to Value::Array
        let args_val = Value::Array(gc.allocate(ArrayState::new(anno.args))?);
        map.insert("args".to_string(), args_val);

        list.push(Value::Map(gc.allocate(pleno_value::MapState::new(map))?));
    }

    Ok(Value::Array(gc.allocate(ArrayState::new(list))?))
}

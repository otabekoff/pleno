use pleno_value::{
    Class, EnumType, MethodInfo, MethodOwner, MethodVisibility, RuntimeError, Value, gc::GcRc,
};
use std::rc::Rc;

pub fn resolve_method(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().methods.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_method(superclass, name))
}

pub fn resolve_enum_method(enum_type: &GcRc<EnumType>, name: &str) -> Option<MethodInfo> {
    enum_type.borrow().methods.get(name).cloned()
}

pub fn resolve_enum_static_method(enum_type: &GcRc<EnumType>, name: &str) -> Option<MethodInfo> {
    enum_type.borrow().static_methods.get(name).cloned()
}

pub fn resolve_field(class: &GcRc<Class>, name: &str) -> Option<MethodVisibility> {
    if let Some(visibility) = class.borrow().fields.get(name) {
        return Some(visibility.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_field(superclass, name))
}

pub fn resolve_super_method(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_method(superclass, name))
}

pub fn resolve_getter(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().getters.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_getter(superclass, name))
}

pub fn resolve_setter(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().setters.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_setter(superclass, name))
}

pub fn resolve_static_method(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().static_methods.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_static_method(superclass, name))
}

pub fn resolve_static_getter(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().static_getters.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_static_getter(superclass, name))
}

pub fn resolve_static_setter(class: &GcRc<Class>, name: &str) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().static_setters.get(name) {
        return Some(info.clone());
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(|superclass| resolve_static_setter(superclass, name))
}

pub fn can_access_method(
    method: &MethodInfo,
    current_class: Option<&GcRc<Class>>,
    current_module: Option<&Rc<String>>,
) -> bool {
    match method.visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => match &method.owner {
            MethodOwner::Class(owner_class) => {
                current_class.map(|c| c == owner_class).unwrap_or(false)
            }
            MethodOwner::Enum(_) | MethodOwner::Interface(_) => {
                // For enums/interfaces, we check module parity for private access
                method
                    .module
                    .as_ref()
                    .map(|m| Some(m) == current_module)
                    .unwrap_or(true)
            }
        },
        MethodVisibility::Protected => match &method.owner {
            MethodOwner::Class(owner_class) => current_class
                .map(|c| is_subclass_of(c, owner_class))
                .unwrap_or(false),
            MethodOwner::Enum(_) | MethodOwner::Interface(_) => {
                // Enums/interfaces don't have subclassing for methods, so protected behaves like internal/private
                method
                    .module
                    .as_ref()
                    .map(|m| Some(m) == current_module)
                    .unwrap_or(true)
            }
        },
        MethodVisibility::Internal => match (&method.module, current_module) {
            (Some(o), Some(c)) => o.as_str() == c.as_str(),
            (None, None) => true,
            _ => false,
        },
    }
}

pub fn can_access_field(
    visibility: &MethodVisibility,
    owner_name: &Rc<String>,
    owner_module: Option<&Rc<String>>,
    current_class: Option<&GcRc<Class>>,
    current_module: Option<&Rc<String>>,
) -> bool {
    match visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class
            .map(|c| c.borrow().name.as_str() == owner_name.as_str())
            .unwrap_or(false),
        MethodVisibility::Protected => current_class
            .map(|c| {
                let class_name = c.borrow().name.to_string();
                class_name == owner_name.as_str() || {
                    let mut current = c.borrow().superclass.clone();
                    while let Some(sc) = current {
                        if sc.borrow().name.as_str() == owner_name.as_str() {
                            return true;
                        }
                        current = sc.borrow().superclass.clone();
                    }
                    false
                }
            })
            .unwrap_or(false),
        MethodVisibility::Internal => match (owner_module, current_module) {
            (Some(o), Some(c)) => o.as_str() == c.as_str(),
            (None, None) => true,
            _ => false,
        },
    }
}

pub fn is_subclass_of(class: &GcRc<Class>, target: &GcRc<Class>) -> bool {
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

pub fn parse_method_info(
    value: Value,
    owner: MethodOwner,
    module: Option<&Rc<String>>,
) -> Result<MethodInfo, RuntimeError> {
    match value {
        Value::Function(handle) => Ok(MethodInfo {
            function: Value::Function(handle),
            visibility: MethodVisibility::Public,
            owner: owner.clone(),
            module: module.cloned(),
            annotations: Vec::new(),
        }),
        Value::Array(array) => {
            let items = array.borrow();
            if items.len() != 2 {
                return Err(RuntimeError::new("method metadata must have 2 items"));
            }
            let handle = match &items[0] {
                Value::Function(handle) => Value::Function(*handle),
                Value::Closure(c) => Value::Closure(c.clone()),
                _ => {
                    return Err(RuntimeError::new(
                        "method entry must be function or closure",
                    ));
                }
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
                annotations: Vec::new(),
            })
        }
        _ => Err(RuntimeError::new(
            "method entry must be function or metadata array",
        )),
    }
}

pub fn visibility_from_code(code: u8) -> Result<MethodVisibility, RuntimeError> {
    match code {
        0 => Ok(MethodVisibility::Public),
        1 => Ok(MethodVisibility::Private),
        2 => Ok(MethodVisibility::Protected),
        3 => Ok(MethodVisibility::Internal),
        _ => Err(RuntimeError::new("invalid visibility code")),
    }
}

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

/// The constructor that runs for `class`: its own, or the nearest inherited one.
pub fn resolve_constructor(class: &GcRc<Class>) -> Option<MethodInfo> {
    if let Some(info) = class.borrow().constructor.clone() {
        return Some(info);
    }
    class
        .borrow()
        .superclass
        .as_ref()
        .and_then(resolve_constructor)
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
            MethodOwner::Enum(_) | MethodOwner::Interface(_) | MethodOwner::Builtin(_) => {
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
            MethodOwner::Enum(_) | MethodOwner::Interface(_) | MethodOwner::Builtin(_) => {
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

/// The class that declares instance field `name`: `class` itself or the
/// nearest superclass that declares it, with the field's visibility.
pub fn find_field_owner(
    class: &GcRc<Class>,
    name: &str,
) -> Option<(GcRc<Class>, MethodVisibility)> {
    let mut current = Some(class.clone());
    while let Some(candidate) = current {
        if let Some(visibility) = candidate.borrow().fields.get(name) {
            return Some((candidate.clone(), visibility.clone()));
        }
        current = candidate.borrow().superclass.clone();
    }
    None
}

/// Whether code running in `current_class` and `current_module` may use a
/// field with `visibility` declared by `owner`.
///
/// - `private`: only inside `owner`'s own methods.
/// - `protected`: inside the methods of `owner` and of its subclasses.
/// - `internal`: anywhere in the module that declares `owner`.
pub fn can_access_field(
    visibility: &MethodVisibility,
    owner: &GcRc<Class>,
    current_class: Option<&GcRc<Class>>,
    current_module: Option<&Rc<String>>,
) -> bool {
    match visibility {
        MethodVisibility::Public => true,
        MethodVisibility::Private => current_class.map(|c| c == owner).unwrap_or(false),
        MethodVisibility::Protected => current_class
            .map(|c| is_subclass_of(c, owner))
            .unwrap_or(false),
        MethodVisibility::Internal => match (owner.borrow().module.as_ref(), current_module) {
            (Some(o), Some(c)) => o.as_str() == c.as_str(),
            (None, None) => true,
            _ => false,
        },
    }
}

/// The error for code that may not use a field: `action` is `read` or
/// `assign to`, `kind` is `field` or `static field`.
pub fn field_access_error(
    action: &str,
    visibility: &MethodVisibility,
    kind: &str,
    name: &str,
    class_name: &str,
) -> String {
    let (word, scope) = match visibility {
        MethodVisibility::Public => ("public", "class"),
        MethodVisibility::Private => ("private", "class"),
        MethodVisibility::Protected => ("protected", "class"),
        MethodVisibility::Internal => ("internal", "module"),
    };
    format!("cannot {action} {word} {kind} '{name}' of {class_name} outside its {scope}")
}

/// The abstract methods of `class` and its ancestors that no class between
/// the declaring one and `class` defines.
pub fn missing_abstract_methods(class: &GcRc<Class>) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = Some(class.clone());
    while let Some(candidate) = current {
        current = candidate.borrow().superclass.clone();
        chain.push(candidate);
    }
    let mut missing: Vec<String> = Vec::new();
    for ancestor in chain.iter().rev() {
        let ancestor = ancestor.borrow();
        missing.retain(|name| {
            !ancestor.methods.contains_key(name) || ancestor.abstract_methods.contains(name)
        });
        for name in &ancestor.abstract_methods {
            if !missing.contains(name) {
                missing.push(name.clone());
            }
        }
    }
    missing
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
        // A closure is a method that captures variables from its enclosing scope.
        function @ (Value::Function(_) | Value::Closure(_)) => Ok(MethodInfo {
            function,
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
    match code & !pleno_compiler::CONST_FIELD_FLAG {
        0 => Ok(MethodVisibility::Public),
        1 => Ok(MethodVisibility::Private),
        2 => Ok(MethodVisibility::Protected),
        3 => Ok(MethodVisibility::Internal),
        _ => Err(RuntimeError::new("invalid visibility code")),
    }
}

//! The names "did you mean ...?" hints choose from when a variable, property
//! or method is not found. Computed only while such an error is reported.

use crate::VM;
use crate::access::{can_access_field, can_access_method};
use pleno_value::suggest::did_you_mean;
use pleno_value::{Class, EnumType, Value, gc::GcRc};
use std::rc::Rc;

/// Whether `name` is one a program could have meant: not an internal global
/// such as `__import`.
fn is_user_name(name: &str) -> bool {
    !name.starts_with("__") && !name.is_empty()
}

impl VM {
    /// The hint for reading the undefined variable `name` at instruction `ip`
    /// of the current function: the closest local, parameter or global in scope.
    pub(crate) fn suggest_variable(&self, name: &str, ip: usize) -> String {
        let mut candidates: Vec<&str> = Vec::new();
        if let Some(frame) = self.frames.last()
            && let Some(function) = self.functions.get(frame.function_index)
        {
            candidates.extend(
                function
                    .chunk
                    .locals
                    .iter()
                    .filter(|local| local.start_ip <= ip && ip <= local.end_ip)
                    .map(|local| local.name.as_str()),
            );
            candidates.extend(function.params.iter().map(|param| param.name.as_str()));
            if let Some(table) = function
                .module_path
                .as_ref()
                .and_then(|path| self.module_globals.get(path.as_str()))
            {
                // A module sees its own names and the built-ins, not the
                // main program's.
                candidates.extend(table.keys().map(String::as_str));
                candidates.extend(
                    self.globals
                        .keys()
                        .map(String::as_str)
                        .filter(|key| !self.program_globals.names.contains(*key)),
                );
                candidates.extend(self.program_globals.replaced.keys().map(String::as_str));
            } else {
                candidates.extend(self.globals.keys().map(String::as_str));
            }
        } else {
            candidates.extend(self.globals.keys().map(String::as_str));
        }
        did_you_mean(name, candidates.into_iter().filter(|c| is_user_name(c)))
    }

    /// The hint for a property or method `name` that `receiver` lacks: its
    /// closest member that code in `current_class` and `module` may use.
    pub(crate) fn suggest_member(
        &self,
        receiver: &Value,
        name: &str,
        current_class: Option<&GcRc<Class>>,
        module: Option<&Rc<String>>,
    ) -> String {
        let candidates = self.member_names(receiver, current_class, module);
        did_you_mean(name, candidates.iter().map(String::as_str))
    }

    fn member_names(
        &self,
        receiver: &Value,
        current_class: Option<&GcRc<Class>>,
        module: Option<&Rc<String>>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        match receiver {
            Value::Object(object) => {
                let object = object.borrow();
                match &object.class {
                    Some(class) => {
                        instance_member_names(class, current_class, module, &mut names);
                        // Fields added by assignment, which no class declares.
                        for key in object.fields.keys() {
                            if !class_declares_field(class, key) {
                                names.push(key.clone());
                            }
                        }
                        if class.borrow().extends_error() {
                            names.push("toString".to_string());
                        }
                    }
                    None => names.extend(object.fields.keys().cloned()),
                }
            }
            Value::Class(class) => static_member_names(class, current_class, module, &mut names),
            Value::Enum(enum_type) => {
                enum_static_names(enum_type, current_class, module, &mut names)
            }
            Value::Variant(variant) => {
                names.push("name".to_string());
                names.push("value".to_string());
                let variant = variant.borrow();
                let enum_type = variant.enum_type.borrow();
                names.extend(
                    enum_type
                        .methods
                        .iter()
                        .filter(|(_, info)| can_access_method(info, current_class, module))
                        .map(|(key, _)| key.clone()),
                );
            }
            Value::Map(map) => {
                names.extend(map.borrow().entries.keys().cloned());
                self.builtin_member_names("Map", &mut names);
            }
            other => {
                let type_name = other.type_name();
                self.builtin_member_names(type_name, &mut names);
                let intrinsic: &[&str] = match other {
                    Value::String(_) | Value::Array(_) | Value::Tuple(_) => &["len", "length"],
                    Value::Set(_) => &["len"],
                    Value::Range(_) => &["start", "end", "inclusive"],
                    Value::Function(_) | Value::Closure(_) => &["name"],
                    _ => &[],
                };
                names.extend(intrinsic.iter().map(|n| n.to_string()));
                if let Value::Tuple(tuple) = other
                    && let Some(fields) = &tuple.borrow().names
                {
                    names.extend(fields.keys().cloned());
                }
            }
        }
        names
    }

    /// The built-in methods of `type_name`, those added with `extend`, and
    /// `toString`.
    fn builtin_member_names(&self, type_name: &str, names: &mut Vec<String>) {
        if let Some(resolver) = self.builtin_resolver {
            names.extend(
                self.builtin_method_names
                    .iter()
                    .filter(|method| resolver(type_name, method).is_some())
                    .map(|method| method.to_string()),
            );
        }
        if let Some(methods) = self.builtin_extensions.get(type_name) {
            names.extend(methods.keys().cloned());
        }
        names.push("toString".to_string());
    }
}

fn class_declares_field(class: &GcRc<Class>, name: &str) -> bool {
    let mut current = Some(class.clone());
    while let Some(candidate) = current {
        if candidate.borrow().fields.contains_key(name) {
            return true;
        }
        current = candidate.borrow().superclass.clone();
    }
    false
}

/// The fields, methods and getters of instances of `class` and its
/// superclasses that code in `current_class` and `module` may use.
fn instance_member_names(
    class: &GcRc<Class>,
    current_class: Option<&GcRc<Class>>,
    module: Option<&Rc<String>>,
    names: &mut Vec<String>,
) {
    let mut current = Some(class.clone());
    while let Some(owner) = current {
        let borrowed = owner.borrow();
        for (field, visibility) in &borrowed.fields {
            if can_access_field(visibility, &owner, current_class, module) {
                names.push(field.clone());
            }
        }
        for (method, info) in borrowed.methods.iter().chain(&borrowed.getters) {
            if can_access_method(info, current_class, module) {
                names.push(method.clone());
            }
        }
        current = borrowed.superclass.clone();
    }
}

/// The static fields, methods and getters of `class` and its superclasses
/// that code in `current_class` and `module` may use.
fn static_member_names(
    class: &GcRc<Class>,
    current_class: Option<&GcRc<Class>>,
    module: Option<&Rc<String>>,
    names: &mut Vec<String>,
) {
    let mut current = Some(class.clone());
    while let Some(owner) = current {
        let borrowed = owner.borrow();
        for field in borrowed.static_fields.keys() {
            let visible = borrowed
                .static_field_visibility
                .get(field)
                .is_none_or(|visibility| {
                    can_access_field(visibility, &owner, current_class, module)
                });
            if visible {
                names.push(field.clone());
            }
        }
        for (method, info) in borrowed
            .static_methods
            .iter()
            .chain(&borrowed.static_getters)
        {
            if can_access_method(info, current_class, module) {
                names.push(method.clone());
            }
        }
        current = borrowed.superclass.clone();
    }
}

fn enum_static_names(
    enum_type: &GcRc<EnumType>,
    current_class: Option<&GcRc<Class>>,
    module: Option<&Rc<String>>,
    names: &mut Vec<String>,
) {
    let enum_type = enum_type.borrow();
    names.extend(enum_type.variants.keys().cloned());
    names.extend(
        enum_type
            .static_methods
            .iter()
            .filter(|(_, info)| can_access_method(info, current_class, module))
            .map(|(key, _)| key.clone()),
    );
}

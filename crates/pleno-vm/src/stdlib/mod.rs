pub mod path;
pub mod process;

use crate::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub fn get_stdlib_modules(gc: &mut pleno_value::gc::GCManager) -> HashMap<String, Value> {
    let mut modules = HashMap::new();

    // std:path
    let mut path_module = HashMap::new();
    path_module.insert("join".to_string(), Value::NativeFunction(path::path_join));
    path_module.insert(
        "dirname".to_string(),
        Value::NativeFunction(path::path_dirname),
    );
    path_module.insert(
        "basename".to_string(),
        Value::NativeFunction(path::path_basename),
    );
    path_module.insert(
        "extname".to_string(),
        Value::NativeFunction(path::path_extname),
    );
    path_module.insert(
        "exists".to_string(),
        Value::NativeFunction(path::path_exists),
    );
    path_module.insert(
        "isFile".to_string(),
        Value::NativeFunction(path::path_is_file),
    );
    path_module.insert(
        "isDir".to_string(),
        Value::NativeFunction(path::path_is_dir),
    );
    path_module.insert(
        "resolve".to_string(),
        Value::NativeFunction(path::path_resolve),
    );
    path_module.insert(
        "separator".to_string(),
        Value::String(Rc::new(std::path::MAIN_SEPARATOR.to_string())),
    );
    modules.insert("std:path".to_string(), Value::Map(gc.allocate(path_module)));

    // std:process
    let mut process_module = HashMap::new();
    process_module.insert(
        "cwd".to_string(),
        Value::NativeFunction(process::process_cwd),
    );
    process_module.insert(
        "exit".to_string(),
        Value::NativeFunction(process::process_exit),
    );
    process_module.insert(
        "args".to_string(),
        Value::NativeFunction(process::process_args),
    );
    process_module.insert(
        "env".to_string(),
        Value::NativeFunction(process::process_env),
    );
    process_module.insert(
        "pid".to_string(),
        Value::NativeFunction(process::process_pid),
    );
    modules.insert(
        "std:process".to_string(),
        Value::Map(gc.allocate(process_module)),
    );

    modules
}

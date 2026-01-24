pub mod io;
pub mod json;
pub mod path;
pub mod process;
pub mod time;

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
    modules.insert(
        "std:path".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(path_module))),
    );

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
        Value::Map(gc.allocate(pleno_value::MapState::new(process_module))),
    );

    // std:io
    let mut file_obj = HashMap::new();
    file_obj.insert("read".to_string(), Value::NativeFunction(io::io_file_read));
    file_obj.insert(
        "write".to_string(),
        Value::NativeFunction(io::io_file_write),
    );
    file_obj.insert(
        "exists".to_string(),
        Value::NativeFunction(io::io_file_exists),
    );

    let mut io_module = HashMap::new();
    io_module.insert(
        "File".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(file_obj))),
    );
    modules.insert(
        "std:io".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(io_module))),
    );

    // std:json
    let mut json_obj = HashMap::new();
    json_obj.insert("parse".to_string(), Value::NativeFunction(json::json_parse));
    json_obj.insert(
        "stringify".to_string(),
        Value::NativeFunction(json::json_stringify),
    );

    let mut json_module = HashMap::new();
    json_module.insert(
        "JSON".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(json_obj))),
    );
    modules.insert(
        "std:json".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(json_module))),
    );

    // std:time
    let mut time_module = HashMap::new();
    time_module.insert("now".to_string(), Value::NativeFunction(time::time_now));
    modules.insert(
        "std:time".to_string(),
        Value::Map(gc.allocate(pleno_value::MapState::new(time_module))),
    );

    modules
}

pub mod builtins;
pub mod concurrency;
pub mod http;
pub mod io;
pub mod json;
pub mod math;
pub mod path;
pub mod process;
pub mod regex;
pub mod time;

use pleno_value::{MapState, VMProvider, Value};
use std::collections::HashMap;
use std::rc::Rc;

pub fn register(vm: &mut dyn VMProvider) {
    // Register global built-ins
    builtins::register_builtins(vm);
    vm.set_builtin_resolver(builtins::resolve_builtin_method);

    let gc = vm.get_gc();
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
        Value::Map(gc.allocate(MapState::new(path_module))),
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
        Value::Map(gc.allocate(MapState::new(process_module))),
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
        Value::Map(gc.allocate(MapState::new(file_obj))),
    );
    modules.insert(
        "std:io".to_string(),
        Value::Map(gc.allocate(MapState::new(io_module))),
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
        Value::Map(gc.allocate(MapState::new(json_obj))),
    );
    modules.insert(
        "std:json".to_string(),
        Value::Map(gc.allocate(MapState::new(json_module))),
    );

    // std:time
    let mut time_module = HashMap::new();
    time_module.insert("now".to_string(), Value::NativeFunction(time::time_now));
    time_module.insert(
        "format".to_string(),
        Value::NativeFunction(time::time_format),
    );
    time_module.insert("parse".to_string(), Value::NativeFunction(time::time_parse));
    time_module.insert("parts".to_string(), Value::NativeFunction(time::time_parts));
    modules.insert(
        "std:time".to_string(),
        Value::Map(gc.allocate(MapState::new(time_module))),
    );

    // std:math
    let mut math_obj = HashMap::new();
    math_obj.insert("abs".to_string(), Value::NativeFunction(math::math_abs));
    math_obj.insert("min".to_string(), Value::NativeFunction(math::math_min));
    math_obj.insert("max".to_string(), Value::NativeFunction(math::math_max));
    math_obj.insert("floor".to_string(), Value::NativeFunction(math::math_floor));
    math_obj.insert("ceil".to_string(), Value::NativeFunction(math::math_ceil));
    math_obj.insert("sqrt".to_string(), Value::NativeFunction(math::math_sqrt));

    let mut math_module = HashMap::new();
    math_module.insert(
        "Math".to_string(),
        Value::Map(gc.allocate(MapState::new(math_obj))),
    );
    modules.insert(
        "std:math".to_string(),
        Value::Map(gc.allocate(MapState::new(math_module))),
    );

    // std:http
    let mut http_module = HashMap::new();
    http_module.insert("get".to_string(), Value::NativeFunction(http::http_get));
    http_module.insert("post".to_string(), Value::NativeFunction(http::http_post));
    http_module.insert("put".to_string(), Value::NativeFunction(http::http_put));
    http_module.insert(
        "delete".to_string(),
        Value::NativeFunction(http::http_delete),
    );
    modules.insert(
        "std:http".to_string(),
        Value::Map(gc.allocate(MapState::new(http_module))),
    );

    // std:regex
    let mut regex_module = HashMap::new();
    regex_module.insert(
        "isMatch".to_string(),
        Value::NativeFunction(regex::regex_is_match),
    );
    regex_module.insert(
        "match".to_string(),
        Value::NativeFunction(regex::regex_match),
    );
    regex_module.insert(
        "findAll".to_string(),
        Value::NativeFunction(regex::regex_find_all),
    );
    regex_module.insert(
        "replace".to_string(),
        Value::NativeFunction(regex::regex_replace),
    );
    modules.insert(
        "std:regex".to_string(),
        Value::Map(gc.allocate(MapState::new(regex_module))),
    );

    // Register each module in the VM
    for (name, value) in modules {
        vm.register_module(name, value);
    }
}

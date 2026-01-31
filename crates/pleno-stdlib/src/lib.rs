pub mod builtins;
pub mod concurrency;
pub mod http;
pub mod io;
pub mod json;
pub mod math;
pub mod path;
pub mod process;
pub mod promise;
pub mod reflect;
pub mod regex;
pub mod test;
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
    file_obj.insert(
        "append".to_string(),
        Value::NativeFunction(io::io_file_append),
    );
    file_obj.insert(
        "delete".to_string(),
        Value::NativeFunction(io::io_file_delete),
    );
    file_obj.insert("copy".to_string(), Value::NativeFunction(io::io_file_copy));
    file_obj.insert("move".to_string(), Value::NativeFunction(io::io_file_move));
    file_obj.insert(
        "readLines".to_string(),
        Value::NativeFunction(io::io_file_read_lines),
    );

    let mut dir_obj = HashMap::new();
    dir_obj.insert(
        "create".to_string(),
        Value::NativeFunction(io::io_directory_create),
    );
    dir_obj.insert(
        "exists".to_string(),
        Value::NativeFunction(io::io_directory_exists),
    );
    dir_obj.insert(
        "delete".to_string(),
        Value::NativeFunction(io::io_directory_delete),
    );
    dir_obj.insert(
        "list".to_string(),
        Value::NativeFunction(io::io_directory_list),
    );
    dir_obj.insert(
        "listFiles".to_string(),
        Value::NativeFunction(io::io_directory_list_files),
    );
    dir_obj.insert(
        "listDirectories".to_string(),
        Value::NativeFunction(io::io_directory_list_directories),
    );
    dir_obj.insert(
        "createRecursive".to_string(),
        Value::NativeFunction(io::io_directory_create_recursive),
    );
    dir_obj.insert(
        "deleteRecursive".to_string(),
        Value::NativeFunction(io::io_directory_delete_recursive),
    );

    let mut io_module = HashMap::new();
    io_module.insert(
        "File".to_string(),
        Value::Map(gc.allocate(MapState::new(file_obj))),
    );
    io_module.insert("open".to_string(), Value::NativeFunction(io::io_open));
    io_module.insert("stdout".to_string(), Value::NativeFunction(io::io_stdout));
    io_module.insert("stderr".to_string(), Value::NativeFunction(io::io_stderr));
    io_module.insert(
        "Directory".to_string(),
        Value::Map(gc.allocate(MapState::new(dir_obj))),
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

    let mut datetime_obj = HashMap::new();
    datetime_obj.insert(
        "now".to_string(),
        Value::NativeFunction(time::time_datetime_now),
    );

    time_module.insert(
        "DateTime".to_string(),
        Value::Map(gc.allocate(MapState::new(datetime_obj))),
    );
    modules.insert(
        "std:time".to_string(),
        Value::Map(gc.allocate(MapState::new(time_module))),
    );

    // std:math
    let mut math_module = HashMap::new();
    math_module.insert("abs".to_string(), Value::NativeFunction(math::math_abs));
    math_module.insert("min".to_string(), Value::NativeFunction(math::math_min));
    math_module.insert("max".to_string(), Value::NativeFunction(math::math_max));
    math_module.insert("floor".to_string(), Value::NativeFunction(math::math_floor));
    math_module.insert("ceil".to_string(), Value::NativeFunction(math::math_ceil));
    math_module.insert("sqrt".to_string(), Value::NativeFunction(math::math_sqrt));
    math_module.insert("PI".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert("E".to_string(), Value::Float(std::f64::consts::E));

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
    http_module.insert(
        "Router".to_string(),
        Value::NativeFunction(http::http_router),
    );
    http_module.insert(
        "Server".to_string(),
        Value::NativeFunction(http::http_server),
    );
    modules.insert(
        "std:http".to_string(),
        Value::Map(gc.allocate(MapState::new(http_module))),
    );

    // std:regex
    let mut regex_module = HashMap::new();
    regex_module.insert(
        "Regex".to_string(),
        Value::NativeFunction(regex::regex_constructor),
    );
    modules.insert(
        "std:regex".to_string(),
        Value::Map(gc.allocate(MapState::new(regex_module))),
    );

    // std:reflect
    let mut reflect_module = HashMap::new();
    reflect_module.insert(
        "getTypeName".to_string(),
        Value::NativeFunction(reflect::reflect_get_type_name),
    );
    reflect_module.insert(
        "getFields".to_string(),
        Value::NativeFunction(reflect::reflect_get_fields),
    );
    reflect_module.insert(
        "getMethods".to_string(),
        Value::NativeFunction(reflect::reflect_get_methods),
    );
    reflect_module.insert(
        "getField".to_string(),
        Value::NativeFunction(reflect::reflect_get_field),
    );
    reflect_module.insert(
        "setField".to_string(),
        Value::NativeFunction(reflect::reflect_set_field),
    );
    reflect_module.insert(
        "isInstanceOf".to_string(),
        Value::NativeFunction(reflect::reflect_is_instance_of),
    );
    reflect_module.insert(
        "callMethod".to_string(),
        Value::NativeFunction(reflect::reflect_call_method),
    );
    let reflect_val = Value::Map(gc.allocate(MapState::new(reflect_module)));
    modules.insert("std:reflect".to_string(), reflect_val.clone());

    // std:test
    let mut assert_obj = HashMap::new();
    assert_obj.insert(
        "equal".to_string(),
        Value::NativeFunction(test::assert_equal),
    );
    assert_obj.insert(
        "notEqual".to_string(),
        Value::NativeFunction(test::assert_not_equal),
    );
    assert_obj.insert("true".to_string(), Value::NativeFunction(test::assert_true));
    assert_obj.insert(
        "false".to_string(),
        Value::NativeFunction(test::assert_false),
    );
    assert_obj.insert(
        "throws".to_string(),
        Value::NativeFunction(test::assert_throws),
    );

    let mut test_module = HashMap::new();
    test_module.insert(
        "describe".to_string(),
        Value::NativeFunction(test::test_describe),
    );
    test_module.insert("test".to_string(), Value::NativeFunction(test::test_test));
    test_module.insert(
        "beforeEach".to_string(),
        Value::NativeFunction(test::test_before_each),
    );
    test_module.insert(
        "afterEach".to_string(),
        Value::NativeFunction(test::test_after_each),
    );
    test_module.insert(
        "assert".to_string(),
        Value::Map(gc.allocate(MapState::new(assert_obj))),
    );
    modules.insert(
        "std:test".to_string(),
        Value::Map(gc.allocate(MapState::new(test_module))),
    );

    // Register each module in the VM
    for (name, value) in modules {
        if name == "std:reflect" {
            vm.register_global("Reflect".to_string(), value.clone());
        }
        if name == "std:io" {
            vm.register_global("IO".to_string(), value.clone());
        }
        vm.register_module(name, value);
    }
}

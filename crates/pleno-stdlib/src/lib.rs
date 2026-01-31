pub mod builtins;
pub mod concurrency;
pub mod dotenv;
pub mod gc;
pub mod http;
pub mod io;
pub mod json;
pub mod math;
pub mod net;
pub mod path;
pub mod process;
pub mod promise;
pub mod reflect;
pub mod regex;
pub mod security;
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
        Value::Map(gc.allocate(MapState::new(path_module)).unwrap()),
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
        Value::Map(gc.allocate(MapState::new(process_module)).unwrap()),
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
        Value::Map(gc.allocate(MapState::new(file_obj)).unwrap()),
    );
    io_module.insert("open".to_string(), Value::NativeFunction(io::io_open));
    io_module.insert("stdout".to_string(), Value::NativeFunction(io::io_stdout));
    io_module.insert("stderr".to_string(), Value::NativeFunction(io::io_stderr));
    io_module.insert(
        "Directory".to_string(),
        Value::Map(gc.allocate(MapState::new(dir_obj)).unwrap()),
    );
    modules.insert(
        "std:io".to_string(),
        Value::Map(gc.allocate(MapState::new(io_module)).unwrap()),
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
        Value::Map(gc.allocate(MapState::new(json_obj)).unwrap()),
    );
    modules.insert(
        "std:json".to_string(),
        Value::Map(gc.allocate(MapState::new(json_module)).unwrap()),
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
        Value::Map(gc.allocate(MapState::new(datetime_obj)).unwrap()),
    );
    modules.insert(
        "std:time".to_string(),
        Value::Map(gc.allocate(MapState::new(time_module)).unwrap()),
    );

    // std:math
    let mut math_module = HashMap::new();
    math_module.insert("abs".to_string(), Value::NativeFunction(math::math_abs));
    math_module.insert("min".to_string(), Value::NativeFunction(math::math_min));
    math_module.insert("max".to_string(), Value::NativeFunction(math::math_max));
    math_module.insert("floor".to_string(), Value::NativeFunction(math::math_floor));
    math_module.insert("ceil".to_string(), Value::NativeFunction(math::math_ceil));
    math_module.insert("round".to_string(), Value::NativeFunction(math::math_round));
    math_module.insert("pow".to_string(), Value::NativeFunction(math::math_pow));
    math_module.insert("sqrt".to_string(), Value::NativeFunction(math::math_sqrt));
    math_module.insert(
        "random".to_string(),
        Value::NativeFunction(math::math_random),
    );
    math_module.insert(
        "randomInt".to_string(),
        Value::NativeFunction(math::math_random_int),
    );
    math_module.insert("sin".to_string(), Value::NativeFunction(math::math_sin));
    math_module.insert("cos".to_string(), Value::NativeFunction(math::math_cos));
    math_module.insert("tan".to_string(), Value::NativeFunction(math::math_tan));
    math_module.insert("log".to_string(), Value::NativeFunction(math::math_log));
    math_module.insert("exp".to_string(), Value::NativeFunction(math::math_exp));
    math_module.insert("PI".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert("E".to_string(), Value::Float(std::f64::consts::E));

    modules.insert(
        "std:math".to_string(),
        Value::Map(gc.allocate(MapState::new(math_module)).unwrap()),
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
        Value::Map(gc.allocate(MapState::new(http_module)).unwrap()),
    );

    // std:regex
    let mut regex_module = HashMap::new();
    regex_module.insert(
        "Regex".to_string(),
        Value::NativeFunction(regex::regex_constructor),
    );
    modules.insert(
        "std:regex".to_string(),
        Value::Map(gc.allocate(MapState::new(regex_module)).unwrap()),
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
    reflect_module.insert(
        "getAnnotations".to_string(),
        Value::NativeFunction(reflect::reflect_get_annotations),
    );
    let reflect_val = Value::Map(gc.allocate(MapState::new(reflect_module)).unwrap());
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
    assert_obj.insert(
        "isTrue".to_string(),
        Value::NativeFunction(test::assert_true),
    );
    assert_obj.insert(
        "isFalse".to_string(),
        Value::NativeFunction(test::assert_false),
    );
    assert_obj.insert(
        "throws".to_string(),
        Value::NativeFunction(test::assert_throws),
    );
    assert_obj.insert(
        "isNull".to_string(),
        Value::NativeFunction(test::assert_null),
    );
    assert_obj.insert(
        "notNull".to_string(),
        Value::NativeFunction(test::assert_not_null),
    );
    assert_obj.insert(
        "contains".to_string(),
        Value::NativeFunction(test::assert_contains),
    );
    assert_obj.insert(
        "lengthOf".to_string(),
        Value::NativeFunction(test::assert_length_of),
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
        Value::Map(gc.allocate(MapState::new(assert_obj)).unwrap()),
    );
    modules.insert(
        "std:test".to_string(),
        Value::Map(gc.allocate(MapState::new(test_module)).unwrap()),
    );

    // std:gc - Garbage Collection control
    let mut gc_module = HashMap::new();
    gc_module.insert("collect".to_string(), Value::NativeFunction(gc::gc_collect));
    gc_module.insert(
        "heapSize".to_string(),
        Value::NativeFunction(gc::gc_heap_size),
    );
    gc_module.insert(
        "isEnabled".to_string(),
        Value::NativeFunction(gc::gc_is_enabled),
    );
    let gc_val = Value::Map(gc.allocate(MapState::new(gc_module)).unwrap());
    modules.insert("std:gc".to_string(), gc_val.clone());

    // std:security - Sandboxing
    let mut security_module = HashMap::new();
    security_module.insert(
        "Sandbox".to_string(),
        Value::NativeFunction(security::sandbox_new),
    );
    let security_val = Value::Map(gc.allocate(MapState::new(security_module)).unwrap());
    modules.insert("std:security".to_string(), security_val.clone());

    // std:net - Low-Level Networking
    let mut net_module = HashMap::new();
    net_module.insert(
        "listen".to_string(),
        Value::NativeFunction(net::net_tcp_listen),
    );
    net_module.insert(
        "connect".to_string(),
        Value::NativeFunction(net::net_tcp_connect),
    );
    modules.insert(
        "std:net".to_string(),
        Value::Map(gc.allocate(MapState::new(net_module)).unwrap()),
    );

    // std:dotenv
    let mut dotenv_module = HashMap::new();
    dotenv_module.insert(
        "config".to_string(),
        Value::NativeFunction(dotenv::dotenv_config),
    );
    modules.insert(
        "std:dotenv".to_string(),
        Value::Map(gc.allocate(MapState::new(dotenv_module)).unwrap()),
    );

    // Register each module in the VM
    for (name, value) in modules {
        // if name == "std:reflect" {
        //     vm.register_global("Reflect".to_string(), value.clone());
        // }
        // if name == "std:io" {
        //     vm.register_global("IO".to_string(), value.clone());
        // }
        // if name == "std:gc" {
        //     vm.register_global("GC".to_string(), value.clone());
        // }
        // Strict imports: No modules are registered as globals by default.
        // Users must use `import IO from "std:io"` etc.
        vm.register_module(name, value);
    }
}

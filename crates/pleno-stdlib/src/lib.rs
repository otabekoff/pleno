pub mod base64;
pub mod builtins;
pub mod concurrency;
pub mod crypto;
pub mod csv;
pub mod db;
pub mod dotenv;
pub mod gc;
pub mod http;
pub mod io;
pub mod json;
pub mod log;
pub mod math;
pub mod net;
pub mod os;
pub mod path;
pub mod process;
pub mod promise;
pub mod random;
pub mod reflect;
pub mod regex;
pub mod security;
pub mod test;
pub mod time;
pub mod toml;
pub mod url;
pub mod uuid;
pub mod xml;
pub mod yaml;

use indexmap::IndexMap;
use pleno_value::{MapState, VMProvider, Value};
use std::rc::Rc;

pub fn register(vm: &mut dyn VMProvider) {
    // Register global built-ins
    builtins::register_builtins(vm);
    vm.set_builtin_resolver(builtins::resolve_builtin_method);

    let gc = vm.get_gc();
    let mut modules = IndexMap::new();

    // std:path
    let mut path_module = IndexMap::new();
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
    let mut process_module = IndexMap::new();
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
    let mut file_obj = IndexMap::new();
    file_obj.insert("read".to_string(), Value::NativeFunction(io::io_file_read));
    file_obj.insert(
        "write".to_string(),
        Value::NativeFunction(io::io_file_write),
    );

    let mut dir_obj = IndexMap::new();
    dir_obj.insert(
        "list_files".to_string(),
        Value::NativeFunction(io::io_directory_list_files),
    );
    dir_obj.insert(
        "list_directories".to_string(),
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

    let mut io_module = IndexMap::new();
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

    // std:base64
    let mut base64_module = IndexMap::new();
    base64_module.insert(
        "encode".to_string(),
        Value::NativeFunction(self::base64::base64_encode),
    );
    base64_module.insert(
        "decode".to_string(),
        Value::NativeFunction(self::base64::base64_decode),
    );
    modules.insert(
        "std:base64".to_string(),
        Value::Map(gc.allocate(MapState::new(base64_module)).unwrap()),
    );

    // std:url
    let mut url_module = IndexMap::new();
    let mut url_obj = IndexMap::new();
    url_obj.insert(
        "parse".to_string(),
        Value::NativeFunction(self::url::url_parse),
    );
    url_module.insert(
        "URL".to_string(),
        Value::Map(gc.allocate(MapState::new(url_obj)).unwrap()),
    );
    modules.insert(
        "std:url".to_string(),
        Value::Map(gc.allocate(MapState::new(url_module)).unwrap()),
    );

    // std:csv
    let mut csv_module = IndexMap::new();
    csv_module.insert(
        "parse".to_string(),
        Value::NativeFunction(self::csv::csv_parse),
    );
    modules.insert(
        "std:csv".to_string(),
        Value::Map(gc.allocate(MapState::new(csv_module)).unwrap()),
    );

    // std:uuid
    let mut uuid_module = IndexMap::new();
    uuid_module.insert("v4".to_string(), Value::NativeFunction(self::uuid::uuid_v4));
    uuid_module.insert("v7".to_string(), Value::NativeFunction(self::uuid::uuid_v7));
    uuid_module.insert(
        "parse".to_string(),
        Value::NativeFunction(self::uuid::uuid_parse),
    );
    modules.insert(
        "std:uuid".to_string(),
        Value::Map(gc.allocate(MapState::new(uuid_module)).unwrap()),
    );

    // std:yaml
    let mut yaml_module = IndexMap::new();
    yaml_module.insert(
        "parse".to_string(),
        Value::NativeFunction(self::yaml::yaml_parse),
    );
    yaml_module.insert(
        "stringify".to_string(),
        Value::NativeFunction(self::yaml::yaml_stringify),
    );
    modules.insert(
        "std:yaml".to_string(),
        Value::Map(gc.allocate(MapState::new(yaml_module)).unwrap()),
    );

    // std:toml
    let mut toml_module = IndexMap::new();
    toml_module.insert(
        "parse".to_string(),
        Value::NativeFunction(self::toml::toml_parse),
    );
    toml_module.insert(
        "stringify".to_string(),
        Value::NativeFunction(self::toml::toml_stringify),
    );
    modules.insert(
        "std:toml".to_string(),
        Value::Map(gc.allocate(MapState::new(toml_module)).unwrap()),
    );

    // std:xml
    let mut xml_module = IndexMap::new();
    xml_module.insert(
        "parse".to_string(),
        Value::NativeFunction(self::xml::xml_parse),
    );
    modules.insert(
        "std:xml".to_string(),
        Value::Map(gc.allocate(MapState::new(xml_module)).unwrap()),
    );

    // std:json
    let mut json_module = IndexMap::new();
    let mut json_obj = IndexMap::new();
    json_obj.insert("parse".to_string(), Value::NativeFunction(json::json_parse));
    json_obj.insert(
        "stringify".to_string(),
        Value::NativeFunction(json::json_stringify),
    );
    json_module.insert(
        "JSON".to_string(),
        Value::Map(gc.allocate(MapState::new(json_obj)).unwrap()),
    );
    modules.insert(
        "std:json".to_string(),
        Value::Map(gc.allocate(MapState::new(json_module)).unwrap()),
    );

    // std:time
    let mut time_module = IndexMap::new();
    time_module.insert("now".to_string(), Value::NativeFunction(time::time_now));
    time_module.insert(
        "format".to_string(),
        Value::NativeFunction(time::time_format),
    );
    time_module.insert("parse".to_string(), Value::NativeFunction(time::time_parse));
    time_module.insert("parts".to_string(), Value::NativeFunction(time::time_parts));
    time_module.insert(
        "setTimeout".to_string(),
        Value::NativeFunction(time::set_timeout),
    );
    time_module.insert(
        "now_ns".to_string(),
        Value::NativeFunction(time::time_now_ns),
    );
    time_module.insert("sleep".to_string(), Value::NativeFunction(time::time_sleep));

    let mut datetime_obj = IndexMap::new();
    datetime_obj.insert(
        "now".to_string(),
        Value::NativeFunction(time::time_datetime_now),
    );
    datetime_obj.insert("sleep".to_string(), Value::NativeFunction(time::time_sleep));
    time_module.insert(
        "DateTime".to_string(),
        Value::Map(gc.allocate(MapState::new(datetime_obj)).unwrap()),
    );

    modules.insert(
        "std:time".to_string(),
        Value::Map(gc.allocate(MapState::new(time_module)).unwrap()),
    );

    // std:math
    let mut math_module = IndexMap::new();
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
    math_module.insert("atan2".to_string(), Value::NativeFunction(math::math_atan2));
    math_module.insert("log10".to_string(), Value::NativeFunction(math::math_log10));
    math_module.insert("trunc".to_string(), Value::NativeFunction(math::math_trunc));
    math_module.insert("sign".to_string(), Value::NativeFunction(math::math_sign));
    math_module.insert("PI".to_string(), Value::Float(std::f64::consts::PI));
    math_module.insert("E".to_string(), Value::Float(std::f64::consts::E));
    math_module.insert("LN2".to_string(), Value::Float(std::f64::consts::LN_2));
    math_module.insert("LN10".to_string(), Value::Float(std::f64::consts::LN_10));
    math_module.insert("LOG2E".to_string(), Value::Float(std::f64::consts::LOG2_E));
    math_module.insert(
        "LOG10E".to_string(),
        Value::Float(std::f64::consts::LOG10_E),
    );
    math_module.insert(
        "SQRT1_2".to_string(),
        Value::Float(std::f64::consts::FRAC_1_SQRT_2),
    );
    math_module.insert("SQRT2".to_string(), Value::Float(std::f64::consts::SQRT_2));

    modules.insert(
        "std:math".to_string(),
        Value::Map(gc.allocate(MapState::new(math_module)).unwrap()),
    );

    // std:regex
    let mut regex_module = IndexMap::new();
    regex_module.insert(
        "Regex".to_string(),
        Value::NativeFunction(regex::regex_constructor),
    );
    modules.insert(
        "std:regex".to_string(),
        Value::Map(gc.allocate(MapState::new(regex_module)).unwrap()),
    );

    // std:reflect
    let mut reflect_module = IndexMap::new();
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
    modules.insert(
        "std:reflect".to_string(),
        Value::Map(gc.allocate(MapState::new(reflect_module)).unwrap()),
    );

    // std:test
    let mut assert_obj = IndexMap::new();
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

    let mut test_module = IndexMap::new();
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

    // std:gc
    let mut gc_module = IndexMap::new();
    gc_module.insert("collect".to_string(), Value::NativeFunction(gc::gc_collect));
    gc_module.insert(
        "heapSize".to_string(),
        Value::NativeFunction(gc::gc_heap_size),
    );
    gc_module.insert(
        "isEnabled".to_string(),
        Value::NativeFunction(gc::gc_is_enabled),
    );
    modules.insert(
        "std:gc".to_string(),
        Value::Map(gc.allocate(MapState::new(gc_module)).unwrap()),
    );

    // std:security
    let mut security_module = IndexMap::new();
    security_module.insert(
        "Sandbox".to_string(),
        Value::NativeFunction(security::sandbox_new),
    );
    modules.insert(
        "std:security".to_string(),
        Value::Map(gc.allocate(MapState::new(security_module)).unwrap()),
    );

    // std:net
    let mut net_module = IndexMap::new();
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

    // std:os
    let mut os_module = IndexMap::new();
    os_module.insert(
        "platform".to_string(),
        Value::NativeFunction(os::os_platform),
    );
    os_module.insert("arch".to_string(), Value::NativeFunction(os::os_arch));
    os_module.insert("homedir".to_string(), Value::NativeFunction(os::os_homedir));
    os_module.insert("tmpdir".to_string(), Value::NativeFunction(os::os_tmpdir));
    os_module.insert(
        "hostname".to_string(),
        Value::NativeFunction(os::os_hostname),
    );
    os_module.insert("uptime".to_string(), Value::NativeFunction(os::os_uptime));
    modules.insert(
        "std:os".to_string(),
        Value::Map(gc.allocate(MapState::new(os_module)).unwrap()),
    );

    // std:concurrency
    let mut concurrency_module = IndexMap::new();
    concurrency_module.insert(
        "Mutex".to_string(),
        Value::NativeFunction(concurrency::native_mutex_new),
    );
    concurrency_module.insert(
        "Channel".to_string(),
        Value::NativeFunction(concurrency::native_channel_new),
    );

    let mut mutex_proto = IndexMap::new();
    mutex_proto.insert(
        "lock".to_string(),
        Value::NativeFunction(concurrency::native_mutex_lock),
    );
    mutex_proto.insert(
        "unlock".to_string(),
        Value::NativeFunction(concurrency::native_mutex_unlock),
    );
    concurrency_module.insert(
        "MutexProto".to_string(),
        Value::Map(gc.allocate(MapState::new(mutex_proto)).unwrap()),
    );

    let mut chan_proto = IndexMap::new();
    chan_proto.insert(
        "send".to_string(),
        Value::NativeFunction(concurrency::native_channel_send),
    );
    chan_proto.insert(
        "recv".to_string(),
        Value::NativeFunction(concurrency::native_channel_recv),
    );
    chan_proto.insert(
        "close".to_string(),
        Value::NativeFunction(concurrency::native_channel_close),
    );
    chan_proto.insert(
        "iterator".to_string(),
        Value::NativeFunction(concurrency::native_channel_iterator),
    );
    chan_proto.insert(
        "next".to_string(),
        Value::NativeFunction(concurrency::native_channel_next),
    );
    concurrency_module.insert(
        "ChannelProto".to_string(),
        Value::Map(gc.allocate(MapState::new(chan_proto)).unwrap()),
    );

    modules.insert(
        "std:concurrency".to_string(),
        Value::Map(gc.allocate(MapState::new(concurrency_module)).unwrap()),
    );

    // std:dotenv
    let mut dotenv_module = IndexMap::new();
    dotenv_module.insert(
        "config".to_string(),
        Value::NativeFunction(dotenv::dotenv_config),
    );
    modules.insert(
        "std:dotenv".to_string(),
        Value::Map(gc.allocate(MapState::new(dotenv_module)).unwrap()),
    );

    // std:log
    let mut log_module = IndexMap::new();
    log_module.insert("trace".to_string(), Value::NativeFunction(log::log_trace));
    log_module.insert("debug".to_string(), Value::NativeFunction(log::log_debug));
    log_module.insert("info".to_string(), Value::NativeFunction(log::log_info));
    log_module.insert("warn".to_string(), Value::NativeFunction(log::log_warn));
    log_module.insert("error".to_string(), Value::NativeFunction(log::log_error));
    log_module.insert("fatal".to_string(), Value::NativeFunction(log::log_fatal));
    log_module.insert(
        "setLevel".to_string(),
        Value::NativeFunction(log::log_set_level),
    );
    log_module.insert(
        "getLevel".to_string(),
        Value::NativeFunction(log::log_get_level),
    );
    log_module.insert("create".to_string(), Value::NativeFunction(log::log_create));
    modules.insert(
        "std:log".to_string(),
        Value::Map(gc.allocate(MapState::new(log_module)).unwrap()),
    );

    // std:crypto
    let mut crypto_module = IndexMap::new();
    crypto_module.insert("hash".to_string(), Value::NativeFunction(crypto::hash));
    crypto_module.insert("hmac".to_string(), Value::NativeFunction(crypto::hmac));
    crypto_module.insert(
        "encrypt".to_string(),
        Value::NativeFunction(crypto::encrypt),
    );
    crypto_module.insert(
        "decrypt".to_string(),
        Value::NativeFunction(crypto::decrypt),
    );
    crypto_module.insert(
        "jwtSign".to_string(),
        Value::NativeFunction(crypto::jwt_sign),
    );
    crypto_module.insert(
        "jwtVerify".to_string(),
        Value::NativeFunction(crypto::jwt_verify),
    );
    modules.insert(
        "std:crypto".to_string(),
        Value::Map(gc.allocate(MapState::new(crypto_module)).unwrap()),
    );

    // std:random
    let mut random_module = IndexMap::new();
    random_module.insert("random".to_string(), Value::NativeFunction(random::random));
    random_module.insert(
        "randint".to_string(),
        Value::NativeFunction(random::randint),
    );
    random_module.insert("choice".to_string(), Value::NativeFunction(random::choice));
    random_module.insert(
        "shuffle".to_string(),
        Value::NativeFunction(random::shuffle),
    );
    modules.insert(
        "std:random".to_string(),
        Value::Map(gc.allocate(MapState::new(random_module)).unwrap()),
    );

    // std:http
    let mut http_module = IndexMap::new();
    http_module.insert("get".to_string(), Value::NativeFunction(http::http_get));
    http_module.insert("post".to_string(), Value::NativeFunction(http::http_post));
    http_module.insert("put".to_string(), Value::NativeFunction(http::http_put));
    http_module.insert(
        "delete".to_string(),
        Value::NativeFunction(http::http_delete),
    );
    http_module.insert("patch".to_string(), Value::NativeFunction(http::http_patch));
    http_module.insert("head".to_string(), Value::NativeFunction(http::http_head));
    http_module.insert(
        "options".to_string(),
        Value::NativeFunction(http::http_options),
    );
    http_module.insert(
        "request".to_string(),
        Value::NativeFunction(http::http_request),
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

    // std:db
    let mut db_module = IndexMap::new();
    let mut db_obj = IndexMap::new();
    db_obj.insert("connect".to_string(), Value::NativeFunction(db::db_connect));
    db_module.insert(
        "Database".to_string(),
        Value::Map(gc.allocate(MapState::new(db_obj)).unwrap()),
    );
    modules.insert(
        "std:db".to_string(),
        Value::Map(gc.allocate(MapState::new(db_module)).unwrap()),
    );

    // Register all in VM
    for (name, value) in modules {
        vm.register_module(name, value);
    }
}

use indexmap::{IndexMap, IndexSet};
use pleno_value::{
    ArrayState, IteratorState, MapState, RuntimeError, SetState, TupleState, VMProvider, Value,
    ValueKey,
};
// use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::rc::Rc;

fn native_to_string(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let val = match args.first() {
        Some(v) => v,
        None => return Ok(Value::String(Rc::new("".to_string()))),
    };
    Ok(Value::String(Rc::new(val.to_string())))
}

fn native_char_is_digit(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.isDigit expects a Char")),
    };
    Ok(Value::Bool(c.is_numeric()))
}

fn native_char_is_alpha(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.isAlpha expects a Char")),
    };
    Ok(Value::Bool(c.is_alphabetic()))
}

fn native_char_is_whitespace(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.isWhitespace expects a Char")),
    };
    Ok(Value::Bool(c.is_whitespace()))
}

fn native_char_to_upper(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.toUpperCase expects a Char")),
    };
    Ok(Value::Char(c.to_uppercase().next().unwrap_or(c)))
}

fn native_char_to_lower(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.toLowerCase expects a Char")),
    };
    Ok(Value::Char(c.to_lowercase().next().unwrap_or(c)))
}

fn native_char_to_int(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let c = match args.first() {
        Some(Value::Char(c)) => *c,
        _ => return Err(RuntimeError::new("Char.toInt expects a Char")),
    };
    Ok(Value::Int(c as i64))
}

fn native_int_parse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match args.get(1).or_else(|| args.first()) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Int.parse expects string")),
    };
    match s.parse::<i64>() {
        Ok(i) => Ok(Value::Int(i)),
        Err(_) => Err(RuntimeError::new(format!("Invalid integer: {}", s))),
    }
}

fn native_float_parse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match args.get(1).or_else(|| args.first()) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Float.parse expects string")),
    };
    match s.parse::<f64>() {
        Ok(f) => Ok(Value::Float(f)),
        Err(_) => Err(RuntimeError::new(format!("Invalid float: {}", s))),
    }
}

pub fn register_builtins(vm: &mut dyn VMProvider) {
    // Globals
    vm.register_global("print".to_string(), Value::NativeFunction(native_print));
    vm.register_global("input".to_string(), Value::NativeFunction(native_input));
    vm.register_global("assert".to_string(), Value::NativeFunction(native_assert));
    vm.register_global("id".to_string(), Value::NativeFunction(native_id));
    vm.register_global("typeof".to_string(), Value::NativeFunction(native_typeof));
    vm.register_global("map".to_string(), Value::NativeFunction(native_global_map));
    vm.register_global(
        "filter".to_string(),
        Value::NativeFunction(native_global_filter),
    );
    vm.register_global(
        "reduce".to_string(),
        Value::NativeFunction(native_global_reduce),
    );

    vm.register_global(
        "Error".to_string(),
        Value::NativeFunction(native_error_constructor),
    );

    // Result type constructors
    vm.register_global("Ok".to_string(), Value::NativeFunction(native_result_ok));
    vm.register_global("Err".to_string(), Value::NativeFunction(native_result_err));

    // Option type constructors
    vm.register_global(
        "Some".to_string(),
        Value::NativeFunction(native_option_some),
    );

    let mut none_map = IndexMap::new();
    none_map.insert(
        "variant".to_string(),
        Value::String(Rc::new("None".to_string())),
    );
    let none_val = Value::Map(vm.get_gc().allocate(MapState::new(none_map)).unwrap());
    vm.register_global("None".to_string(), none_val.clone());

    // Global Math, JSON, OS removed. Import from std modules.

    // Promise methods
    let mut promise_obj_map = IndexMap::new();
    promise_obj_map.insert(
        "__call__".to_string(),
        Value::NativeFunction(crate::promise::promise_constructor),
    );
    promise_obj_map.insert(
        "all".to_string(),
        Value::NativeFunction(crate::promise::promise_all),
    );
    promise_obj_map.insert(
        "race".to_string(),
        Value::NativeFunction(crate::promise::promise_race),
    );
    promise_obj_map.insert(
        "resolve".to_string(),
        Value::NativeFunction(crate::promise::promise_resolve_static),
    );
    promise_obj_map.insert(
        "reject".to_string(),
        Value::NativeFunction(crate::promise::promise_reject_static),
    );
    let promise_val = Value::Map(
        vm.get_gc()
            .allocate(MapState::new(promise_obj_map))
            .unwrap(),
    );
    vm.register_global("Promise".to_string(), promise_val.clone());

    // Types
    let int_map = {
        let mut m = IndexMap::new();
        m.insert("parse".to_string(), Value::NativeFunction(native_int_parse));
        m.insert(
            "__native_type".to_string(),
            Value::String(Rc::new("Int".to_string())),
        );
        m.insert(
            "__implements".to_string(),
            Value::Array(
                vm.get_gc()
                    .allocate(ArrayState::new(vec![Value::String(Rc::new(
                        "Comparable".to_string(),
                    ))]))
                    .unwrap(),
            ),
        );
        vm.get_gc().allocate(MapState::new(m)).unwrap()
    };
    vm.register_global("Int".to_string(), Value::Map(int_map));

    let float_map = {
        let mut m = IndexMap::new();
        m.insert(
            "parse".to_string(),
            Value::NativeFunction(native_float_parse),
        );
        m.insert(
            "__native_type".to_string(),
            Value::String(Rc::new("Float".to_string())),
        );
        vm.get_gc().allocate(MapState::new(m)).unwrap()
    };
    vm.register_global("Float".to_string(), Value::Map(float_map));

    for ty in &[
        "Bool", "String", "Array", "Map", "Object", "Null", "Any", "Char",
    ] {
        vm.register_global(ty.to_string(), Value::String(Rc::new(ty.to_string())));
    }
    vm.register_global(
        "Set".to_string(),
        Value::NativeFunction(native_set_constructor),
    );

    vm.register_global(
        "spawn".to_string(),
        Value::NativeFunction(native_async_spawn),
    );
    vm.register_global(
        "stopIteration".to_string(),
        Value::NativeFunction(native_stop_iteration),
    );

    // These are globals; the modules let code import them explicitly.
    let mut core_module = IndexMap::new();
    core_module.insert("Ok".to_string(), Value::NativeFunction(native_result_ok));
    core_module.insert("Err".to_string(), Value::NativeFunction(native_result_err));
    core_module.insert(
        "Some".to_string(),
        Value::NativeFunction(native_option_some),
    );
    core_module.insert("None".to_string(), none_val);
    let core_module = Value::Map(vm.get_gc().allocate(MapState::new(core_module)).unwrap());
    vm.register_module("std:core".to_string(), core_module);

    let mut promise_module = IndexMap::new();
    promise_module.insert("Promise".to_string(), promise_val);
    promise_module.insert(
        "spawn".to_string(),
        Value::NativeFunction(native_async_spawn),
    );
    let promise_module = Value::Map(vm.get_gc().allocate(MapState::new(promise_module)).unwrap());
    vm.register_module("std:promise".to_string(), promise_module);
}

fn native_stop_iteration(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Missing)
}

fn native_iterator_next(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected iterator"));
    }
    match &args[0] {
        Value::Iterator(iter) => {
            let mut state = iter.borrow_mut();
            match &mut *state {
                IteratorState::Array {
                    array,
                    index,
                    is_entries,
                } => {
                    let arr = array.borrow();
                    if *index < arr.elements.len() {
                        let val = arr.elements[*index].clone();
                        let result = if *is_entries {
                            let key_val = Value::Int(*index as i64);
                            Value::Tuple(
                                vm.get_gc()
                                    .allocate(TupleState::new(vec![key_val, val], None))?,
                            )
                        } else {
                            val
                        };
                        *index += 1;
                        Ok(result)
                    } else {
                        Ok(Value::Missing)
                    }
                }
                IteratorState::Tuple { tuple, index } => {
                    let t = tuple.borrow();
                    if *index < t.elements.len() {
                        let val = t.elements[*index].clone();
                        *index += 1;
                        Ok(val)
                    } else {
                        Ok(Value::Missing)
                    }
                }
                IteratorState::Range { range, current } => {
                    let end = range.end.unwrap_or(i64::MAX);
                    if *current < end || (range.inclusive && *current == end) {
                        let val = Value::Int(*current);
                        *current += 1;
                        Ok(val)
                    } else {
                        Ok(Value::Missing)
                    }
                }
                IteratorState::Object { .. } => Err(RuntimeError::new(
                    "nested object iterator not supported in native next()",
                )),
                IteratorState::Map {
                    map,
                    index,
                    is_entries,
                } => {
                    let map_borrow = map.borrow();
                    if *index < map_borrow.entries.len() {
                        let (key_str, value_val) = map_borrow.entries.get_index(*index).unwrap();
                        let val = if *is_entries {
                            let key_val = Value::String(Rc::new(key_str.clone()));
                            Value::Tuple(vm.get_gc().allocate(TupleState::new(
                                vec![key_val, value_val.clone()],
                                None,
                            ))?)
                        } else {
                            Value::String(Rc::new(key_str.clone()))
                        };
                        *index += 1;
                        Ok(val)
                    } else {
                        Ok(Value::Missing)
                    }
                }
                IteratorState::Set { set, index } => {
                    let set_borrow = set.borrow();
                    if *index < set_borrow.elements.len() {
                        let key = set_borrow.elements.get_index(*index).unwrap();
                        let val = key.to_value();
                        *index += 1;
                        Ok(val)
                    } else {
                        Ok(Value::Missing)
                    }
                }
            }
        }
        _ => Err(RuntimeError::new("next() called on non-iterator")),
    }
}

fn native_array_iterator(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected array"));
    }
    match &args[0] {
        Value::Array(array) => {
            let state = IteratorState::Array {
                array: array.clone(),
                index: 0,
                is_entries: false,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("iterator() called on non-array")),
    }
}

fn native_tuple_iterator(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected tuple"));
    }
    match &args[0] {
        Value::Tuple(tuple) => {
            let state = IteratorState::Tuple {
                tuple: tuple.clone(),
                index: 0,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("iterator() called on non-tuple")),
    }
}

fn native_range_iterator(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected range"));
    }
    match &args[0] {
        Value::Range(range) => {
            let state = IteratorState::Range {
                range: range.clone(),
                current: range.start.unwrap_or(0),
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("iterator() called on non-range")),
    }
}

fn native_array_entries(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected array"));
    }
    match &args[0] {
        Value::Array(array) => {
            let state = IteratorState::Array {
                array: array.clone(),
                index: 0,
                is_entries: true,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("entries() called on non-array")),
    }
}

fn native_map_iterator(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected map"));
    }
    match &args[0] {
        Value::Map(map) => {
            let state = IteratorState::Map {
                map: map.clone(),
                index: 0,
                is_entries: false,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("iterator() called on non-map")),
    }
}

fn native_map_entries(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected map"));
    }
    match &args[0] {
        Value::Map(map) => {
            let state = IteratorState::Map {
                map: map.clone(),
                index: 0,
                is_entries: true,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("entries() called on non-map")),
    }
}

fn native_set_iterator(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("expected set"));
    }
    match &args[0] {
        Value::Set(set) => {
            let state = IteratorState::Set {
                set: set.clone(),
                index: 0,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)?))
        }
        _ => Err(RuntimeError::new("iterator() called on non-set")),
    }
}

fn native_file_write(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let file = match args.first() {
        Some(Value::File(f)) => f,
        _ => return Err(RuntimeError::new("Expected file")),
    };
    let content = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Expected string content")),
    };
    let file_state = file.borrow();
    let mut inner = file_state.inner.borrow_mut();
    if let Some(writer) = inner.as_mut() {
        let writer: &mut dyn Write = writer;
        write!(writer, "{}", content)
            .map_err(|e| RuntimeError::new(format!("Write failed: {}", e)))?;
        Ok(Value::Null)
    } else {
        Err(RuntimeError::new("File is closed"))
    }
}

fn native_file_flush(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let file = match args.first() {
        Some(Value::File(f)) => f,
        _ => return Err(RuntimeError::new("Expected file")),
    };
    let file_state = file.borrow();
    let mut inner = file_state.inner.borrow_mut();
    if let Some(writer) = inner.as_mut() {
        writer
            .flush()
            .map_err(|e| RuntimeError::new(format!("Flush failed: {}", e)))?;
        Ok(Value::Null)
    } else {
        Err(RuntimeError::new("File is closed"))
    }
}

fn native_file_close(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let file = match args.first() {
        Some(Value::File(f)) => f,
        _ => return Err(RuntimeError::new("Expected file")),
    };
    let file_state = file.borrow();
    let mut inner = file_state.inner.borrow_mut();
    *inner = None;
    Ok(Value::Null)
}

/// The contents of a file opened with `File.open(path, "r")`.
fn read_open_file(args: &[Value]) -> Result<String, RuntimeError> {
    let file = match args.first() {
        Some(Value::File(f)) => f,
        _ => return Err(RuntimeError::new("Expected file")),
    };
    let path = file
        .borrow()
        .read_path
        .clone()
        .ok_or_else(|| RuntimeError::new("file is not open for reading"))?;
    std::fs::read_to_string(&path).map_err(|e| RuntimeError::new(e.to_string()))
}

fn native_file_read(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::String(Rc::new(read_open_file(args)?)))
}

fn native_file_read_lines(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let lines = read_open_file(args)?
        .lines()
        .map(|line| Value::String(Rc::new(line.to_string())))
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(lines))?))
}

pub fn resolve_builtin_method(ty: &str, name: &str) -> Option<pleno_value::NativeFunction> {
    match (ty, name) {
        ("String", "toUpperCase") | ("String", "upper") => Some(native_string_upper),
        ("String", "toLowerCase") | ("String", "lower") => Some(native_string_lower),
        ("String", "trim") => Some(native_string_trim),
        ("String", "split") => Some(native_string_split),
        ("String", "contains") => Some(native_string_contains),
        ("String", "startsWith") => Some(native_string_starts_with),
        ("String", "endsWith") => Some(native_string_ends_with),
        ("String", "replace") => Some(native_string_replace),
        ("String", "substring") => Some(native_string_substring),
        ("String", "charAt") => Some(native_string_char_at),
        ("String", "indexOf") => Some(native_string_index_of),
        ("String", "repeat") => Some(native_string_repeat),
        ("String", "toInt") => Some(native_string_to_int),
        ("String", "toFloat") => Some(native_string_to_float),
        ("Array", "push") => Some(native_array_push),
        ("Array", "pop") => Some(native_array_pop),
        ("Array", "shift") => Some(native_array_shift),
        ("Array", "unshift") => Some(native_array_unshift),
        ("Array", "insert") => Some(native_array_insert),
        ("Array", "remove") => Some(native_array_remove_at),
        ("Array", "clear") => Some(native_array_clear),
        ("Array", "slice") => Some(native_array_slice),
        // `clone()` is `slice()` with no bounds: a shallow copy.
        ("Array", "clone") => Some(native_array_slice),
        ("Array", "join") => Some(native_array_join),
        ("Array", "find") => Some(native_array_find),
        ("Array", "map") => Some(native_array_map),
        ("Array", "filter") => Some(native_array_filter),
        ("Array", "reduce") => Some(native_array_reduce),
        ("Array", "forEach") => Some(native_array_for_each),
        ("Array", "reverse") => Some(native_array_reverse),
        ("Array", "flatMap") => Some(native_array_flat_map),
        ("Array", "every") => Some(native_array_every),
        ("Array", "some") => Some(native_array_some),
        ("Array", "sort") => Some(native_array_sort),
        ("Array", "contains") => Some(native_array_contains),
        ("Array", "indexOf") => Some(native_array_index_of),
        ("Array", "isEmpty") => Some(native_array_is_empty),
        ("Array", "with") => Some(native_array_with),
        ("Map", "put") => Some(native_map_put),
        ("Map", "get") => Some(native_map_get),
        ("Map", "with") => Some(native_map_with),
        ("Map", "remove") => Some(native_map_remove),
        ("Map", "clear") => Some(native_map_clear),
        ("Map", "containsKey") => Some(native_map_contains_key),
        ("Map", "keys") => Some(native_map_keys),
        ("Map", "values") => Some(native_map_values),
        ("Map", "size") => Some(native_map_size),
        ("Map", "isEmpty") => Some(native_map_is_empty),
        ("Set", "clear") => Some(native_set_clear),
        ("Map", "map") => Some(native_map_map),
        ("Map", "filter") => Some(native_map_filter),
        ("Map", "forEach") => Some(native_map_for_each),
        ("Set", "add") => Some(native_set_add),
        ("Set", "remove") => Some(native_set_remove),
        ("Set", "contains") => Some(native_set_contains),
        ("Set", "size") => Some(native_set_size),
        ("Set", "isEmpty") => Some(native_set_is_empty),
        ("Set", "map") => Some(native_set_map),
        ("Set", "filter") => Some(native_set_filter),
        ("Set", "forEach") => Some(native_set_for_each),
        ("Set", "with") => Some(native_set_with),
        ("Set", "toArray") => Some(native_set_to_array),
        ("Set", "union") => Some(native_set_union),
        ("Set", "intersection") => Some(native_set_intersection),
        ("Set", "difference") => Some(native_set_difference),
        ("Result", "unwrap") => Some(native_result_unwrap),
        ("Result", "unwrapOr") => Some(native_result_unwrap_or),
        ("Result", "expect") => Some(native_result_expect),
        ("Result", "isOk") => Some(native_result_is_ok),
        ("Result", "isErr") => Some(native_result_is_err),
        ("Option", "unwrap") => Some(native_option_unwrap),
        ("Option", "unwrapOr") => Some(native_option_unwrap_or),
        ("Option", "expect") => Some(native_option_expect),
        ("Option", "isSome") => Some(native_option_is_some),
        ("Option", "isNone") => Some(native_option_is_none),
        ("File", "write") => Some(native_file_write),
        ("File", "flush") => Some(native_file_flush),
        ("File", "close") => Some(native_file_close),
        ("File", "read") => Some(native_file_read),
        ("File", "readLines") => Some(native_file_read_lines),
        ("Promise", "then") => Some(crate::promise::promise_then),
        ("Promise", "catch") => Some(crate::promise::promise_catch),
        ("Promise", "finally") => Some(crate::promise::promise_finally),
        ("Mutex", "lock") => Some(crate::concurrency::native_mutex_lock),
        ("Mutex", "unlock") => Some(crate::concurrency::native_mutex_unlock),
        ("Channel", "send") => Some(crate::concurrency::native_channel_send),
        ("Channel", "recv") => Some(crate::concurrency::native_channel_recv),
        ("Channel", "close") => Some(crate::concurrency::native_channel_close),
        ("Channel", "iterator") => Some(crate::concurrency::native_channel_iterator),
        ("Channel", "next") => Some(crate::concurrency::native_channel_next),
        ("Regex", "test") => Some(crate::regex::regex_test),
        ("Regex", "match") => Some(crate::regex::regex_match),
        ("Regex", "findAll") => Some(crate::regex::regex_find_all),
        ("Regex", "replace") => Some(crate::regex::regex_replace),
        ("Array", "iterator") => Some(native_array_iterator),
        ("Array", "entries") => Some(native_array_entries),
        ("Map", "iterator") => Some(native_map_iterator),
        ("Map", "entries") => Some(native_map_entries),
        ("Set", "iterator") => Some(native_set_iterator),
        ("Tuple", "iterator") => Some(native_tuple_iterator),
        ("Range", "iterator") => Some(native_range_iterator),
        ("Range", "contains") => Some(native_range_contains),
        ("Range", "toArray") => Some(native_range_to_array),
        ("Iterator", "next") => Some(native_iterator_next),
        ("Char", "isDigit") => Some(native_char_is_digit),
        ("Char", "isAlpha") => Some(native_char_is_alpha),
        ("Char", "isWhitespace") => Some(native_char_is_whitespace),
        ("Char", "toUpperCase") => Some(native_char_to_upper),
        ("Char", "toLowerCase") => Some(native_char_to_lower),
        ("Char", "toInt") => Some(native_char_to_int),
        (_, "toString") => Some(native_to_string),
        _ => None,
    }
}

// Helper
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Missing => false,
        Value::Bool(v) => *v,
        _ => true,
    }
}

// Global functions
/// Helper function for print implementation with configurable default end
fn print_impl(
    vm: &mut dyn VMProvider,
    args: &[Value],
    default_end: &str,
) -> Result<Value, RuntimeError> {
    vm.check_purity()?;
    if !vm.is_io_allowed() {
        return Err(RuntimeError::new("SecurityError: IO access denied"));
    }
    let (sep, end, file_arg, flush_arg) = {
        let named_args = vm.get_named_args();
        let sep = named_args
            .get("sep")
            .and_then(|v| v.as_str())
            .unwrap_or(" ")
            .to_string();
        let end = named_args
            .get("end")
            .and_then(|v| v.as_str())
            .unwrap_or(default_end)
            .to_string();
        let file_arg = named_args.get("file").cloned();
        let flush_arg = named_args.get("flush").cloned();
        (sep, end, file_arg, flush_arg)
    };

    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push_str(&sep);
        }
        out.push_str(&format!("{}", value));
    }
    out.push_str(&end);

    if let Some(ref f_val) = file_arg {
        match f_val {
            Value::File(f) => {
                let file_state = f.borrow();
                let mut inner = file_state.inner.borrow_mut();
                if let Some(writer) = inner.as_mut() {
                    let writer: &mut dyn Write = writer;
                    write!(writer, "{}", out).map_err(|e| {
                        RuntimeError::new(format!("print failed to write to file: {}", e))
                    })?;
                } else {
                    return Err(RuntimeError::new("print: file is closed"));
                }
            }
            _ => return Err(RuntimeError::new("print: file argument must be a File")),
        }
    } else {
        vm.print(&out);
    }

    let should_flush = if let Some(fl) = flush_arg {
        fl.as_bool().unwrap_or(false)
    } else {
        file_arg.is_none()
    };

    if should_flush {
        if let Some(ref f_val) = file_arg {
            if let Value::File(f) = f_val {
                let file_state = f.borrow();
                let mut inner = file_state.inner.borrow_mut();
                if let Some(writer) = inner.as_mut() {
                    writer
                        .flush()
                        .map_err(|e| RuntimeError::new(format!("flush failed: {}", e)))?;
                }
            }
        } else {
            let _ = std::io::stdout().flush();
        }
    }
    Ok(Value::Null)
}

/// print: Outputs text WITH a trailing newline by default (Python-style)
fn native_print(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    print_impl(vm, args, "\n")
}

/// assert: Throws an error if condition is false
fn native_assert(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let condition = args.first().unwrap_or(&Value::Bool(false));
    let message = args
        .get(1)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "Assertion failed".to_string());

    if !is_truthy(condition) {
        return Err(RuntimeError::new(format!("AssertionError: {}", message)));
    }
    Ok(Value::Null)
}

/// input: Reads a line from stdin, optionally displaying a prompt
fn native_input(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    vm.check_purity()?;
    if !vm.is_io_allowed() {
        return Err(RuntimeError::new("SecurityError: IO access denied"));
    }

    // Print optional prompt (without newline)
    if let Some(prompt) = args.first() {
        vm.print(&prompt.to_string());
        std::io::stdout().flush().ok();
    }

    // Read line from stdin
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| RuntimeError::new(format!("input error: {}", e)))?;

    // Trim trailing newline/carriage return
    let result = line.trim_end_matches(&['\r', '\n'][..]).to_string();
    Ok(Value::String(Rc::new(result)))
}

fn native_id(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.first().cloned().unwrap_or(Value::Null))
}

fn native_typeof(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    Ok(Value::String(Rc::new(val.type_name().to_string())))
}

fn native_async_spawn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("spawn() requires a function"));
    }
    let callee = args[0].clone();
    let call_args = args[1..].to_vec();
    vm.spawn(callee, call_args)
}

fn native_global_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("map expects collection"));
    }
    match &args[0] {
        Value::Array(_) => native_array_map(vm, args),
        Value::Map(_) => native_map_map(vm, args),
        Value::Set(_) => native_set_map(vm, args),
        Value::Range(_) => {
            let args = range_args_as_array(vm, args)?;
            native_array_map(vm, &args)
        }
        _ => Err(RuntimeError::new(
            "map expects a collection (Array, Map, Set or Range)",
        )),
    }
}

fn native_global_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("filter expects collection"));
    }
    match &args[0] {
        Value::Array(_) => native_array_filter(vm, args),
        Value::Map(_) => native_map_filter(vm, args),
        Value::Set(_) => native_set_filter(vm, args),
        Value::Range(_) => {
            let args = range_args_as_array(vm, args)?;
            native_array_filter(vm, &args)
        }
        _ => Err(RuntimeError::new(
            "filter expects a collection (Array, Map, Set or Range)",
        )),
    }
}

fn native_global_reduce(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("reduce expects collection"));
    }
    match &args[0] {
        Value::Array(_) => native_array_reduce(vm, args),
        Value::Range(_) => {
            let args = range_args_as_array(vm, args)?;
            native_array_reduce(vm, &args)
        }
        _ => Err(RuntimeError::new("reduce expects an Array or Range")),
    }
}

/// `args` with the range in front replaced by an array of its values, so the
/// array versions of map, filter and reduce handle ranges.
fn range_args_as_array(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Vec<Value>, RuntimeError> {
    let array = native_range_to_array(vm, &args[..1])?;
    Ok(std::iter::once(array)
        .chain(args[1..].iter().cloned())
        .collect())
}

// Result/Option constructors
fn native_result_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = IndexMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Ok".to_string())),
    );
    res.insert(
        "value".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))?))
}

fn native_result_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = IndexMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Err".to_string())),
    );
    res.insert(
        "error".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))?))
}

fn native_option_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = IndexMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Some".to_string())),
    );
    res.insert(
        "value".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))?))
}

fn native_error_constructor(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = IndexMap::new();
    res.insert(
        "message".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    // Also add variant for potential pattern matching
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Error".to_string())),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))?))
}

// ... more primitive methods ...
fn native_string_upper(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_uppercase())))
}
fn native_string_lower(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.to_lowercase())))
}
fn native_string_trim(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    Ok(Value::String(Rc::new(s.trim().to_string())))
}
fn native_string_split(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let sep = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => ",",
    };
    let parts: Vec<Value> = s
        .split(sep)
        .map(|p| Value::String(Rc::new(p.to_string())))
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(parts))?))
}
fn native_string_substring(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => 0,
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        _ => s.len(),
    };
    let start = start.min(s.len());
    let end = end.min(s.len()).max(start);
    Ok(Value::String(Rc::new(s[start..end].to_string())))
}
fn native_string_char_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let idx = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => 0,
    };
    if idx >= s.len() {
        Ok(Value::Null)
    } else {
        Ok(Value::String(Rc::new(s[idx..idx + 1].to_string())))
    }
}
fn native_string_index_of(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let search = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Int(-1)),
    };
    Ok(Value::Int(s.find(search).map(|i| i as i64).unwrap_or(-1)))
}
fn native_string_repeat(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let n = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => 0,
    };
    Ok(Value::String(Rc::new(s.repeat(n))))
}

fn native_string_to_int(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str().trim(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    match s.parse::<i64>() {
        Ok(i) => Ok(Value::Int(i)),
        Err(_) => Err(RuntimeError::new(format!("Cannot convert '{}' to Int", s))),
    }
}

fn native_string_to_float(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str().trim(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    match s.parse::<f64>() {
        Ok(f) => Ok(Value::Float(f)),
        Err(_) => Err(RuntimeError::new(format!(
            "Cannot convert '{}' to Float",
            s
        ))),
    }
}

fn native_array_push(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    vm.get_gc().track_allocation(std::mem::size_of::<Value>())?;
    arr.borrow_mut()
        .elements
        .push(args.get(1).cloned().unwrap_or(Value::Null));
    Ok(Value::Null)
}
fn native_array_pop(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    Ok(arr.borrow_mut().elements.pop().unwrap_or(Value::Null))
}
fn native_array_shift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    if arr.borrow().elements.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(arr.borrow_mut().elements.remove(0))
    }
}

fn native_array_unshift(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    arr.borrow_mut()
        .elements
        .insert(0, args.get(1).cloned().unwrap_or(Value::Null));
    Ok(Value::Null)
}
fn native_array_insert(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Err(RuntimeError::new("expected integer index")),
    };
    let value = args.get(2).cloned().unwrap_or(Value::Null);
    let mut arr = arr.borrow_mut();
    if index <= arr.elements.len() {
        arr.elements.insert(index, value);
    }
    Ok(Value::Null)
}
fn native_array_remove_at(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    let idx = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => return Ok(Value::Null),
    };
    let mut b = arr.borrow_mut();
    if idx < b.elements.len() {
        Ok(b.elements.remove(idx))
    } else {
        Ok(Value::Null)
    }
}
fn native_array_clear(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    arr.borrow_mut().elements.clear();
    Ok(Value::Null)
}
fn native_array_slice(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let b = arr.borrow();
    let start = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => 0,
    };
    let end = match args.get(2) {
        Some(Value::Int(i)) => *i as usize,
        _ => b.elements.len(),
    };
    let start = start.min(b.elements.len());
    let end = end.min(b.elements.len()).max(start);
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(
        b.elements[start..end].to_vec(),
    ))?))
}
fn native_array_join(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let sep = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => ",",
    };
    let b = arr.borrow();
    let s: Vec<String> = b.elements.iter().map(|v| v.to_string()).collect();
    Ok(Value::String(Rc::new(s.join(sep))))
}
fn native_array_find(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("find expects callback"))?;
    let items = arr.borrow().elements.clone();
    for item in items {
        if is_truthy(&vm.call(cb.clone(), vec![item.clone()])?) {
            return Ok(item);
        }
    }
    Ok(Value::Null)
}
fn native_array_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("map expects callback"))?;
    let items = arr.borrow().elements.clone();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        results.push(vm.call(cb.clone(), vec![item])?);
    }
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(results))?,
    ))
}
fn native_array_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("filter expects callback"))?;
    let items = arr.borrow().elements.clone();
    let mut results = Vec::new();
    for item in items {
        if is_truthy(&vm.call(cb.clone(), vec![item.clone()])?) {
            results.push(item);
        }
    }
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(results))?,
    ))
}
fn native_array_reduce(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("reduce expects callback"))?;
    let b = arr.borrow();
    if b.elements.is_empty() {
        return args
            .get(2)
            .cloned()
            .ok_or_else(|| RuntimeError::new("reduce empty array without initial"));
    }
    let (mut acc, start) = if let Some(init) = args.get(2) {
        (init.clone(), 0)
    } else {
        (b.elements[0].clone(), 1)
    };
    for item in b.elements.iter().skip(start) {
        acc = vm.call(cb.clone(), vec![acc, item.clone()])?;
    }
    Ok(acc)
}
fn native_array_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("forEach expects callback"))?;
    let items = arr.borrow().elements.clone();
    for item in items {
        vm.call(cb.clone(), vec![item])?;
    }
    Ok(Value::Null)
}

fn native_array_flat_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("flatMap expects callback"))?;
    let items = arr.borrow().elements.clone();
    let mut results = Vec::new();
    for item in items {
        let res = vm.call(cb.clone(), vec![item])?;
        match res {
            Value::Array(inner) => {
                results.extend(inner.borrow().elements.clone());
            }
            _ => results.push(res),
        }
    }
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(results))?,
    ))
}

fn native_array_every(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("every expects callback"))?;
    let items = arr.borrow().elements.clone();
    for item in items {
        let res = vm.call(cb.clone(), vec![item])?;
        if !is_truthy(&res) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn native_array_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("some expects callback"))?;
    let items = arr.borrow().elements.clone();
    for item in items {
        let res = vm.call(cb.clone(), vec![item])?;
        if is_truthy(&res) {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

fn native_array_reverse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    arr.borrow_mut().elements.reverse();
    Ok(args[0].clone())
}
fn native_array_sort(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut items = arr.borrow().elements.clone();
    items.sort_by_key(|v| v.to_string());
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(items))?))
}
fn native_array_contains(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let val = args.get(1).unwrap_or(&Value::Null);
    Ok(Value::Bool(arr.borrow().elements.contains(val)))
}
fn native_array_index_of(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let val = args.get(1).unwrap_or(&Value::Null);
    Ok(Value::Int(
        arr.borrow()
            .elements
            .iter()
            .position(|v| v == val)
            .map(|i| i as i64)
            .unwrap_or(-1),
    ))
}
fn native_array_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    Ok(Value::Bool(arr.borrow().elements.is_empty()))
}

fn native_array_with(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut elements = arr.borrow().elements.clone();

    // `with(value)` returns an immutable copy with `value` appended.
    if args.len() == 2 {
        elements.push(args[1].clone());
        return Ok(Value::Array(vm.get_gc().allocate(ArrayState {
            elements,
            is_immutable: true,
        })?));
    }

    // `with(index, value)` returns an immutable copy with the element replaced.
    let index = match args.get(1) {
        Some(Value::Int(i)) => *i,
        _ => return Err(RuntimeError::new("expected integer index")),
    };
    let value = args.get(2).cloned().unwrap_or(Value::Null);

    let len = elements.len() as i64;

    let normalized_index = if index < 0 { len + index } else { index };

    if normalized_index < 0 || normalized_index >= len {
        return Err(RuntimeError::new(format!("index out of bounds: {}", index)));
    }

    elements[normalized_index as usize] = value;

    Ok(Value::Array(vm.get_gc().allocate(ArrayState {
        elements,
        is_immutable: true,
    })?))
}

fn native_map_put(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    map.borrow().ensure_mutable()?;
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => return Err(RuntimeError::new("map key must be string")),
    };
    map.borrow_mut()
        .entries
        .insert(key, args.get(2).cloned().unwrap_or(Value::Null));
    Ok(Value::Null)
}

fn native_map_with(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => return Err(RuntimeError::new("map key must be string")),
    };
    let mut entries = map.borrow().entries.clone();
    entries.insert(key, args.get(2).cloned().unwrap_or(Value::Null));

    Ok(Value::Map(vm.get_gc().allocate(MapState {
        entries,
        is_immutable: true,
    })?))
}

fn native_map_get(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Null),
    };
    Ok(map
        .borrow()
        .entries
        .get(key)
        .cloned()
        .unwrap_or(Value::Null))
}
fn native_map_remove(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    map.borrow().ensure_mutable()?;
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(
        map.borrow_mut().entries.shift_remove(key).is_some(),
    ))
}
fn native_map_keys(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let keys: Vec<Value> = map
        .borrow()
        .entries
        .keys()
        .map(|k| Value::String(Rc::new(k.clone())))
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(keys))?))
}
fn native_map_values(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let values: Vec<Value> = map.borrow().entries.values().cloned().collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))?))
}
fn native_map_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    Ok(Value::Int(map.borrow().entries.len() as i64))
}

fn native_set_add(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow().ensure_mutable()?;
    let key = ValueKey::try_from_value(args.get(1).unwrap_or(&Value::Null))
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    set.borrow_mut().insert(key);
    Ok(Value::Null)
}
fn native_set_remove(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow().ensure_mutable()?;
    let key = ValueKey::try_from_value(args.get(1).unwrap_or(&Value::Null))
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    Ok(Value::Bool(set.borrow_mut().shift_remove(&key)))
}
fn native_set_contains(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    let key = ValueKey::try_from_value(args.get(1).unwrap_or(&Value::Null))
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    Ok(Value::Bool(set.borrow().contains(&key)))
}
fn native_set_size(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    Ok(Value::Int(set.borrow().len() as i64))
}

fn native_map_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Map(map) => Ok(Value::Bool(map.borrow().entries.is_empty())),
        _ => Err(RuntimeError::new("expected map")),
    }
}

fn native_set_is_empty(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Set(set) => Ok(Value::Bool(set.borrow().is_empty())),
        _ => Err(RuntimeError::new("expected set")),
    }
}

fn expect_range<'a>(
    args: &'a [Value],
    method: &str,
) -> Result<&'a pleno_value::Range, RuntimeError> {
    match args.first() {
        Some(Value::Range(range)) => Ok(range),
        _ => Err(RuntimeError::new(format!("{method}() called on non-range"))),
    }
}

fn native_range_contains(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let range = expect_range(args, "contains")?;
    let value = match args.get(1) {
        Some(Value::Int(value)) => *value,
        _ => return Ok(Value::Bool(false)),
    };
    let above_start = range.start.is_none_or(|start| value >= start);
    let below_end = match range.end {
        None => true,
        Some(end) if range.inclusive => value <= end,
        Some(end) => value < end,
    };
    Ok(Value::Bool(above_start && below_end))
}

fn native_range_to_array(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let range = expect_range(args, "toArray")?;
    let (Some(start), Some(end)) = (range.start, range.end) else {
        return Err(RuntimeError::new(
            "toArray() needs a range with a start and an end",
        ));
    };
    let end = if range.inclusive {
        end.saturating_add(1)
    } else {
        end
    };
    let len = end.saturating_sub(start).max(0) as usize;
    vm.get_gc()
        .track_allocation(len * std::mem::size_of::<Value>())?;
    let elements = (start..end).map(Value::Int).collect();
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(elements))?,
    ))
}

fn native_set_with(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    let mut elements = set.borrow().elements.clone();
    let key = ValueKey::try_from_value(args.get(1).unwrap_or(&Value::Null))
        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
    elements.insert(key);

    Ok(Value::Set(vm.get_gc().allocate(SetState {
        elements,
        is_immutable: true,
    })?))
}

// Result/Option methods
fn native_result_unwrap(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Err(RuntimeError::new("expected Result")),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Ok"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    let error = map.get("error").cloned().unwrap_or(Value::Null);
    Err(RuntimeError::new(format!("called unwrap on Err({error})")))
}
fn native_result_unwrap_or(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(args.get(1).cloned().unwrap_or(Value::Null)),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Ok"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    Ok(args.get(1).cloned().unwrap_or(Value::Null))
}
fn native_result_expect(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Err(RuntimeError::new("expected Result")),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Ok"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    let msg = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "expect failed",
    };
    Err(RuntimeError::new(msg))
}
fn native_result_is_ok(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(
        matches!(map.get("variant"), Some(Value::String(v)) if v.as_str() == "Ok"),
    ))
}
fn native_result_is_err(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(
        matches!(map.get("variant"), Some(Value::String(v)) if v.as_str() == "Err"),
    ))
}

fn native_option_unwrap(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Err(RuntimeError::new("expected Option")),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Some"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    Err(RuntimeError::new("called unwrap on None"))
}
fn native_option_unwrap_or(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(args.get(1).cloned().unwrap_or(Value::Null)),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Some"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    Ok(args.get(1).cloned().unwrap_or(Value::Null))
}
fn native_option_expect(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Err(RuntimeError::new("expected Option")),
    };
    if let Some(Value::String(v)) = map.get("variant")
        && v.as_str() == "Some"
    {
        return Ok(map.get("value").cloned().unwrap_or(Value::Null));
    }
    let msg = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "expect failed",
    };
    Err(RuntimeError::new(msg))
}
fn native_option_is_some(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(
        matches!(map.get("variant"), Some(Value::String(v)) if v.as_str() == "Some"),
    ))
}
fn native_option_is_none(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.borrow(),
        _ => return Ok(Value::Bool(true)),
    };
    Ok(Value::Bool(
        matches!(map.get("variant"), Some(Value::String(v)) if v.as_str() == "None"),
    ))
}

// Stubs removed, using crate::promise::*

fn native_map_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("expected map")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("map expects callback"))?;
    let entries: Vec<(String, Value)> = map
        .borrow()
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut results = IndexMap::new();
    for (key, value) in entries {
        let res = vm.call(cb.clone(), vec![Value::String(Rc::new(key.clone())), value])?;
        results.insert(key, res);
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))?))
}

fn native_map_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("expected map")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("filter expects callback"))?;
    let entries: Vec<(String, Value)> = map
        .borrow()
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut results = IndexMap::new();
    for (key, value) in entries {
        if is_truthy(&vm.call(
            cb.clone(),
            vec![Value::String(Rc::new(key.clone())), value.clone()],
        )?) {
            results.insert(key, value);
        }
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))?))
}

fn native_map_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(RuntimeError::new("expected map")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("forEach expects callback"))?;
    let entries: Vec<(String, Value)> = map
        .borrow()
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, value) in entries {
        vm.call(cb.clone(), vec![Value::String(Rc::new(key)), value])?;
    }
    Ok(Value::Null)
}

fn native_set_clear(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    set.borrow().ensure_mutable()?;
    set.borrow_mut().clear();
    Ok(Value::Null)
}

fn native_map_clear(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    map.borrow().ensure_mutable()?;
    map.borrow_mut().entries.clear();
    Ok(Value::Null)
}

fn native_map_contains_key(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let key = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(map.borrow().entries.contains_key(key)))
}

fn native_string_contains(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let sub = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(s.contains(sub)))
}

fn native_string_starts_with(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let pre = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(s.starts_with(pre)))
}

fn native_string_ends_with(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let suf = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::Bool(false)),
    };
    Ok(Value::Bool(s.ends_with(suf)))
}

fn native_string_replace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("expected string")),
    };
    let old = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(args[0].clone()),
    };
    let new = match args.get(2) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    Ok(Value::String(Rc::new(s.replace(old, new))))
}

fn native_set_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("map expects callback"))?;
    let items: Vec<Value> = set.borrow().iter().map(|k| k.to_value()).collect();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        results.push(vm.call(cb.clone(), vec![item])?);
    }
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(results))?,
    ))
}

fn native_set_filter(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("filter expects callback"))?;
    let mut result = IndexSet::new();
    for key in set.borrow().iter() {
        let val = key.to_value();
        if is_truthy(&vm.call(cb.clone(), vec![val])?) {
            result.insert(key.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))?))
}

fn native_set_for_each(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s.clone(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let cb = args
        .get(1)
        .cloned()
        .ok_or_else(|| RuntimeError::new("forEach expects callback"))?;
    for key in set.borrow().iter() {
        let val = key.to_value();
        vm.call(cb.clone(), vec![val])?;
    }
    Ok(Value::Null)
}

fn native_set_to_array(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    let values: Vec<Value> = set.borrow().iter().map(|k| k.to_value()).collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))?))
}

fn native_set_union(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s.borrow().elements.clone(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s.borrow(),
        _ => return Err(RuntimeError::new("union expects a set")),
    };
    let mut result = set1;
    for item in set2.iter() {
        result.insert(item.clone());
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))?))
}

fn native_set_intersection(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s.borrow(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s.borrow(),
        _ => return Err(RuntimeError::new("intersection expects a set")),
    };
    let mut result = IndexSet::new();
    for item in set1.iter() {
        if set2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))?))
}

fn native_set_difference(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set1 = match &args[0] {
        Value::Set(s) => s.borrow(),
        _ => return Err(RuntimeError::new("expected set")),
    };
    let set2 = match args.get(1) {
        Some(Value::Set(s)) => s.borrow(),
        _ => return Err(RuntimeError::new("difference expects a set")),
    };
    let mut result = IndexSet::new();
    for item in set1.iter() {
        if !set2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))?))
}

fn native_set_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = IndexSet::new();
    if let Some(arg) = args.first() {
        match arg {
            Value::Array(arr) => {
                for val in arr.borrow().iter() {
                    let key = ValueKey::try_from_value(val)
                        .ok_or_else(|| RuntimeError::new("invalid value for set"))?;
                    result.insert(key);
                }
            }
            Value::Set(s) => {
                result = s.borrow().elements.clone();
            }
            Value::Null => {}
            _ => return Err(RuntimeError::new("Set constructor expects array or set")),
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))?))
}

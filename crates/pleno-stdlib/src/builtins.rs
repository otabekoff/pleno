use crate::concurrency::*;
use pleno_value::{
    ArrayState, IteratorState, MapState, RuntimeError, SetState, TupleState, VMProvider, Value,
    ValueKey,
};
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

pub fn register_builtins(vm: &mut dyn VMProvider) {
    // Globals
    vm.register_global("print".to_string(), Value::NativeFunction(native_print));
    vm.register_global("println".to_string(), Value::NativeFunction(native_print));
    vm.register_global("id".to_string(), Value::NativeFunction(native_id));
    vm.register_global("typeof".to_string(), Value::NativeFunction(native_typeof));
    vm.register_global(
        "__native_sleep".to_string(),
        Value::NativeFunction(native_sleep),
    );
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

    let mut none_map = HashMap::new();
    none_map.insert(
        "variant".to_string(),
        Value::String(Rc::new("None".to_string())),
    );
    let none_val = Value::Map(vm.get_gc().allocate(MapState::new(none_map)));
    vm.register_global("None".to_string(), none_val);

    vm.register_global(
        "Promise".to_string(),
        Value::NativeFunction(crate::promise::promise_constructor),
    );

    // Modules registered as globals for backward compatibility
    let mut math_map = HashMap::new();
    math_map.insert(
        "abs".to_string(),
        Value::NativeFunction(crate::math::math_abs),
    );
    math_map.insert(
        "pow".to_string(),
        Value::NativeFunction(crate::math::math_pow),
    ); // needs modular math_pow
    math_map.insert(
        "sqrt".to_string(),
        Value::NativeFunction(crate::math::math_sqrt),
    );
    math_map.insert(
        "floor".to_string(),
        Value::NativeFunction(crate::math::math_floor),
    );
    math_map.insert(
        "ceil".to_string(),
        Value::NativeFunction(crate::math::math_ceil),
    );
    math_map.insert(
        "random".to_string(),
        Value::NativeFunction(crate::math::math_random),
    ); // register random
    let math_val = Value::Map(vm.get_gc().allocate(MapState::new(math_map)));
    vm.register_global("Math".to_string(), math_val);

    let mut json_map = HashMap::new();
    json_map.insert(
        "parse".to_string(),
        Value::NativeFunction(crate::json::json_parse),
    );
    json_map.insert(
        "stringify".to_string(),
        Value::NativeFunction(crate::json::json_stringify),
    );
    let json_val = Value::Map(vm.get_gc().allocate(MapState::new(json_map)));
    vm.register_global("JSON".to_string(), json_val);

    let mut os_map = HashMap::new();
    os_map.insert(
        "exit".to_string(),
        Value::NativeFunction(crate::process::process_exit),
    );
    os_map.insert(
        "getenv".to_string(),
        Value::NativeFunction(crate::process::process_env),
    );
    os_map.insert(
        "args".to_string(),
        Value::NativeFunction(crate::process::process_args),
    );
    os_map.insert(
        "cwd".to_string(),
        Value::NativeFunction(crate::process::process_cwd),
    );
    let os_val = Value::Map(vm.get_gc().allocate(MapState::new(os_map)));
    vm.register_global("OS".to_string(), os_val);

    // Promise methods
    let mut promise_obj_map = HashMap::new();
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
    let promise_val = Value::Map(vm.get_gc().allocate(MapState::new(promise_obj_map)));
    vm.register_global("Promise".to_string(), promise_val);

    // Types
    for ty in &[
        "Int", "Float", "Bool", "String", "Array", "Map", "Object", "Null", "Any",
    ] {
        vm.register_global(ty.to_string(), Value::String(Rc::new(ty.to_string())));
    }
    vm.register_global(
        "Set".to_string(),
        Value::NativeFunction(native_set_constructor),
    );

    // Internal hook for time
    vm.register_global(
        "__native_time_now".to_string(),
        Value::NativeFunction(crate::time::time_now),
    );
    vm.register_global(
        "__native_gc_collect".to_string(),
        Value::NativeFunction(native_gc_collect),
    );
    vm.register_global(
        "__native_gc_stats".to_string(),
        Value::NativeFunction(native_gc_stats),
    );
    vm.register_global(
        "spawn".to_string(),
        Value::NativeFunction(native_async_spawn),
    );
    vm.register_global("Mutex".to_string(), Value::NativeFunction(native_mutex_new));
    vm.register_global(
        "Channel".to_string(),
        Value::NativeFunction(native_channel_new),
    );
    vm.register_global(
        "stopIteration".to_string(),
        Value::NativeFunction(native_stop_iteration),
    );
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
                                    .allocate(TupleState::new(vec![key_val, val], None)),
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
                    keys,
                    index,
                    is_entries,
                } => {
                    if *index < keys.len() {
                        let key_str = &keys[*index];
                        let val = if *is_entries {
                            let key_val = Value::String(Rc::new(key_str.clone()));
                            let value_val =
                                map.borrow().get(key_str).cloned().unwrap_or(Value::Null);
                            Value::Tuple(
                                vm.get_gc()
                                    .allocate(TupleState::new(vec![key_val, value_val], None)),
                            )
                        } else {
                            Value::String(Rc::new(key_str.clone()))
                        };
                        *index += 1;
                        Ok(val)
                    } else {
                        Ok(Value::Missing)
                    }
                }
                IteratorState::Set { values, index, .. } => {
                    if *index < values.len() {
                        let key = &values[*index];
                        let val = match key {
                            ValueKey::Null => Value::Null,
                            ValueKey::Bool(b) => Value::Bool(*b),
                            ValueKey::Int(i) => Value::Int(*i),
                            ValueKey::Float(f) => Value::Float(f64::from_bits(*f)),
                            ValueKey::String(s) => Value::String(Rc::clone(s)),
                        };
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
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            let keys = map.borrow().keys().cloned().collect::<Vec<_>>();
            let state = IteratorState::Map {
                map: map.clone(),
                keys,
                index: 0,
                is_entries: false,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            let keys = map.borrow().keys().cloned().collect::<Vec<_>>();
            let state = IteratorState::Map {
                map: map.clone(),
                keys,
                index: 0,
                is_entries: true,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
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
            let values = set.borrow().iter().cloned().collect::<Vec<_>>();
            let state = IteratorState::Set {
                set: set.clone(),
                values,
                index: 0,
            };
            Ok(Value::Iterator(vm.get_gc().allocate(state)))
        }
        _ => Err(RuntimeError::new("iterator() called on non-set")),
    }
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
        ("Array", "push") => Some(native_array_push),
        ("Array", "pop") => Some(native_array_pop),
        ("Array", "shift") => Some(native_array_shift),
        ("Array", "unshift") => Some(native_array_unshift),
        ("Array", "insert") => Some(native_array_insert),
        ("Array", "remove") => Some(native_array_remove_at),
        ("Array", "clear") => Some(native_array_clear),
        ("Array", "slice") => Some(native_array_slice),
        ("Array", "join") => Some(native_array_join),
        ("Array", "find") => Some(native_array_find),
        ("Array", "map") => Some(native_array_map),
        ("Array", "filter") => Some(native_array_filter),
        ("Array", "reduce") => Some(native_array_reduce),
        ("Array", "forEach") => Some(native_array_for_each),
        ("Array", "reverse") => Some(native_array_reverse),
        ("Array", "sort") => Some(native_array_sort),
        ("Array", "contains") => Some(native_array_contains),
        ("Array", "indexOf") => Some(native_array_index_of),
        ("Array", "isEmpty") => Some(native_array_is_empty),
        ("Map", "put") => Some(native_map_put),
        ("Map", "get") => Some(native_map_get),
        ("Map", "remove") => Some(native_map_remove),
        ("Map", "clear") => Some(native_map_clear),
        ("Map", "containsKey") => Some(native_map_contains_key),
        ("Map", "keys") => Some(native_map_keys),
        ("Map", "values") => Some(native_map_values),
        ("Map", "size") => Some(native_map_size),
        ("Set", "clear") => Some(native_set_clear),
        ("Map", "map") => Some(native_map_map),
        ("Map", "filter") => Some(native_map_filter),
        ("Map", "forEach") => Some(native_map_for_each),
        ("Set", "add") => Some(native_set_add),
        ("Set", "remove") => Some(native_set_remove),
        ("Set", "contains") => Some(native_set_contains),
        ("Set", "size") => Some(native_set_size),
        ("Set", "map") => Some(native_set_map),
        ("Set", "filter") => Some(native_set_filter),
        ("Set", "forEach") => Some(native_set_for_each),
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
        ("Promise", "then") => Some(crate::promise::promise_then),
        ("Mutex", "lock") => Some(crate::concurrency::native_mutex_lock),
        ("Mutex", "unlock") => Some(crate::concurrency::native_mutex_unlock),
        ("Channel", "send") => Some(crate::concurrency::native_channel_send),
        ("Channel", "recv") => Some(crate::concurrency::native_channel_recv),
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
        ("Iterator", "next") => Some(native_iterator_next),
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
fn native_print(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    vm.check_purity()?;
    let mut out = String::new();
    for (idx, value) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{}", value));
    }
    println!("{out}");
    let _ = std::io::stdout().flush();
    Ok(Value::Null)
}

fn native_id(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(args.first().cloned().unwrap_or(Value::Null))
}

fn native_typeof(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let val = args.first().unwrap_or(&Value::Null);
    Ok(Value::String(Rc::new(val.type_name().to_string())))
}

fn native_sleep(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let ms = match args.first() {
        Some(Value::Int(m)) => *m as u64,
        _ => return Err(RuntimeError::new("sleep() expects integer milliseconds")),
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(Value::Null)
}

fn native_async_spawn(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("spawn() requires a function"));
    }
    let callee = args[0].clone();
    let call_args = args[1..].to_vec();
    vm.spawn(callee, call_args)
}

fn native_gc_collect(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    vm.collect();
    Ok(Value::Null)
}

fn native_gc_stats(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc_manager = vm.get_gc();
    let mut stats_map = HashMap::new();
    stats_map.insert(
        "heap_size".to_string(),
        Value::Int(gc_manager.gc.get_heap_size() as i64),
    );
    stats_map.insert(
        "threshold".to_string(),
        Value::Int(gc_manager.gc.allocation_threshold as i64),
    );
    stats_map.insert(
        "current_allocation".to_string(),
        Value::Int(gc_manager.current_allocation as i64),
    );
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(stats_map))))
}

fn native_global_map(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("map expects collection"));
    }
    match &args[0] {
        Value::Array(_) => native_array_map(vm, args),
        Value::Map(_) => native_map_map(vm, args),
        Value::Set(_) => native_set_map(vm, args),
        _ => Err(RuntimeError::new(
            "map expects collection (Array, Map, or Set)",
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
        _ => Err(RuntimeError::new(
            "filter expects collection (Array, Map, or Set)",
        )),
    }
}

fn native_global_reduce(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("reduce expects collection"));
    }
    match &args[0] {
        Value::Array(_) => native_array_reduce(vm, args),
        _ => Err(RuntimeError::new("reduce expects Array")),
    }
}

// Result/Option constructors
fn native_result_ok(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = HashMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Ok".to_string())),
    );
    res.insert(
        "value".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))))
}

fn native_result_err(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = HashMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Err".to_string())),
    );
    res.insert(
        "error".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))))
}

fn native_option_some(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = HashMap::new();
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Some".to_string())),
    );
    res.insert(
        "value".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))))
}

fn native_error_constructor(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    let mut res = HashMap::new();
    res.insert(
        "message".to_string(),
        args.first().cloned().unwrap_or(Value::Null),
    );
    // Also add variant for potential pattern matching
    res.insert(
        "variant".to_string(),
        Value::String(Rc::new("Error".to_string())),
    );
    Ok(Value::Map(gc.allocate(MapState::new(res))))
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
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(parts))))
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

fn native_array_push(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
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
    ))))
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
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
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
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
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
fn native_array_reverse(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a,
        _ => return Err(RuntimeError::new("expected array")),
    };
    arr.borrow().ensure_mutable()?;
    arr.borrow_mut().elements.reverse();
    Ok(Value::Null)
}
fn native_array_sort(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(RuntimeError::new("expected array")),
    };
    let mut items = arr.borrow().elements.clone();
    items.sort_by_key(|v| v.to_string());
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(items))))
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
    Ok(Value::Bool(map.borrow_mut().entries.remove(key).is_some()))
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
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(keys))))
}
fn native_map_values(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let map = match &args[0] {
        Value::Map(m) => m,
        _ => return Err(RuntimeError::new("expected map")),
    };
    let values: Vec<Value> = map.borrow().entries.values().cloned().collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))))
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
    Ok(Value::Bool(set.borrow_mut().remove(&key)))
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
    Err(RuntimeError::new("called unwrap on Err"))
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
    let mut results = std::collections::HashMap::new();
    for (key, value) in entries {
        let res = vm.call(cb.clone(), vec![Value::String(Rc::new(key.clone())), value])?;
        results.insert(key, res);
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))))
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
    let mut results = std::collections::HashMap::new();
    for (key, value) in entries {
        if is_truthy(&vm.call(
            cb.clone(),
            vec![Value::String(Rc::new(key.clone())), value.clone()],
        )?) {
            results.insert(key, value);
        }
    }
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(results))))
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
    let items: Vec<Value> = set
        .borrow()
        .iter()
        .map(|k| match k {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(s) => Value::String(Rc::clone(s)),
        })
        .collect();
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        results.push(vm.call(cb.clone(), vec![item])?);
    }
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(results))))
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
    let mut result = std::collections::HashSet::new();
    for key in set.borrow().iter() {
        let val = match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(s) => Value::String(Rc::clone(s)),
        };
        if is_truthy(&vm.call(cb.clone(), vec![val])?) {
            result.insert(key.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))))
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
        let val = match key {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(s) => Value::String(Rc::clone(s)),
        };
        vm.call(cb.clone(), vec![val])?;
    }
    Ok(Value::Null)
}

fn native_set_to_array(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let set = match &args[0] {
        Value::Set(s) => s,
        _ => return Err(RuntimeError::new("expected set")),
    };
    let values: Vec<Value> = set
        .borrow()
        .iter()
        .map(|k| match k {
            ValueKey::Null => Value::Null,
            ValueKey::Bool(v) => Value::Bool(*v),
            ValueKey::Int(v) => Value::Int(*v),
            ValueKey::Float(bits) => Value::Float(f64::from_bits(*bits)),
            ValueKey::String(s) => Value::String(Rc::clone(s)),
        })
        .collect();
    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(values))))
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
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))))
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
    let mut result = std::collections::HashSet::new();
    for item in set1.iter() {
        if set2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))))
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
    let mut result = std::collections::HashSet::new();
    for item in set1.iter() {
        if !set2.contains(item) {
            result.insert(item.clone());
        }
    }
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))))
}

fn native_set_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = std::collections::HashSet::new();
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
    Ok(Value::Set(vm.get_gc().allocate(SetState::new(result))))
}

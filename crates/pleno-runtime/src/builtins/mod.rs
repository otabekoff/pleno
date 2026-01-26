pub mod array;
pub mod concurrency;
pub mod http;
pub mod map;
pub mod math;
pub mod misc;
pub mod os;
pub mod regex;
pub mod result_option;
pub mod string;

use pleno_value::NativeFunction;

pub fn resolve_builtin_method(ty: &str, name: &str) -> Option<NativeFunction> {
    match (ty, name) {
        // String methods
        ("String", "toUpperCase") | ("String", "upper") => Some(string::string_upper),
        ("String", "toLowerCase") | ("String", "lower") => Some(string::string_lower),
        ("String", "trim") => Some(string::string_trim),
        ("String", "split") => Some(string::string_split),
        ("String", "contains") => Some(string::string_contains),
        ("String", "startsWith") => Some(string::string_starts_with),
        ("String", "endsWith") => Some(string::string_ends_with),
        ("String", "replace") => Some(string::string_replace),
        ("String", "substring") => Some(string::string_substring),
        ("String", "charAt") => Some(string::string_char_at),
        ("String", "indexOf") => Some(string::string_index_of),
        ("String", "repeat") => Some(string::string_repeat),

        // Array methods
        ("Array", "push") => Some(array::array_push),
        ("Array", "pop") => Some(array::array_pop),
        ("Array", "shift") => Some(array::array_shift),
        ("Array", "unshift") => Some(array::array_unshift),
        ("Array", "insert") => Some(array::array_insert),
        ("Array", "remove") => Some(array::array_remove_at),
        ("Array", "clear") => Some(array::array_clear),
        ("Array", "slice") => Some(array::array_slice),
        ("Array", "join") => Some(array::array_join),
        ("Array", "find") => Some(array::array_find),
        ("Array", "map") => Some(array::array_map),
        ("Array", "filter") => Some(array::array_filter),
        ("Array", "reduce") => Some(array::array_reduce),
        ("Array", "forEach") => Some(array::array_for_each),
        ("Array", "reverse") => Some(array::array_reverse),
        ("Array", "contains") => Some(array::array_contains),
        ("Array", "indexOf") => Some(array::array_index_of),
        ("Array", "sort") => Some(array::array_sort),
        ("Array", "isEmpty") => Some(array::array_is_empty),

        // Map methods
        ("Map", "put") => Some(map::map_put),
        ("Map", "get") => Some(map::map_get),
        ("Map", "remove") => Some(map::map_remove),
        ("Map", "containsKey") => Some(map::map_contains_key),
        ("Map", "keys") => Some(map::map_keys),
        ("Map", "values") => Some(map::map_values),
        ("Map", "entries") => Some(map::map_entries),
        ("Map", "size") => Some(map::map_size),
        ("Map", "isEmpty") => Some(map::map_is_empty),
        ("Map", "clear") => Some(map::map_clear),
        ("Map", "map") => Some(map::map_map),
        ("Map", "filter") => Some(map::map_filter),
        ("Map", "forEach") => Some(map::map_for_each),

        // Set methods
        ("Set", "add") => Some(map::set_add),
        ("Set", "remove") => Some(map::set_remove),
        ("Set", "delete") => Some(map::set_remove),
        ("Set", "contains") => Some(map::set_contains),
        ("Set", "has") => Some(map::set_contains),
        ("Set", "size") => Some(map::set_size),
        ("Set", "isEmpty") => Some(map::set_is_empty),
        ("Set", "clear") => Some(map::set_clear),
        ("Set", "union") => Some(map::set_union),
        ("Set", "intersection") => Some(map::set_intersection),
        ("Set", "difference") => Some(map::set_difference),
        ("Set", "toArray") => Some(map::set_to_array),
        ("Set", "map") => Some(map::set_map),
        ("Set", "filter") => Some(map::set_filter),
        ("Set", "forEach") => Some(map::set_for_each),

        // Result methods
        ("Result", "unwrap") => Some(result_option::result_unwrap),
        ("Result", "unwrapOr") => Some(result_option::result_unwrap_or),
        ("Result", "expect") => Some(result_option::result_expect),
        ("Result", "isOk") => Some(result_option::result_is_ok),
        ("Result", "isErr") => Some(result_option::result_is_err),

        // Option methods
        ("Option", "unwrap") => Some(result_option::option_unwrap),
        ("Option", "unwrapOr") => Some(result_option::option_unwrap_or),
        ("Option", "expect") => Some(result_option::option_expect),
        ("Option", "isSome") => Some(result_option::option_is_some),
        ("Option", "isNone") => Some(result_option::option_is_none),

        // Concurrency methods (built-in types)
        ("Mutex", "lock") => Some(concurrency::mutex_lock),
        ("Mutex", "unlock") => Some(concurrency::mutex_unlock),
        ("Channel", "send") => Some(concurrency::channel_send),
        ("Channel", "recv") => Some(concurrency::channel_recv),

        // Promise methods
        ("Promise", "then") => Some(promise::promise_then),

        _ => None,
    }
}

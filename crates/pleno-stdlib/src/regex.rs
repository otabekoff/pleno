use indexmap::IndexMap;
use pleno_value::{ArrayState, BoundNativeMethodState, MapState, RuntimeError, VMProvider, Value};
use std::rc::Rc;

pub fn regex_constructor(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let pattern = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => {
            return Err(RuntimeError::new(
                "Regex constructor expects pattern string",
            ));
        }
    };

    let flags = match args.get(1) {
        None | Some(Value::Null) | Some(Value::Missing) => Rc::new(String::new()),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(RuntimeError::new("Regex flags must be a string")),
    };
    // Compile once here so a bad pattern or flag fails at construction.
    build_regex(&pattern, &flags)?;

    let mut map = IndexMap::new();
    map.insert(
        "variant".to_string(),
        Value::String(Rc::new("Regex".to_string())),
    );
    map.insert("pattern".to_string(), Value::String(pattern));
    map.insert("flags".to_string(), Value::String(flags));

    let map_val = Value::Map(vm.get_gc().allocate(MapState::new(map))?);

    // Bind methods
    if let Value::Map(state) = &map_val {
        let mut state = state.borrow_mut();
        let gc = vm.get_gc();
        state.entries.insert(
            "test".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_test,
            })?),
        );
        state.entries.insert(
            "match".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_match,
            })?),
        );
        state.entries.insert(
            "findAll".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_find_all,
            })?),
        );
        state.entries.insert(
            "replace".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_replace,
            })?),
        );
        state.entries.insert(
            "split".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: map_val.clone(),
                function: regex_split,
            })?),
        );
    }

    Ok(map_val)
}

/// Compiles `pattern` with `flags`: any of `i` (case-insensitive), `m` (`^`
/// and `$` match at line breaks), `s` (`.` matches a newline) and `x`
/// (verbose: whitespace and `#` comments in the pattern are ignored).
fn build_regex(pattern: &str, flags: &str) -> Result<regex::Regex, RuntimeError> {
    let mut builder = regex::RegexBuilder::new(pattern);
    for flag in flags.chars() {
        match flag {
            'i' => builder.case_insensitive(true),
            'm' => builder.multi_line(true),
            's' => builder.dot_matches_new_line(true),
            'x' => builder.ignore_whitespace(true),
            other => {
                return Err(RuntimeError::new(format!(
                    "invalid regex flag '{other}': expected i, m, s or x"
                )));
            }
        };
    }
    builder
        .build()
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))
}

/// The compiled regex of a `Regex` object (its `pattern` and `flags`).
fn receiver_regex(receiver: Option<&Value>, method: &str) -> Result<regex::Regex, RuntimeError> {
    let Some(Value::Map(m)) = receiver else {
        return Err(RuntimeError::new(format!(
            "Regex.{method}: invalid receiver"
        )));
    };
    let this = m.borrow();
    let pattern = match this.get("pattern") {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(RuntimeError::new("Regex instance missing pattern")),
    };
    let flags = match this.get("flags") {
        Some(Value::String(s)) => s.clone(),
        _ => Rc::new(String::new()),
    };
    drop(this);
    build_regex(&pattern, &flags)
}

pub fn regex_test(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let re = receiver_regex(args.first(), "test")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.test expects text string")),
    };

    Ok(Value::Bool(re.is_match(text)))
}

pub fn regex_match(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let re = receiver_regex(args.first(), "match")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.match expects text string")),
    };

    let caps = re.captures(text);
    match caps {
        Some(caps) => {
            let index = caps
                .get(0)
                .map_or(0, |m| pleno_value::text::char_index(text, m.start()));
            match_object(vm, &caps, index)
        }
        None => Ok(Value::Null),
    }
}

/// A `Match`: `value` (the whole match), `index` (the character position
/// where it starts, passed in as `index`), `groups`
/// (the capture groups) and `group(i)`. It can also be read like an array:
/// `m[0]` is the whole match, `m[1]` the first group, and `m.length` the count.
fn match_object(
    vm: &mut dyn VMProvider,
    caps: &regex::Captures,
    index: usize,
) -> Result<Value, RuntimeError> {
    let text_of = |i: usize| {
        caps.get(i)
            .map(|m| Value::String(Rc::new(m.as_str().to_string())))
            .unwrap_or(Value::Null)
    };
    let mut entries = IndexMap::new();
    for i in 0..caps.len() {
        entries.insert(i.to_string(), text_of(i));
    }
    entries.insert("length".to_string(), Value::Int(caps.len() as i64));
    entries.insert("value".to_string(), text_of(0));
    entries.insert("index".to_string(), Value::Int(index as i64));
    let groups = (1..caps.len()).map(text_of).collect();
    let groups = Value::Array(vm.get_gc().allocate(ArrayState::new(groups))?);
    entries.insert("groups".to_string(), groups);

    let match_val = Value::Map(vm.get_gc().allocate(MapState::new(entries))?);
    let group = Value::BoundNativeMethod(vm.get_gc().allocate(BoundNativeMethodState {
        receiver: match_val.clone(),
        function: match_group,
    })?);
    if let Value::Map(m) = &match_val {
        m.borrow_mut().entries.insert("group".to_string(), group);
    }
    Ok(match_val)
}

/// `m.group(i)`: capture group `i` (0 is the whole match), or null.
fn match_group(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (Some(Value::Map(m)), Some(Value::Int(i))) = (args.first(), args.get(1)) else {
        return Err(RuntimeError::new("Match.group expects an integer index"));
    };
    Ok(m.borrow()
        .entries
        .get(&i.to_string())
        .cloned()
        .unwrap_or(Value::Null))
}

/// `Regex.isMatch(pattern, text)`.
pub fn regex_is_match_static(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let (Some(Value::String(pattern)), Some(Value::String(text))) = (args.first(), args.get(1))
    else {
        return Err(RuntimeError::new("Regex.isMatch expects pattern and text"));
    };
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(text)))
}

/// `Regex.replace(pattern, text, replacement)`: replaces every match.
pub fn regex_replace_static(
    _vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let (Some(Value::String(pattern)), Some(Value::String(text)), Some(Value::String(with))) =
        (args.first(), args.get(1), args.get(2))
    else {
        return Err(RuntimeError::new(
            "Regex.replace expects pattern, text and replacement",
        ));
    };
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new(format!("invalid regex: {}", e)))?;
    Ok(Value::String(Rc::new(
        re.replace_all(text, with.as_str()).to_string(),
    )))
}

pub fn regex_find_all(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let re = receiver_regex(args.first(), "findAll")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.findAll expects text string")),
    };

    let all_caps: Vec<_> = re.captures_iter(text).collect();
    let mut matches = Vec::new();
    // Count characters from the previous match on, so this stays linear.
    let (mut byte, mut index) = (0, 0);
    for caps in &all_caps {
        if let Some(m) = caps.get(0) {
            index += text[byte..m.start()].chars().count();
            byte = m.start();
        }
        matches.push(match_object(vm, caps, index)?);
    }
    Ok(Value::Array(
        vm.get_gc().allocate(ArrayState::new(matches))?,
    ))
}

pub fn regex_replace(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let re = receiver_regex(args.first(), "replace")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Regex.replace expects text string")),
    };
    let replacement = match args.get(2) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Regex.replace expects replacement string",
            ));
        }
    };

    let result = re.replace_all(text, replacement);
    Ok(Value::String(Rc::new(result.to_string())))
}

/// `pattern.split(text)`: the pieces of `text` between matches.
pub fn regex_split(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let re = receiver_regex(args.first(), "split")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(RuntimeError::new("Regex.split expects text string")),
    };
    let parts = re
        .split(&text)
        .map(|part| Value::String(Rc::new(part.to_string())))
        .collect();
    Ok(Value::Array(
        vm.get_gc().allocate(pleno_value::ArrayState::new(parts))?,
    ))
}

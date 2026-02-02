use ::url::Url;
use indexmap::IndexMap;
use pleno_value::{MapState, RuntimeError, Value};
use std::rc::Rc;

pub fn url_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("Url.parse expects string")),
    };

    let parsed = Url::parse(input).map_err(|e| RuntimeError::new(format!("Invalid URL: {}", e)))?;

    let mut map = IndexMap::new();
    map.insert(
        "href".to_string(),
        Value::String(Rc::new(parsed.to_string())),
    );
    map.insert(
        "protocol".to_string(),
        Value::String(Rc::new(parsed.scheme().to_string())),
    );
    map.insert(
        "username".to_string(),
        Value::String(Rc::new(parsed.username().to_string())),
    );
    map.insert(
        "password".to_string(),
        parsed
            .password()
            .map(|p| Value::String(Rc::new(p.to_string())))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "host".to_string(),
        parsed
            .host_str()
            .map(|h| Value::String(Rc::new(h.to_string())))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "port".to_string(),
        parsed
            .port()
            .map(|p| Value::Int(p as i64))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "path".to_string(),
        Value::String(Rc::new(parsed.path().to_string())),
    );
    map.insert(
        "query".to_string(),
        parsed
            .query()
            .map(|q| Value::String(Rc::new(q.to_string())))
            .unwrap_or(Value::Null),
    );
    map.insert(
        "hash".to_string(),
        parsed
            .fragment()
            .map(|f| Value::String(Rc::new(f.to_string())))
            .unwrap_or(Value::Null),
    );

    Ok(Value::Map(vm.get_gc().allocate(MapState::new(map))?))
}

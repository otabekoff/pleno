use indexmap::IndexMap;
use pleno_value::{ArrayState, MapState, RuntimeError, Value};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::rc::Rc;

pub fn xml_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("xml.parse expects string")),
    };

    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<(String, IndexMap<String, Value>, Vec<Value>)> = Vec::new();

    // We need a root container to hold the top-level element(s)
    // XML allows one root, but comments/declarations exist.
    // Let's return the root element as a Map.

    // Actually, to handle the parsing loop correctly:
    // When Start(tag), push to stack.
    // When End(tag), pop and add to parent.
    // If stack empty after pop, that's our result (if it was the root).

    // But we might have multiple roots in malformed fragments, or just one.
    // We'll wrap the logic to return the first root element found.

    let mut root_value: Option<Value> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attributes = IndexMap::new();
                for attr in e.attributes() {
                    let attr =
                        attr.map_err(|err| RuntimeError::new(format!("XML attr error: {}", err)))?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let val = String::from_utf8_lossy(&attr.value).to_string();
                    attributes.insert(key, Value::String(Rc::new(val)));
                }
                stack.push((name, attributes, Vec::new()));
            }
            Ok(Event::End(_)) => {
                if let Some((name, attrs, children)) = stack.pop() {
                    // Create element map
                    let mut map = IndexMap::new();
                    map.insert("name".to_string(), Value::String(Rc::new(name)));
                    map.insert(
                        "attributes".to_string(),
                        Value::Map(vm.get_gc().allocate(MapState::new(attrs))?),
                    );
                    map.insert(
                        "children".to_string(),
                        Value::Array(vm.get_gc().allocate(ArrayState::new(children))?),
                    );

                    let element = Value::Map(vm.get_gc().allocate(MapState::new(map))?);

                    if stack.is_empty() {
                        root_value = Some(element);
                        break;
                    } else {
                        stack.last_mut().unwrap().2.push(element);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                // Self-closing tag <tag />
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attributes = IndexMap::new();
                for attr in e.attributes() {
                    let attr =
                        attr.map_err(|err| RuntimeError::new(format!("XML attr error: {}", err)))?;
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let val = String::from_utf8_lossy(&attr.value).to_string();
                    attributes.insert(key, Value::String(Rc::new(val)));
                }

                let mut map = IndexMap::new();
                map.insert("name".to_string(), Value::String(Rc::new(name)));
                map.insert(
                    "attributes".to_string(),
                    Value::Map(vm.get_gc().allocate(MapState::new(attributes))?),
                );
                map.insert(
                    "children".to_string(),
                    Value::Array(vm.get_gc().allocate(ArrayState::new(Vec::new()))?),
                );
                let element = Value::Map(vm.get_gc().allocate(MapState::new(map))?);

                if stack.is_empty() {
                    root_value = Some(element);
                    break;
                } else {
                    stack.last_mut().unwrap().2.push(element);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(parent) = stack.last_mut() {
                    let text = e
                        .unescape()
                        .map_err(|err| RuntimeError::new(err.to_string()))?;
                    parent.2.push(Value::String(Rc::new(text.into_owned())));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(RuntimeError::new(format!("XML parse error: {}", e))),
            _ => {
                // Ignore Comments, Decl, DocType for simple parsing
            }
        }
        buf.clear();
    }

    match root_value {
        Some(v) => Ok(v),
        None => Ok(Value::Null), // Empty or comment-only XML
    }
}

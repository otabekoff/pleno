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

    // quick-xml reports `&entity;` as separate `GeneralRef` events, and its text trimming would
    // trim around each reference, so text runs are collected raw and trimmed/unescaped as a whole.
    let mut reader = Reader::from_str(input);
    let mut text = String::new();

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
        let event = reader.read_event_into(&mut buf);
        if !matches!(event, Ok(Event::Text(_) | Event::GeneralRef(_))) {
            flush_text(&mut text, &mut stack)?;
        }
        match event {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_string();
                let mut attributes = IndexMap::new();
                for attr in e.attributes() {
                    let attr =
                        attr.map_err(|err| RuntimeError::new(format!("XML attr error: {}", err)))?;
                    let key = attr.key.as_ref().to_string();
                    let val = attr.value.into_owned();
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
                let name = e.name().as_ref().to_string();
                let mut attributes = IndexMap::new();
                for attr in e.attributes() {
                    let attr =
                        attr.map_err(|err| RuntimeError::new(format!("XML attr error: {}", err)))?;
                    let key = attr.key.as_ref().to_string();
                    let val = attr.value.into_owned();
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
            Ok(Event::Text(e)) => text.push_str(&e),
            Ok(Event::GeneralRef(e)) => {
                text.push('&');
                text.push_str(&e);
                text.push(';');
            }
            Ok(Event::Eof) => {
                if let Some((name, _, _)) = stack.last() {
                    return Err(RuntimeError::new(format!(
                        "XML parse error: element <{name}> is not closed"
                    )));
                }
                break;
            }
            Err(e) => return Err(RuntimeError::new(format!("XML parse error: {}", e))),
            _ => {
                // Ignore Comments, Decl, DocType for simple parsing
            }
        }
        buf.clear();
    }

    // Empty input, or only a declaration, comments or text: nothing to return.
    root_value.ok_or_else(|| RuntimeError::new("XML parse error: no root element"))
}

/// Adds the collected raw text run (trimmed, then unescaped) as a child of the open element.
fn flush_text(
    text: &mut String,
    stack: &mut [(String, IndexMap<String, Value>, Vec<Value>)],
) -> Result<(), RuntimeError> {
    let raw = text.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if let (false, Some(parent)) = (raw.is_empty(), stack.last_mut()) {
        let unescaped =
            quick_xml::escape::unescape(raw).map_err(|err| RuntimeError::new(err.to_string()))?;
        parent
            .2
            .push(Value::String(Rc::new(unescaped.into_owned())));
    }
    text.clear();
    Ok(())
}

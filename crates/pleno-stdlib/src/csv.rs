use pleno_value::{ArrayState, RuntimeError, Value};
use std::rc::Rc;

/// `CSV.parse(text, options?)`: the rows of `text` as arrays of strings.
///
/// By default the first row is a header and is not returned. With
/// `{ header: false }` (or the named argument `header: false`) every row is
/// returned, the first one included.
pub fn csv_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(RuntimeError::new("CSV.parse expects a string")),
    };
    let has_header = header_option(vm, args.get(1))?;

    let mut rdr = ::csv::ReaderBuilder::new()
        .has_headers(has_header)
        .from_reader(input.as_bytes());
    let mut result = Vec::new();

    for result_record in rdr.records() {
        let record = result_record.map_err(csv_error)?;
        let mut row = Vec::new();
        for field in record.iter() {
            row.push(Value::String(Rc::new(field.to_string())));
        }
        result.push(Value::Array(vm.get_gc().allocate(ArrayState::new(row))?));
    }

    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(result))?))
}

/// The `header` option: from an options map, or a named argument.
fn header_option(
    vm: &mut dyn pleno_value::VMProvider,
    options: Option<&Value>,
) -> Result<bool, RuntimeError> {
    let named = vm.get_named_args().get("header").cloned();
    let value = match options {
        None | Some(Value::Null) | Some(Value::Missing) => named,
        Some(Value::Map(map)) => {
            let map = map.borrow();
            if let Some(key) = map.entries.keys().find(|key| key.as_str() != "header") {
                return Err(RuntimeError::new(format!(
                    "CSV.parse: unknown option '{key}'"
                )));
            }
            map.entries.get("header").cloned()
        }
        Some(_) => {
            return Err(RuntimeError::new(
                "CSV.parse options must be a map such as { header: false }",
            ));
        }
    };
    match value {
        None | Some(Value::Missing) => Ok(true),
        Some(Value::Bool(b)) => Ok(b),
        Some(_) => Err(RuntimeError::new("CSV.parse: header must be a Bool")),
    }
}

/// A CSV error message with a single `CSV error:` prefix. The csv crate's
/// messages already start with `CSV error:` or `CSV parse error:`.
fn csv_error(error: ::csv::Error) -> RuntimeError {
    let message = error.to_string();
    if message.starts_with("CSV ") {
        RuntimeError::new(message)
    } else {
        RuntimeError::new(format!("CSV error: {message}"))
    }
}

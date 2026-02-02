use pleno_value::{ArrayState, RuntimeError, Value};
use std::rc::Rc;

pub fn csv_parse(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let input = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("csv.parse expects string")),
    };

    let mut rdr = ::csv::Reader::from_reader(input.as_bytes());
    let mut result = Vec::new();

    // Iterate over records
    for result_record in rdr.records() {
        let record = result_record.map_err(|e| RuntimeError::new(format!("CSV error: {}", e)))?;
        let mut row = Vec::new();
        for field in record.iter() {
            row.push(Value::String(Rc::new(field.to_string())));
        }
        result.push(Value::Array(vm.get_gc().allocate(ArrayState::new(row))?));
    }

    Ok(Value::Array(vm.get_gc().allocate(ArrayState::new(result))?))
}

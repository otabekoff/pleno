use pleno_value::{RuntimeError, VMProvider, Value};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn dotenv_config(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => ".env",
    };

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let text = line.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = text.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                // Basic quote handling could be added here if needed
                // Singleton VM constraint makes this relatively safe
                unsafe {
                    std::env::set_var(key, val);
                }
            }
        }
    }

    // Return true if file existed? Or just null?
    // Standard dotenv usually returns check.
    // For now, simple Null.
    Ok(Value::Null)
}

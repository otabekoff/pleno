use pleno_value::Value;
use std::fs;
use std::rc::Rc;

pub fn io_file_read(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.read expects path string",
            ));
        }
    };

    match fs::read_to_string(path) {
        Ok(content) => Ok(Value::String(Rc::new(content))),
        Err(e) => Err(pleno_value::RuntimeError::new(format!(
            "File.read failed: {}",
            e
        ))),
    }
}

pub fn io_file_write(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.write expects path string",
            ));
        }
    };
    let content = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.write expects content string",
            ));
        }
    };

    match fs::write(path, content) {
        Ok(_) => Ok(Value::Null),
        Err(e) => Err(pleno_value::RuntimeError::new(format!(
            "File.write failed: {}",
            e
        ))),
    }
}

pub fn io_file_exists(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.exists expects path string",
            ));
        }
    };

    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

pub fn io_file_append(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.append expects path string",
            ));
        }
    };
    let content = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.append expects content string",
            ));
        }
    };

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.append open failed: {}", e)))?;

    write!(file, "{}", content)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.append write failed: {}", e)))?;

    Ok(Value::Null)
}

pub fn io_file_delete(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.delete expects path string",
            ));
        }
    };

    fs::remove_file(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.delete failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_directory_create(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.create expects path",
            ));
        }
    };
    fs::create_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.create failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_directory_exists(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.exists expects path",
            ));
        }
    };
    let meta = fs::metadata(path);
    Ok(Value::Bool(meta.map(|m| m.is_dir()).unwrap_or(false)))
}

pub fn io_directory_delete(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.delete expects path",
            ));
        }
    };
    fs::remove_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.delete failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_directory_list(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.list expects path",
            ));
        }
    };
    let entries = fs::read_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.list failed: {}", e)))?;
    let mut results = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| pleno_value::RuntimeError::new(e.to_string()))?;
        results.push(Value::String(Rc::new(
            entry.file_name().to_string_lossy().to_string(),
        )));
    }
    Ok(Value::Array(
        vm.get_gc().allocate(pleno_value::ArrayState::new(results)),
    ))
}

pub fn io_directory_list_files(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.listFiles expects path",
            ));
        }
    };
    let entries = fs::read_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.list failed: {}", e)))?;
    let mut results = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| pleno_value::RuntimeError::new(e.to_string()))?;
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            results.push(Value::String(Rc::new(
                entry.file_name().to_string_lossy().to_string(),
            )));
        }
    }
    Ok(Value::Array(
        vm.get_gc().allocate(pleno_value::ArrayState::new(results)),
    ))
}

pub fn io_directory_list_directories(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.listDirectories expects path",
            ));
        }
    };
    let entries = fs::read_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.list failed: {}", e)))?;
    let mut results = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| pleno_value::RuntimeError::new(e.to_string()))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            results.push(Value::String(Rc::new(
                entry.file_name().to_string_lossy().to_string(),
            )));
        }
    }
    Ok(Value::Array(
        vm.get_gc().allocate(pleno_value::ArrayState::new(results)),
    ))
}

pub fn io_directory_create_recursive(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.createRecursive expects path",
            ));
        }
    };
    fs::create_dir_all(path).map_err(|e| {
        pleno_value::RuntimeError::new(format!("Directory.createRecursive failed: {}", e))
    })?;
    Ok(Value::Null)
}

pub fn io_directory_delete_recursive(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Directory.deleteRecursive expects path",
            ));
        }
    };
    fs::remove_dir_all(path).map_err(|e| {
        pleno_value::RuntimeError::new(format!("Directory.deleteRecursive failed: {}", e))
    })?;
    Ok(Value::Null)
}

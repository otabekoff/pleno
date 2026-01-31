use pleno_value::{FileState, RuntimeError, Value};
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

pub fn io_file_read(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }

    match fs::read_to_string(path) {
        Ok(content) => Ok(Value::String(Rc::new(content))),
        Err(e) => Err(pleno_value::RuntimeError::new(format!(
            "File.read failed: {}",
            e
        ))),
    }
}

pub fn io_file_write(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
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
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }

    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

pub fn io_file_append(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
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
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }

    fs::remove_file(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.delete failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_directory_create(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
    fs::create_dir(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("Directory.create failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_directory_exists(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }
    let meta = fs::metadata(path);
    Ok(Value::Bool(meta.map(|m| m.is_dir()).unwrap_or(false)))
}

pub fn io_directory_delete(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }
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
        vm.get_gc()
            .allocate(pleno_value::ArrayState::new(results))?,
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }
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
        vm.get_gc()
            .allocate(pleno_value::ArrayState::new(results))?,
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
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }
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
        vm.get_gc()
            .allocate(pleno_value::ArrayState::new(results))?,
    ))
}

pub fn io_directory_create_recursive(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
    fs::create_dir_all(path).map_err(|e| {
        pleno_value::RuntimeError::new(format!("Directory.createRecursive failed: {}", e))
    })?;
    Ok(Value::Null)
}

pub fn io_directory_delete_recursive(
    vm: &mut dyn pleno_value::VMProvider,
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
    if !vm.is_write_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied",
        ));
    }
    fs::remove_dir_all(path).map_err(|e| {
        pleno_value::RuntimeError::new(format!("Directory.deleteRecursive failed: {}", e))
    })?;
    Ok(Value::Null)
}

pub fn io_open(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("open expects path string")),
    };
    let mode = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        None => "r",
        _ => return Err(RuntimeError::new("open mode must be string")),
    };

    match mode {
        "r" => {
            if !vm.is_read_allowed(path) {
                return Err(RuntimeError::new("SecurityError: Read access denied"));
            }
        }
        "w" | "a" => {
            if !vm.is_write_allowed(path) {
                return Err(RuntimeError::new("SecurityError: Write access denied"));
            }
        }
        _ => {}
    }

    let inner: Box<dyn std::io::Write> = match mode {
        "w" => Box::new(fs::File::create(path).map_err(|e| RuntimeError::new(e.to_string()))?),
        "a" => Box::new(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| RuntimeError::new(e.to_string()))?,
        ),
        "r" => {
            return Err(RuntimeError::new(
                "read mode not supported for File objects yet",
            ));
        }
        _ => return Err(RuntimeError::new("invalid mode")),
    };

    Ok(Value::File(vm.get_gc().allocate(FileState {
        inner: RefCell::new(Some(inner)),
        name: format!("<file {}>", path),
    })?))
}

pub fn io_stdout(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_io_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: IO access denied",
        ));
    }
    Ok(Value::File(vm.get_gc().allocate(FileState {
        inner: RefCell::new(Some(Box::new(std::io::stdout()))),
        name: "<stdout>".to_string(),
    })?))
}

pub fn io_stderr(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_io_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: IO access denied",
        ));
    }
    Ok(Value::File(vm.get_gc().allocate(FileState {
        inner: RefCell::new(Some(Box::new(std::io::stderr()))),
        name: "<stderr>".to_string(),
    })?))
}

pub fn io_file_copy(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let src = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected source path string",
            ));
        }
    };
    let dst = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected destination path string",
            ));
        }
    };
    if !vm.is_read_allowed(src) {
        return Err(RuntimeError::new(
            "SecurityError: Read access denied (source)",
        ));
    }
    if !vm.is_write_allowed(dst) {
        return Err(RuntimeError::new(
            "SecurityError: Write access denied (destination)",
        ));
    }
    fs::copy(src, dst)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.copy failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_file_move(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if args.len() < 2 {
        return Err(pleno_value::RuntimeError::new(
            "File.move requires source and destination paths",
        ));
    }
    let src = match &args[0] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected source path string",
            ));
        }
    };
    let dst = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Expected destination path string",
            ));
        }
    };
    if !vm.is_read_allowed(src) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied (source)",
        ));
    }
    if !vm.is_write_allowed(dst) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Write access denied (destination)",
        ));
    }
    fs::rename(src, dst)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.move failed: {}", e)))?;
    Ok(Value::Null)
}

pub fn io_file_read_lines(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let path = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "File.readLines expects path string",
            ));
        }
    };
    if !vm.is_read_allowed(path) {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Read access denied",
        ));
    }

    use std::io::BufRead;
    let file = fs::File::open(path)
        .map_err(|e| pleno_value::RuntimeError::new(format!("File.readLines failed: {}", e)))?;
    let reader = std::io::BufReader::new(file);
    let mut lines = Vec::new();

    for line in reader.lines() {
        let line = line
            .map_err(|e| pleno_value::RuntimeError::new(format!("File.readLines failed: {}", e)))?;
        lines.push(Value::String(Rc::new(line)));
    }

    Ok(Value::Array(
        vm.get_gc().allocate(pleno_value::ArrayState::new(lines))?,
    ))
}

use indexmap::IndexMap;
use pleno_value::{
    BoundNativeMethodState, MapState, RuntimeError, SandboxConfig, VMProvider, Value,
};
use std::rc::Rc;

pub fn sandbox_new(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let config_val = args.first().cloned();

    let mut config = SandboxConfig::default();

    if let Some(Value::Map(map)) = config_val {
        let borrowed = map.borrow();
        if let Some(Value::Bool(allow)) = borrowed.entries.get("allowIO") {
            config.allow_io = *allow;
        }
        if let Some(Value::Bool(allow)) = borrowed.entries.get("allowNetwork") {
            config.allow_network = *allow;
        }
        if let Some(Value::Int(limit)) = borrowed.entries.get("maxMemory") {
            config.max_memory = *limit as usize;
        }
        if let Some(Value::Int(limit)) = borrowed.entries.get("maxInstructions") {
            config.max_instructions = *limit as u64;
        }
        if let Some(Value::Array(paths)) = borrowed.entries.get("allowedReadPaths") {
            for v in &paths.borrow().elements {
                if let Value::String(s) = v {
                    config.allowed_read_paths.push(s.to_string());
                }
            }
        }
        if let Some(Value::Array(paths)) = borrowed.entries.get("allowedWritePaths") {
            for v in &paths.borrow().elements {
                if let Value::String(s) = v {
                    config.allowed_write_paths.push(s.to_string());
                }
            }
        }
    }

    // Create the sandbox object
    let mut sandbox_data = IndexMap::new();
    sandbox_data.insert("_allowIO".to_string(), Value::Bool(config.allow_io));
    sandbox_data.insert(
        "_allowNetwork".to_string(),
        Value::Bool(config.allow_network),
    );
    sandbox_data.insert(
        "_maxMemory".to_string(),
        Value::Int(config.max_memory as i64),
    );
    sandbox_data.insert(
        "_maxInstructions".to_string(),
        Value::Int(config.max_instructions as i64),
    );

    let mut read_paths = Vec::new();
    for p in &config.allowed_read_paths {
        read_paths.push(Value::String(Rc::new(p.clone())));
    }
    sandbox_data.insert(
        "_allowedReadPaths".to_string(),
        Value::Array(
            vm.get_gc()
                .allocate(pleno_value::ArrayState::new(read_paths))?,
        ),
    );

    let mut write_paths = Vec::new();
    for p in &config.allowed_write_paths {
        write_paths.push(Value::String(Rc::new(p.clone())));
    }
    sandbox_data.insert(
        "_allowedWritePaths".to_string(),
        Value::Array(
            vm.get_gc()
                .allocate(pleno_value::ArrayState::new(write_paths))?,
        ),
    );

    let gc = vm.get_gc();
    let sandbox_map = Value::Map(gc.allocate(MapState::new(sandbox_data))?);

    let run_method = Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
        receiver: sandbox_map.clone(),
        function: sandbox_run,
    })?);

    if let Value::Map(ref ptr) = sandbox_map {
        ptr.borrow_mut()
            .entries
            .insert("run".to_string(), run_method);
    }

    Ok(sandbox_map)
}

pub fn sandbox_run(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let receiver = args
        .first()
        .ok_or_else(|| RuntimeError::new("Missing receiver"))?;
    let func = args
        .get(1)
        .ok_or_else(|| RuntimeError::new("Sandbox.run expects a function"))?;

    let mut allow_io = false;
    let mut allow_network = false;
    let mut max_memory = 0;
    let mut max_instructions = 0;
    let mut allowed_read_paths = Vec::new();
    let mut allowed_write_paths = Vec::new();

    if let Value::Map(map) = receiver {
        let borrowed = map.borrow();
        if let Some(Value::Bool(v)) = borrowed.entries.get("_allowIO") {
            allow_io = *v;
        }
        if let Some(Value::Bool(v)) = borrowed.entries.get("_allowNetwork") {
            allow_network = *v;
        }
        if let Some(Value::Int(v)) = borrowed.entries.get("_maxMemory") {
            max_memory = *v as usize;
        }
        if let Some(Value::Int(v)) = borrowed.entries.get("_maxInstructions") {
            max_instructions = *v as u64;
        }
        if let Some(Value::Array(paths)) = borrowed.entries.get("_allowedReadPaths") {
            for v in &paths.borrow().elements {
                if let Value::String(s) = v {
                    allowed_read_paths.push(s.to_string());
                }
            }
        }
        if let Some(Value::Array(paths)) = borrowed.entries.get("_allowedWritePaths") {
            for v in &paths.borrow().elements {
                if let Value::String(s) = v {
                    allowed_write_paths.push(s.to_string());
                }
            }
        }
    } else {
        return Err(RuntimeError::new(
            "Sandbox.run called on non-sandbox object",
        ));
    }

    let config = SandboxConfig {
        allow_io,
        allow_network,
        max_memory,
        max_instructions,
        allowed_read_paths,
        allowed_write_paths,
    };

    // Create Sandbox using VMProvider trait
    let mut sandbox_vm = vm.create_sandbox(config)?;

    // Run the function in the sandbox
    // Note: We are passing 'func' (from parent VM) to child VM.
    // If 'func' is a closure with upvalues, they point to parent VM heap.
    // This is allowed in v1 but implies shared state.

    // A limit is fatal inside the sandbox, so the sandboxed code cannot catch
    // it; the program that started the sandbox gets an error it can catch.
    sandbox_vm.call(func.clone(), Vec::new()).map_err(|err| {
        if err.fatal {
            RuntimeError::new(err.message)
        } else {
            err
        }
    })
}

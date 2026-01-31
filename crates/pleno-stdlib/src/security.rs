use pleno_value::{MapState, RuntimeError, SandboxConfig, VMProvider, Value};
use std::collections::HashMap;

pub fn sandbox_new(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let config_val = args.first().cloned();

    let mut config = SandboxConfig::default();

    if let Some(Value::Map(map)) = config_val {
        let borrowed = map.borrow();
        if let Some(Value::Bool(allow)) = borrowed.entries.get("allowIO") {
            // eprintln!("DEBUG: Sandbox config allowIO: {}", allow);
            config.allow_io = *allow;
        }
        if let Some(Value::Bool(allow)) = borrowed.entries.get("allowNetwork") {
            config.allow_network = *allow;
        }
        if let Some(Value::Int(limit)) = borrowed.entries.get("maxMemory") {
            config.max_memory = *limit as usize;
        }
    }

    // Create the sandbox object
    let mut sandbox_data = HashMap::new();
    sandbox_data.insert("_allowIO".to_string(), Value::Bool(config.allow_io));
    sandbox_data.insert(
        "_allowNetwork".to_string(),
        Value::Bool(config.allow_network),
    );
    sandbox_data.insert(
        "_maxMemory".to_string(),
        Value::Int(config.max_memory as i64),
    );

    let gc = vm.get_gc();
    let sandbox_map = Value::Map(gc.allocate(MapState::new(sandbox_data))?);

    let run_method = Value::BoundNativeMethod {
        receiver: Box::new(sandbox_map.clone()),
        function: sandbox_run,
    };

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
    let mut max_memory = 0;

    if let Value::Map(map) = receiver {
        let borrowed = map.borrow();
        if let Some(Value::Bool(v)) = borrowed.entries.get("_allowIO") {
            allow_io = *v;
        }
        if let Some(Value::Int(v)) = borrowed.entries.get("_maxMemory") {
            max_memory = *v as usize;
        }
    } else {
        return Err(RuntimeError::new(
            "Sandbox.run called on non-sandbox object",
        ));
    }

    let config = SandboxConfig {
        allow_io,
        allow_network: false,
        max_memory,
    };

    // Create Sandbox using VMProvider trait
    let mut sandbox_vm = vm.create_sandbox(config)?;

    // Run the function in the sandbox
    // Note: We are passing 'func' (from parent VM) to child VM.
    // If 'func' is a closure with upvalues, they point to parent VM heap.
    // This is allowed in v1 but implies shared state.

    sandbox_vm.call(func.clone(), Vec::new())
}

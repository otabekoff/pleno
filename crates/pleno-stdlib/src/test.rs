use pleno_value::{RuntimeError, VMProvider, Value};

// Basic test runner state (global for now, in a real implementation this might be per-VM context)
// For this MVP, we'll just print to stdout

pub fn test_describe(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("describe expects 2 arguments: name, fn"));
    }
    let name = args[0]
        .as_str()
        .ok_or_else(|| RuntimeError::new("describe name must be string"))?;

    println!("\nSuite: {}", name);

    // Execute the callback function
    vm.call(args[1].clone(), vec![])?;

    Ok(Value::Null)
}

pub fn test_test(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("test expects 2 arguments: name, fn"));
    }
    let name = args[0]
        .as_str()
        .ok_or_else(|| RuntimeError::new("test name must be string"))?;

    // print!("  - {}", name);

    // Execute the test function
    match vm.call(args[1].clone(), vec![]) {
        Ok(_) => {
            println!("  ✓ {}", name);
        }
        Err(e) => {
            println!("  ✗ {} (FAILED)", name);
            println!("    Error: {}", e.message);
        }
    }

    Ok(Value::Null)
}

pub fn test_before_each(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // TODO: persist this hook for the current suite
    Ok(Value::Null)
}

pub fn test_after_each(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // TODO: persist this hook for the current suite
    Ok(Value::Null)
}

// Assertions

pub fn assert_equal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("assert.equal expects 2 arguments"));
    }
    // Simple equality check (this might need deep equality for objects/arrays)
    // For MVP, we use the Debug representation or simple partial eq if possible,
    // but Value doesn't implement PartialEq fully in a way that matches JS semantics perfectly yet.
    // Let's assume we rely on the Values being structurally equal if they are simple types.

    // Checking equality via string representation is a cheap MVP hack,
    // real implementation should use a deep_equal helper.
    let actual = format!("{:?}", args[0]);
    let expected = format!("{:?}", args[1]);

    if actual != expected {
        return Err(RuntimeError::new(format!(
            "Assertion failed: expected {}, got {}",
            expected, actual
        )));
    }
    Ok(Value::Null)
}

pub fn assert_not_equal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("assert.notEqual expects 2 arguments"));
    }
    let actual = format!("{:?}", args[0]);
    let expected = format!("{:?}", args[1]);

    if actual == expected {
        return Err(RuntimeError::new(format!(
            "Assertion failed: expected {} to not equal {}",
            actual, expected
        )));
    }
    Ok(Value::Null)
}

pub fn assert_true(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.true expects 1 argument"));
    }
    match args[0] {
        Value::Bool(true) => Ok(Value::Null),
        _ => Err(RuntimeError::new(format!(
            "Assertion failed: expected true, got {:?}",
            args[0]
        ))),
    }
}

pub fn assert_false(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.false expects 1 argument"));
    }
    match args[0] {
        Value::Bool(false) => Ok(Value::Null),
        _ => Err(RuntimeError::new(format!(
            "Assertion failed: expected false, got {:?}",
            args[0]
        ))),
    }
}

pub fn assert_throws(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.throws expects 1 argument"));
    }

    match vm.call(args[0].clone(), vec![]) {
        Ok(_) => Err(RuntimeError::new(
            "Assertion failed: expected function to throw",
        )),
        Err(_) => Ok(Value::Null), // It threw, so assertion passes
    }
}

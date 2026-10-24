use pleno_common::style::{self, Stream};
use pleno_value::{RuntimeError, VMProvider, Value};
use std::cell::RefCell;

thread_local! {
    static CONTEXT: RefCell<TestContext> = RefCell::new(TestContext::default());
}

#[derive(Default)]
struct TestContext {
    suites: Vec<SuiteState>,
    /// Tests that failed since the last `take_failures`.
    failures: usize,
}

/// The number of `test(...)` calls that failed since the last call, which a
/// runner uses to fail the file: a failing test does not stop the script.
pub fn take_failures() -> usize {
    CONTEXT.with(|ctx| std::mem::take(&mut ctx.borrow_mut().failures))
}

#[derive(Default)]
struct SuiteState {
    _name: String,
    before_each: Vec<Value>,
    after_each: Vec<Value>,
}

pub fn test_describe(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("describe expects 2 arguments: name, fn"));
    }
    let name = args[0]
        .as_str()
        .ok_or_else(|| RuntimeError::new("describe name must be string"))?;

    let depth = CONTEXT.with(|ctx| ctx.borrow().suites.len());
    let indent = "  ".repeat(depth);
    println!("{}{}", indent, name);

    CONTEXT.with(|ctx| {
        ctx.borrow_mut().suites.push(SuiteState {
            _name: name.to_string(),
            ..Default::default()
        });
    });

    // Execute the callback function
    let res = vm.call(args[1].clone(), vec![]);

    CONTEXT.with(|ctx| {
        ctx.borrow_mut().suites.pop();
    });

    res?;

    Ok(Value::Null)
}

pub fn test_test(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("test expects 2 arguments: name, fn"));
    }
    let name = args[0]
        .as_str()
        .ok_or_else(|| RuntimeError::new("test name must be string"))?;

    let hooks = CONTEXT.with(|ctx| {
        let ctx = ctx.borrow();
        let be = ctx
            .suites
            .iter()
            .flat_map(|s| s.before_each.iter().cloned())
            .collect::<Vec<_>>();
        let ae = ctx
            .suites
            .iter()
            .rev()
            .flat_map(|s| s.after_each.iter().cloned())
            .collect::<Vec<_>>();
        (be, ae)
    });

    let depth = CONTEXT.with(|ctx| ctx.borrow().suites.len());
    let indent = "  ".repeat(depth);

    // Run before_each hooks
    for hook in hooks.0 {
        vm.call(hook, vec![])?;
    }

    // Execute the test function
    let result = match vm.call(args[1].clone(), vec![]) {
        Ok(_) => {
            println!(
                "{}{} {}",
                indent,
                style::green(Stream::Stdout, "\u{2713}"),
                name
            );
            Ok(Value::Null)
        }
        Err(e) => {
            // Other tests still run; the runner fails the file at the end.
            CONTEXT.with(|ctx| ctx.borrow_mut().failures += 1);
            println!(
                "{}{} {}",
                indent,
                style::red(Stream::Stdout, "\u{2717}"),
                name
            );
            let message = e
                .message
                .strip_prefix("uncaught exception: ")
                .unwrap_or(&e.message);
            println!("{}  {}", indent, message);
            Ok(Value::Null)
        }
    };

    // Run after_each hooks
    for hook in hooks.1 {
        // Even if a hook fails, we should try to run others?
        // For simplicity, if a hook fails, we abort the current test cleanup.
        vm.call(hook, vec![])?;
    }

    result
}

pub fn test_before_each(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("beforeEach expects 1 argument: fn"));
    }
    CONTEXT.with(|ctx| {
        if let Some(suite) = ctx.borrow_mut().suites.last_mut() {
            suite.before_each.push(args[0].clone());
        }
    });
    Ok(Value::Null)
}

pub fn test_after_each(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("afterEach expects 1 argument: fn"));
    }
    CONTEXT.with(|ctx| {
        if let Some(suite) = ctx.borrow_mut().suites.last_mut() {
            suite.after_each.push(args[0].clone());
        }
    });
    Ok(Value::Null)
}

// Assertions

pub fn assert_equal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("assert.equal expects 2 arguments"));
    }
    if !deep_equal(&args[0], &args[1]) {
        return Err(RuntimeError::new(format!(
            "Assertion failed: expected equal, got {} and {}",
            args[0], args[1]
        )));
    }
    Ok(Value::Null)
}

pub fn assert_not_equal(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new("assert.notEqual expects 2 arguments"));
    }
    if deep_equal(&args[0], &args[1]) {
        return Err(RuntimeError::new(format!(
            "Assertion failed: expected not equal, got {} and {}",
            args[0], args[1]
        )));
    }
    Ok(Value::Null)
}

pub fn assert_true(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.true expects 1 argument"));
    }
    if let Value::Bool(true) = args[0] {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::new(format!(
            "Assertion failed: expected true, got {}",
            args[0]
        )))
    }
}

pub fn assert_false(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.false expects 1 argument"));
    }
    if let Value::Bool(false) = args[0] {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::new(format!(
            "Assertion failed: expected false, got {}",
            args[0]
        )))
    }
}

pub fn assert_null(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.null expects 1 argument"));
    }
    if let Value::Null = args[0] {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::new(format!(
            "Assertion failed: expected null, got {}",
            args[0]
        )))
    }
}

pub fn assert_not_null(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::new("assert.notNull expects 1 argument"));
    }
    if let Value::Null = args[0] {
        Err(RuntimeError::new("Assertion failed: expected not null"))
    } else {
        Ok(Value::Null)
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
        Err(_) => Ok(Value::Null),
    }
}

pub fn assert_contains(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "assert.contains expects 2 arguments: collection, item",
        ));
    }
    let collection = &args[0];
    let item = &args[1];

    let found = match collection {
        Value::String(s) => {
            let item_s = item.as_str().ok_or_else(|| {
                RuntimeError::new("assert.contains item must be string for string collection")
            })?;
            s.contains(item_s)
        }
        Value::Array(a) => a.borrow().iter().any(|v| deep_equal(v, item)),
        Value::Map(m) => {
            let m = m.borrow();
            if m.entries.contains_key(&format!("{:?}", item)) {
                true
            } else if let Some(s) = item.as_str() {
                m.entries.contains_key(s)
            } else {
                false
            }
        }
        _ => {
            return Err(RuntimeError::new(
                "assert.contains expects string, array, or map",
            ));
        }
    };

    if !found {
        Err(RuntimeError::new(format!(
            "Assertion failed: collection does not contain {}",
            item
        )))
    } else {
        Ok(Value::Null)
    }
}

pub fn assert_length_of(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(RuntimeError::new(
            "assert.lengthOf expects 2 arguments: collection, length",
        ));
    }
    let len_expected = args[1]
        .as_int()
        .ok_or_else(|| RuntimeError::new("assert.lengthOf length must be integer"))?
        as usize;

    let actual_len = match &args[0] {
        Value::String(s) => s.len(),
        Value::Array(a) => a.borrow().len(),
        Value::Map(m) => m.borrow().entries.len(),
        _ => {
            return Err(RuntimeError::new(
                "assert.lengthOf expects string, array, or map",
            ));
        }
    };

    if actual_len != len_expected {
        Err(RuntimeError::new(format!(
            "Assertion failed: expected length {}, got {}",
            len_expected, actual_len
        )))
    } else {
        Ok(Value::Null)
    }
}

fn deep_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Char(a), Value::Char(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            if a.len() != b.len() {
                return false;
            }
            a.iter().zip(b.iter()).all(|(v1, v2)| deep_equal(v1, v2))
        }
        (Value::Map(a), Value::Map(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            if a.entries.len() != b.entries.len() {
                return false;
            }
            // Simple check: compare stringified keys and values
            // This is still an MVP level check but better than before
            for (k, v) in &a.entries {
                if let Some(v2) = b.entries.get(k) {
                    if !deep_equal(v, v2) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        _ => format!("{:?}", a) == format!("{:?}", b), // Fallback for complex types
    }
}

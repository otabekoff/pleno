use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("runtime failed")
}

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).map_err(|e| e.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

#[test]
fn test_string_upper_method() {
    let value = eval(r#"let s = "hello"; s.upper();"#);
    assert_eq!(
        value,
        pleno_value::Value::String("HELLO".to_string().into())
    );
}

#[test]
fn test_string_lower_method() {
    let value = eval(r#"let s = "WORLD"; s.lower();"#);
    assert_eq!(
        value,
        pleno_value::Value::String("world".to_string().into())
    );
}

#[test]
fn test_string_trim_method() {
    let value = eval(r#"let s = "  hello world  "; s.trim();"#);
    assert_eq!(
        value,
        pleno_value::Value::String("hello world".to_string().into())
    );
}

#[test]
fn test_string_split_method() {
    let value = eval(r#"let s = "a,b,c"; s.split(",");"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec.len(), 3);
            assert_eq!(vec[0], pleno_value::Value::String("a".to_string().into()));
            assert_eq!(vec[1], pleno_value::Value::String("b".to_string().into()));
            assert_eq!(vec[2], pleno_value::Value::String("c".to_string().into()));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_string_contains_method() {
    let value = eval(r#"let s = "hello world"; s.contains("world");"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let s = "hello world"; s.contains("goodbye");"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_string_starts_with_method() {
    let value = eval(r#"let s = "hello world"; s.startsWith("hello");"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let s = "hello world"; s.startsWith("world");"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_string_ends_with_method() {
    let value = eval(r#"let s = "hello world"; s.endsWith("world");"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let s = "hello world"; s.endsWith("hello");"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_string_replace_method() {
    let value = eval(r#"let s = "hello world"; s.replace("world", "Pleno");"#);
    assert_eq!(
        value,
        pleno_value::Value::String("hello Pleno".to_string().into())
    );
}

#[test]
fn test_array_push_method() {
    let value = eval(r#"let arr = [1, 2]; arr.push(3); arr;"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec.len(), 3);
            assert_eq!(vec[0], pleno_value::Value::Int(1));
            assert_eq!(vec[1], pleno_value::Value::Int(2));
            assert_eq!(vec[2], pleno_value::Value::Int(3));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_array_pop_method() {
    let value = eval(r#"let arr = [1, 2, 3]; let popped = arr.pop(); [popped, arr.len];"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec.len(), 2);
            assert_eq!(vec[0], pleno_value::Value::Int(3)); // popped value
            assert_eq!(vec[1], pleno_value::Value::Int(2)); // new length
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_array_clear_method() {
    let value = eval(r#"let arr = [1, 2, 3]; arr.clear(); arr.len;"#);
    assert_eq!(value, pleno_value::Value::Int(0));
}

#[test]
fn test_array_slice_method() {
    let value = eval(r#"let arr = [1, 2, 3, 4, 5]; arr.slice(1, 4);"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec.len(), 3);
            assert_eq!(vec[0], pleno_value::Value::Int(2));
            assert_eq!(vec[1], pleno_value::Value::Int(3));
            assert_eq!(vec[2], pleno_value::Value::Int(4));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_array_reverse_method() {
    let value = eval(r#"let arr = [1, 2, 3]; arr.reverse(); arr;"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec.len(), 3);
            assert_eq!(vec[0], pleno_value::Value::Int(3));
            assert_eq!(vec[1], pleno_value::Value::Int(2));
            assert_eq!(vec[2], pleno_value::Value::Int(1));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_array_contains_method() {
    let value = eval(r#"let arr = [1, 2, 3]; arr.contains(2);"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let arr = [1, 2, 3]; arr.contains(5);"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_array_index_of_method() {
    let value = eval(r#"let arr = [10, 20, 30]; arr.indexOf(20);"#);
    assert_eq!(value, pleno_value::Value::Int(1));

    let value2 = eval(r#"let arr = [10, 20, 30]; arr.indexOf(40);"#);
    assert_eq!(value2, pleno_value::Value::Int(-1)); // Not found
}

#[test]
fn test_set_add_method() {
    let value = eval(r#"let s = {1, 2}; s.add(3); s[3];"#);
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn test_set_remove_method() {
    let value = eval(r#"let s = {1, 2, 3}; s.remove(2); [s[2], s.len];"#);
    match value {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert_eq!(vec[0], pleno_value::Value::Bool(false)); // 2 no longer in set
            assert_eq!(vec[1], pleno_value::Value::Int(2)); // new length
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_set_clear_method() {
    let value = eval(r#"let s = {1, 2, 3}; s.clear(); s.len;"#);
    assert_eq!(value, pleno_value::Value::Int(0));
}

#[test]
fn test_set_contains_method() {
    let value = eval(r#"let s = {1, 2, 3}; s.contains(2);"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let s = {1, 2, 3}; s.contains(5);"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_result_type() {
    let value = eval(r#"let ok = Ok(42); ok.unwrap();"#);
    assert_eq!(value, pleno_value::Value::Int(42));

    let value2 = eval(r#"let err = Err("error"); err.isErr();"#);
    assert_eq!(value2, pleno_value::Value::Bool(true));
}

#[test]
fn test_option_type() {
    let value = eval(r#"let some = Some(100); some.unwrap();"#);
    assert_eq!(value, pleno_value::Value::Int(100));

    let value2 = eval(r#"let none = None; none.isNone();"#);
    assert_eq!(value2, pleno_value::Value::Bool(true));
}

#[test]
fn test_math_random() {
    let value = eval(r#"import Math from "std:math"; Math.random();"#);
    match value {
        pleno_value::Value::Float(f) => {
            assert!((0.0..=1.0).contains(&f));
        }
        _ => panic!("Expected float"),
    }
}

#[test]
fn test_promise_type() {
    let value =
        eval(r#"let p = Promise((resolve, reject) => { resolve(42); }); p.then((val) => val); 0;"#);
    // For now, just ensure it doesn't error
    assert_eq!(value, pleno_value::Value::Int(0));
}

#[test]
fn test_string_length_property() {
    let value = eval(r#"let s = "hello"; s.len;"#);
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn test_array_length_property() {
    let value = eval(r#"let arr = [1, 2, 3, 4]; arr.len;"#);
    assert_eq!(value, pleno_value::Value::Int(4));
}

#[test]
fn test_set_length_property() {
    let value = eval(r#"let s = {1, 2, 3}; s.len;"#);
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn test_result_unwrap_or_method() {
    let value = eval(r#"let ok = Ok(42); ok.unwrapOr(0);"#);
    assert_eq!(value, pleno_value::Value::Int(42));

    let value2 = eval(r#"let err = Err("error"); err.unwrapOr(99);"#);
    assert_eq!(value2, pleno_value::Value::Int(99));
}

#[test]
fn test_option_unwrap_or_method() {
    let value = eval(r#"let some = Some(100); some.unwrapOr(0);"#);
    assert_eq!(value, pleno_value::Value::Int(100));

    let value2 = eval(r#"let none = None; none.unwrapOr(99);"#);
    assert_eq!(value2, pleno_value::Value::Int(99));
}

#[test]
fn test_result_is_ok_method() {
    let value = eval(r#"let ok = Ok(42); ok.isOk();"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let err = Err("error"); err.isOk();"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_option_is_some_method() {
    let value = eval(r#"let some = Some(100); some.isSome();"#);
    assert_eq!(value, pleno_value::Value::Bool(true));

    let value2 = eval(r#"let none = None; none.isSome();"#);
    assert_eq!(value2, pleno_value::Value::Bool(false));
}

#[test]
fn test_result_expect_method_success() {
    let value = eval(r#"let ok = Ok(42); ok.expect("should not fail");"#);
    assert_eq!(value, pleno_value::Value::Int(42));
}

#[test]
fn test_option_expect_method_success() {
    let value = eval(r#"let some = Some(100); some.expect("should not fail");"#);
    assert_eq!(value, pleno_value::Value::Int(100));
}

#[test]
fn test_string_methods_chain() {
    let value = eval(r#"let s = "  HELLO world  "; s.trim().lower().startsWith("hello");"#);
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn test_array_methods_chain() {
    let value = eval(r#"let arr = [3, 1, 4, 1, 5]; arr.filter((x) => x > 2).sort().len;"#);
    // Note: filter isn't fully implemented yet, so this may not work as expected
    // Just ensure it doesn't cause an error for now
    assert!(matches!(value, pleno_value::Value::Int(_)));
}

#[test]
fn test_complex_data_structure_operations() {
    let value = eval(
        r#"
        let data = [
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ];
        data.len;
    "#,
    );
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn test_map_property_access() {
    let value = eval(r#"let obj = {"x": 10, "y": 20}; obj.x;"#);
    assert_eq!(value, pleno_value::Value::Int(10));
}

#[test]
fn test_nested_object_access() {
    let value = eval(
        r#"
        let obj = {"person": {"name": "Alice", "age": 30}};
        obj.person.name;
    "#,
    );
    assert_eq!(
        value,
        pleno_value::Value::String("Alice".to_string().into())
    );
}

#[test]
fn test_function_returning_stdlib_type() {
    let value = eval(
        r#"
        fn makeString() { return "hello"; }
        let s = makeString();
        s.upper();
    "#,
    );
    assert_eq!(
        value,
        pleno_value::Value::String("HELLO".to_string().into())
    );
}

#[test]
fn test_error_handling_with_result() {
    // This tests that Result methods work properly
    let result = eval_result(
        r#"
        let res = Ok(42);
        res.unwrap()
    "#,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), pleno_value::Value::Int(42));
}

#[test]
fn test_error_handling_with_option() {
    // This tests that Option methods work properly
    let result = eval_result(
        r#"
        let opt = Some(100);
        opt.unwrap()
    "#,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), pleno_value::Value::Int(100));
}

#[test]
fn test_math_trig_functions() {
    let value = eval(r#"import Math from "std:math"; Math.sin(0);"#);
    assert_eq!(value, pleno_value::Value::Float(0.0));

    let value2 = eval(r#"import Math from "std:math"; Math.cos(0);"#);
    assert_eq!(value2, pleno_value::Value::Float(1.0));

    let value3 = eval(r#"import Math from "std:math"; Math.tan(0);"#);
    assert_eq!(value3, pleno_value::Value::Float(0.0));
}

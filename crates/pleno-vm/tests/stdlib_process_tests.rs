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

#[test]
fn test_process_cwd() {
    let result = eval(
        r#"
        import Process from "std:process";
        Process.cwd();
    "#,
    );

    // Should be a string path
    assert!(result.as_str().is_some());
    // Should contain current dir component (e.g. project name or repo name)
    // Hard to check exact value but we can check it's not empty
    assert!(!result.as_str().unwrap().is_empty());
}

#[test]
fn test_process_pid() {
    let result = eval(
        r#"
        import Process from "std:process";
        Process.pid();
    "#,
    );

    // Should be an integer
    assert!(result.as_int().is_some());
    assert!(result.as_int().unwrap() > 0);
}

#[test]
fn test_process_args() {
    let result = eval(
        r#"
        import Process from "std:process";
        Process.args();
    "#,
    );

    // Should be an array of strings
    match result {
        pleno_value::Value::Array(arr) => {
            let vec = arr.borrow();
            assert!(!vec.is_empty()); // At least the binary name
            assert!(vec[0].as_str().is_some());
        }
        _ => panic!("Expected array of strings"),
    }
}

#[test]
fn test_process_env() {
    // Set a test variable
    unsafe {
        std::env::set_var("PLENO_TEST_ENV", "test_value");
    }

    let result = eval(
        r#"
        import Process from "std:process";
        Process.env("PLENO_TEST_ENV");
    "#,
    );

    assert_eq!(result.as_str(), Some("test_value"));
}

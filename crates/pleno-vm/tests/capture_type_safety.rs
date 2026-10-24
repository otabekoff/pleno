use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

#[test]
fn test_capture_reassignment_type_error() {
    let source = r#"
        let x: Int = 1;
        fn mutate() {
            x = "string";
        }
        mutate();
    "#;
    let err = eval_result(source).unwrap_err();
    assert!(
        err.contains("Type error") || err.contains("expected Int"),
        "Expected type error, got: {}",
        err
    );
}

#[test]
fn test_capture_reassignment_valid() {
    let source = r#"
        let x: Int = 1;
        fn mutate() {
            x = 2;
        }
        mutate();
        x;
    "#;
    let result = eval_result(source).unwrap();
    assert_eq!(result, pleno_value::Value::Int(2));
}

#[test]
fn test_capture_nested_reassignment_type_error() {
    let source = r#"
        let x: Int = 1;
        fn outer() {
            fn inner() {
                x = "string";
            }
            inner();
        }
        outer();
    "#;
    let err = eval_result(source).unwrap_err();
    assert!(
        err.contains("Type error") || err.contains("expected Int"),
        "Expected type error, got: {}",
        err
    );
}

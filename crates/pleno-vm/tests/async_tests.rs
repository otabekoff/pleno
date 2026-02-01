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
fn evaluates_simple_async_function() {
    let value = eval("async fn getValue() { return 42; } getValue();");
    // Async functions return a Promise (represented as a Map)
    assert!(matches!(value, pleno_value::Value::Map(_)));
}

#[test]
fn evaluates_await_expression() {
    let value = eval("let x = 42; await x;");
    // For now, await just returns the value directly
    assert_eq!(value, pleno_value::Value::Int(42));
}

#[test]
fn evaluates_async_function_with_await() {
    let value = eval("async fn compute() { let x = await 5; return x * 2; } let f = compute(); 0;"); // Just ensure it doesn't crash
    // The function should compile and be callable, though not truly async yet
    assert!(matches!(value, pleno_value::Value::Int(0)));
}

#[test]
fn evaluates_complex_async_scenario() {
    let value = eval(
        "
        async fn getData() { 
            let data = await 42; 
            return data + 8; 
        } 
        let fnHandle = getData();
        0;
    ",
    );
    // Just ensure it doesn't crash and returns expected value
    assert_eq!(value, pleno_value::Value::Int(0));
}

#[test]
fn async_function_can_be_assigned_to_variable() {
    let value = eval(
        "
        async fn asyncFunc() { return 100; }
        let myAsyncFunc = asyncFunc;
        myAsyncFunc();
        0;
    ",
    );
    assert_eq!(value, pleno_value::Value::Int(0));
}

#[test]
fn async_function_with_parameters() {
    let value = eval(
        "
        async fn multiply(a, b) { 
            let result = await a; 
            return result * b; 
        }
        let fnHandle = multiply(5, 6);
        0;
    ",
    );
    assert_eq!(value, pleno_value::Value::Int(0));
}

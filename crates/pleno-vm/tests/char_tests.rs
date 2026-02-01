use pleno_compiler::Compiler;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;
use std::rc::Rc;

fn run_code(code: &str) -> Value {
    let tokens = pleno_lexer::lex(code).expect("lex failed");
    let mut parser = Parser::new(tokens, code);
    let program = parser.parse_program().expect("Failed to parse");
    let mut compiler = Compiler::new(code.to_string());
    let chunk = compiler.compile(&program).expect("Failed to compile");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);

    vm.execute(chunk).expect("Failed to run")
}

#[test]
fn test_char_literal() {
    let result = run_code("'A'");
    assert_eq!(result, Value::Char('A'));
}

#[test]
fn test_char_equality() {
    let result = run_code("'A' == 'A'");
    assert_eq!(result, Value::Bool(true));

    let result = run_code("'A' == 'B'");
    assert_eq!(result, Value::Bool(false));

    let result = run_code("'A' != 'B'");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_char_string_concat() {
    let result = run_code("'H' + \"ello\"");
    assert_eq!(result, Value::String(Rc::new("Hello".to_string())));

    let result = run_code("\"Hell\" + 'o'");
    assert_eq!(result, Value::String(Rc::new("Hello".to_string())));

    let result = run_code("'A' + 'B'");
    assert_eq!(result, Value::String(Rc::new("AB".to_string())));
}

#[test]
fn test_char_in_array() {
    let result = run_code("['A', 'B', 'C'][1]");
    assert_eq!(result, Value::Char('B'));
}

#[test]
fn test_char_in_set() {
    let result = run_code("({'A', 'B'}).contains('A')");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_char_type_reflection() {
    let result = run_code("import Reflect from \"std:reflect\"; Reflect.getTypeName('A')");
    assert_eq!(result, Value::String(Rc::new("Char".to_string())));
}

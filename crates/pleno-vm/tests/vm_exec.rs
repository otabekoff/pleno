use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    vm.execute(chunk).expect("runtime failed")
}

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&program).map_err(|e| e.message)?;
    let mut vm = VM::new();
    vm.execute(chunk).map_err(|e| e.message)
}

#[test]
fn evaluates_arithmetic() {
    let value = eval("1 + 2 * 3;");
    assert_eq!(value, pleno_value::Value::Int(7));
}

#[test]
fn evaluates_unary() {
    let value = eval("-(4 + 1);");
    assert_eq!(value, pleno_value::Value::Int(-5));
}

#[test]
fn evaluates_comparisons() {
    let value = eval("1 < 2;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_if_else() {
    let value = eval("if (1 < 2) { 10; } else { 20; } 30;");
    assert_eq!(value, pleno_value::Value::Int(30));
}

#[test]
fn evaluates_while_loop() {
    let value = eval("while (false) { 1; } 5;");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_logical_ops() {
    let value = eval("true && false; false || true;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_nested_blocks() {
    let value = eval("{ { 1; } 2; }");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_chained_comparisons() {
    let value = eval("1 < 2 && 2 < 3;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_mutating_while() {
    let value = eval("let mut x = 0; while (x < 3) { x = x + 1; } x;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_for_range() {
    let value = eval("let mut sum = 0; for (i in 0..=3) { sum = sum + i; } sum;");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_for_array() {
    let value = eval("let mut sum = 0; for (x in [1, 2, 3]) { sum = sum + x; } sum;");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_for_map_keys() {
    let value = eval("let mut count = 0; for (k in {\"a\": 1, \"b\": 2}) { count = count + 1; } count;");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_for_set() {
    let value = eval("let mut count = 0; for (x in {1, 2, 3}) { count = count + 1; } count;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_range_expression() {
    let value = eval("1..=3;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: Some(1),
            end: Some(3),
            inclusive: true
        })
    );
}

#[test]
fn evaluates_open_ended_ranges() {
    let value = eval("..10;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: None,
            end: Some(10),
            inclusive: false
        })
    );
    let value = eval("5..;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: Some(5),
            end: None,
            inclusive: false
        })
    );
}

#[test]
fn evaluates_map_property_access() {
    let value = eval("let m = {\"a\": 1, \"b\": 2}; m.a;");
    assert_eq!(value, pleno_value::Value::Int(1));
}

#[test]
fn property_access_on_non_map_errors() {
    let err = eval_result("let a = 1; a.b;").unwrap_err();
    assert!(err.contains("non-map"));
}

#[test]
fn evaluates_array_literal_and_index() {
    let value = eval("let a = [1, 2, 3]; a[1];");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_map_index() {
    let value = eval("let m = {\"x\": 10}; m[\"x\"];");
    assert_eq!(value, pleno_value::Value::Int(10));
}

#[test]
fn evaluates_set_literal_and_has() {
    let value = eval("let s = {1, 2}; s[1];");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

use pleno_compiler::Compiler;

fn compile(source: &str) -> Result<(), String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new();
    compiler.compile(&program).map(|_| ()).map_err(|e| e.message)
}

#[test]
fn const_requires_initializer() {
    let err = compile("const x;").unwrap_err();
    assert!(err.contains("initializer"));
}

#[test]
fn immutable_assignment_fails() {
    let err = compile("let x = 1; x = 2;").unwrap_err();
    assert!(err.contains("immutable"));
}

#[test]
fn for_requires_range_expression() {
    let err = compile("for (i in 10) { i; }").unwrap_err();
    assert!(err.contains("expects"));
}

#[test]
fn for_requires_bounded_range() {
    let err = compile("for (i in ..) { i; }").unwrap_err();
    assert!(err.contains("start or end"));
}

#[test]
fn for_rejects_non_array_iterable() {
    let err = compile("for (i in 42) { i; }").unwrap_err();
    assert!(err.contains("expects"));
}

#[test]
fn break_outside_loop_errors() {
    let err = compile("break;").unwrap_err();
    assert!(err.contains("break used outside"));
}

#[test]
fn continue_outside_loop_errors() {
    let err = compile("continue;").unwrap_err();
    assert!(err.contains("continue used outside"));
}

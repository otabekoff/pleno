use pleno_lexer::lex;
use pleno_parser::Parser;

#[test]
fn test_regression_deep_type_nesting() {
    // Failing input: type I:(
    let source = "type I:(";
    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let _ = parser.parse_program();
}

#[test]
fn test_regression_nested_tuples_crash() {
    // Failing byte sequence: [40, 114, 44, 40, 114, 44, 40, 74, 58, 114, 44, 9, 93]
    let bytes = [40, 114, 44, 40, 114, 44, 40, 74, 58, 114, 44, 9, 93];
    let source = std::str::from_utf8(&bytes).unwrap();
    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let _ = parser.parse_program();
}

#[test]
fn test_recursion_limit_expr() {
    let source = "(".repeat(50) + &")".repeat(50);
    let tokens = lex(&source).unwrap();
    let mut parser = Parser::new(tokens, &source);
    let result = parser.parse_program();
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .message
            .contains("Recursion limit exceeded")
    );
}

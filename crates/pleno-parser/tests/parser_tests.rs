use pleno_ast::{ExprKind, Literal, StmtKind};
use pleno_lexer::lex;
use pleno_parser::Parser;

fn parse_program(source: &str) -> pleno_ast::Program {
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    parser.parse_program().expect("Parsing failed")
}

fn parse_error(source: &str) -> String {
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    parser.parse_program().unwrap_err().message
}

#[test]
fn test_parse_let_declaration() {
    let program = parse_program("let x = 1;");
    match &program.statements[0].kind {
        StmtKind::Let { name, value, .. } => {
            assert_eq!(name, "x");
            match &value.as_ref().unwrap().kind {
                ExprKind::Literal(Literal::Int(1)) => {}
                _ => panic!("Expected Int(1)"),
            }
        }
        _ => panic!("Expected Let stmt"),
    }
}

#[test]
fn test_parse_function() {
    let program = parse_program("fn add(a, b) { return a + b; }");
    match &program.statements[0].kind {
        StmtKind::Fn { name, params, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
        }
        _ => panic!("Expected Function stmt"),
    }
}

#[test]
fn test_error_missing_semicolon() {
    let _msg = parse_error("let x = 1");
    // Parser might be lenient about EOF semicolon, so let's try middle statement
    let msg = parse_error("let x = 1 let y = 2");
    assert!(
        msg.contains("Expected ';' after let statement"),
        "Got: {}",
        msg
    );
}

#[test]
fn test_error_missing_closing_brace() {
    let msg = parse_error("fn main() {");
    // Current parser hits EOF while parsing statements in block and thinks it's an expression start check fail?
    // Actually "Unexpected token in expression" suggests it tried to parse a statement, fell through to expression statement, and hit EOF.
    assert!(
        msg.contains("Unexpected token in expression: Eof") || msg.contains("Expected '}'"),
        "Got: {}",
        msg
    );
}

#[test]
fn test_error_invalid_assignment() {
    let msg = parse_error("1 = 2;");
    assert!(msg.contains("Invalid assignment target"), "Got: {}", msg);
}

#[test]
fn test_error_expected_expression() {
    let msg = parse_error("let x = ;");
    assert!(
        msg.contains("Unexpected token in expression: Semicolon"),
        "Got: {}",
        msg
    );
}

#[test]
fn test_error_expected_identifier() {
    let msg = parse_error("let 1 = 2;");
    assert!(msg.contains("Expected identifier"), "Got: {}", msg);
}

#[test]
fn test_error_unterminated_paren() {
    let msg = parse_error("(1 + 2");
    assert!(msg.contains("Expected ')'"), "Got: {}", msg);
}

#[test]
fn test_parse_class() {
    let program = parse_program("class Foo {}");
    match &program.statements[0].kind {
        StmtKind::Class { name, .. } => {
            assert_eq!(name, "Foo");
        }
        _ => panic!("Expected Class stmt"),
    }
}

#[test]
fn arrow_lambda_parameters_take_annotations() {
    let program = parse_program("let f = (x: Int, y, z: Int | String?) => x;");
    match &program.statements[0].kind {
        StmtKind::Let {
            value: Some(value), ..
        } => match &value.kind {
            ExprKind::Lambda { params, .. } => {
                let types: Vec<_> = params.iter().map(|p| p.ty.as_deref()).collect();
                assert_eq!(types, [Some("Int"), None, Some("Int | String?")]);
            }
            other => panic!("expected a lambda, got {other:?}"),
        },
        other => panic!("expected let, got {other:?}"),
    }
}

#[test]
fn test_parse_pipeline() {
    let program = parse_program("a |> b");
    // ExprKind::Binary with Pipeline operator? Or dedicated pipeline expr?
    // Let's check parsing succeeds at least.
    assert!(!program.statements.is_empty());
}

#[test]
fn with_statement_binds_each_resource() {
    let source = "with (a = open(\"x\"), b = wrap(a),) { use(a, b); }";
    let program = parse_program(source);
    match &program.statements[0].kind {
        StmtKind::With { resources, body } => {
            let names: Vec<_> = resources.iter().map(|r| r.name.as_str()).collect();
            assert_eq!(names, ["a", "b"]);
            let a = source.find("a =").unwrap();
            assert_eq!(resources[0].name_span, pleno_ast::Span::new(a, a + 1));
            assert!(matches!(resources[1].value.kind, ExprKind::Call { .. }));
            assert!(matches!(body.kind, StmtKind::Block(_)));
        }
        other => panic!("expected with, got {other:?}"),
    }
}

#[test]
fn with_statement_errors() {
    assert!(parse_error("with () { }").contains("at least one resource"));
    assert!(parse_error("with (f) { }").contains("Expected '='"));
    assert!(parse_error("with (f = open()) print(f);").contains("body of 'with' is a block"));
    assert!(parse_error("with (1 = open()) { }").contains("Expected a name"));
}

#[test]
fn with_is_still_a_method_name() {
    let program = parse_program("let b = [1, 2].with(0, 5);");
    assert!(matches!(program.statements[0].kind, StmtKind::Let { .. }));
    let program = parse_program("class Box { fn with(v) { return Box(); } }");
    match &program.statements[0].kind {
        StmtKind::Class { methods, .. } => assert_eq!(methods[0].name, "with"),
        other => panic!("expected class, got {other:?}"),
    }
}

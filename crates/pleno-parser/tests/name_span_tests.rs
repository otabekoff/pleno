use pleno_ast::StmtKind;
use pleno_lexer::lex;
use pleno_parser::Parser;

fn parse_program(source: &str) -> (pleno_ast::Program, String) {
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    (
        parser.parse_program().expect("Parsing failed"),
        source.to_string(),
    )
}

fn check_name_span(source: &str, span: pleno_ast::Span, expected_name: &str) {
    assert!(
        span.start < span.end,
        "Span {:?} is invalid (start >= end)",
        span
    );
    assert!(
        span.end <= source.len(),
        "Span {:?} out of bounds (len {})",
        span,
        source.len()
    );
    let actual_name = &source[span.start..span.end];
    assert_eq!(actual_name, expected_name, "Name span content mismatch");
}

#[test]
fn test_let_name_span() {
    let (program, source) = parse_program("let my_var = 10;");
    match &program.statements[0].kind {
        StmtKind::Let {
            name, name_span, ..
        } => {
            assert_eq!(name, "my_var");
            check_name_span(&source, *name_span, "my_var");
        }
        _ => panic!("Expected Let"),
    }
}

#[test]
fn test_fn_name_span() {
    let (program, source) = parse_program("fn my_func() {}");
    match &program.statements[0].kind {
        StmtKind::Fn {
            name, name_span, ..
        } => {
            assert_eq!(name, "my_func");
            check_name_span(&source, *name_span, "my_func");
        }
        _ => panic!("Expected Fn"),
    }
}

#[test]
fn test_class_name_span() {
    let (program, source) = parse_program("class MyClass {}");
    match &program.statements[0].kind {
        StmtKind::Class {
            name, name_span, ..
        } => {
            assert_eq!(name, "MyClass");
            check_name_span(&source, *name_span, "MyClass");
        }
        _ => panic!("Expected Class"),
    }
}

#[test]
fn test_interface_name_span() {
    let (program, source) = parse_program("interface MyInterface {}");
    match &program.statements[0].kind {
        StmtKind::Interface {
            name, name_span, ..
        } => {
            assert_eq!(name, "MyInterface");
            check_name_span(&source, *name_span, "MyInterface");
        }
        _ => panic!("Expected Interface"),
    }
}

#[test]
fn test_enum_name_span() {
    let (program, source) = parse_program("enum MyEnum { A, B }");
    match &program.statements[0].kind {
        StmtKind::Enum(decl) => {
            assert_eq!(decl.name, "MyEnum");
            check_name_span(&source, decl.name_span, "MyEnum");

            // Check variants
            assert_eq!(decl.variants[0].name, "A");
            check_name_span(&source, decl.variants[0].name_span, "A");

            assert_eq!(decl.variants[1].name, "B");
            check_name_span(&source, decl.variants[1].name_span, "B");
        }
        _ => panic!("Expected Enum"),
    }
}

#[test]
fn test_type_alias_name_span() {
    let (program, source) = parse_program("type MyType = Int;");
    match &program.statements[0].kind {
        StmtKind::TypeAlias {
            name, name_span, ..
        } => {
            assert_eq!(name, "MyType");
            check_name_span(&source, *name_span, "MyType");
        }
        _ => panic!("Expected TypeAlias"),
    }
}

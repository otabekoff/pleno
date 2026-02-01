use pleno_ast::StmtKind;
use pleno_parser::Parser;

fn parse(input: &str) -> pleno_ast::Program {
    let tokens = pleno_lexer::lex(input).unwrap();
    let mut parser = Parser::new(tokens, input);
    parser.parse_program().unwrap()
}

#[test]
fn test_generic_function() {
    let program = parse("fn identity<T>(x: T) { return x; }");
    let stmt = &program.statements[0];
    if let StmtKind::Fn { generics, .. } = &stmt.kind {
        assert_eq!(generics.len(), 1);
        assert_eq!(generics[0].name, "T");
        assert!(generics[0].constraints.is_empty());
    } else {
        panic!("Expected Fn statement");
    }
}

#[test]
fn test_generic_class() {
    let program = parse("class Box<T> { field: T; }");
    let stmt = &program.statements[0];
    if let StmtKind::Class { generics, .. } = &stmt.kind {
        assert_eq!(generics.len(), 1);
        assert_eq!(generics[0].name, "T");
        assert!(generics[0].constraints.is_empty());
    } else {
        panic!("Expected Class statement");
    }
}

#[test]
fn test_multiple_generics() {
    let program = parse("fn pair<K, V>(k: K, v: V) {}");
    let stmt = &program.statements[0];
    if let StmtKind::Fn { generics, .. } = &stmt.kind {
        assert_eq!(generics.len(), 2);
        assert_eq!(generics[0].name, "K");
        assert_eq!(generics[1].name, "V");
    } else {
        panic!("Expected Fn statement");
    }
}

#[test]
fn test_generic_constraints() {
    let program = parse("fn comparable<T: Comparable + Eq>(x: T) {}");
    let stmt = &program.statements[0];
    if let StmtKind::Fn { generics, .. } = &stmt.kind {
        assert_eq!(generics.len(), 1);
        let param = &generics[0];
        assert_eq!(param.name, "T");
        assert_eq!(param.constraints.len(), 2);
        assert_eq!(param.constraints[0], "Comparable");
        assert_eq!(param.constraints[1], "Eq");
    } else {
        panic!("Expected Fn statement");
    }
}

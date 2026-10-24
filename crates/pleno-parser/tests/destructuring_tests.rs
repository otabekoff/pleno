//! Parsing destructuring `let` patterns and tuple type annotations.

use pleno_ast::{BindingPattern, BindingPatternKind, StmtKind};
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

/// The pattern, mutability and type of the first statement, which must be a `LetPattern`.
fn let_pattern(source: &str) -> (BindingPattern, bool, Option<String>) {
    let program = parse_program(source);
    match program.statements.into_iter().next().map(|s| s.kind) {
        Some(StmtKind::LetPattern {
            pattern,
            mutable,
            ty,
            ..
        }) => (pattern, mutable, ty),
        other => panic!("expected LetPattern, got {other:?}"),
    }
}

fn names(pattern: &BindingPattern) -> Vec<String> {
    pattern
        .bindings()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect()
}

#[test]
fn tuple_pattern() {
    let (pattern, mutable, ty) = let_pattern("let (a, b) = pair;");
    assert!(!mutable);
    assert!(ty.is_none());
    match &pattern.kind {
        BindingPatternKind::Tuple(items) => assert_eq!(items.len(), 2),
        other => panic!("expected tuple, got {other:?}"),
    }
    assert_eq!(names(&pattern), ["a", "b"]);
}

#[test]
fn mutable_nested_pattern_with_underscore() {
    let (pattern, mutable, _) = let_pattern("let mut (a, (_, c), [d, e]) = value;");
    assert!(mutable);
    assert_eq!(names(&pattern), ["a", "c", "d", "e"]);
    let BindingPatternKind::Tuple(items) = &pattern.kind else {
        panic!("expected tuple");
    };
    assert!(matches!(&items[1].kind, BindingPatternKind::Tuple(inner)
        if matches!(inner[0].kind, BindingPatternKind::Ignore)));
    assert!(matches!(
        &items[2].kind,
        BindingPatternKind::Array { rest: None, .. }
    ));
}

#[test]
fn binding_spans_point_at_names() {
    let source = "let (first, second) = pair;";
    let (pattern, _, _) = let_pattern(source);
    for (name, span) in pattern.bindings() {
        assert_eq!(&source[span.start..span.end], name);
    }
    assert_eq!(
        &source[pattern.span.start..pattern.span.end],
        "(first, second)"
    );
}

#[test]
fn array_pattern_with_rest() {
    let (pattern, _, _) = let_pattern("let [first, ...rest] = items;");
    match &pattern.kind {
        BindingPatternKind::Array { elements, rest } => {
            assert_eq!(elements.len(), 1);
            let rest = rest.as_ref().expect("rest");
            assert!(matches!(&rest.kind, BindingPatternKind::Name(n) if n == "rest"));
        }
        other => panic!("expected array, got {other:?}"),
    }
    assert_eq!(names(&pattern), ["first", "rest"]);
}

#[test]
fn array_pattern_with_bare_rest() {
    let (pattern, _, _) = let_pattern("let [first, ...] = items;");
    assert!(matches!(
        &pattern.kind,
        BindingPatternKind::Array { rest: Some(rest), .. }
            if matches!(rest.kind, BindingPatternKind::Ignore)
    ));
    assert_eq!(names(&pattern), ["first"]);
}

#[test]
fn one_element_tuple_needs_trailing_comma() {
    let (pattern, _, _) = let_pattern("let (a,) = t;");
    assert!(matches!(&pattern.kind, BindingPatternKind::Tuple(items) if items.len() == 1));
    let err = parse_error("let (a) = t;");
    assert!(err.contains("trailing comma"), "{err}");
}

#[test]
fn typed_destructuring() {
    let (_, _, ty) = let_pattern("let (a, b): (Int, String) = pair;");
    assert_eq!(ty.as_deref(), Some("(Int, String)"));
}

#[test]
fn pattern_errors() {
    let err = parse_error("let (a, ...rest) = t;");
    assert!(err.contains("only allowed in array patterns"), "{err}");
    let err = parse_error("let [a, ...rest, b] = items;");
    assert!(err.contains("must be the last element"), "{err}");
    let err = parse_error("let (a, b);");
    assert!(err.contains("needs a value"), "{err}");
    let err = parse_error("let lazy (a, b) = t;");
    assert!(err.contains("cannot destructure"), "{err}");
    let err = parse_error("let (a, 1) = t;");
    assert!(err.contains("destructuring pattern"), "{err}");
}

#[test]
fn plain_let_is_unchanged() {
    let program = parse_program("let x = (1, 2);");
    assert!(matches!(program.statements[0].kind, StmtKind::Let { .. }));
}

#[test]
fn tuple_type_annotations() {
    let program = parse_program(
        "let p: (Int, String) = x;\n\
         let q: (Int, (Bool, Float))[] = y;\n\
         let r: (Int, Int)? = null;\n\
         fn f(a: (Int, Int)): (String, Int) { return (\"\", 0); }",
    );
    let ty = |i: usize| match &program.statements[i].kind {
        StmtKind::Let { ty, .. } => ty.clone().unwrap(),
        other => panic!("expected let, got {other:?}"),
    };
    assert_eq!(ty(0), "(Int, String)");
    assert_eq!(ty(1), "(Int, (Bool, Float))[]");
    assert_eq!(ty(2), "(Int, Int)?");
    match &program.statements[3].kind {
        StmtKind::Fn {
            params, return_ty, ..
        } => {
            assert_eq!(params[0].ty.as_deref(), Some("(Int, Int)"));
            assert_eq!(return_ty.as_deref(), Some("(String, Int)"));
        }
        other => panic!("expected fn, got {other:?}"),
    }
}

#[test]
fn function_types_and_grouping_still_parse() {
    let program = parse_program("let f: (Int, Int) => Int = g;\nlet h: (Int | String) = 1;");
    match &program.statements[0].kind {
        StmtKind::Let { ty, .. } => assert_eq!(ty.as_deref(), Some("(Int, Int) => Int")),
        other => panic!("expected let, got {other:?}"),
    }
    match &program.statements[1].kind {
        StmtKind::Let { ty, .. } => assert_eq!(ty.as_deref(), Some("Int | String")),
        other => panic!("expected let, got {other:?}"),
    }
}

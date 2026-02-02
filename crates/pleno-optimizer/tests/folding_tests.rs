use pleno_ast::{ExprKind, Literal, Program, StmtKind};
use pleno_lexer::lex;
use pleno_optimizer::ConstantFolder;
use pleno_parser::Parser;

fn fold_code(code: &str) -> Program {
    let tokens = lex(code).unwrap();
    let mut parser = Parser::new(tokens, code);
    let mut program = parser.parse_program().unwrap();
    let mut folder = ConstantFolder::new();
    folder.fold_program(&mut program);
    program
}

#[test]
fn test_fold_int_arithmetic() {
    let program = fold_code("let x = 1 + 2 * 3;");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Int(7)));
    } else {
        panic!("Expected let statement with folded value");
    }
}

#[test]
fn test_fold_float_arithmetic() {
    let program = fold_code("let x = 1.0 + 2.5;");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Float(3.5)));
    } else {
        panic!("Expected let statement with folded value");
    }
}

#[test]
fn test_fold_string_concat() {
    let program = fold_code("let x = \"hello \" + \"world\";");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(
            expr.kind,
            ExprKind::Literal(Literal::String("hello world".to_string()))
        );
    } else {
        panic!("Expected let statement with folded value");
    }
}

#[test]
fn test_fold_boolean_logic() {
    let program = fold_code("let x = true && false || !false;");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Bool(true)));
    } else {
        panic!("Expected let statement with folded value");
    }
}

#[test]
fn test_fold_complex_nested() {
    let program = fold_code("let x = (10 + 5) * (20 / 4);");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Int(75)));
    } else {
        panic!("Expected let statement with folded value");
    }
}

#[test]
fn test_fold_null_coalesce() {
    let program = fold_code("let x = null ?? 10;");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Int(10)));
    } else {
        panic!("Expected let statement with folded value");
    }

    let program2 = fold_code("let x = 5 ?? 10;");
    if let StmtKind::Let {
        value: Some(expr), ..
    } = &program2.statements[0].kind
    {
        assert_eq!(expr.kind, ExprKind::Literal(Literal::Int(5)));
    } else {
        panic!("Expected let statement with folded value");
    }
}

use pleno_ast::*;
use pleno_optimizer::optimize;
use pleno_parser::Parser;

fn parse(code: &str) -> Program {
    let tokens = pleno_lexer::lex(code).unwrap();
    let mut parser = Parser::new(tokens, code);
    parser.parse_program().unwrap()
}

#[test]
fn test_dce_unused_variable() {
    let code = r#"
        let used = 10;
        let unused = 20;
        print(used);
    "#;
    let mut program = parse(code);
    optimize(&mut program);

    // used should stay, unused should be gone
    let mut names = Vec::new();
    for stmt in &program.statements {
        if let StmtKind::Let { name, .. } = &stmt.kind {
            names.push(name.clone());
        }
    }
    assert!(names.contains(&"used".to_string()));
    assert!(!names.contains(&"unused".to_string()));
}

#[test]
fn test_inlining() {
    let code = r#"
        @inline
        fn add(a, b) {
            return a + b;
        }
        let x = add(1, 2);
        print(x);
    "#;
    let mut program = parse(code);
    optimize(&mut program);

    // The call should be inlined to (1 + 2) and then folded to 3
    // Wait, add(1, 2) inlined -> (1 + 2) -> folded -> 3
    let mut found_3 = false;
    for stmt in &program.statements {
        if let StmtKind::Let {
            value: Some(expr), ..
        } = &stmt.kind
            && let ExprKind::Literal(Literal::Int(3)) = &expr.kind
        {
            found_3 = true;
        }
    }
    assert!(
        found_3,
        "Call to add(1, 2) should be inlined and folded to 3"
    );
}

#[test]
fn test_no_optimize() {
    let code = r#"
        @no_optimize
        fn slow() {
            let x = 1 + 2;
        }
    "#;
    let mut program = parse(code);
    optimize(&mut program);

    // Inside slow, 1+2 should NOT be folded to 3
    let mut folded = true;
    for stmt in &program.statements {
        if let StmtKind::Fn { name, body, .. } = &stmt.kind
            && name == "slow"
            && let StmtKind::Block(stmts) = &body.kind
            && let StmtKind::Let {
                value: Some(expr), ..
            } = &stmts[0].kind
            && let ExprKind::Binary { .. } = &expr.kind
        {
            folded = false;
        }
    }
    assert!(
        !folded,
        "1+2 should NOT be folded inside @no_optimize function"
    );
}

#[test]
fn test_unroll_with_no_optimize() {
    let code = r#"
        @no_optimize
        fn loop_fn() {
            for (let i = 0; i < 3; i++) {
                print(i);
            }
        }
    "#;
    let mut program = parse(code);
    optimize(&mut program);

    // The loop should NOT be unrolled
    let mut unrolled = true;
    for stmt in &program.statements {
        if let StmtKind::Fn { name, body, .. } = &stmt.kind
            && name == "loop_fn"
            && let StmtKind::Block(stmts) = &body.kind
            && matches!(&stmts[0].kind, StmtKind::CFor { .. })
        {
            unrolled = false;
        }
    }
    assert!(
        !unrolled,
        "Loop should NOT be unrolled inside @no_optimize function"
    );
}

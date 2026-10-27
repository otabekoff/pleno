//! Loop unrolling copies small constant loops into straight-line code, but
//! never past a size budget and never when the body changes the loop variable.

use pleno_ast::*;
use pleno_optimizer::unroll::LoopUnroller;

fn unrolled(code: &str) -> Program {
    let tokens = pleno_lexer::lex(code).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, code)
        .parse_program()
        .unwrap();
    LoopUnroller::new().unroll_program(&mut program);
    program
}

fn is_loop(stmt: &Stmt) -> bool {
    matches!(stmt.kind, StmtKind::For { .. } | StmtKind::CFor { .. })
}

#[test]
fn a_small_constant_loop_is_unrolled() {
    let program = unrolled("for (let i = 0; i < 3; i = i + 1) { print(i); }");
    assert!(!is_loop(&program.statements[0]));
    let program = unrolled("for (i in 0..3) { print(i); }");
    assert!(!is_loop(&program.statements[0]));
}

#[test]
fn nested_loops_do_not_multiply_past_the_budget() {
    // 3^11 copies of the body made the compiler run out of memory (found by
    // the fuzz_compile target); now the outer loops stay loops.
    let mut code = String::new();
    for k in 0..11 {
        code.push_str(&format!("for (let i{k} = 0; i{k} < 3; i{k} = i{k} + 1)\n"));
    }
    code.push_str("print(1);\n");
    let program = unrolled(&code);
    assert!(is_loop(&program.statements[0]));
    assert!(format!("{program:?}").len() < 200_000);
}

#[test]
fn a_loop_that_changes_its_variable_is_not_unrolled() {
    // Unrolled copies bind the variable with `let`, which cannot be assigned:
    // this valid program failed to compile with `cannot assign to immutable binding`.
    for body in ["{ i = i + 1; print(i); }", "{ i++; }", "{ --i; }"] {
        let program = unrolled(&format!("for (let mut i = 0; i < 3; i = i + 1) {body}"));
        assert!(is_loop(&program.statements[0]), "{body}");
    }
}

//! Destructuring patterns in `for ... in` loops.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

/// The printed form of the program's last expression.
fn eval(source: &str) -> String {
    eval_result(source)
        .unwrap_or_else(|e| panic!("{source}\nfailed: {e}"))
        .to_string()
}

fn eval_err(source: &str) -> String {
    match eval_result(source) {
        Ok(value) => panic!("{source}\nexpected an error, got {value}"),
        Err(e) => e
            .strip_prefix("uncaught exception: ")
            .map(str::to_string)
            .unwrap_or(e),
    }
}

#[test]
fn tuple_pattern_binds_each_element() {
    let source = r#"
        let mut out = "";
        for ((n, word) in [(1, "one"), (2, "two")]) { out = out + word + n; }
        out;
    "#;
    assert_eq!(eval(source), "one1two2");
}

#[test]
fn array_pattern_and_rest() {
    assert_eq!(
        eval("let mut s = 0; for ([a, b] in [[1, 2], [3, 4]]) { s = s + a * b; } s;"),
        "14"
    );
    assert_eq!(
        eval("let mut s = []; for ([h, ...t] in [[1, 2, 3], [4]]) { s.push(h + t.length); } s;"),
        "[3, 4]"
    );
}

#[test]
fn nested_patterns_and_underscore() {
    let source = "
        let mut s = 0;
        for ((i, (x, _)) in [(1, (10, 99)), (2, (20, 99))]) { s = s + i * x; }
        s;
    ";
    assert_eq!(eval(source), "50");
}

#[test]
fn map_entries_destructure_as_tuples() {
    let source = r#"
        let m = {"a": 1, "b": 2};
        let mut keys = "";
        let mut total = 0;
        for ((k, v) in m) { keys = keys + k; total = total + v; }
        for ((k, _) in m.entries()) { keys = keys + k; }
        keys + total;
    "#;
    assert_eq!(eval(source), "abab3");
}

#[test]
fn two_name_and_one_name_forms_still_work() {
    let source = r#"
        let mut s = "";
        for (k, v in {"a": 1}) { s = s + k + v; }
        for (i, x in [5]) { s = s + i + x; }
        for (key in {"b": 2}) { s = s + key; }
        s;
    "#;
    assert_eq!(eval(source), "a105b");
}

#[test]
fn an_iterator_can_be_looped_over() {
    assert_eq!(
        eval(r#"let mut s = []; for (e in {"a": 1}.entries()) { s.push(e); } s;"#),
        r#"[("a", 1)]"#
    );
}

#[test]
fn loop_bindings_are_fresh_per_iteration_and_reassignable() {
    let source = "
        let fs = [];
        for ((k, v) in [(1, 2), (3, 4)]) { k = k + v; fs.push(fn() => k); }
        fs[0]() * 10 + fs[1]();
    ";
    assert_eq!(eval(source), "37");
}

#[test]
fn break_and_continue_work_in_pattern_loops() {
    let source = "
        let mut s = 0;
        for ((a, b) in [(1, 1), (2, 2), (3, 3), (4, 4)]) {
            if (a == 2) { continue; }
            if (a == 4) { break; }
            s = s + b;
        }
        s;
    ";
    assert_eq!(eval(source), "4");
}

#[test]
fn shape_errors_match_destructuring_let() {
    assert_eq!(
        eval_err("for ((a, b, c) in [(1, 2)]) { }"),
        "tuple has 2 elements, pattern expects 3"
    );
    assert_eq!(
        eval_err("for ([a, b] in [[1]]) { }"),
        "array has 1 element, pattern expects 2"
    );
    assert_eq!(
        eval_err("for ((a, b) in [1, 2]) { }"),
        "cannot destructure Int as a tuple"
    );
    assert_eq!(
        eval_err(r#"for ([a, b] in {"k": 1}) { }"#),
        "cannot destructure Tuple as an array"
    );
}

#[test]
fn a_range_cannot_be_destructured() {
    assert!(
        eval_err("for ((a, b) in 0..3) { }")
            .contains("a range yields numbers, which a destructuring pattern cannot match")
    );
}

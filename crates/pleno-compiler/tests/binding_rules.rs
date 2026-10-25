//! A binding declared without `mut` cannot be changed from anywhere, and code
//! the optimizer would drop is still checked.

use pleno_compiler::Compiler;

fn compile(source: &str) -> Result<(), String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .map_err(|e| e.message)?;
    Compiler::new(source.to_string())
        .compile(&mut program)
        .map(|_| ())
        .map_err(|e| e.diagnostic.message)
}

fn error(source: &str) -> String {
    compile(source).expect_err(source)
}

#[test]
fn captured_immutable_bindings_cannot_be_assigned() {
    for source in [
        "let x = 1; fn f() { x = 2; } f();",
        "const K = 1; fn f() { K = 2; } f();",
        "let x = 1; let f = fn() { x += 1; }; f();",
        "fn outer() { let x = 1; let f = fn() { x = 2; }; f(); } outer();",
    ] {
        assert!(
            error(source).contains("cannot assign to immutable binding"),
            "{source}"
        );
    }
    let err = error("let n = 0; let f = fn() { n++; }; f();");
    assert!(err.contains("cannot increment immutable variable"), "{err}");
}

#[test]
fn captured_mutable_bindings_can_be_assigned() {
    compile("let mut x = 1; fn f() { x = 2; x += 1; x++; } f();").unwrap();
    compile("fn outer() { let mut c = 0; let f = fn() { c += 1; }; f(); } outer();").unwrap();
}

#[test]
fn declarations_cannot_be_assigned() {
    for (source, expected) in [
        ("fn g() {} g = 1;", "cannot assign to function 'g'"),
        (
            "fn g() {} fn f() { g = 1; } f(); g();",
            "cannot assign to function 'g'",
        ),
        ("class A {} A = 1;", "cannot assign to class 'A'"),
        (
            "enum E { X } fn f() { E = 1; } f();",
            "cannot assign to enum 'E'",
        ),
        ("interface I {} I = 1;", "cannot assign to interface 'I'"),
    ] {
        let err = error(source);
        assert!(err.contains(expected), "{source}: {err}");
    }
}

#[test]
fn unused_code_is_still_checked() {
    for (source, expected) in [
        ("let x = 1; fn unused() { x = 2; }", "immutable"),
        ("fn unused() { break; }", "break"),
        ("class Unused { fn m() { continue; } }", "continue"),
        ("fn f() { return 1; break; } f();", "break"),
    ] {
        let err = error(source);
        assert!(err.contains(expected), "{source}: {err}");
    }
}

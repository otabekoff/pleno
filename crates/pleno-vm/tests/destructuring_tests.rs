//! Destructuring `let` and tuple type annotations.

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
fn tuple_destructuring_binds_each_element() {
    assert_eq!(eval(r#"let (a, b) = (1, "x"); a + b;"#), "1x");
}

#[test]
fn nested_tuple_and_underscore() {
    assert_eq!(eval("let (a, (_, c), _) = (1, (2, 3), 4); a + c;"), "4");
}

#[test]
fn one_element_tuple_pattern() {
    assert_eq!(eval("let (a,) = (7,); a;"), "7");
}

#[test]
fn named_tuple_destructures_by_position() {
    assert_eq!(eval("let (x, y) = (x: 1, y: 2); x * 10 + y;"), "12");
}

#[test]
fn let_mut_destructuring_allows_assignment() {
    assert_eq!(eval("let mut (a, b) = (1, 2); a = a + b; a;"), "3");
}

#[test]
fn immutable_destructured_binding_rejects_assignment() {
    let err = eval_err("let (a, b) = (1, 2); a = 3;");
    assert!(err.contains("immutable"), "{err}");
}

#[test]
fn array_destructuring() {
    assert_eq!(eval("let [x, y, z] = [1, 2, 3]; x + y + z;"), "6");
}

#[test]
fn array_rest_element() {
    assert_eq!(eval("let [head, ...tail] = [1, 2, 3]; tail;"), "[2, 3]");
    assert_eq!(eval("let [head, ...tail] = [1]; tail;"), "[]");
    assert_eq!(eval("let [first, ...] = [5, 6, 7]; first;"), "5");
}

#[test]
fn mixed_nesting() {
    assert_eq!(
        eval(r#"let [(k, v), [n, ...more]] = [("a", 1), [2, 3, 4]]; k + v + n + more.length;"#),
        "a122"
    );
}

#[test]
fn destructuring_inside_functions_and_closures() {
    let source = "
        fn make() {
            let (base, step) = (10, 2);
            return fn(n) { return base + step * n; };
        }
        make()(3);
    ";
    assert_eq!(eval(source), "16");
}

#[test]
fn destructured_name_can_be_shadowed_in_inner_block() {
    assert_eq!(
        eval(
            "let (a, b) = (1, 2); let mut r = 0; { let (a, c) = (10, 20); r = a + c; } r + a + b;"
        ),
        "33"
    );
}

#[test]
fn not_a_tuple_is_an_error() {
    assert_eq!(
        eval_err("let (a, b) = 5;"),
        "cannot destructure Int as a tuple"
    );
    assert_eq!(
        eval_err("let (a, b) = [1, 2];"),
        "cannot destructure Array as a tuple"
    );
}

#[test]
fn tuple_length_mismatch_is_an_error() {
    assert_eq!(
        eval_err("let (a, b, c) = (1, 2);"),
        "tuple has 2 elements, pattern expects 3"
    );
    assert_eq!(
        eval_err("let (a, b) = (1, 2, 3);"),
        "tuple has 3 elements, pattern expects 2"
    );
}

#[test]
fn array_errors() {
    assert_eq!(
        eval_err(r#"let [a, b] = "ab";"#),
        "cannot destructure String as an array"
    );
    assert_eq!(
        eval_err("let [a, b] = [1];"),
        "array has 1 element, pattern expects 2"
    );
    assert!(eval_err("let [a] = [1, 2];").starts_with("array has 2 elements, pattern expects 1"));
    assert_eq!(
        eval_err("let [a, b, ...rest] = [1];"),
        "array has 1 element, pattern expects at least 2"
    );
}

#[test]
fn nested_shape_errors() {
    assert_eq!(
        eval_err("let (a, (b, c)) = (1, 2);"),
        "cannot destructure Int as a tuple"
    );
}

#[test]
fn duplicate_names_are_rejected() {
    let err = eval_err("fn f() { let (a, a) = (1, 2); } f();");
    assert!(err.contains("already defined"), "{err}");
}

#[test]
fn tuple_type_annotation_on_let() {
    assert_eq!(eval(r#"let p: (Int, String) = (1, "a"); p.1;"#), "a");
    let err = eval_err("let p: (Int, String) = (1, 2);");
    assert!(
        err.ends_with("expected (Int, String), found Tuple"),
        "{err}"
    );
    let err = eval_err("let p: (Int, Int) = (1, 2, 3);");
    assert!(err.ends_with("expected (Int, Int), found Tuple"), "{err}");
    let err = eval_err("let p: (Int, Int) = [1, 2];");
    assert!(err.ends_with("expected (Int, Int), found Array"), "{err}");
}

#[test]
fn tuple_type_annotation_on_destructuring_let() {
    assert_eq!(eval("let (a, b): (Int, Int) = (1, 2); a + b;"), "3");
    assert!(eval_err(r#"let (a, b): (Int, Int) = (1, "x");"#).contains("expected (Int, Int)"));
}

#[test]
fn tuple_types_in_signatures() {
    let source = "
        fn swap(p: (Int, String)): (String, Int) {
            let (n, s) = p;
            return (s, n);
        }
        let (s, n) = swap((1, \"one\"));
        s + n;
    ";
    assert_eq!(eval(source), "one1");
    let bad_arg = eval_err("fn f(p: (Int, Int)) { return p; } f((1, true));");
    assert!(bad_arg.contains("(Int, Int)"), "{bad_arg}");
    // Return annotations parse but, like every return annotation, are not checked yet.
    assert_eq!(eval("fn g(): (Int, Int) { return (1, 2); } g().1;"), "2");
}

#[test]
fn composite_tuple_types() {
    assert_eq!(
        eval("let ps: (Int, Int)[] = [(1, 2), (3, 4)]; ps.length;"),
        "2"
    );
    assert_eq!(eval("let m: (Int, Int)? = null; m;"), "null");
    assert_eq!(
        eval(r#"let u: (Int | Null, String) = (null, "a"); u.1;"#),
        "a"
    );
    assert_eq!(
        eval(r#"let n: ((Int, Int), String) = ((1, 2), "a"); n.1;"#),
        "a"
    );
    assert_eq!(
        eval(r#"let g: (Map<String, Int>, Int) = ({"a": 1}, 2); g.1;"#),
        "2"
    );
    assert!(eval_err("let ps: (Int, Int)[] = [(1, 2), (3, \"x\")];").contains("(Int, Int)[]"));
}

#[test]
fn function_types_still_work() {
    assert_eq!(
        eval("let f: (Int, Int) => Int = fn(a, b) { return a + b; }; f(1, 2);"),
        "3"
    );
    assert_eq!(
        eval("let t: (Int, (Int) => Int) = (1, fn(x) { return x; }); t.0;"),
        "1"
    );
}

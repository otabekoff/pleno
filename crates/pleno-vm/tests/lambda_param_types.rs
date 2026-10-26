//! Parameter annotations on lambdas and anonymous functions are checked when
//! they are called, the same as on named functions.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn compile_and_run(source: &str) -> Result<Value, String> {
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

fn eval(source: &str) -> String {
    compile_and_run(source)
        .unwrap_or_else(|err| panic!("{err}\n--- source:\n{source}"))
        .to_string()
}

fn assert_error(source: &str, expected: &str) {
    match compile_and_run(source) {
        Ok(value) => panic!("expected an error, got {value:?}\n--- source:\n{source}"),
        Err(err) => assert!(
            err.contains(expected),
            "expected error containing {expected:?}, got {err:?}\n--- source:\n{source}"
        ),
    }
}

#[test]
fn fn_lambda_parameters_are_checked() {
    assert_error(
        r#"let f = fn(x: Int) => x; f("s");"#,
        "TypeError: parameter 'x' expected Int, got String",
    );
    assert_error(
        r#"let f = fn(x: Int, y: String) { return x; }; f(1, 2);"#,
        "TypeError: parameter 'y' expected String, got Int",
    );
    assert_eq!(eval("let f = fn(x: Int) => x + 1; f(1);"), "2");
}

#[test]
fn arrow_lambda_parameters_take_and_check_annotations() {
    assert_error(
        r#"let f = (x: Int) => x; f("s");"#,
        "TypeError: parameter 'x' expected Int, got String",
    );
    assert_eq!(eval("[1, 2].map((x: Int) => x * 2);"), "[2, 4]");
    assert_error(
        r#"["a"].map((x: Int) => x);"#,
        "TypeError: parameter 'x' expected Int, got String",
    );
}

#[test]
fn int_widens_to_float() {
    assert_eq!(eval("let f = (x: Float) => typeof(x); f(3);"), "Float");
    assert_eq!(
        eval("let f = fn(x: Float | String) => typeof(x); f(3);"),
        "Float"
    );
    // An `Int` stays an `Int` where the annotation also allows `Int`.
    assert_eq!(eval("let f = (x: Int | Float) => typeof(x); f(3);"), "Int");
}

#[test]
fn optional_and_union_annotations() {
    assert_eq!(eval("let f = (x: Int?) => x; f(null);"), "null");
    assert_eq!(eval("let f = (x: Int?) => x; f(2);"), "2");
    assert_error(
        r#"let f = (x: Int?) => x; f("s");"#,
        "TypeError: parameter 'x' expected Int?, got String",
    );
    assert_eq!(eval(r#"let f = (x: Int | String) => x; f("a");"#), "a");
    assert_error(
        "let f = (x: Int | String) => x; f(true);",
        "TypeError: parameter 'x' expected Int | String, got Bool",
    );
}

#[test]
fn classes_arrays_aliases_and_defaults() {
    let class = "class P { v: Int; constructor(v: Int) { this.v = v; } }\n";
    assert_eq!(
        eval(&format!("{class}let f = (p: P) => p.v; f(P(4));")),
        "4"
    );
    assert_error(
        &format!("{class}let f = (p: P) => p.v; f(4);"),
        "TypeError: parameter 'p' expected P, got Int",
    );
    assert_error(
        r#"let f = (xs: Int[]) => xs; f("no");"#,
        "TypeError: parameter 'xs' expected Int[], got String",
    );
    assert_eq!(
        eval("type Num = Int | Float; let f = (x: Num) => x; f(1.5);"),
        "1.5"
    );
    // The default is checked too, after it is filled in.
    assert_eq!(eval("let f = fn(x: Int = 5) => x; f();"), "5");
    assert_error(
        r#"let f = fn(x: Int = "s") => x; f();"#,
        "TypeError: parameter 'x' expected Int, got String",
    );
}

#[test]
fn any_and_unannotated_parameters_take_anything() {
    assert_eq!(eval(r#"let f = (x: Any) => x; f("s");"#), "s");
    assert_eq!(eval(r#"let f = (x) => x; f("s");"#), "s");
    assert_eq!(eval(r#"let f = |x| => x; f("s");"#), "s");
}

#[test]
fn type_parameters_of_enclosing_functions_are_not_checked() {
    let source = r#"
        fn wrap<T>(v: T) {
            let g = (y: T) => y;
            return g(v);
        }
        wrap<Int>(3) + wrap("x");
    "#;
    assert_eq!(eval(source), "3x");
}

#[test]
fn async_lambda_parameters_are_checked() {
    assert_error(
        r#"let f = async fn(x: Int) => x; await f("s");"#,
        "TypeError: parameter 'x' expected Int, got String",
    );
}

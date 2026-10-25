use pleno_compiler::check::{Severity, check_program};

/// The messages the static checks report for `source`, with their severity
/// and the source text each one points at.
fn check(source: &str) -> Vec<(Severity, String, String)> {
    let tokens = pleno_lexer::lex(source).expect("lex");
    let program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .expect("parse");
    check_program(&program)
        .into_iter()
        .map(|d| {
            let text = source[d.span.start..d.span.end].to_string();
            (d.severity, d.message, text)
        })
        .collect()
}

fn messages(source: &str) -> Vec<String> {
    check(source).into_iter().map(|(_, m, _)| m).collect()
}

fn assert_clean(source: &str) {
    assert_eq!(check(source), Vec::new(), "for {source}");
}

#[test]
fn too_many_arguments_with_defaults() {
    let found = check("fn greet(name, greeting = \"Hi\") { print(name); }\ngreet(1, 2, 3);");
    assert_eq!(
        found,
        vec![(
            Severity::Error,
            "greet expects 1 to 2 arguments, got 3".to_string(),
            "greet(1, 2, 3)".to_string()
        )]
    );
}

#[test]
fn too_few_and_exact_counts() {
    assert_eq!(
        messages("fn f(a, b) {}\nf(1);"),
        ["f expects 2 arguments, got 1"]
    );
    assert_eq!(
        messages("fn f() {}\nf(1);"),
        ["f expects no arguments, got 1"]
    );
    assert_eq!(
        messages("fn f(a) {}\nf();"),
        ["f expects 1 argument, got 0"]
    );
    assert_clean("fn f(a, b = 2) {}\nf(1); f(1, 2);");
}

#[test]
fn variadic_parameters() {
    assert_eq!(
        messages("fn f(a, ...rest) {}\nf();"),
        ["f expects at least 1 argument, got 0"]
    );
    assert_clean("fn f(a, ...rest) {}\nf(1); f(1, 2, 3, 4);");
    assert_clean("fn sum(...xs) {}\nsum(); sum(1, 2);");
}

#[test]
fn named_and_keyword_only_arguments() {
    assert_clean("fn f(a, b = 2) {}\nf(b: 3, a: 1); f(1, b: 2);");
    assert_eq!(
        messages("fn f(a) {}\nf(a: 1, b: 2);"),
        ["f has no parameter named 'b'"]
    );
    assert_eq!(
        messages("fn f(a, b) {}\nf(1, a: 2);"),
        ["f got more than one value for 'a'"]
    );
    assert_eq!(
        messages("fn f(a, b) {}\nf(b: 2);"),
        ["f is missing argument 'a'"]
    );
    assert_clean("fn f(host, *, port, tls = true) {}\nf(\"h\", port: 1);");
    assert_eq!(
        messages("fn f(host, *, port) {}\nf(\"h\");"),
        ["f is missing named argument 'port'"]
    );
    assert_eq!(
        messages("fn f(host, *, port) {}\nf(\"h\", 80);"),
        ["f expects 1 positional argument, got 2"]
    );
}

#[test]
fn pipelines_supply_the_first_argument() {
    assert_clean("fn f(a) {}\n1 |> f();");
    assert_clean("fn f(a, b) {}\n1 |> f(2); 2 |> f(1, _);");
    assert_clean("fn f(a) {}\n1 |> f;");
    assert_eq!(
        messages("fn f(a) {}\n1 |> f(2);"),
        ["f expects 1 argument, got 2"]
    );
}

#[test]
fn partial_application_and_spread_are_skipped() {
    assert_clean("fn f(a, b) {}\nlet g = f(1, _);\nlet xs = [1, 2, 3];\nf(...xs);");
}

#[test]
fn rebound_names_are_skipped() {
    assert_clean("fn f(a) {}\nfn g(f) { f(1, 2); }");
    assert_clean("fn f(a) {}\nfn g() { let f = fn(a, b) {}; f(1, 2); }");
    assert_clean("fn f(a) {}\nfor (f in [1]) { f(1, 2); }");
    assert_clean("import { f } from \"./other.pln\";\nfn f(a) {}\nf(1, 2);");
    assert_clean("fn f(a) {}\nfn f(a, b) {}\nf(1, 2);");
}

#[test]
fn decorated_functions_are_skipped() {
    assert_clean("fn twice(f) { return f; }\n@twice\nfn f(a) {}\nf(1, 2);");
    assert_eq!(
        messages("@pure\nfn f(a) { return a; }\nf(1, 2);"),
        ["f expects 1 argument, got 2"]
    );
}

#[test]
fn calls_that_are_expected_to_fail_are_skipped() {
    assert_clean("fn f(a) {}\ntry { f(1, 2); } catch (e) { print(e); }");
    assert_clean("fn f(a) {}\nassert.throws(fn() { f(1, 2); });");
    assert_eq!(
        messages("fn f(a) {}\ntry { print(1); } finally { f(1, 2); }"),
        ["f expects 1 argument, got 2"]
    );
}

#[test]
fn constructors() {
    assert_eq!(
        messages("class P { constructor(x, y) {} }\nP(1, 2, 3);"),
        ["P expects 2 arguments, got 3"]
    );
    assert_eq!(
        messages("class A {}\nA(1);"),
        ["A expects no arguments, got 1"]
    );
    assert_eq!(
        messages("class B { constructor(a) {} }\nclass C extends B {}\nC(1, 2);"),
        ["C expects 1 argument, got 2"]
    );
    assert_clean("class C extends Error {}\nC(1, 2, 3);");
    assert_clean("class P { constructor(x, y = 0) {} }\nP(1); P(x: 1, y: 2);");
    assert_clean("fn deco(c) { return c; }\n@deco\nclass A {}\nA(1);");
    assert_clean("class A {}\nextend A { fn m() {} }\nA(1);");
}

#[test]
fn methods_called_on_this() {
    assert_eq!(
        messages("class A {\n  fn m(a) {}\n  fn n() { this.m(1, 2); }\n}"),
        ["A.m expects 1 argument, got 2"]
    );
    let found = check("class A {\n  fn n() { this.missing(); }\n}");
    assert_eq!(
        found,
        vec![(
            Severity::Warning,
            "A has no method 'missing'".to_string(),
            "this.missing".to_string()
        )]
    );
    assert_eq!(
        messages(
            "class B { fn base() {} }\nclass A extends B {\n  fn n() { this.base(); this.nope(); }\n}"
        ),
        ["A has no method 'nope'"]
    );
}

#[test]
fn methods_on_this_that_may_exist_are_not_reported() {
    // Overridden or defined by a subclass.
    assert_clean("class A { fn n() { this.render(); } }\nclass B extends A { fn render() {} }");
    // A field, property or getter holding a function.
    assert_clean("class A {\n  callback: Function;\n  fn n() { this.callback(); }\n}");
    assert_clean("class A {\n  constructor(cb) { this.cb = cb; }\n  fn n() { this.cb(1, 2); }\n}");
    assert_clean("class A {\n  get handler() { return fn() {}; }\n  fn n() { this.handler(); }\n}");
    // Members from outside the file, decorators, interfaces and extensions.
    assert_clean("class A extends Imported { fn n() { this.inherited(); } }");
    assert_clean("fn deco(c) { return c; }\n@deco\nclass A { fn n() { this.added(); } }");
    assert_clean("interface I { fn m(); }\nclass A implements I { fn n() { this.m(); } }");
    assert_clean("class A { fn n() { this.later(); } }\nextend A { fn later() {} }");
    assert_clean("class A { fn n() { this.dyn(); } }\nreflect.setField(A(), \"dyn\", fn() {});");
    // Exported classes may be extended by an importer.
    assert_clean("export class A { fn n() { this.later(); } }");
    // A static method or a nested class.
    assert_clean("class A {\n  static fn s() {}\n  fn n() { this.s(); }\n}");
}

#[test]
fn nested_classes_use_their_own_members() {
    assert_clean(
        "class Outer {\n  fn a() {}\n  class Inner { fn b() { this.c(); } }\n}\nclass Inner {}",
    );
}

#[test]
fn literal_type_mismatches() {
    let found = check("let x: Int = \"hi\";");
    assert_eq!(
        found,
        vec![(
            Severity::Error,
            "'x' is declared Int but given a String literal".to_string(),
            "\"hi\"".to_string()
        )]
    );
    assert_eq!(
        messages("fn f(s: String) {}\nf(1);"),
        ["f expects String for 's', found an Int literal"]
    );
    assert_eq!(
        messages("fn f(n: Int) {}\nf(n: true);"),
        ["f expects Int for 'n', found a Bool literal"]
    );
    assert_eq!(
        messages("let x: Int = null;"),
        ["'x' is declared Int but given null"]
    );
    assert_eq!(
        messages("let x: Int | Bool = \"s\";"),
        ["'x' is declared Int | Bool but given a String literal"]
    );
}

#[test]
fn int_literals_widen_to_float_but_not_back() {
    assert_clean("fn f(x: Float) {}\nf(3); let a: Float = 3; let b: Float? = 1;");
    assert_clean("let c: Int | Float = 2.5; let d: Number = 1;");
    assert_eq!(
        messages("let x: Int = 2.5;"),
        ["'x' is declared Int but given a Float literal"]
    );
    assert_eq!(
        messages("fn f(n: Int) {}\nf(1.5);"),
        ["f expects Int for 'n', found a Float literal"]
    );
}

#[test]
fn returned_literals_are_checked_against_the_return_annotation() {
    assert_eq!(
        messages("fn f(): Int { return \"s\"; }"),
        ["f is declared to return Int but returns a String literal"]
    );
    assert_eq!(
        messages("class A { fn m(): String { return 1; } }"),
        ["m is declared to return String but returns an Int literal"]
    );
    assert_eq!(
        messages("let g = fn(): Int => \"s\";"),
        ["lambda is declared to return Int but returns a String literal"]
    );
    assert_eq!(
        messages("async fn f(): Promise<Int> { return true; }"),
        ["f is declared to return Int but returns a Bool literal"]
    );
    assert_clean("fn f(): Float { return 1; }\nfn g(): Void { return 1; }");
    assert_clean("fn f<T>(): T { return 1; }\nfn h(): Int? { return null; }");
    assert_clean("fn f(): Int { let g = fn() { return \"s\"; }; return 1; }");
}

#[test]
fn literals_that_may_match_are_not_reported() {
    assert_clean("let a: Int? = null; let b: Float = 1; let c: Number = -2.5;");
    assert_clean("let d: Any = \"s\"; let e: Int | String = \"s\"; let f: String = `x ${1}`;");
    assert_clean("let g: Foo = 1; let h: Array<Int> = 1; let i: Int[] = \"s\";");
    assert_clean("type Name = Int;\nlet n: Name = \"s\";");
    assert_clean("type String = Int;\nlet s: String = 1;");
    assert_clean("fn f<T>(x: T) {}\nf(\"s\");");
    assert_clean("let x: Int = 1 + 2; let y: String = name;");
    assert_clean("let lazy z: Int = 1;");
}

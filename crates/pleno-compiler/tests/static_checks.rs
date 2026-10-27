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
fn misspelled_names_get_a_suggestion() {
    assert_eq!(
        messages("fn f(name) {}\nf(nmae: 1);"),
        ["f has no parameter named 'nmae'; did you mean 'name'?"]
    );
    assert_eq!(
        messages("class A { count: Int; fn go() {} fn run() { this.og(); this.cuont(); } }"),
        [
            "A has no method 'og'; did you mean 'go'?",
            "A has no method 'cuont'; did you mean 'count'?"
        ]
    );
    // A superclass's private members are not suggested to a subclass.
    assert_eq!(
        messages(
            "class A { private fn hidden() {} }\nclass B extends A { fn f() { this.hiden(); } }"
        ),
        ["B has no method 'hiden'"]
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

const POINT: &str = "class Point {\n  x; y;\n  constructor(x, y) { this.x = x; this.y = y; }\n  fn shift(dx, dy) { return Point(this.x + dx, this.y + dy); }\n}\n";

#[test]
fn methods_called_on_a_value_of_a_known_class() {
    let found = check(&format!("{POINT}let p = Point(1, 2);\np.shfit(1, 1);"));
    assert_eq!(
        found,
        vec![(
            Severity::Warning,
            "Point has no method 'shfit'; did you mean 'shift'?".to_string(),
            "p.shfit".to_string()
        )]
    );
    assert_eq!(
        messages(&format!("{POINT}let p = Point(1, 2);\np.shift(1);")),
        ["Point.shift expects 2 arguments, got 1"]
    );
    assert_eq!(
        messages(&format!("{POINT}Point(1, 2).shift(1, 2, 3);")),
        ["Point.shift expects 2 arguments, got 3"]
    );
    // In a function declared after the `let`, and through a second name.
    assert_eq!(
        messages(&format!(
            "{POINT}let p = Point(1, 2);\nlet q = p;\nfn f() {{ q.shift(); }}"
        )),
        ["Point.shift expects 2 arguments, got 0"]
    );
    // Inherited methods are found.
    assert_eq!(
        messages(
            "class A { fn base(a) {} }\nclass B extends A {}\nlet b = B();\nb.base(); b.nope();"
        ),
        ["B.base expects 1 argument, got 0", "B has no method 'nope'"]
    );
    // A private method is not suggested outside its class.
    assert_eq!(
        messages("class A { private fn hidden() {} }\nlet a = A();\na.hiden();"),
        ["A has no method 'hiden'"]
    );
}

#[test]
fn methods_on_values_that_may_have_them_are_not_reported() {
    // Fields, getters and properties assigned anywhere may hold functions.
    assert_clean("class A { cb; constructor(cb) { this.cb = cb; } }\nlet a = A(fn() {});\na.cb();");
    assert_clean("class A { get handler() { return fn() {}; } }\nlet a = A();\na.handler();");
    assert_clean("class A {}\nlet a = A();\na.later = fn() {};\na.later();");
    // `extend`, decorators, interfaces, exports and reflection may add methods.
    assert_clean("class A {}\nextend A { fn later() {} }\nlet a = A();\na.later();");
    assert_clean("fn deco(c) { return c; }\n@deco\nclass A {}\nlet a = A();\na.added();");
    assert_clean(
        "interface I { fn m(); }\nclass A implements I { fn m() {} }\nlet a = A();\na.x();",
    );
    assert_clean("export class A {}\nlet a = A();\na.added();");
    assert_clean("class A {}\nlet a = A();\nreflect.setField(a, \"dyn\", fn() {});\na.dyn();");
    // A class from elsewhere, or a name bound more than once or with `mut`.
    assert_clean("class A extends Imported {}\nlet a = A();\na.inherited();");
    assert_clean("class A {}\nlet a = A();\nfn f(a) { a.other(); }");
    assert_clean("class A {}\nlet mut a = A();\na.other();");
    // Out of the block that bound it, the name means something else.
    assert_clean("class A {}\nif (true) { let a = A(); }\na.other();");
    // A partial application makes a function, not an instance.
    assert_clean("class A { constructor(x, y) {} }\nlet make = A(1, _);\nmake.other();");
}

#[test]
fn static_methods() {
    let class = "class Point {\n  static fn origin() { return 1; }\n  static fn at(x, y = 0) { return 1; }\n}\n";
    assert_eq!(
        messages(&format!("{class}Point.origin(1);")),
        ["Point.origin expects no arguments, got 1"]
    );
    assert_eq!(
        messages(&format!("{class}Point.at();")),
        ["Point.at expects 1 to 2 arguments, got 0"]
    );
    let found = check(&format!("{class}Point.orgin();"));
    assert_eq!(
        found,
        vec![(
            Severity::Warning,
            "Point has no static method 'orgin'; did you mean 'origin'?".to_string(),
            "Point.orgin".to_string()
        )]
    );
    // Static methods are inherited.
    assert_eq!(
        messages("class A { static fn make(a) {} }\nclass B extends A {}\nB.make();"),
        ["B.make expects 1 argument, got 0"]
    );
    assert_clean(&format!(
        "{class}Point.origin(); Point.at(1); Point.at(1, 2);"
    ));
    // A static field may hold a function; other members are left to the VM.
    assert_clean("class A { static handler = fn(x) { return x; }; }\nA.handler(1, 2);");
    assert_clean("class A { fn inst() {} }\nA.inst();");
    assert_clean("class A {}\nextend A { static later() {} }\nA.later();");
}

#[test]
fn lambdas_bound_with_let() {
    assert_eq!(
        messages("let f = (x) => x;\nf(1, 2);"),
        ["f expects 1 argument, got 2"]
    );
    assert_eq!(
        messages("let g = fn(a, b = 1) { return a; };\ng();"),
        ["g expects 1 to 2 arguments, got 0"]
    );
    assert_eq!(
        messages("let h = (n: Int) => n;\nh(\"s\");"),
        ["h expects Int for 'n', found a String literal"]
    );
    assert_clean("let f = (x) => x;\nf(1); 1 |> f; f(x: 2);");
    assert_clean("let f = fn(...xs) { return xs; };\nf(); f(1, 2, 3);");
    assert_clean("let mut f = (x) => x;\nf = (x, y) => x;\nf(1, 2);");
}

#[test]
fn values_of_a_known_type_that_cannot_match() {
    assert_eq!(
        messages("class P {}\nlet n: Int = P();"),
        ["'n' is declared Int but given a P instance"]
    );
    assert_eq!(
        messages("let s = \"hi\";\nfn f(n: Int) {}\nf(s);"),
        ["f expects Int for 'n', found a String value"]
    );
    assert_eq!(
        messages("fn count(): Int { return 1; }\nlet s: String = count();"),
        ["'s' is declared String but given an Int value"]
    );
    assert_eq!(
        messages("let x: Float = 1;\nfn f(): Int { return x; }"),
        ["f is declared to return Int but returns a Float value"]
    );
    assert_eq!(
        messages("let n = null;\nlet m: Int = n;"),
        ["'m' is declared Int but given null"]
    );
}

#[test]
fn values_that_may_match_are_not_reported() {
    // An Int widens to a Float; `Number`, unions, optionals and `Any` accept more.
    assert_clean("fn count(): Int { return 1; }\nlet f: Float = count(); let g: Int? = count();");
    assert_clean("let n = 1;\nlet a: Number = n; let b: Int | String = n; let c: Any = n;");
    // A class's own name, a superclass or an interface may match an instance.
    assert_clean("class A {}\nclass B extends A {}\nlet a: A = B(); let b: Shape = B();");
    // An annotation other than Int, Float, String or Bool guarantees nothing.
    assert_clean("let n: Number = 1;\nlet i: Int = n;");
    assert_clean("let c: Char = 'a';\nlet s: String = c;");
    // `@inline`, async and decorated functions are not trusted.
    assert_clean("@inline\nfn f(): Int { return 1; }\nlet s: String = f();");
    assert_clean("async fn f(): Int { return 1; }\nlet s: String = f();");
    assert_clean("fn d(g) { return g; }\n@d\nfn f(): Int { return 1; }\nlet s: String = f();");
    // A class replaced by a decorator, or one that extends a built-in type.
    assert_clean("fn d(c) { return c; }\n@d\nclass A {}\nlet n: Int = A();");
    assert_clean("class E extends Error {}\nlet s: String = E();");
    // A type that shadows a built-in name.
    assert_clean("type Int = String;\nfn f(): Int { return \"s\"; }\nlet s: String = f();");
}

/// A folder with `files` in it, removed on drop.
struct Folder(std::path::PathBuf);

impl Folder {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir().join(format!("pleno-static-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (file, text) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Folder(dir)
    }

    /// The messages for `source`, as if read from `main.pln` in the folder.
    fn messages(&self, source: &str) -> Vec<String> {
        let tokens = pleno_lexer::lex(source).expect("lex");
        let program = pleno_parser::Parser::new(tokens, source)
            .parse_program()
            .expect("parse");
        pleno_compiler::check::check_program_in(&program, Some(&self.0.join("main.pln")))
            .into_iter()
            .map(|d| d.message)
            .collect()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn imported_functions_and_names() {
    let folder = Folder::new(
        "imports",
        &[(
            "lib/m.pln",
            "export fn helper(a, b = 2) { return a; }\nfn other() {}\nexport { other as renamed };\nexport let name = 1;\nexport let twice = (x) => x * 2;\n",
        )],
    );
    assert_eq!(
        folder.messages("import { helper, nmae } from \"./lib/m.pln\";\nhelper();"),
        [
            "'./lib/m.pln' does not export 'nmae'; did you mean 'name'?",
            "helper expects 1 to 2 arguments, got 0"
        ]
    );
    // Without the extension, with aliases, and through the module object.
    assert_eq!(
        folder.messages(
            "import { renamed as r, twice } from \"./lib/m\";\nimport m from \"./lib/m.pln\";\nr(1); twice(); m.helper(1, 2, 3);"
        ),
        [
            "r expects no arguments, got 1",
            "twice expects 1 argument, got 0",
            "m.helper expects 1 to 2 arguments, got 3"
        ]
    );
    assert!(
        folder
            .messages(
                "import { helper, renamed, name } from \"./lib/m.pln\";\nhelper(1); renamed();"
            )
            .is_empty()
    );
    // Missing files and packages are not read.
    assert!(
        folder
            .messages("import { x } from \"./missing.pln\";\nx(1, 2);")
            .is_empty()
    );
    assert!(
        folder
            .messages("import { x } from \"pkg\";\nx(1, 2);")
            .is_empty()
    );
    // Without a file, nothing is read.
    assert_clean("import { nmae } from \"./lib/m.pln\";");
}

#[test]
fn modules_whose_exports_are_not_all_known() {
    let folder = Folder::new(
        "partial",
        &[
            ("broken.pln", "export fn thing( {\n"),
            (
                "nested.pln",
                "module inner {\n  export fn hidden() {}\n}\nexport fn shown() {}\n",
            ),
            (
                "deco.pln",
                "fn d(f) { return f; }\n@d\nexport fn wrapped(a) {}\n",
            ),
            (
                "renames.pln",
                "type Int = String;\nexport fn f(n: Int) {}\n",
            ),
        ],
    );
    assert!(
        folder
            .messages("import { anything } from \"./broken.pln\";")
            .is_empty()
    );
    assert!(
        folder
            .messages("import { hidden } from \"./nested.pln\";")
            .is_empty()
    );
    assert!(
        folder
            .messages("import { wrapped } from \"./deco.pln\";\nwrapped(1, 2);")
            .is_empty()
    );
    assert!(
        folder
            .messages("import { f } from \"./renames.pln\";\nf(\"s\");")
            .is_empty()
    );
}

#[test]
fn functions_that_catch_what_they_call_expect_failures() {
    assert_clean(
        "fn errorOf(f) { try { f(); } catch (e) { return e; } return null; }\nfn g(n: Int) {}\nerrorOf(() => g(\"s\"));",
    );
    assert_eq!(
        messages("fn run(f) { f(); }\nfn g(n: Int) {}\nrun(() => g(\"s\"));"),
        ["g expects Int for 'n', found a String literal"]
    );
}

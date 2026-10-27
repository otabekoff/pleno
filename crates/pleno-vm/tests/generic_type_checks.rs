//! Runtime checks of annotations that the earlier checks left out: `Void`
//! returns, annotations that mention a type parameter (in returns, and in
//! closures inside generic functions and classes), the type arguments of user
//! generic classes, and `@inline` functions. Each case is tested both ways:
//! right code runs, wrong code throws a `TypeError`. A type parameter that was
//! inferred (no explicit type arguments) never rejects right code.

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

fn eval(source: &str) -> Value {
    compile_and_run(source).unwrap_or_else(|err| panic!("{err}\n--- source:\n{source}"))
}

/// Compares values as printed, since arrays are compared by identity.
fn assert_value(source: &str, expected: Value) {
    assert_eq!(
        eval(source).to_string(),
        expected.to_string(),
        "--- source:\n{source}"
    );
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

fn string(text: &str) -> Value {
    Value::String(text.to_string().into())
}

// --- Void -------------------------------------------------------------------

#[test]
fn void_function_returning_a_value_is_an_error() {
    assert_error(
        "fn f(): Void { return 1; } f();",
        "TypeError: f returned Int, expected Void",
    );
    assert_error(
        r#"class Log { write(): Void { return "done"; } } Log().write();"#,
        "TypeError: Log.write returned String, expected Void",
    );
    assert_error(
        "let g = fn(): Void { return [1]; }; g();",
        "TypeError: lambda returned Array, expected Void",
    );
}

#[test]
fn void_function_returns_null() {
    assert_value("fn f(): Void { return; } f() == null;", Value::Bool(true));
    assert_value(
        "fn f(): Void { return null; } f() == null;",
        Value::Bool(true),
    );
    // The last expression of the body is not the result.
    assert_value("fn f(): Void { 5; } f() == null;", Value::Bool(true));
    assert_value("fn f() { 5; } f();", Value::Int(5));
    // An expression body is run for its effect.
    assert_value(
        "let xs = [1, 2]; let pop = fn(): Void => xs.pop(); [pop(), xs.length];",
        eval("[null, 1];"),
    );
}

// --- Returns that mention a type parameter -----------------------------------

#[test]
fn inferred_type_parameter_accepts_any_return() {
    let source = r#"
        fn id<T>(x: T): T { return x; }
        fn first<T>(xs: T[]): T { return xs[0]; }
        fn wrap<T>(x: T): T[] { return [x]; }
        fn none<T>(x: T): T? { return null; }
        fn pair<A, B>(a: A, b: B): (A, B) { return (a, b); }
        // Wrong for any binding, but nothing says what T is.
        fn loose<T>(x: T): T { return "s"; }
        [id(1), id("s"), first(["a"]), wrap(2), none(3), pair(1, "a"), loose(1)];
    "#;
    assert_value(source, eval(r#"[1, "s", "a", [2], null, (1, "a"), "s"];"#));
}

#[test]
fn explicit_type_argument_checks_the_return() {
    let ok = r#"
        fn first<T>(xs: T[]): T { return xs[0]; }
        fn wrap<T>(x: T): T[] { return [x]; }
        fn id<T>(x: T): T { return x; }
        [first<String>(["a"]), wrap<Int>(1), id<Float>(1)];
    "#;
    assert_value(ok, eval(r#"["a", [1], 1];"#));
    assert_error(
        r#"fn first<T>(xs: T[]): T { return "s"; } first<Int>([1, 2]);"#,
        "TypeError: first returned String, expected T",
    );
    assert_error(
        r#"fn both<T>(x: T): T[] { return [x, "s"]; } both<Int>(1);"#,
        "TypeError: both returned Array, expected T[]",
    );
}

#[test]
fn constrained_type_parameter_checks_the_constraint_when_inferred() {
    let source = r#"
        interface Named { name(): String; }
        class Rock { name(): String { return "rock"; } }
        fn keep<T: Named>(x: T): T { return x; }
        fn lose<T: Named>(x: T): T { return 5; }
        keep(Rock()).name();
    "#;
    assert_value(source, string("rock"));
    assert_error(
        &format!("{source} lose(Rock());"),
        "TypeError: lose returned Int, expected T",
    );
}

#[test]
fn class_type_parameter_in_a_method_return() {
    let class = r#"
        class Box<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            unwrap(): T { return this.value; }
            broken(): T { return "s"; }
        }
    "#;
    assert_value(
        &format!("{class} [Box<Int>(1).unwrap(), Box(2).broken()];"),
        eval(r#"[1, "s"];"#),
    );
    assert_error(
        &format!("{class} Box<Int>(1).broken();"),
        "TypeError: Box.broken returned String, expected T",
    );
}

// --- Closures inside generic functions and classes --------------------------

#[test]
fn lambda_parameter_of_an_enclosing_type_parameter() {
    let source = r#"
        fn apply<T>(x: T, y) { let g = (z: T) => z; return g(y); }
    "#;
    assert_value(
        &format!("{source} [apply(1, \"s\"), apply<Int>(1, 2)];"),
        eval(r#"["s", 2];"#),
    );
    assert_error(
        &format!("{source} apply<Int>(1, \"s\");"),
        "TypeError: parameter 'z' expected T, got String",
    );
}

#[test]
fn lambda_return_and_let_of_an_enclosing_type_parameter() {
    let source = r#"
        fn give<T>(x: T, y) { let g = fn(): T => y; return g(); }
        fn hold<T>(x: T, y) { let g = () => { let z: T = y; return z; }; return g(); }
        fn nest<T>(x: T, y) {
            let g = () => { let h = (z: T[]) => z; return h([y]); };
            return g();
        }
    "#;
    assert_value(
        &format!("{source} [give(1, \"a\"), hold(1, \"b\"), nest(1, \"c\"), give<Int>(1, 2)];"),
        eval(r#"["a", "b", ["c"], 2];"#),
    );
    assert_error(
        &format!("{source} give<Int>(1, \"s\");"),
        "TypeError: lambda returned String, expected T",
    );
    assert_error(&format!("{source} hold<Int>(1, \"s\");"), "expected T");
    assert_error(
        &format!("{source} nest<Int>(1, \"s\");"),
        "TypeError: parameter 'z' expected T[], got Array",
    );
}

#[test]
fn nested_function_of_an_enclosing_type_parameter() {
    let source = r#"
        fn outer<T>(x: T, y) { fn inner(z: T) { return z; } return inner(y); }
    "#;
    assert_value(&format!("{source} outer(1, \"s\");"), string("s"));
    assert_error(
        &format!("{source} outer<Int>(1, \"s\");"),
        "TypeError: parameter 'z' expected T, got String",
    );
}

#[test]
fn lambda_in_a_method_of_a_generic_class() {
    let source = r#"
        class Box<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            check(y) { let g = (z: T) => z; return g(y); }
        }
    "#;
    assert_value(&format!("{source} Box(1).check(\"s\");"), string("s"));
    assert_error(
        &format!("{source} Box<Int>(1).check(\"s\");"),
        "TypeError: parameter 'z' expected T, got String",
    );
}

#[test]
fn inferred_constraint_does_not_narrow_a_closure() {
    // T was inferred from a Rock; a Tree is a `Named` too, so it is accepted.
    let source = r#"
        interface Named { name(): String; }
        class Rock { name(): String { return "rock"; } }
        class Tree { name(): String { return "tree"; } }
        fn call<T: Named>(x: T, y) { let g = (z: T) => z.name(); return g(y); }
        call(Rock(), Tree());
    "#;
    assert_value(source, string("tree"));
}

#[test]
fn type_parameter_comes_before_a_global_of_the_same_name() {
    let source = r#"
        class T {}
        fn id<T>(x: T): T { return x; }
        id("s");
    "#;
    assert_value(source, string("s"));
}

#[test]
fn class_inside_a_generic_function_is_not_second_guessed() {
    // The method's frame cannot see the function's T, so it is not checked.
    let source = r#"
        fn make<T>(x: T) {
            class Local { fetch(): T { return "s"; } }
            return Local().fetch();
        }
        make<Int>(1);
    "#;
    assert_value(source, string("s"));
}

// --- Type arguments of user generic classes ---------------------------------

const BOX: &str = r#"
    class Box<T> {
        value: T;
        constructor(value: T) { this.value = value; }
    }
"#;

#[test]
fn constructor_and_field_check_the_type_argument() {
    assert_error(
        &format!("{BOX} Box<Int>(\"s\");"),
        "TypeError: parameter 'value' expected T, got String",
    );
    assert_error(
        &format!("{BOX} let b = Box<Int>(1); b.value = \"s\";"),
        "TypeError: field 'value' expected T, got String",
    );
    assert_value(&format!("{BOX} Box<Float>(1).value;"), Value::Int(1));
    // Set from outside the class: the instance's type argument is used.
    assert_value(
        &format!(
            "{BOX} let b = Box<Int>(1); b.value = 2; let c = Box(1); c.value = \"s\"; [b.value, c.value];"
        ),
        eval(r#"[2, "s"];"#),
    );
}

#[test]
fn field_of_a_composite_type_uses_the_type_argument() {
    let source = r#"
        class Stack<T> {
            items: T[];
            constructor() { this.items = []; }
        }
        let s = Stack<Int>();
        s.items = [1, 2];
    "#;
    assert_value(&format!("{source} s.items.length;"), Value::Int(2));
    assert_error(
        &format!("{source} s.items = [\"a\"];"),
        "TypeError: field 'items' expected T[], got Array",
    );
}

#[test]
fn parameter_checks_type_arguments_and_accepts_inferred_ones() {
    let source = format!(
        r#"{BOX}
        fn take(b: Box<Int>) {{ return b.value; }}
        fn takeAny<T>(b: Box<T>): T {{ return b.value; }}
    "#
    );
    // `Box(1)` was made without type arguments, so it may be a `Box<Int>`.
    assert_value(
        &format!(
            "{source} [take(Box<Int>(1)), take(Box(2)), takeAny(Box<Int>(3)), takeAny(Box(4)), takeAny<Int>(Box<Int>(5))];"
        ),
        eval("[1, 2, 3, 4, 5];"),
    );
    assert_error(
        &format!("{source} take(Box<String>(\"s\"));"),
        "TypeError: parameter 'b' expected Box<Int>, got Box",
    );
    // Without type arguments, the fields annotated `T` must fit.
    assert_error(
        &format!("{source} take(Box(\"s\"));"),
        "TypeError: parameter 'b' expected Box<Int>, got Box",
    );
    assert_error(
        &format!("{source} takeAny<String>(Box<Int>(1));"),
        "TypeError: parameter 'b' expected Box<T>, got Box",
    );
}

#[test]
fn return_checks_type_arguments() {
    let source = format!(
        r#"{BOX}
        fn good(): Box<Int> {{ return Box<Int>(1); }}
        fn inferred(): Box<Int> {{ return Box(2); }}
        fn wrong(): Box<Int> {{ return Box<String>("s"); }}
        fn wrongInferred(): Box<Int> {{ return Box("s"); }}
        fn maybe(): Box<Int>? {{ return null; }}
    "#
    );
    assert_value(
        &format!("{source} [good().value, inferred().value, maybe()];"),
        eval("[1, 2, null];"),
    );
    assert_error(
        &format!("{source} wrong();"),
        "TypeError: wrong returned Box, expected Box<Int>",
    );
    assert_error(
        &format!("{source} wrongInferred();"),
        "TypeError: wrongInferred returned Box, expected Box<Int>",
    );
}

// --- @inline ----------------------------------------------------------------

#[test]
fn inline_function_keeps_its_parameter_checks() {
    let source = r#"
        @inline
        fn add(a: Int, b: Int): Int { return a + b; }
        @inline
        fn half(x: Float): Float { return x / 2; }
    "#;
    assert_value(
        &format!("{source} let n = 3; [add(1, 2), half(n), typeof(half(n)), n];"),
        eval(r#"[3, 1.5, "Float", 3];"#),
    );
    assert_error(
        &format!("{source} add(\"a\", 2);"),
        "TypeError: parameter 'a' expected Int, got String",
    );
    // Inside a function the call is not inlined, and checked the same way.
    assert_error(
        &format!("{source} fn g() {{ return add(1, 2.5); }} g();"),
        "TypeError: parameter 'b' expected Int, got Float",
    );
}

#[test]
fn inline_function_keeps_its_return_checks() {
    let source = r#"
        @inline
        fn sign(a: Int): Int { if (a > 0) { return "pos"; } return a; }
        @inline
        fn tail(a: Int): Int { a; }
        @inline
        fn quiet(): Void { return 1; }
    "#;
    assert_value(&format!("{source} [sign(-1), tail(2)];"), eval("[-1, 2];"));
    assert_error(
        &format!("{source} sign(1);"),
        "TypeError: sign returned String, expected Int",
    );
    assert_error(
        &format!("{source} quiet();"),
        "TypeError: quiet returned Int, expected Void",
    );
}

#[test]
fn inline_function_behaves_like_a_call() {
    let source = r#"
        let y = 10;
        @inline
        fn readY(): Int { return y; }
        @inline
        fn bump(x) { x = x + 1; return x; }
        @inline
        fn pick(a) { if (a > 0) { return "pos"; } return "neg"; }
        @inline
        fn twice(a: Int, b: Int = 10): Int { let t = a * 2; return t + b; }
        @inline
        fn fact(n: Int): Int { if (n <= 1) { return 1; } return n * fact(n - 1); }
        let mut a = 1;
        let results = [bump(a), a, pick(1), pick(-1), twice(1), twice(1, 0), fact(5)];
        {
            let y = "shadowed";
            results.push(readY());
        }
        results;
    "#;
    assert_value(source, eval(r#"[2, 1, "pos", "neg", 12, 2, 120, 10];"#));
}

#[test]
fn many_inlined_calls_in_a_row() {
    let mut source = String::from(
        "@inline\nfn twice(a: Int): Int { if (a > 1000) { return a; } return a * 2; }\n\
         @inline\nfn quiet(): Void { }\nlet mut total = 0;\n",
    );
    for i in 0..400 {
        source.push_str(&format!("total = total + twice({i}); quiet();\n"));
    }
    source.push_str("total;\n");
    assert_value(&source, Value::Int(159600));
}

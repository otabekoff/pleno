//! Runtime type annotations: `Int` widening to `Float`, return annotations,
//! intersection types and `&mut` parameters.

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

fn eval_string(source: &str) -> String {
    match eval(source) {
        Value::String(s) => s.to_string(),
        other => panic!("expected a string, got {other:?}\n--- source:\n{source}"),
    }
}

fn error_of(source: &str) -> String {
    match compile_and_run(source) {
        Ok(value) => panic!("expected an error, got {value:?}\n--- source:\n{source}"),
        Err(err) => err,
    }
}

fn assert_error(source: &str, expected: &str) {
    let err = error_of(source);
    assert!(
        err.contains(expected),
        "expected error containing {expected:?}, got {err:?}\n--- source:\n{source}"
    );
}

// --- Int where Float is annotated -------------------------------------------

#[test]
fn int_argument_for_float_parameter_becomes_float() {
    let source = "fn f(x: Float): String { return typeof(x); } f(3);";
    assert_eq!(eval_string(source), "Float");
    assert_eq!(
        eval("fn half(x: Float): Float { return x / 2; } half(7);"),
        Value::Float(3.5)
    );
}

#[test]
fn int_for_float_let_and_assignment_becomes_float() {
    assert_eq!(eval_string("let y: Float = 3; typeof(y);"), "Float");
    assert_eq!(eval("let y: Float = 3; y;"), Value::Float(3.0));
    assert_eq!(
        eval_string("let mut y: Float = 1.5; y = 2; typeof(y);"),
        "Float"
    );
    // The source variable keeps its own value and type.
    assert_eq!(
        eval_string("let x = 1; let y: Float = x; typeof(x) + typeof(y);"),
        "IntFloat"
    );
}

#[test]
fn int_for_optional_union_and_alias_of_float() {
    assert_eq!(eval_string("let z: Float? = 3; typeof(z);"), "Float");
    assert_eq!(eval("let z: Float? = null; z == null;"), Value::Bool(true));
    assert_eq!(
        eval_string("let s: Float | String = 3; typeof(s);"),
        "Float"
    );
    assert_eq!(
        eval_string("type Money = Float; let m: Money = 3; typeof(m);"),
        "Float"
    );
    let nested_alias = r#"
        type Money = Float;
        fn f(a: Money[], b: Money | String, c: Money?): String { return typeof(c); }
        f([1], "x", 2);
    "#;
    assert_eq!(eval_string(nested_alias), "Float");
    let alias_field = r#"
        type Money = Float;
        class Wallet { amount: Money; constructor(a) { this.amount = a; } }
        typeof(Wallet(3).amount);
    "#;
    assert_eq!(eval_string(alias_field), "Float");
    // In a union that contains Int, an Int stays an Int.
    assert_eq!(eval_string("let u: Int | Float = 3; typeof(u);"), "Int");
    assert_eq!(eval_string("let n: Number = 3; typeof(n);"), "Int");
    assert_eq!(
        eval_string("fn f(x: String | Int | Float): String { return typeof(x); } f(3);"),
        "Int"
    );
}

#[test]
fn float_array_accepts_ints_without_converting() {
    assert_eq!(
        eval_string("let a: Float[] = [1, 2.5]; typeof(a[0]);"),
        "Int"
    );
    assert_eq!(
        eval_string("fn f(a: Float[]): String { return typeof(a[0]); } f([1]);"),
        "Int"
    );
}

#[test]
fn int_for_float_field_becomes_float() {
    let source = r#"
        class Point {
            x: Float;
            constructor(x) { this.x = x; }
        }
        let p = Point(3);
        typeof(p.x);
    "#;
    assert_eq!(eval_string(source), "Float");
}

#[test]
fn float_annotation_still_rejects_other_types() {
    assert_error(
        r#"fn f(x: Float) { return x; } f("3");"#,
        "TypeError: parameter 'x' expected Float, got String",
    );
    assert_error(
        r#"let y: Float = "3";"#,
        "TypeError: expected Float, found String",
    );
    assert_error("let i: Int = 3.5;", "TypeError: expected Int, found Float");
}

// --- Return annotations ------------------------------------------------------

#[test]
fn wrong_return_value_is_a_type_error() {
    assert_error(
        r#"fn f(): Int { return "s"; } f();"#,
        "TypeError: f returned String, expected Int",
    );
}

#[test]
fn early_and_implicit_returns_are_checked() {
    let early = r#"
        fn f(x: Int): Int {
            if (x > 0) { return "big"; }
            return 0;
        }
        f(0);
        f(1);
    "#;
    assert_error(early, "TypeError: f returned String, expected Int");
    let implicit = r#"
        fn f(x: Int): Int {
            if (x > 0) { return 1; }
        }
        f(1);
        f(0);
    "#;
    assert_error(implicit, "TypeError: f returned Null, expected Int");
}

#[test]
fn matching_returns_pass_and_widen() {
    assert_eq!(eval("fn f(): Int { return 4; } f();"), Value::Int(4));
    assert_eq!(
        eval_string("fn f(): Float { return 2; } typeof(f());"),
        "Float"
    );
    assert_eq!(
        eval("fn f(): Int? { return null; } f() == null;"),
        Value::Bool(true)
    );
    assert_eq!(eval("fn f(): Int? { } f() == null;"), Value::Bool(true));
    assert_eq!(
        eval(r#"fn f(): Int | String { return "a"; } f();"#),
        Value::String("a".to_string().into())
    );
    assert_eq!(
        eval("fn f(): Void { print(1); } f() == null;"),
        Value::Bool(true)
    );
}

#[test]
fn method_and_lambda_returns_are_checked() {
    let method = r#"
        class Counter {
            count(): Int { return "many"; }
        }
        Counter().count();
    "#;
    assert_error(
        method,
        "TypeError: Counter.count returned String, expected Int",
    );
    assert_error(
        r#"let g = fn(): Int => "s"; g();"#,
        "TypeError: lambda returned String, expected Int",
    );
    assert_eq!(
        eval("let g = fn(x: Int): Int => x + 1; g(1);"),
        Value::Int(2)
    );
}

#[test]
fn inferred_generic_returns_are_accepted() {
    let source = r#"
        fn id<T>(x: T): T { return x; }
        class Box<T> {
            value: T;
            constructor(value: T) { this.value = value; }
            unwrap(): T { return this.value; }
        }
        id(1);
        id("s");
        Box("a").unwrap();
    "#;
    assert_eq!(eval(source), Value::String("a".to_string().into()));
}

/// The value a script's final promise resolved to.
fn resolved(source: &str) -> Value {
    match eval(source) {
        Value::Map(map) => {
            let map = map.borrow();
            assert_eq!(
                map.get("variant"),
                Some(&Value::String("Resolved".to_string().into())),
                "promise not resolved\n--- source:\n{source}"
            );
            map.get("value").cloned().unwrap_or(Value::Null)
        }
        other => panic!("expected a promise, got {other:?}"),
    }
}

#[test]
fn async_return_annotation_describes_the_resolved_value() {
    let ok = r#"
        async fn one(): Int { return 1; }
        async fn two(): Promise<Int> { return 2; }
        async fn main(): Int { return await one() + await two(); }
        main();
    "#;
    assert_eq!(resolved(ok), Value::Int(3));
    let bad = r#"
        async fn one(): Int { return "one"; }
        async fn main() {
            try { await one(); return "no error"; } catch (e) { return e.message; }
        }
        main();
    "#;
    assert_eq!(
        resolved(bad),
        Value::String("one returned String, expected Int".to_string().into())
    );
}

// --- Intersection types ------------------------------------------------------

const PERSON: &str = r#"
    interface Nameable { fn getName(): String; }
    interface Ageable { fn getAge(): Int; }
    type Person = Nameable & Ageable;
    class Employee implements Nameable, Ageable {
        fn getName(): String { return "Ann"; }
        fn getAge(): Int { return 40; }
    }
    class Pet implements Nameable {
        fn getName(): String { return "Rex"; }
    }
    fn describe(p: Person): String { return p.getName() + " " + p.getAge(); }
"#;

#[test]
fn intersection_accepts_values_satisfying_every_part() {
    assert_eq!(
        eval_string(&format!("{PERSON} describe(Employee());")),
        "Ann 40"
    );
    assert_eq!(
        eval_string(&format!(
            "{PERSON} let p: Person = Employee(); p.getName();"
        )),
        "Ann"
    );
}

#[test]
fn intersection_rejects_values_missing_a_part() {
    assert_error(
        &format!("{PERSON} describe(Pet());"),
        "TypeError: parameter 'p' expected Person, got Pet",
    );
}

// --- &mut parameters ---------------------------------------------------------

#[test]
fn assigning_a_mut_borrowed_parameter_is_a_compile_error() {
    let source = r#"
        fn shout(value: &mut String) { value = value + "!"; }
        let mut s = "hi";
        shout(s);
    "#;
    let err = error_of(source);
    assert!(
        err.contains("cannot assign to '&mut' parameter 'value'"),
        "unexpected error: {err}"
    );
    // Mutating a reference value in place is what `&mut` is for.
    let in_place = r#"
        fn add(items: &mut Int[]) { items.push(4); }
        let mut xs = [1, 2, 3];
        add(xs);
        xs.length;
    "#;
    assert_eq!(eval(in_place), Value::Int(4));
}

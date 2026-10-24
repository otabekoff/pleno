//! Regressions for language and standard library behavior the docs describe.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<Value, String> {
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
    eval_result(source).unwrap_or_else(|err| panic!("{err}\n--- source:\n{source}"))
}

fn eval_string(source: &str) -> String {
    match eval(source) {
        Value::String(s) => s.to_string(),
        other => panic!("expected a string, got {other:?}"),
    }
}

#[test]
fn backtick_strings_allow_quotes_and_interpolate() {
    assert_eq!(
        eval_string(r#"let name = "Al"; `{"name": "${name}"}`;"#),
        r#"{"name": "Al"}"#
    );
    assert_eq!(eval_string("`a\\`b`;"), "a`b");
}

#[test]
fn escaped_dollar_is_not_interpolation() {
    assert_eq!(eval_string(r#"let x = 5; "\${x} ${x}";"#), "${x} 5");
}

#[test]
fn assert_true_false_and_null_are_callable() {
    let source = r#"
        import { assert } from "std:test";
        assert.true(1 < 2);
        assert.false(2 < 1);
        assert.null(null);
        1;
    "#;
    assert_eq!(eval(source), Value::Int(1));
}

#[test]
fn enum_methods_after_semicolon_and_return_in_match_arms() {
    let source = r#"
        enum Direction {
            North,
            South;

            fn opposite(): Direction {
                match (this) {
                    case Direction.North => return Direction.South,
                    case Direction.South => return Direction.North
                }
            }
        }
        Direction.North.opposite() == Direction.South;
    "#;
    assert_eq!(eval(source), Value::Bool(true));
}

#[test]
fn generic_type_aliases_expand_in_functions() {
    let source = r#"
        type Table<V> = Map<String, V>;
        fn f() { let t: Table<Int> = {"a": 1}; return t["a"]; }
        f();
    "#;
    assert_eq!(eval(source), Value::Int(1));
    let err =
        eval_result(r#"type Table<V> = Map<String, V>; fn f() { let t: Table<Int> = 3; } f();"#)
            .unwrap_err();
    assert!(err.contains("expected Table<Int>, found Int"), "{err}");
}

#[test]
fn enum_values_match_their_enum_type() {
    let source = r#"
        enum Color { Red, Green }
        enum Shape<T> { Circle(T) }
        let c: Color = Color.Red;
        let s: Shape<Int> = Shape.Circle(1);
        fn pick(x: Color): Color { return x; }
        pick(Color.Green) == Color.Green;
    "#;
    assert_eq!(eval(source), Value::Bool(true));
    let err = eval_result("enum A { X } enum B { Y } let a: A = B.Y;").unwrap_err();
    assert!(err.contains("expected A, found B"), "{err}");
}

#[test]
fn result_and_option_annotations() {
    let source = r#"
        let r: Result<Int, String> = Ok(1);
        let o: Option<Int> = None;
        fn f(): Option<Int> { return Some(3); }
        f().unwrap() + r.unwrap();
    "#;
    assert_eq!(eval(source), Value::Int(4));
    assert!(eval_result("let r: Result<Int, String> = 5;").is_err());
}

#[test]
fn a_skipped_typed_catch_does_not_clobber_the_error() {
    let source = r#"
        class A extends Error {}
        let mut out = "";
        try {
            throw Error("boom");
        } catch (e: A) {
            out = "A";
        } catch (e: Error) {
            out = e.message;
        }
        out;
    "#;
    assert_eq!(eval_string(source), "boom");
}

#[test]
fn pipeline_inserts_the_value_as_first_argument_or_at_the_placeholder() {
    assert_eq!(
        eval("fn add(a, b) { return a + b; } 5 |> add(1);"),
        Value::Int(6)
    );
    assert_eq!(
        eval("fn sub(a, b) { return a - b; } 5 |> sub(10, _);"),
        Value::Int(5)
    );
    assert_eq!(
        eval(
            "[1, 2, 3, 4] |> filter(fn(x) => x % 2 == 0) |> map(fn(x) => x * x) |> reduce(fn(a, x) => a + x, 0);"
        ),
        Value::Int(20)
    );
}

#[test]
fn functions_have_a_name() {
    assert_eq!(
        eval_string("fn calculate() {} calculate.name;"),
        "calculate"
    );
}

#[test]
fn collection_helpers() {
    assert_eq!(eval("let s = {1}; s.isEmpty();"), Value::Bool(false));
    assert_eq!(eval("let m = {\"a\": 1}; m.isEmpty();"), Value::Bool(false));
    assert_eq!(eval("(0..10).contains(10);"), Value::Bool(false));
    assert_eq!(eval("(0..=10).contains(10);"), Value::Bool(true));
    assert_eq!(eval("(1..=3).toArray().length;"), Value::Int(3));
    assert_eq!(
        eval("let a = [1]; let b = a.clone(); b.push(2); a.length;"),
        Value::Int(1)
    );
}

#[test]
fn iterating_a_channel_waits_for_values_until_it_is_closed() {
    let source = r#"
        import { Channel } from "std:concurrency";
        let ch = Channel(2);
        spawn(fn() {
            for (i in 0..5) { ch.send(i); }
            ch.close();
        });
        let mut sum = 0;
        for (item in ch) { sum += item; }
        assert.equal(sum, EXPECTED);
    "#;
    // The script suspends while it waits, so its final value is not the
    // sum; the assertion runs after the loop instead.
    let source = format!(
        "import {{ assert }} from \"std:test\";
{source}"
    );
    eval_result(&source.replace("EXPECTED", "10")).unwrap();
    let err = eval_result(&source.replace("EXPECTED", "11")).unwrap_err();
    assert!(err.contains("Assertion failed"), "{err}");
}

#[test]
fn contravariant_interfaces_accept_supertypes() {
    let source = r#"
        class Animal {}
        class Dog extends Animal {}
        interface Consumer<in T> { fn consume(item: T): Void; }
        class AnimalConsumer implements Consumer<Animal> { fn consume(item: Animal): Void {} }
        let c: Consumer<Dog> = AnimalConsumer();
        1;
    "#;
    assert_eq!(eval(source), Value::Int(1));
}

#[test]
fn class_annotations_are_metadata_not_decorators() {
    let source = r#"
        @annotation
        class Route {
            public path: String;
            constructor(path: String) { this.path = path; }
        }
        @Route("/users")
        fn users() { return 7; }
        users();
    "#;
    assert_eq!(eval(source), Value::Int(7));
}

#[test]
fn generic_extensions() {
    let source = r#"
        extend <T> Array<T> {
            fn second(): T? { return this.length > 1 ? this[1] : null; }
        }
        [4, 5].second();
    "#;
    assert_eq!(eval(source), Value::Int(5));
}

#[test]
fn log_level_can_be_set_and_read() {
    let source = r#"
        import Log from "std:log";
        Log.setLevel("warn");
        let level = Log.getLevel();
        Log.setLevel("info");
        level;
    "#;
    assert_eq!(eval_string(source), "warn");
}

#[test]
fn datetime_and_duration() {
    let source = r#"
        import { DateTime, Duration } from "std:time";
        let d = DateTime.parse("2026-02-03", "%Y-%m-%d");
        let next = d.add(Duration.days(1));
        next.format("%Y-%m-%d") + " " + next.isAfter(d).toString() + " " + Duration.hours(2).getSeconds().toString();
    "#;
    assert_eq!(eval_string(source), "2026-02-04 true 7200");
}

#[test]
fn json_pretty_printing_and_regex_split() {
    assert_eq!(
        eval_string(
            r#"import { JSON } from "std:json"; JSON.stringifyPretty({"a": 1}, indent: 4);"#
        ),
        "{\n    \"a\": 1\n}"
    );
    assert_eq!(
        eval(r#"import { Regex } from "std:regex"; Regex(r",\s*").split("a, b,  c").length;"#),
        Value::Int(3)
    );
}

#[test]
fn std_modules_offer_namespaces() {
    let source = r#"
        import { Path } from "std:path";
        import { Process } from "std:process";
        import { Promise, spawn } from "std:promise";
        import { Ok } from "std:core";
        Path.basename("a/b.pln") + " " + (Process.cwd() != null).toString() + " " + Ok(1).unwrap().toString();
    "#;
    assert_eq!(eval_string(source), "b.pln true 1");
}

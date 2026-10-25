//! `object.name(args)` compiles to `GetMethod` + `CallMethod`, which call a
//! method without allocating a bound method. These tests pin down that such
//! calls behave as reading the property and calling its value did.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn run(source: &str) -> Result<(Value, String), String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    let value = vm.execute(chunk).map_err(|e| e.message)?;
    Ok((value, vm.output.take().unwrap_or_default()))
}

/// What the program printed.
fn output(source: &str) -> String {
    match run(source) {
        Ok((_, printed)) => printed,
        Err(err) => panic!("{err}\n{source}"),
    }
}

fn error(source: &str) -> String {
    match run(source) {
        Ok(result) => panic!("expected an error, got {result:?}\n{source}"),
        Err(err) => err,
    }
}

const CLASSES: &str = r#"
class Base {
    private secret: Int;
    protected tag: String;
    constructor(s: Int) { this.secret = s; this.tag = "base"; }
    fn value(): Int { return this.secret; }
    fn add(a, b = 10) { return this.secret + a + b; }
    private fn hidden(): Int { return 42; }
    fn callHidden(): Int { return this.hidden(); }
    protected fn prot(): String { return "prot " + this.tag; }
    fn chain() { return this; }
    fn rest(...xs) { return xs.length; }
    static fn make(): Base { return Base(7); }
    property adder {
        get { let s = this.secret; return |x| => x + s; }
    }
}
class Child extends Base {
    constructor(s: Int) { super(s); }
    fn useProt(): String { return this.prot(); }
    fn value(): Int { return super.value() + 1000; }
    fn tryHidden() { return this.hidden(); }
}
let b = Base(5);
let c = Child(1);
"#;

#[test]
fn class_methods_defaults_variadics_and_statics() {
    let source = format!(
        "{CLASSES}
        print(b.value(), b.add(1), b.add(1, 2), b.callHidden(), b.chain().chain().value());
        print(b.rest(1, 2, 3), Base.make().value(), b.adder(3));
        print(c.value(), c.useProt(), c.add(2));"
    );
    assert_eq!(output(&source), "5 16 8 42 5\n3 7 8\n1001 prot base 13\n");
}

#[test]
fn method_visibility_is_checked_against_the_declaring_class() {
    for call in ["b.hidden();", "b.prot();", "c.tryHidden();"] {
        let err = error(&format!("{CLASSES}\n{call}"));
        assert!(err.contains("method access denied"), "{call}: {err}");
    }
    let err = error(&format!("{CLASSES}\nb.missing();"));
    assert!(
        err.contains("cannot read property 'missing' of Object"),
        "{err}"
    );
}

#[test]
fn fields_holding_functions_are_called_and_read_before_the_arguments() {
    let source = r#"
        class Holder {
            f;
            constructor() { this.f = |x| => x * 3; }
            fn swap() { this.f = |x| => x * 100; return 1; }
        }
        let h = Holder();
        print(h.f(2), h.f(h.swap()), h.f(2));
    "#;
    assert_eq!(output(source), "6 3 200\n");
}

#[test]
fn getters_run_before_the_arguments() {
    let source = r#"
        let mut log = [];
        class G {
            property fun {
                get { log.push("get"); return |x| => x; }
            }
        }
        let g = G();
        g.fun(log.push("arg"));
        print(log);
    "#;
    assert_eq!(output(source), "[get, arg]\n");
}

#[test]
fn maps_builtins_and_extensions() {
    let source = r#"
        let m = {"f": |x| => x + 1, "k": 5};
        print(m.f(1), m.get("k"), m.containsKey("k"));
        let arr = [3, 1, 2];
        arr.push(9);
        print(arr, arr.map(|x| => x * 2), "abc".toUpperCase(), 5.toString(), 'x'.toUpperCase());
        extend Array {
            fn second() { return this[1]; }
            fn push(x) { return "shadowed"; }
        }
        extend String {
            fn shout() { return this.toUpperCase() + "!"; }
        }
        print(arr.second(), "hi".shout(), arr.push(1), arr.length);
    "#;
    assert_eq!(
        output(source),
        "2 5 true\n[3, 1, 2, 9] [6, 2, 4, 18] ABC 5 X\n1 HI! shadowed 4\n"
    );
}

#[test]
fn missing_methods_are_described() {
    let err = error(r#"let m = {"k": 1}; m.contains("k");"#);
    assert!(
        err.contains("Map has no method 'contains' (and no key 'contains')"),
        "{err}"
    );
    let err = error("[1].nope();");
    assert!(
        err.contains("cannot read property 'nope' of Array"),
        "{err}"
    );
    let err = error("let n = null; n.foo();");
    assert!(err.contains("cannot read property 'foo' of Null"), "{err}");
    let err = error(r#"let m = {"k": 1}; m.k();"#);
    assert!(err.contains("not callable"), "{err}");
}

#[test]
fn optional_chains_super_enums_and_errors() {
    let source = format!(
        r#"{CLASSES}
        let maybe = null;
        print(maybe?.value(), b?.value(), maybe?.value()?.x);
        enum Color {{
            Red,
            fn label() {{ return "color"; }}
        }}
        print(Color.Red.label(), Error("boom").toString());"#
    );
    assert_eq!(output(&source), "null 5 null\ncolor Error: boom\n");
}

#[test]
fn async_named_and_spread_method_calls() {
    let source = r#"
        class A { async fn get(x) { return x + 1; } }
        async fn main() { let v = await A().get(1); print("async", v); }
        main();
        class N { fn sub(a, b) { return a - b; } }
        let n = N();
        print(n.sub(b: 1, a: 10), n.sub(...[5, 2]));
    "#;
    assert_eq!(output(source), "9 3\nasync 2\n");
}

#[test]
fn recursive_methods_and_bound_methods() {
    let source = r#"
        class Fib { fn fib(k) { if k < 2 { return k; } return this.fib(k - 1) + this.fib(k - 2); } }
        let f = Fib();
        let bound = f.fib;
        print(f.fib(20), bound(10));
    "#;
    assert_eq!(output(source), "6765 55\n");
}

#[test]
fn method_calls_survive_collections() {
    // Small collection threshold: the receiver and arguments live only in the
    // registers `GetMethod` filled while the call runs.
    let source = r#"
        class Box { v; constructor(v) { this.v = v; } fn get() { return this.v; } }
        let mut total = 0;
        let items = [];
        for i in 0..500 {
            items.push(Box(i));
            total = total + items[i].get() + [i].map(|x| => x).length;
        }
        print(total, items.length);
    "#;
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.gc.gc.allocation_threshold = 64;
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap();
    assert_eq!(vm.output.unwrap(), "125250 500\n");
}

//! Calls whose arguments are passed in registers (`CallRegs`) and the fast
//! paths for calling and returning from bytecode functions.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn run(source: &str, gc_threshold: Option<usize>) -> Result<Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    if let Some(threshold) = gc_threshold {
        vm.gc.gc.allocation_threshold = threshold;
    }
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

fn eval(source: &str) -> Value {
    run(source, None).unwrap_or_else(|err| panic!("{err}\n{source}"))
}

#[test]
fn recursion_and_arguments() {
    let source = "fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } fib(20);";
    assert_eq!(eval(source), Value::Int(6765));
    let source = "fn sub(a, b, c) { return a - b - c; } sub(10, 3, 2);";
    assert_eq!(eval(source), Value::Int(5));
}

#[test]
fn defaults_fill_missing_arguments() {
    let source = "fn f(a, b = 10, c = 100) { return a + b + c; } f(1) + f(1, 2) * 1000;";
    assert_eq!(eval(source), Value::Int(111 + 103 * 1000));
}

#[test]
fn closures_keep_their_captures() {
    let source = "
        fn counter(start) {
            let mut n = start;
            return () => { n = n + 1; return n; };
        }
        let next = counter(5);
        next();
        next();
        let add = |x, y| => x + y;
        add(next(), 100);
    ";
    assert_eq!(eval(source), Value::Int(108));
}

#[test]
fn argument_count_errors() {
    let err = run("fn add(a, b) { return a + b; } add(1);", None).unwrap_err();
    assert!(err.contains("missing required argument"), "{err}");
    let err = run("fn add(a, b) { return a + b; } add(1, 2, 3);", None).unwrap_err();
    assert!(err.contains("too many positional arguments"), "{err}");
}

#[test]
fn missing_method_is_described() {
    let err = run("let m = {\"a\": 1}; m.contains(\"a\");", None).unwrap_err();
    assert!(err.contains("Map has no method 'contains'"), "{err}");
}

#[test]
fn natives_variadics_and_methods_still_work() {
    let source = "
        fn total(...xs) { let mut s = 0; for (x in xs) { s = s + x; } return s; }
        class P { constructor(x) { this.x = x; } scale(k) { return this.x * k; } }
        let p = P(4);
        total(1, 2, 3) + p.scale(10) + [1, 2, 3].map(|x| => x * 2).length;
    ";
    assert_eq!(eval(source), Value::Int(6 + 40 + 3));
}

#[test]
fn values_only_in_registers_survive_collections() {
    // A tiny threshold collects between almost every instruction, while the
    // arrays below live only in the registers of the frames on the stack.
    let source = "
        fn build(n) {
            let items = [n, n + 1];
            if n == 0 { return items; }
            let rest = build(n - 1);
            let both = [items, rest];
            return both;
        }
        fn depth(x) {
            if x.length == 2 && x[0] is Int { return x[0]; }
            return depth(x[1]) + x[0][0];
        }
        let mut sum = 0;
        for (i in 0..5) { sum = sum + depth(build(30)); }
        sum;
    ";
    let expected = (0..=30).sum::<i64>() * 5;
    let result = run(source, Some(64)).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(result, Value::Int(expected));
}

#[test]
fn native_callbacks_return_what_the_callback_returns() {
    // A callback's first argument once came back in place of a `null` result
    // when it was a map with a "variant" key (a `Result`), as if it were the
    // promise of an async callback.
    let source = "
        let rs = [Ok(1), Err(\"x\")];
        let a = rs.map(|r| => null);
        fn f(r) { return null; }
        let b = rs.map(f);
        let mut n = a.length + b.length;
        if a[0] == null { n = n + 10; }
        if b[1] == null { n = n + 100; }
        n;
    ";
    assert_eq!(eval(source), Value::Int(114));
    // Callbacks with defaults, closures over locals, and nested callbacks.
    let source = "
        let k = 3;
        fn scaled(x, y = 10) { return x * 3 + y; }
        let xs = [1, 2, 3].map(scaled);
        let nested = [[1, 2], [3]].map(|row| => row.map(|v| => v + k).reduce(|a, b| => a + b, 0));
        xs.reduce(|a, b| => a + b, 0) * 100 + nested[0] * 10 + nested[1];
    ";
    assert_eq!(eval(source), Value::Int(4800 + 90 + 6));
}

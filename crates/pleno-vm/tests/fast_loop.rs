//! The small interpreter loop (`run_fast`) runs simple instructions unless a
//! debugger is attached or `VM::fast_loop` is off. Every program here runs
//! both ways and must give the same result, the same output and the same
//! instruction count, also under sandbox limits.

use pleno_compiler::Compiler;
use pleno_value::{SandboxConfig, Value};
use pleno_vm::VM;

#[derive(Debug, PartialEq)]
struct Run {
    result: Result<String, String>,
    output: String,
    instructions: u64,
}

fn run_once(source: &str, general_loop_only: bool) -> Run {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap_or_else(|e| panic!("{}", e.diagnostic.message));
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.fast_loop = !general_loop_only;
    let result = vm
        .execute(chunk)
        .map(|value| value.to_string())
        .map_err(|e| e.message);
    Run {
        result,
        output: vm.output.take().unwrap_or_default(),
        instructions: vm.instruction_count,
    }
}

/// Runs `source` with and without the fast loop and checks they agree.
fn same_both_ways(source: &str) -> Run {
    let general = run_once(source, true);
    let fast = run_once(source, false);
    assert_eq!(fast, general, "{source}");
    fast
}

#[test]
fn arithmetic_at_the_edges_of_inline_integers() {
    let run = same_both_ways(
        "
        let big = 35184372088831;
        let mut x = big;
        x = x + 1;
        let y = x - 1;
        let z = big * 4;
        let w = 0 - big - 2;
        let mut n = 70368744177660;
        n = n + 10;
        print(x, y, z, w, n, 3 * 4, 7 - 9, 10 % 3, -7 % 3, 2.5 * 2.0, 1.5 + 1.5, 0.1 - 0.3);
        print(5.5 % 2.0, 1 + 1.5, 2.0 < 3.0, 3 < 2.5);
        ",
    );
    assert_eq!(
        run.output,
        "35184372088832 35184372088831 140737488355324 -35184372088833 70368744177670 12 -2 1 -1 5.0 3.0 -0.19999999999999998\n1.5 2.5 true false\n"
    );
}

#[test]
fn errors_come_from_the_general_instructions() {
    for source in [
        "let a = 0; 5 % a;",
        "let a = 9223372036854775807; a + 1;",
        "let a = 3037000500; let b = 3037000500; a * b;",
        "let s = \"x\"; s - 1;",
        "let n = null; n < 1;",
    ] {
        let run = same_both_ways(source);
        assert!(run.result.is_err(), "{source}: {run:?}");
    }
}

#[test]
fn comparisons_jumps_and_loops() {
    let run = same_both_ways(
        "
        let mut count = 0;
        for i in 0..1000 {
            if i % 3 == 0 { count = count + 1; }
            if i < 10 && i != 5 { count = count + 100; }
            if 2.5 < i { count = count + 0; }
            if \"a\" == \"a\" { count = count + 0; }
        }
        let mut j = 0;
        while (j <= 50) { j = j + 7; }
        print(count, j);
        ",
    );
    assert_eq!(run.output, "1234 56\n");
}

#[test]
fn calls_returns_and_recursion() {
    let run = same_both_ways(
        "
        fn fib(n) { if n <= 1 { return n; } return fib(n - 1) + fib(n - 2); }
        fn defaults(a, b = 10) { return a + b; }
        fn rest(...xs) { return xs.length; }
        let add = |x, y| => x + y;
        let k = 3;
        let addk = |x| => x + k;
        async fn later(x) { return x; }
        print(fib(18), defaults(1), defaults(1, 2), rest(1, 2, 3), add(2, 3), addk(4), later(1));
        ",
    );
    assert!(run.output.starts_with("2584 11 3 3 5 7"), "{}", run.output);
}

#[test]
fn globals_defined_later_and_strings_in_globals() {
    let run = same_both_ways(
        "
        fn callLater() { return later(); }
        let mut first = \"\";
        try { callLater(); } catch (e) { first = e.message; }
        fn later() { return 5; }
        let mut total = 0;
        for i in 0..300 { total = total + callLater(); }
        const NAME = \"name\";
        fn readName() { return NAME; }
        let objects = [];
        for i in 0..300 { objects.push([readName()]); }
        print(first, total, objects.length, objects[299][0]);
        ",
    );
    assert_eq!(run.output, "undefined variable 'later' 1500 300 name\n");
}

/// The REPL runs each input as a new script in the same VM; a later input may
/// declare a function again, and code from earlier inputs must call the new one.
#[test]
fn redeclared_functions_in_later_scripts_are_called() {
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    for source in [
        "fn f() { return 1; } fn g() { let mut s = 0; for i in 0..100 { s = s + f(); } return s; } print(g());",
        "fn f() { return 2; } print(f(), g());",
    ] {
        let tokens = pleno_lexer::lex(source).unwrap();
        let mut parser = pleno_parser::Parser::new(tokens, source);
        let mut program = parser.parse_program().unwrap();
        let chunk = Compiler::new(source.to_string())
            .compile(&mut program)
            .unwrap();
        vm.execute(chunk).unwrap();
    }
    assert_eq!(vm.output.unwrap(), "100\n2 200\n");
}

#[test]
fn methods_and_callbacks() {
    let run = same_both_ways(
        "
        let xs = [];
        for i in 0..200 { xs.push(i * 2); }
        let ys = xs.map(|x| => x % 7).filter(|x| => x > 2);
        let total = ys.reduce(|a, b| => a + b, 0);
        let s = \"abc\";
        class P { v; constructor(v) { this.v = v; } fn get() { return this.v; } }
        let p = P(5);
        let m = {\"f\": |x| => x + 1};
        print(ys.length, total, s.toUpperCase(), s.length, p.get(), m.f(1), xs.pop(), xs.length);
        ",
    );
    assert_eq!(run.output, "114 514 ABC 3 5 2 398 199\n");
}

#[test]
fn native_errors_inside_the_fast_loop() {
    let run = same_both_ways(
        "
        let xs = [1, 2, 3];
        let mut caught = \"\";
        try { xs.map(|x| => x.nope()); } catch (e) { caught = e.message; }
        try { \"abc\".substring(\"x\"); } catch (e) { caught = caught + \"|\" + e.message; }
        print(caught);
        ",
    );
    assert!(run.output.contains("nope"), "{}", run.output);
}

#[test]
fn a_frozen_array_still_refuses_a_push() {
    let run = same_both_ways(
        "
        const deep xs = [1];
        let mut message = \"\";
        try { xs.push(2); } catch (e) { message = e.message; }
        print(message, xs);
        ",
    );
    assert!(run.output.ends_with("[1]\n"), "{}", run.output);
}

#[test]
fn value_is_still_a_value() {
    // Results computed in the fast loop convert back to the same values.
    let run = same_both_ways("let a = 2; let b = 0.5; a * 3 + 1;");
    assert_eq!(run.result, Ok(Value::Int(7).to_string()));
}

fn run_limited(source: &str, fast_loop: bool, max_instructions: u64, max_memory: usize) -> Run {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.fast_loop = fast_loop;
    vm.apply_config(SandboxConfig {
        allow_io: false,
        allow_network: false,
        max_memory,
        max_instructions,
        allowed_read_paths: Vec::new(),
        allowed_write_paths: Vec::new(),
    });
    let result = vm
        .execute(chunk)
        .map(|value| value.to_string())
        .map_err(|e| e.message);
    Run {
        result,
        output: vm.output.take().unwrap_or_default(),
        instructions: vm.instruction_count,
    }
}

#[test]
fn sandbox_limits_hold_in_the_fast_loop() {
    // An endless loop stops at the instruction limit either way.
    let endless = "let mut i = 0; while (true) { i += 1; }";
    for fast in [true, false] {
        let run = run_limited(endless, fast, 100_000, 0);
        let err = run.result.unwrap_err();
        assert!(err.contains("instruction limit exceeded"), "{err}");
        assert!(
            run.instructions < 100_200,
            "fast={fast}: {}",
            run.instructions
        );
    }
    // Growing an array without end stops at the memory limit either way.
    let growing = "let xs = []; while (true) { xs.push(\"some text\"); }";
    for fast in [true, false] {
        let err = run_limited(growing, fast, 0, 4 * 1024 * 1024)
            .result
            .unwrap_err();
        assert!(err.contains("memory limit exceeded"), "fast={fast}: {err}");
    }
    // A program stopped by the limit stops at the same count both ways (found by
    // fuzzing: the general loop checked the limit only every 100 instructions).
    let recursion =
        "fn fib(n) { if (n <= 0) { return n; } return fib(n - 1) + fib(n - 2); } print(fib(20));";
    for limit in [200_000, 100_001, 12_345] {
        let fast = run_limited(recursion, true, limit, 0);
        assert!(
            fast.result
                .as_ref()
                .unwrap_err()
                .contains("instruction limit exceeded")
        );
        assert_eq!(
            fast,
            run_limited(recursion, false, limit, 0),
            "limit {limit}"
        );
    }
    // Under limits it does not reach, a program gives the same result both ways.
    let program =
        "fn fib(n) { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); } print(fib(15));";
    assert_eq!(
        run_limited(program, true, 10_000_000, 0),
        run_limited(program, false, 10_000_000, 0)
    );
}

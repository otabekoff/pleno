//! Programs whose bytecode uses the compiler's shortcuts: `AddInt`/`SubInt`
//! for a small constant operand, and results written straight into the
//! assigned variable or call argument instead of through a `Move`. Each must
//! behave exactly as the general instructions do.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn run(source: &str) -> Result<Value, String> {
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
    run(source).unwrap_or_else(|err| panic!("{err}\n{source}"))
}

fn string(value: &str) -> Value {
    Value::String(std::rc::Rc::new(value.to_string()))
}

#[test]
fn constant_operands_work_for_every_type() {
    assert_eq!(eval("let x = 41; x + 1;"), Value::Int(42));
    assert_eq!(eval("let x = 43; x - 1;"), Value::Int(42));
    assert_eq!(eval("let x = 1.5; x + 1;"), Value::Float(2.5));
    assert_eq!(eval("let x = 1.5; x - 1;"), Value::Float(0.5));
    assert_eq!(eval("let s = \"a\"; s + 1;"), string("a1"));
    assert_eq!(eval("let x = 0; x - 255;"), Value::Int(-255));
    let err = run("let s = \"a\"; s - 1;").unwrap_err();
    assert!(err.contains("invalid operands for -"), "{err}");
    let err = run("let x = 9223372036854775807; x + 1;").unwrap_err();
    assert!(err.contains("integer overflow in addition"), "{err}");
    let err = run("let x = -9223372036854775807 - 1; x - 1;").unwrap_err();
    assert!(err.contains("integer overflow in subtraction"), "{err}");
    // Integers beyond the inline NaN-box range.
    assert_eq!(
        eval("let x = 70368744177663; x + 1;"),
        Value::Int(70368744177664)
    );
}

#[test]
fn constant_operands_use_operator_overloading() {
    let source = "
        class V {
            constructor(n) { this.n = n; }
            operator +(other) { return V(this.n + other * 10); }
            operator -(other) { return V(this.n - other * 10); }
        }
        let v = V(5);
        (v + 1).n * 1000 + (v - 2).n;
    ";
    assert_eq!(eval(source), Value::Int(15 * 1000 - 15));
}

#[test]
fn range_loops_count_as_before() {
    let source = "let mut s = 0; for (i in 0..10) { s = s + i; } s;";
    assert_eq!(eval(source), Value::Int(45));
    let source = "let mut s = 0; for (i in 1..=4) { s = s * 10 + i; } s;";
    assert_eq!(eval(source), Value::Int(1234));
    let source = "let mut n = 0; for (i in 0..1000) { if i == 7 { break; } n = n + 1; } n;";
    assert_eq!(eval(source), Value::Int(7));
}

#[test]
fn assignments_from_branching_expressions_take_every_branch() {
    // Each right-hand side ends in a jump target, so its result must still
    // reach the variable on every path.
    let source = "
        let mut out = [];
        for (i in 0..4) {
            let mut x = 0;
            x = i > 1 && i < 3;
            out.push(x);
            x = i == 0 || i == 3;
            out.push(x);
            x = if i % 2 == 0 { i * 10 } else { i + 100 };
            out.push(x);
            x = i == 1 ? -1 : i - 1;
            out.push(x);
        }
        out.toString();
    ";
    assert_eq!(
        eval(source),
        string(
            "[false, true, 0, -1, false, false, 101, -1, true, false, 20, 1, \
             false, true, 103, 2]"
        )
    );
}

#[test]
fn assignments_read_the_old_value() {
    let source = "
        let mut a = 3;
        let mut b = 4;
        a = a + b;
        b = a - b;
        a = a - b;
        a * 10 + b;
    ";
    assert_eq!(eval(source), Value::Int(43));
    let source = "
        fn twice(f, x) { return f(f(x)); }
        let mut n = 2;
        n = twice(|v| => v * n, n + 1);
        n;
    ";
    assert_eq!(eval(source), Value::Int(12));
}

#[test]
fn big_integers_and_strings_are_freed_without_object_collections() {
    // Neither loop allocates objects, so only the collections triggered by new
    // boxed values free the old ones.
    let source = "
        let mut sum = 70368744177664;
        for (i in 0..200000) { sum = sum + i; }
        let mut s = \"\";
        for (i in 0..3000) { s = s + \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"; }
        sum - 70368744177664 + s.length;
    ";
    assert_eq!(eval(source), Value::Int(19999900000 + 3000 * 67));
    let (ints, strings, _) = pleno_value::nan_value::boxed_value_counts();
    assert!(ints < 100_000, "{ints} big integers still boxed");
    assert!(strings < 3000, "{strings} strings still boxed");
}

#[test]
fn fused_instructions_behave_like_separate_ones() {
    // Compare and jump, global read and thunk forcing, callee copy and call.
    let source = "
        let lazy base = 40;
        fn add(a, b) { return a + b; }
        let mut n = 0;
        let mut i = 0;
        while (i <= 5) {
            if i > 2 { n = add(n, i); } else { n = n + base; }
            i = i + 1;
        }
        n + (1.5 < 2 ? 1 : 0) + (2 >= 2.5 ? 100 : 0);
    ";
    assert_eq!(eval(source), Value::Int(120 + 12 + 1));
    // A small constant on the right: inline, boxed and float left operands.
    let source = "
        let mut hits = 0;
        for (x in [1, 2, 3, 70368744177664, -70368744177664]) {
            if x < 2 { hits = hits + 1; }
            if x >= 3 { hits = hits + 10; }
        }
        let f = 1.5;
        if f <= 1 { hits = hits + 100; }
        if f > 1 { hits = hits + 1000; }
        hits;
    ";
    assert_eq!(eval(source), Value::Int(2 + 20 + 1000));
    let err = run("let x = \"a\"; if x < 1 { 1; }").unwrap_err();
    assert!(err.contains("invalid operands for comparison"), "{err}");
}

#[test]
fn call_arguments_from_branching_expressions() {
    let source = "
        fn pair(a, b) { return a * 100 + b; }
        let mut total = 0;
        for (i in 0..3) {
            total = total + pair(i > 0 ? i : 9, i == 2 || i == 0 ? 1 : 2);
        }
        total;
    ";
    assert_eq!(eval(source), Value::Int(901 + 102 + 201));
}

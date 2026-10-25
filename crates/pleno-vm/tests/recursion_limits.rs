//! Runaway recursion raises a catchable `RecursionError` instead of exhausting
//! memory (plain calls) or overflowing the Rust stack (calls back into Pleno
//! from native code). Found by the fuzz_run target.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run(source: &str) -> (Result<String, String>, String) {
    let source = source.to_string();
    // Debug builds use more stack per nested call than release builds; give
    // the program the large stack the CLI also runs it on.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let tokens = pleno_lexer::lex(&source).unwrap();
            let mut program = pleno_parser::Parser::new(tokens, &source)
                .parse_program()
                .unwrap();
            let chunk = Compiler::new(source.clone()).compile(&mut program).unwrap();
            let mut vm = VM::new();
            vm.output = Some(String::new());
            pleno_stdlib::register(&mut vm);
            let result = vm
                .execute(chunk)
                .map(|value| value.to_string())
                .map_err(|e| e.message);
            (result, vm.output.take().unwrap_or_default())
        })
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn plain_recursion_raises_a_recursion_error() {
    let (result, output) = run("fn f(n) { return f(n + 1); }\n\
         try { f(0); } catch (e) { print(e.name); }\n\
         print(\"after\");");
    result.unwrap();
    assert_eq!(output, "RecursionError\nafter\n");
}

#[test]
fn recursion_through_callbacks_raises_a_recursion_error() {
    let (result, output) = run("fn g(n) { return [1].map(fn(x) { return g(n + 1); }); }\n\
         try { g(0); } catch (e) { print(e.message.endsWith(\"nested callbacks)\")); }\n\
         print(\"after\");");
    result.unwrap();
    assert_eq!(output, "true\nafter\n");
}

#[test]
fn deep_but_finite_recursion_still_works() {
    let (result, _) = run(
        "fn depth(n) { if (n == 0) { return 0; } return 1 + depth(n - 1); }\n\
         fn nested(n) { if (n == 0) { return 0; } return [n].map(fn(x) { return nested(x - 1); })[0] + 1; }\n\
         depth(20000) + nested(150);",
    );
    assert_eq!(result.unwrap(), "20150");
}

#[test]
fn a_test_that_registers_tests_forever_stops() {
    // The program the fuzzer found: `test` inside `afterEach` adds a test per test.
    let (result, _) = run("import { test, describe, afterEach } from \"std:test\";\n\
         describe(\"outer\", fn() {\n\
             afterEach(fn() { test(\"again\", fn() {}); });\n\
             test(\"first\", fn() {});\n\
         });");
    let err = result.unwrap_err();
    assert!(err.contains("RecursionError"), "{err}");
}

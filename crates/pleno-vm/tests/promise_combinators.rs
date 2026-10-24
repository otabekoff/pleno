use pleno_compiler::Compiler;
use pleno_vm::VM;

/// Runs a script and returns what it printed, or the error message.
fn run(source: &str) -> Result<String, String> {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .expect("parse failed");
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .expect("compile failed");
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    let outcome = vm.execute(chunk);
    let output = vm.output.take().unwrap_or_default();
    outcome.map(|_| output).map_err(|e| e.message)
}

#[test]
fn promise_all_keeps_every_result_in_order() {
    let output = run(r#"
        async fn double(x: Int): Int { return x * 2; }
        print(await Promise.all([spawn(double, 1), spawn(double, 2), spawn(double, 3)]));
        print(await Promise.all([Promise.resolve(1), Promise.resolve(2), 3]));
        print(await Promise.all([double(1), double(2), 7]));
        print(await Promise.all([Promise.resolve(1), 2]));
        print(await Promise.all([]));
    "#)
    .unwrap();
    assert_eq!(output, "[2, 4, 6]\n[1, 2, 3]\n[2, 4, 7]\n[1, 2]\n[]\n");
}

#[test]
fn awaiting_a_promise_that_never_settles_is_an_error() {
    let error = run(r#"
        let never = Promise(fn(resolve, reject) {});
        await never;
        print("unreachable");
    "#)
    .unwrap_err();
    assert!(error.contains("can never settle"), "{error}");
}

#[test]
fn calling_a_missing_method_names_it() {
    let error = run(r#"let m = {"a": 1}; m.contains("a");"#).unwrap_err();
    assert!(error.contains("Map has no method 'contains'"), "{error}");

    let error = run(r#"let items = [1]; items.size();"#).unwrap_err();
    assert!(error.contains("'size' of Array"), "{error}");
}

#[test]
fn an_async_call_returned_by_a_plain_function_keeps_its_registers() {
    // Used to panic: returning from `start` freed the suspended call's registers.
    let output = run(r#"
        import { sleep } from "std:time";
        async fn work(id: Int): Int {
            let a = id * 10;
            await sleep(1);
            return a + id;
        }
        fn start(id: Int) { return work(id); }
        fn busy(): Int { let mut t = 0; for (i in 0..50) { t += i; } return t; }
        let first = start(1);
        let x = busy();
        print(await first, x);
    "#)
    .unwrap();
    assert_eq!(output, "11 1225\n");
}

#[test]
fn async_calls_made_by_native_functions_return_their_promises() {
    let output = run(r#"
        import { sleep } from "std:time";
        async fn work(id: Int): Int { await sleep(1); return id * 10; }
        print(await Promise.all([3, 4] |> map(fn(id) => work(id))));
    "#)
    .unwrap();
    assert_eq!(output, "[30, 40]\n");
}

#[test]
fn async_tests_report_when_they_settle() {
    let output = run(r#"
        import { test, assert } from "std:test";
        import { sleep } from "std:time";
        test("passes", async fn() { await sleep(1); assert.equal(1, 1); });
        test("fails", async fn() { await sleep(1); assert.equal(1, 2); });
    "#)
    .unwrap();
    assert!(
        output.contains("passes") && output.contains("fails"),
        "{output}"
    );
    assert_eq!(pleno_stdlib::test::take_failures(), 1);
}

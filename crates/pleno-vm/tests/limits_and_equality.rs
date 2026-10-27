//! A sandbox limit ends the program even inside `try` or a callback, errors
//! thrown in callbacks keep their own message, and `!=` follows a class's `==`.

use pleno_compiler::Compiler;
use pleno_value::SandboxConfig;
use pleno_vm::VM;

fn run(source: &str, max_instructions: u64, max_memory: usize) -> (Result<String, String>, String) {
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
    if max_instructions > 0 || max_memory > 0 {
        vm.apply_config(SandboxConfig {
            allow_io: false,
            allow_network: false,
            max_memory,
            max_instructions,
            allowed_read_paths: Vec::new(),
            allowed_write_paths: Vec::new(),
        });
    }
    let result = vm
        .execute(chunk)
        .map(|value| value.to_string())
        .map_err(|e| e.message);
    (result, vm.output.take().unwrap_or_default())
}

#[test]
fn an_instruction_limit_cannot_be_caught() {
    for fast_loop_body in [
        "try { let mut i = 0; while (true) { i += 1; } } catch (e) { print(\"caught\"); }",
        "try { [1].map(fn(x) { let mut i = 0; while (true) { i += 1; } }); } catch (e) { print(\"caught\"); }",
        "while (true) { try { let mut i = 0; while (true) { i += 1; } } catch (e) {} }",
    ] {
        let source = format!("{fast_loop_body}\nprint(\"after\");");
        let (result, output) = run(&source, 100_000, 0);
        let err = result.unwrap_err();
        assert!(
            err.contains("SandboxError: instruction limit exceeded"),
            "{err}"
        );
        assert!(
            !output.contains("caught") && !output.contains("after"),
            "{source}: {output}"
        );
    }
}

// Loops whose steps are fused instructions add two or three to the count at
// a time. The limit was checked only when the count was a multiple of 100, so
// these could step past every check and run forever. Each runs in a thread so
// that a regression fails the test instead of hanging it.
#[test]
fn the_instruction_limit_stops_loops_of_fused_instructions() {
    for source in [
        "let mut count = 0; while (count < 2) { }",
        "let mut i = 0; do { i = i + 0; } while (i < 5);",
        "fn f() { let mut c = 0; while (c < 2) { } } f();",
    ] {
        for fast_loop in [true, false] {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let tokens = pleno_lexer::lex(source).unwrap();
                let mut program = pleno_parser::Parser::new(tokens, source)
                    .parse_program()
                    .unwrap();
                let chunk = Compiler::new(source.to_string())
                    .compile(&mut program)
                    .unwrap();
                let mut vm = VM::new();
                vm.fast_loop = fast_loop;
                vm.apply_config(SandboxConfig {
                    max_instructions: 100_000,
                    ..Default::default()
                });
                let _ = tx.send(
                    vm.execute(chunk)
                        .map(|v| v.to_string())
                        .map_err(|e| e.message),
                );
            });
            let result = rx
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("{source} (fast loop {fast_loop}) did not stop"));
            let err = result.unwrap_err();
            assert!(
                err.contains("SandboxError: instruction limit exceeded"),
                "{source}: {err}"
            );
        }
    }
}

#[test]
fn a_memory_limit_cannot_be_caught() {
    let source =
        "try { let xs = []; while (true) { xs.push(\"text\"); } } catch (e) { print(\"caught\"); }";
    let (result, output) = run(source, 0, 4 * 1024 * 1024);
    assert!(result.unwrap_err().contains("memory limit exceeded"));
    assert!(!output.contains("caught"), "{output}");
}

#[test]
fn errors_from_callbacks_keep_their_message() {
    let (result, output) = run(
        "try { [1].map(fn(x) { throw Error(\"inner\"); }); } catch (e) { print(e.message); }",
        0,
        0,
    );
    result.unwrap();
    assert_eq!(output.trim(), "inner");
}

#[test]
fn not_equal_follows_the_class_equality_operator() {
    let (result, output) = run(
        "class V { constructor(x) { this.x = x; } operator ==(o) { return this.x == o.x; } }\n\
         class W { operator ==(o) { return true; } operator !=(o) { return \"own\"; } }\n\
         class Plain {}\n\
         let p = Plain();\n\
         print(V(1) != V(1), V(1) != V(2), W() != W(), p != p, Plain() != Plain());\n\
         if (V(1) != V(2)) { print(\"branch\"); }",
        0,
        0,
    );
    result.unwrap();
    assert_eq!(output, "false true own false true\nbranch\n");
}

#[test]
fn the_program_that_runs_a_sandbox_can_catch_its_limit() {
    let (result, output) = run(
        "import { Sandbox } from \"std:security\";\n\
         let sandbox = Sandbox({ allowIo: false, allowNetwork: false, maxMemory: 0, maxInstructions: 10000 });\n\
         try {\n\
             sandbox.run(fn() { let mut i = 0; while (true) { i += 1; } });\n\
         } catch (e) { print(e.name, e.message); }\n\
         print(\"continues\");",
        0,
        0,
    );
    result.unwrap();
    assert_eq!(
        output,
        "SandboxError instruction limit exceeded\ncontinues\n"
    );
}

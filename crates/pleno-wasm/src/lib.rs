use pleno_bytecode::Chunk;
use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
// use pleno_value::Value;
use pleno_vm::VM;
use wasm_bindgen::prelude::*;

/// How deeply native calls back into Pleno can nest in the browser, within
/// WebAssembly's 1 MB stack (a nested call uses about 8 KB).
const WASM_MAX_NESTED_RUNS: usize = 64;

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn compile(source: String) -> Result<Vec<u8>, String> {
    let tokens = lex(&source).map_err(|e| format!("Lexer error: {}", e.message))?;
    let mut parser = Parser::new(tokens, &source);
    let mut program = parser
        .parse_program()
        .map_err(|e| format!("Parser error: {}", e.message))?;

    let mut compiler = Compiler::new(source);
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| format!("Compiler error: {}", e.diagnostic.message))?;

    // Serializing chunk? For now, let's just use a placeholder or JSON if bincode isn't ready.
    // Actually, pleno-bytecode might not have a binary serializer yet.
    // I'll use serde_json for now as it's easier for WASM integration.
    serde_json::to_vec(&chunk).map_err(|e| format!("Serialization error: {}", e))
}

#[wasm_bindgen]
pub fn run(bytecode: Vec<u8>) -> Result<String, String> {
    let chunk: Chunk =
        serde_json::from_slice(&bytecode).map_err(|e| format!("Deserialization error: {}", e))?;

    let mut vm = VM::new();
    vm.output = Some(String::new());
    // WebAssembly has a 1 MB stack, so callbacks may nest less deeply.
    vm.max_nested_runs = WASM_MAX_NESTED_RUNS;

    // Register stdlib
    pleno_stdlib::register(&mut vm);

    // `exit(0)` ends the program normally; another code is an error.
    match vm.execute(chunk).or_else(|e| match e.exit_code {
        Some(0) => Ok(pleno_value::Value::Null),
        _ => Err(e),
    }) {
        Ok(_) => Ok(vm
            .output
            .unwrap_or_else(|| "No output captured".to_string())),
        Err(e) => {
            let mut out = vm.output.unwrap_or_default();
            out.push_str(&format!("\nRuntime Error: {}", e.message));
            Err(out)
        }
    }
}

#[wasm_bindgen]
pub fn run_source(source: String) -> String {
    let bytecode = match compile(source) {
        Ok(b) => b,
        Err(e) => return format!("Compilation failed: {}", e),
    };

    match run(bytecode) {
        Ok(out) => out,
        Err(e) => e,
    }
}

/// Runs a program for the documentation playground and returns JSON:
/// `{"ok": bool, "output": "...", "error": "..."}`. Memory and instruction
/// limits stop runaway programs.
/// Errors are reported as the CLI reports them, with the line and a caret.
#[wasm_bindgen]
pub fn run_playground(source: String) -> String {
    run_with_limit(source, 50_000_000)
}

fn run_with_limit(source: String, max_instructions: u64) -> String {
    use pleno_diagnostics::{DiagnosticReporter, SourceMap, ToDiagnostic};

    let source_map = SourceMap::new("main.pln".to_string(), source.clone());
    let report = |diagnostic: pleno_diagnostics::Diagnostic| {
        DiagnosticReporter::report(&diagnostic, &source_map)
    };
    let result = |ok: bool, output: String, error: String| {
        serde_json::json!({ "ok": ok, "output": output, "error": error }).to_string()
    };

    let tokens = match lex(&source) {
        Ok(tokens) => tokens,
        Err(e) => {
            let diagnostic =
                pleno_diagnostics::Diagnostic::new(pleno_diagnostics::Severity::Error, e.message)
                    .with_label(e.span, "");
            return result(false, String::new(), report(diagnostic));
        }
    };
    let mut program = match Parser::new(tokens, &source).parse_program() {
        Ok(program) => program,
        Err(e) => {
            let diagnostic =
                pleno_diagnostics::Diagnostic::new(pleno_diagnostics::Severity::Error, e.message)
                    .with_label(e.span, "");
            return result(false, String::new(), report(diagnostic));
        }
    };
    let chunk = match Compiler::new(source.clone()).compile(&mut program) {
        Ok(chunk) => chunk,
        Err(e) => return result(false, String::new(), report(e.to_diagnostic())),
    };

    let mut vm = VM::new();
    vm.output = Some(String::new());
    // WebAssembly has a 1 MB stack, so callbacks may nest less deeply.
    vm.max_nested_runs = WASM_MAX_NESTED_RUNS;
    // Console output needs `allow_io` (it also covers printing); the browser
    // build has no file system or network to reach anyway.
    vm.apply_config(pleno_value::SandboxConfig {
        allow_io: true,
        allow_network: false,
        max_memory: 256 * 1024 * 1024,
        max_instructions,
        allowed_read_paths: Vec::new(),
        allowed_write_paths: Vec::new(),
    });
    pleno_stdlib::register(&mut vm);
    // The count is per thread, and the playground's worker runs many programs.
    pleno_stdlib::test::take_failures();
    let outcome = vm.execute(chunk).or_else(|e| match e.exit_code {
        Some(0) => Ok(pleno_value::Value::Null),
        _ => Err(e),
    });
    let output = vm.output.take().unwrap_or_default();
    match outcome {
        Ok(_) => match pleno_stdlib::test::take_failures() {
            0 => result(true, output, String::new()),
            1 => result(false, output, "1 test failed".to_string()),
            n => result(false, output, format!("{n} tests failed")),
        },
        Err(e) => result(false, output, report(e.to_diagnostic())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playground_shows_test_results_and_fails_on_a_failed_test() {
        let source = r#"
            import { test, assert } from "std:test";
            test("passes", fn() { assert.equal(1, 1); });
            test("fails", fn() { assert.equal(1, 2); });
        "#;
        let value: serde_json::Value =
            serde_json::from_str(&run_with_limit(source.to_string(), 1_000_000)).unwrap();
        let output = value["output"].as_str().unwrap();
        assert!(
            output.contains("passes") && output.contains("fails"),
            "{output}"
        );
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "1 test failed");

        // The next run starts with no failures.
        let value: serde_json::Value =
            serde_json::from_str(&run_with_limit("print(1);".to_string(), 1_000_000)).unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn playground_prints_collections_like_the_cli() {
        let json = run_with_limit(
            "print(\"a\", [1, 2] |> map(fn(x) => x * 3), {\"k\": [1]});".to_string(),
            1_000_000,
        );
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["output"],
            "a [3, 6] {\"k\": [1]}
",
            "{json}"
        );
    }

    #[test]
    fn runs_source_and_captures_output() {
        let output = run_source("let x = 6 * 7; print(\"answer: \" + x);".to_string());
        assert_eq!(output.trim(), "answer: 42");
    }

    #[test]
    fn playground_reports_output_errors_and_limits() {
        let ok: serde_json::Value =
            serde_json::from_str(&run_playground("print(6 * 7);".into())).unwrap();
        assert_eq!(ok["ok"], true);
        assert_eq!(ok["output"].as_str().unwrap().trim(), "42");

        let err: serde_json::Value = serde_json::from_str(&run_playground(
            "print(1);
print(foo);"
                .into(),
        ))
        .unwrap();
        assert_eq!(err["ok"], false);
        assert_eq!(err["output"].as_str().unwrap().trim(), "1");
        assert!(
            err["error"]
                .as_str()
                .unwrap()
                .contains("undefined variable 'foo'")
        );
        assert!(err["error"].as_str().unwrap().contains("2:7"));

        let looped: serde_json::Value =
            serde_json::from_str(&run_with_limit("while (true) {}".into(), 10_000)).unwrap();
        assert_eq!(looped["ok"], false);
    }

    #[test]
    fn reports_compile_and_runtime_errors() {
        assert!(run_source("let x = ;".to_string()).starts_with("Compilation failed"));
        assert!(run_source("print(1 / 0);".to_string()).contains("division by zero"));
    }
}

use pleno_bytecode::Chunk;
use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
// use pleno_value::Value;
use pleno_vm::VM;
use wasm_bindgen::prelude::*;

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

    // Register stdlib
    pleno_stdlib::register(&mut vm);

    match vm.execute(chunk) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_source_and_captures_output() {
        let output = run_source("let x = 6 * 7; print(\"answer: \" + x);".to_string());
        assert_eq!(output.trim(), "answer: 42");
    }

    #[test]
    fn reports_compile_and_runtime_errors() {
        assert!(run_source("let x = ;".to_string()).starts_with("Compilation failed"));
        assert!(run_source("print(1 / 0);".to_string()).contains("division by zero"));
    }
}

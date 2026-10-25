//! Functions with more than 256 constants: literals take a 16-bit index, and
//! names (8-bit operands) push loaded-only literals above index 255.

use pleno_compiler::Compiler;
use pleno_vm::VM;

/// Runs on a thread with a large stack, as the `pleno` CLI does: long
/// expressions nest deeply, and debug builds use big stack frames.
fn run(source: &str) -> Result<String, String> {
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || run_here(&source))
        .unwrap()
        .join()
        .unwrap()
}

fn run_here(source: &str) -> Result<String, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .map_err(|e| e.message)?;
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message.clone())?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)?;
    Ok(vm.output.take().unwrap_or_default())
}

#[test]
fn a_function_with_hundreds_of_literals_runs() {
    let parts: String = (0..400).map(|i| format!(" + \"p{i}\"")).collect();
    let source = format!("fn f(x) {{ return x{parts}; }}\nprint(f(\"\").length);");
    // "p0".."p9" are 2 characters, "p10".."p99" 3, "p100".."p399" 4.
    assert_eq!(
        run(&source).unwrap(),
        format!("{}\n", 10 * 2 + 90 * 3 + 300 * 4)
    );
}

#[test]
fn names_after_many_literals_keep_every_value() {
    // 300 literals fill the first slots; the names that follow (variables,
    // properties, functions) move literals up, and every load must still see
    // its own value.
    let literals: Vec<String> = (0..300).map(|i| format!("\"v{i}\"")).collect();
    let fields: String = (0..40).map(|i| format!("f{i}: {i}, ")).collect();
    let reads: String = (0..40).map(|i| format!(" + m[\"f{i}\"]")).collect();
    let source = format!(
        "fn f() {{\n    let values = [{}];\n    let m = {{{fields}}};\n    let sum = 0{reads};\n    \
         return `${{values[0]}} ${{values[150]}} ${{values[299]}} ${{sum}}`;\n}}\nprint(f());",
        literals.join(", ")
    );
    assert_eq!(run(&source).unwrap(), "v0 v150 v299 780\n");
}

#[test]
fn too_many_names_in_one_function_is_a_compile_error() {
    let fields: String = (0..300).map(|i| format!("o.k{i} = {i};\n")).collect();
    let source = format!("fn f() {{\n let o = {{}};\n{fields} return o; }}\nf();");
    let error = run(&source).unwrap_err();
    assert!(error.contains("more than 256 different names"), "{error}");
}

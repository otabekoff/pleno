//! A statement can chain at most 1,000 operators or calls; a longer one is a
//! parse error instead of a stack overflow in the compiler (found by the
//! fuzz_compile target).

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run(source: &str) -> Result<String, String> {
    let source = source.to_string();
    // A debug build needs about 40 KB of stack per chain link to compile; the
    // tools run programs on a 64 MB thread, and so does this test.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
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
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)?;
    Ok(vm.output.take().unwrap_or_default())
}

fn sum(terms: usize) -> String {
    format!(
        "let mut one = 1;\nprint({});",
        vec!["one"; terms].join(" + ")
    )
}

#[test]
fn the_longest_allowed_chain_compiles_and_runs() {
    // 1,000 additions, or 1,000 chained calls: exactly the limit.
    assert_eq!(run(&sum(1_001)).unwrap(), "1001\n");
    let calls = format!("print([1]{});", ".map(fn(v) => v)".repeat(1_000));
    assert_eq!(run(&calls).unwrap(), "[1]\n");
}

#[test]
fn a_longer_chain_is_a_parse_error() {
    let err = run(&sum(1_002)).unwrap_err();
    assert!(err.contains("expression too long"), "{err}");
    let err = run(&format!("print([1]{});", ".map(fn(v) => v)".repeat(1_001))).unwrap_err();
    assert!(err.contains("expression too long"), "{err}");
}

#[test]
fn each_statement_has_its_own_budget() {
    let body: String = (0..5)
        .map(|i| format!("    let x{i} = {};\n", vec!["1"; 800].join(" + ")))
        .collect();
    assert_eq!(
        run(&format!("fn f() {{\n{body}    return x4;\n}}\nprint(f());")).unwrap(),
        "800\n"
    );
}

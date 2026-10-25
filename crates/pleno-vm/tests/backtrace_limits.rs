use pleno_compiler::Compiler;
use pleno_value::RuntimeError;
use pleno_vm::VM;

fn run(source: &str) -> Result<pleno_value::Value, RuntimeError> {
    let tokens = pleno_lexer::lex(source).expect("lex");
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("parse");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("compile");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
}

#[test]
fn a_shallow_backtrace_keeps_every_frame() {
    let err = run("fn g(d) { if (d == 0) { nope(); } g(d - 1); }\ng(5);").unwrap_err();
    let names: Vec<_> = err.stack_trace.iter().map(|f| f.describe()).collect();
    assert_eq!(names.len(), 7, "{names:?}");
    assert!(names[..6].iter().all(|n| n == "at g"), "{names:?}");
    assert_eq!(names[6], "at <script>");
}

#[test]
fn a_deep_backtrace_keeps_both_ends() {
    let err = run("fn g(d) { if (d == 0) { nope(); } g(d - 1); }\ng(1000);").unwrap_err();
    let trace = &err.stack_trace;
    // 1001 frames of `g` and the script: 50 innermost, the omitted line, 10 outermost.
    assert_eq!(trace.len(), 61);
    assert!(trace[..50].iter().all(|f| f.function_name == "g"));
    assert!(trace[50].is_omitted());
    assert_eq!(trace[50].describe(), "... 942 more frames");
    assert_eq!(trace[60].function_name, "<script>");
}

#[test]
fn rethrowing_in_every_catch_stays_fast() {
    // Each level's `catch` throws again, so 20,000 errors are made at depths
    // up to 20,000. Copying the whole stack each time took seconds.
    let started = std::time::Instant::now();
    let err = run(
        "fn f(d) { if (d == 0) { nope(); } try { f(d - 1); } catch (e) { nope(); } }\nf(20000);",
    )
    .unwrap_err();
    assert!(
        err.message.contains("undefined variable 'nope'"),
        "{}",
        err.message
    );
    assert!(
        started.elapsed().as_secs() < 10,
        "took {:?}",
        started.elapsed()
    );
}

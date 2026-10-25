#![no_main]
use libfuzzer_sys::fuzz_target;
use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::SandboxConfig;
use pleno_vm::VM;

/// What one run of a program produced.
#[derive(Debug, PartialEq)]
struct Run {
    result: Result<String, String>,
    output: String,
    instructions: u64,
}

fn run(source: &str, fast_loop: bool) -> Option<Run> {
    let tokens = lex(source).ok()?;
    let mut program = Parser::new(tokens, source).parse_program().ok()?;
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .ok()?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.fast_loop = fast_loop;
    // Small limits keep each input fast; no files, network or other effects.
    vm.apply_config(SandboxConfig {
        allow_io: false,
        allow_network: false,
        max_memory: 32 * 1024 * 1024,
        max_instructions: 200_000,
        allowed_read_paths: Vec::new(),
        allowed_write_paths: Vec::new(),
    });
    let result = vm
        .execute(chunk)
        .map(|value| value.to_string())
        .map_err(|e| e.message);
    Some(Run {
        result,
        output: vm.output.take().unwrap_or_default(),
        instructions: vm.instruction_count,
    })
}

/// Names whose results differ between two runs of the same program (time,
/// randomness, collector statistics, input), so the runs are not compared.
const NONDETERMINISTIC: &[&str] = &[
    "random", "Random", "now", "Date", "time", "Time", "uuid", "Uuid", "Stats", "gc", "GC",
    "sleep", "input", "env", "Env", "process", "Process", "spawn", "Channel", "Promise",
];

// Any program must run or fail with an error, never panic, and the small
// interpreter loop must agree with the general one: same result, output and
// instruction count.
fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let Some(fast) = run(source, true) else {
        return;
    };
    let general = run(source, false).expect("compiled once, compiles again");
    if !NONDETERMINISTIC.iter().any(|name| source.contains(name)) {
        assert_eq!(fast, general, "the fast and general loops disagree");
    }
});

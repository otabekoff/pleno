//! Every Pleno program in the repository runs the same with and without the
//! small interpreter loop: same result, output and instruction count. The
//! `fuzz_run` fuzz target checks the same on generated programs.

use pleno_compiler::Compiler;
use pleno_value::SandboxConfig;
use pleno_vm::VM;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
struct Run {
    result: Result<String, String>,
    output: String,
    instructions: u64,
}

fn run(source: &str, fast_loop: bool) -> Option<Run> {
    let tokens = pleno_lexer::lex(source).ok()?;
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .ok()?;
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .ok()?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.fast_loop = fast_loop;
    vm.apply_config(SandboxConfig {
        allow_io: false,
        allow_network: false,
        max_memory: 64 * 1024 * 1024,
        max_instructions: 5_000_000,
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

/// Programs whose output differs from run to run, or that need their own
/// folder (relative imports) or input.
const SKIP: &[&str] = &[
    "random", "Random", "now(", "Date", "time", "Time", "uuid", "Uuid", "Stats", "gc", "GC",
    "sleep", "input", "env", "process", "Process", "spawn", "Channel", "Promise", "async", "\"./",
    "\"../", "File", "Http", "http", "net", "readLine", "stdin",
];

fn pln_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            pln_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pln") {
            out.push(path);
        }
    }
}

#[test]
fn repository_programs_run_the_same_in_both_loops() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["tests", "examples", "dev-tests", "bench"] {
        pln_files(&root.join(dir), &mut files);
    }
    let mut compared = 0;
    let mut differences = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        if SKIP.iter().any(|word| source.contains(word)) {
            continue;
        }
        let Some(fast) = run(&source, true) else {
            continue;
        };
        let general = run(&source, false).unwrap();
        compared += 1;
        if fast != general {
            differences.push(format!(
                "{}:\n  fast:    {fast:?}\n  general: {general:?}",
                path.strip_prefix(&root).unwrap_or(path).display()
            ));
        }
    }
    assert!(compared > 100, "compared only {compared} programs");
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

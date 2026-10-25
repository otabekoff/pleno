use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use chrono::Datelike;
use pleno_common::style::{self, Stream};
use pleno_compiler::BuildProfile;
use pleno_package::{Dependency, DependencyDetail, Manifest};

/// Stack size for the thread that runs the CLI. Parsing, optimizing and
/// compiling deeply nested code recurses, which can exceed the 1 MB default
/// main-thread stack on Windows.
const MAIN_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Runs `entry` on a thread with a large stack and exits with its status.
pub fn run_with_large_stack(entry: fn()) {
    let worker = std::thread::Builder::new()
        .name("pleno-main".to_string())
        .stack_size(MAIN_STACK_SIZE)
        .spawn(entry);
    match worker.map(|handle| handle.join()) {
        Ok(Ok(())) => {}
        // The panic message has already been printed by the panic hook.
        Ok(Err(_)) => process::exit(101),
        Err(err) => {
            eprintln!("failed to start pleno: {err}");
            process::exit(1);
        }
    }
}

pub fn run_pleno() {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        start_default();
        return;
    }

    let command = args.next().unwrap();

    match command.as_str() {
        "-h" | "--help" => {
            print_help();
        }
        "-v" | "--version" => {
            println!(
                "{} {}",
                style::gradient(Stream::Stdout, "pleno", &style::BRAND),
                env!("CARGO_PKG_VERSION")
            );
        }
        "init" => {
            handle_init(&mut args);
        }
        "install" => {
            handle_install(&mut args);
        }
        "update" => {
            handle_update(&mut args);
        }
        "remove" => {
            handle_remove(&mut args);
        }
        "build" => {
            handle_build(&mut args);
        }
        "plm" => {
            run_plm_core(&mut args);
        }
        "run" => {
            handle_run(&mut args);
        }
        "format" | "fmt" => {
            let mut path = None;
            let mut stdout = false;

            for arg in args.by_ref() {
                if arg == "--stdout" {
                    stdout = true;
                } else if path.is_none() {
                    path = Some(arg);
                }
            }

            let path = path.unwrap_or_else(|| {
                eprintln!("Error: 'format' requires a file or directory path");
                process::exit(1);
            });
            format_file(&path, stdout);
        }
        "lint" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'lint' requires a file or directory path");
                process::exit(1);
            });
            lint_file(&path);
        }
        "check" => {
            let mut path = None;
            let mut json_output = false;
            for arg in args.by_ref() {
                if arg == "--json" {
                    json_output = true;
                } else {
                    path = Some(arg);
                }
            }
            let path = path.unwrap_or_else(|| {
                eprintln!("Error: 'check' requires a file path");
                process::exit(1);
            });
            check_file(&path, json_output);
        }
        "test" => {
            let mut test_path = default_test_dir();
            let mut json_output = false;
            for arg in args.by_ref() {
                if arg == "--json" {
                    json_output = true;
                } else {
                    test_path = arg;
                }
            }
            run_tests(&test_path, json_output, BuildProfile::Test);
        }
        "debug-ast" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'debug-ast' requires a file path");
                process::exit(1);
            });
            debug_ast(&path);
        }
        "lsp" => {
            if let Err(e) = pleno_lsp::start() {
                eprintln!("LSP server error: {e}");
                process::exit(1);
            }
        }
        "repl" => {
            if let Err(e) = pleno_repl::start() {
                eprintln!("REPL error: {e}");
                process::exit(1);
            }
        }
        "compile" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'compile' requires a file path");
                process::exit(1);
            });
            compile_file(&path, BuildProfile::Debug);
        }
        "watch" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'watch' requires a file path");
                process::exit(1);
            });
            watch_file(&path);
        }
        "doc" => {
            let path = args.next();
            let output = args.next();
            handle_doc(path.as_deref(), output.as_deref());
        }
        "dap" => {
            handle_dap(&mut args);
        }
        _ => {
            run_file(&command, None, BuildProfile::Debug);
        }
    }
}

pub fn run_plm() {
    let mut args = env::args().skip(1).peekable();
    run_plm_core(&mut args);
}

fn run_plm_core(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let sub_command = args.next().unwrap_or_else(|| {
        eprintln!("Pleno Module Manager (plm)\n\nUsage: plm <command> [args]\n\nCommands: init, install, update, remove, run, publish, build");
        process::exit(1)
    });
    match sub_command.as_str() {
        "init" => handle_init(args),
        "install" => handle_install(args),
        "update" => handle_update(args),
        "remove" => handle_remove(args),
        "run" => handle_plm_run(args),
        "publish" => handle_plm_publish(args),
        "search" => handle_plm_search(args),
        "build" => handle_build(args),
        "test" => {
            let mut test_path = default_test_dir();
            let mut json_output = false;
            for arg in args.by_ref() {
                if arg == "--json" {
                    json_output = true;
                } else {
                    test_path = arg;
                }
            }
            run_tests(&test_path, json_output, BuildProfile::Test);
        }
        "info" => handle_plm_info(args),
        "-h" | "--help" => {
            println!("Pleno Module Manager (plm)");
            println!();
            println!("Usage: plm <command> [args]");
            println!();
            println!("Commands:");
            println!("  init [name]      Initialize a new Pleno package");
            println!("  install [url]    Install dependencies or a specific package");
            println!("  update           Update all dependencies");
            println!("  remove <name>    Remove a dependency");
            println!("  run <script>     Run a script defined in pleno.toml");
            println!("  info             Show the package from pleno.toml");
            println!("  build            Build the current package");
            println!("  test             Run the package's tests");
            println!("  publish          Publish to the registry named by PLENO_REGISTRY");
            println!("  search <query>   Search for packages in the registry");
            println!();
            println!("Options:");
            println!("  -h, --help       Show this help message");
            println!("  -v, --version    Show version information");
        }
        "-v" | "--version" => {
            println!("plm {}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            eprintln!("Error: Unknown plm command '{}'", sub_command);
            process::exit(1);
        }
    }
}

fn start_default() {
    if let Err(e) = pleno_repl::start() {
        eprintln!("REPL error: {}", e);
        process::exit(1);
    }
}

pub fn print_help() {
    const SECTIONS: &[(&str, &[(&str, &str)])] = &[
        (
            "Commands",
            &[
                (
                    "run <file.pln|.plc>",
                    "Run a Pleno file or compiled bytecode",
                ),
                ("plm <command>", "Package management (see below)"),
                (
                    "format <path>",
                    "Format Pleno files (also: fmt; --stdout prints)",
                ),
                ("lint <path>", "Lint Pleno files"),
                (
                    "check <file.pln>",
                    "Check a file without running it (--json for editors)",
                ),
                ("test <path>", "Run tests in a file or directory (--json)"),
                ("compile <file.pln>", "Compile a file to .plc bytecode"),
                (
                    "build [file]",
                    "Compile the package's main file (--release, --test)",
                ),
                ("watch <file.pln>", "Run a file again whenever it changes"),
                ("doc [path] [out]", "Generate documentation"),
                ("repl", "Start the interactive REPL"),
                ("lsp", "Start the language server"),
                ("dap", "Start the debug adapter on stdio"),
            ],
        ),
        (
            "Package commands (pleno plm <command>, or plm <command>)",
            &[
                ("init [name]", "Create a Pleno package in this folder"),
                ("info", "Show the package from pleno.toml"),
                (
                    "install [url]",
                    "Install dependencies or a specific package",
                ),
                ("update", "Update all dependencies"),
                ("remove <name>", "Remove a dependency"),
                ("run <script>", "Run a script defined in pleno.toml"),
                ("build, test", "Build or test the package"),
                ("publish", "Publish to the registry named by PLENO_REGISTRY"),
                ("search <term>", "Search the registry"),
            ],
        ),
        (
            "Options",
            &[
                ("-h, --help", "Show this help message"),
                ("-v, --version", "Show version information"),
                ("--max-memory <bytes>", "Set the memory limit"),
                ("--max-instructions <n>", "Set the instruction limit"),
                ("--allow-io <bool>", "Allow or deny file access"),
                ("--allow-network <bool>", "Allow or deny network access"),
            ],
        ),
    ];
    let out = Stream::Stdout;
    println!(
        "{} {}",
        style::gradient(out, "Pleno", &style::BRAND),
        style::dim(out, env!("CARGO_PKG_VERSION"))
    );
    println!();
    println!("{} pleno <command> [arguments]", style::bold(out, "Usage:"));
    for (heading, rows) in SECTIONS {
        println!();
        println!("{}", style::bold(out, &format!("{heading}:")));
        for (command, description) in *rows {
            // Pad before coloring, so the escape codes do not skew the columns.
            println!(
                "  {}  {description}",
                style::cyan(out, &format!("{command:<23}"))
            );
        }
    }
    println!();
    println!("{}", style::bold(out, "Examples:"));
    for example in ["pleno run main.pln", "plm init my-app", "plm run dev"] {
        println!("  {}", style::dim(out, example));
    }
}

fn run_tests(path: &str, json_output: bool, profile: BuildProfile) {
    let path_buf = PathBuf::from(path);
    let mut stats = TestStats::default();

    if path_buf.is_dir() {
        run_tests_in_dir(&path_buf, &mut stats, json_output, profile);
    } else {
        run_single_test(&path_buf, &mut stats, json_output, profile);
    }

    if json_output {
        let out = serde_json::json!({
            "passed": stats.passed,
            "failed": stats.failed,
            "total": stats.passed + stats.failed,
            "results": stats.results
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        let out = Stream::Stdout;
        println!("\n{}", style::bold(out, "Test Summary:"));
        println!("  Passed: {}", style::green(out, &stats.passed.to_string()));
        let failed = stats.failed.to_string();
        let failed = if stats.failed > 0 {
            style::red(out, &failed)
        } else {
            failed
        };
        println!("  Failed: {failed}");
        println!("  Total:  {}", stats.passed + stats.failed);
    }

    if stats.failed > 0 {
        process::exit(1);
    }
}

#[derive(Default)]
struct TestStats {
    passed: usize,
    failed: usize,
    results: Vec<serde_json::Value>,
}

fn run_tests_in_dir(
    dir: &PathBuf,
    stats: &mut TestStats,
    json_output: bool,
    profile: BuildProfile,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                run_tests_in_dir(&path, stats, json_output, profile);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                run_single_test(&path, stats, json_output, profile);
            }
        }
    }
}

fn run_single_test(
    path: &PathBuf,
    stats: &mut TestStats,
    json_output: bool,
    profile: BuildProfile,
) {
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            if !json_output {
                eprintln!("Failed to read {}: {err}", path.display());
            }
            stats.failed += 1;
            stats.results.push(serde_json::json!({
                "file": path.display().to_string(),
                "status": "error",
                "message": format!("Failed to read file: {err}")
            }));
            return;
        }
    };

    if !json_output {
        print!("Running {} ... ", path.display());
        let _ = std::io::stdout().flush();
    }

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            if !json_output {
                println!("{}", style::red(Stream::Stdout, "FAILED (Lex Error)"));
                eprintln!(
                    "  Lex error at {}..{}: {}",
                    err.span.start, err.span.end, err.message
                );
            }
            stats.failed += 1;
            stats.results.push(serde_json::json!({
                "file": path.display().to_string(),
                "status": "failed",
                "error_type": "lex",
                "message": err.message,
                "span": { "start": err.span.start, "end": err.span.end }
            }));
            return;
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens, &source);
    let mut program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            if !json_output {
                println!("{}", style::red(Stream::Stdout, "FAILED (Parse Error)"));
                eprintln!(
                    "  Parse error at {}..{}: {}",
                    err.span.start, err.span.end, err.message
                );
            }
            stats.failed += 1;
            stats.results.push(serde_json::json!({
                "file": path.display().to_string(),
                "status": "failed",
                "error_type": "parse",
                "message": err.message,
                "span": { "start": err.span.start, "end": err.span.end }
            }));
            return;
        }
    };

    // A file that declares `module Name;` is a library other files import,
    // not a test.
    if program
        .statements
        .iter()
        .any(|stmt| matches!(stmt.kind, pleno_ast::StmtKind::Module { body: None, .. }))
    {
        if !json_output {
            println!("{}", style::dim(Stream::Stdout, "skipped (module)"));
        }
        stats.results.push(serde_json::json!({
            "file": path.display().to_string(),
            "status": "skipped",
            "message": "module file"
        }));
        return;
    }

    let is_expected_compile_error = source.contains("// EXPECT_COMPILE_ERROR");
    let is_expected_runtime_error = source.contains("// EXPECT_RUNTIME_ERROR");

    let compiler = pleno_compiler::Compiler::new(source);
    let chunk = match compiler.with_profile(profile).compile(&mut program) {
        Ok(chunk) => {
            if is_expected_compile_error {
                if !json_output {
                    println!(
                        "{}",
                        style::red(
                            Stream::Stdout,
                            "FAILED (Expected Compile Error but succeeded)"
                        )
                    );
                }
                stats.failed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "failed",
                    "message": "Expected compile error but succeeded"
                }));
                return;
            }
            chunk
        }
        Err(err) => {
            if is_expected_compile_error {
                if !json_output {
                    println!(
                        "{}",
                        style::green(Stream::Stdout, "PASSED (Expected Compile Error)")
                    );
                }
                stats.passed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "passed",
                    "message": "Expected compile error occurred"
                }));
                return;
            }
            if !json_output {
                println!("{}", style::red(Stream::Stdout, "FAILED (Compile Error)"));
                eprintln!("  Compile error: {}", err.diagnostic.message);
            }
            stats.failed += 1;
            stats.results.push(serde_json::json!({
                "file": path.display().to_string(),
                "status": "failed",
                "error_type": "compile",
                "message": err.diagnostic.message
            }));
            return;
        }
    };

    let mut vm = pleno_vm::VM::new();
    vm.main_path = std::fs::canonicalize(path).ok();
    pleno_stdlib::register(&mut vm);
    pleno_stdlib::test::take_failures();
    let result = vm.execute(chunk).and_then(|value| {
        // Failed `test(...)` cases fail the file even though it ran to the end.
        match pleno_stdlib::test::take_failures() {
            0 => Ok(value),
            n => Err(pleno_value::RuntimeError::new(format!(
                "{n} test(s) failed"
            ))),
        }
    });
    match result {
        Ok(_) => {
            if is_expected_runtime_error {
                if !json_output {
                    println!(
                        "{}",
                        style::red(
                            Stream::Stdout,
                            "FAILED (Expected Runtime Error but succeeded)"
                        )
                    );
                }
                stats.failed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "failed",
                    "message": "Expected runtime error but succeeded"
                }));
            } else {
                if !json_output {
                    println!("{}", style::green(Stream::Stdout, "PASSED"));
                }
                stats.passed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "passed"
                }));
            }
        }
        Err(err) => {
            if is_expected_runtime_error {
                if !json_output {
                    println!(
                        "{}",
                        style::green(Stream::Stdout, "PASSED (Expected Runtime Error)")
                    );
                }
                stats.passed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "passed",
                    "message": "Expected runtime error occurred"
                }));
            } else {
                if !json_output {
                    println!("{}", style::red(Stream::Stdout, "FAILED (Runtime Error)"));
                    eprintln!("  Runtime error: {}", err.message);
                }
                stats.failed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "failed",
                    "error_type": "runtime",
                    "message": err.message
                }));
            }
        }
    }
}

fn debug_ast(path: &str) {
    if !path.ends_with(".pln") {
        eprintln!("Error: Only .pln format file extensions are supported");
        process::exit(1);
    }
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {path}: {err}");
            process::exit(1);
        }
    };

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            eprintln!(
                "Lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens, &source);
    match parser.parse_program() {
        Ok(program) => {
            println!("{:#?}", program);
        }
        Err(err) => {
            eprintln!(
                "Parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    }
}

fn run_file(path: &str, config: Option<pleno_value::SandboxConfig>, profile: BuildProfile) {
    use pleno_diagnostics::{DiagnosticReporter, ToDiagnostic};
    if !path.ends_with(".pln") {
        eprintln!("Error: Only .pln format file extensions are supported");
        process::exit(1);
    }
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {path}: {err}");
            process::exit(1);
        }
    };
    let source_map = pleno_diagnostics::SourceMap::new(path.to_string(), source.clone());

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens, &source);
    let mut program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            process::exit(1);
        }
    };

    let compiler = pleno_compiler::Compiler::new(source.clone());
    let chunk = match compiler.with_profile(profile).compile(&mut program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!(
                "Compile error: {}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            process::exit(1);
        }
    };

    execute_main_chunk(path, chunk, config, Some((&source, &source_map)));
}

/// Runs the compiled main chunk of `path` and exits non-zero on failure.
/// `source` is the script's text, used to point at errors; a `.plc` file has none.
fn execute_main_chunk(
    path: &str,
    chunk: pleno_bytecode::Chunk,
    config: Option<pleno_value::SandboxConfig>,
    source: Option<(&str, &pleno_diagnostics::SourceMap)>,
) {
    use pleno_diagnostics::{DiagnosticReporter, ToDiagnostic};
    let mut vm = pleno_vm::VM::new();
    if let Some(cfg) = config {
        vm.apply_config(cfg);
    }
    vm.main_path = std::fs::canonicalize(path).ok();
    pleno_stdlib::register(&mut vm);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| vm.execute(chunk)));

    match result {
        Ok(Ok(_)) => {
            // A script that runs `test(...)` cases fails when any of them did.
            let failures = pleno_stdlib::test::take_failures();
            if failures > 0 {
                eprintln!(
                    "{} {failures} test(s) failed",
                    style::red(Stream::Stderr, "Error:")
                );
                process::exit(1);
            }
        }
        Ok(Err(err)) => {
            match source {
                Some((_, source_map)) => eprintln!(
                    "{}",
                    DiagnosticReporter::report(&err.to_diagnostic(), source_map)
                ),
                None => eprintln!("{} {}", style::red(Stream::Stderr, "Error:"), err.message),
            }

            // Manual stack trace printing for now as Diagnostic doesn't support it fully yet
            if !err.stack_trace.is_empty() {
                eprintln!("\nStack backtrace:");
                for frame in err.stack_trace {
                    if let Some(span) = frame.span {
                        // Fallback line calculation if outside main file?
                        // For now assume main file or just print span
                        if let Some((source, _)) = source
                            && span.end <= source.len()
                        {
                            let (line, col) = get_line_col(source, span.start);
                            eprintln!("  at {} ({}:{})", frame.function_name, line, col);
                        } else {
                            eprintln!("  at {} (unknown location)", frame.function_name);
                        }
                    } else {
                        eprintln!("  at {}", frame.function_name);
                    }
                }
            }
            process::exit(1);
        }
        Err(e) => {
            eprintln!("VM Panicked: {:?}", e);
            process::exit(1);
        }
    }
}

fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn format_file(path: &str, stdout_flag: bool) {
    let path_buf = PathBuf::from(path);
    if path_buf.is_dir() {
        format_directory(&path_buf, stdout_flag);
    } else {
        if !path.ends_with(".pln") {
            eprintln!("Error: Only .pln format file extensions are supported");
            process::exit(1);
        }
        format_single_file(&path_buf, stdout_flag);
    }
}

fn format_single_file(path: &PathBuf, stdout_flag: bool) {
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", path.display());
            process::exit(1);
        }
    };

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            eprintln!(
                "Lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens, &source);
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!(
                "Parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    };

    let mut formatter = pleno_formatter::Formatter::new().with_source(&source);
    let formatted = formatter.format_program(&program, 0);

    if stdout_flag {
        print!("{}", formatted);
    } else {
        if let Err(err) = fs::write(path, formatted) {
            eprintln!("Failed to write to {}: {err}", path.display());
            process::exit(1);
        }
        println!("Formatted {}", path.display());
    }
}

fn format_directory(dir: &PathBuf, stdout_flag: bool) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                format_directory(&path, stdout_flag);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                format_single_file(&path, stdout_flag);
            }
        }
    }
}

fn lint_file(path: &str) {
    let path_buf = PathBuf::from(path);
    if path_buf.is_dir() {
        lint_directory(&path_buf);
    } else {
        lint_single_file(&path_buf);
    }
}

fn lint_single_file(path: &PathBuf) {
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", path.display());
            process::exit(1);
        }
    };

    let issues = pleno_linter::lint(&source);

    if issues.is_empty() {
        println!("No linting issues found in {}", path.display());
    } else {
        println!("{}", pleno_linter::format_diagnostics(&issues));
        process::exit(1);
    }
}

fn lint_directory(dir: &PathBuf) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                lint_directory(&path);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                lint_single_file(&path);
            }
        }
    }
}

/// Lexes, parses, compiles and statically checks `path` without running it.
/// Problems are printed as `file:line:column: error: message` (or `warning:`),
/// or with `--json` as a JSON array of
/// `{line, column, endLine, endColumn, severity, message}` (1-based, columns in
/// UTF-16 units, as editors count them). Only errors make the exit status 1.
fn check_file(path: &str, json_output: bool) {
    if !path.ends_with(".pln") {
        eprintln!("Error: Only .pln format file extensions are supported");
        process::exit(1);
    }
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {path}: {err}");
            process::exit(1);
        }
    };

    let problems = check_source(&source);
    let has_errors = problems.iter().any(|p| p.severity == Severity::Error);
    if json_output {
        let problems: Vec<serde_json::Value> = problems
            .iter()
            .map(|problem| {
                let (line, column) = line_and_column(&source, problem.start);
                let (end_line, end_column) =
                    line_and_column(&source, problem.end.max(problem.start + 1));
                serde_json::json!({
                    "line": line,
                    "column": column,
                    "endLine": end_line,
                    "endColumn": end_column,
                    "severity": problem.severity.label(),
                    "message": problem.message,
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(problems));
    } else {
        for problem in &problems {
            let (line, column) = line_and_column(&source, problem.start);
            let severity = problem.severity.label();
            eprintln!("{path}:{line}:{column}: {severity}: {}", problem.message);
        }
        if !has_errors {
            println!("Type check passed for {}", path);
        }
    }
    if has_errors {
        process::exit(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One problem `pleno check` reports, with its byte range in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Problem {
    message: String,
    start: usize,
    end: usize,
    severity: Severity,
}

impl Problem {
    fn error(message: String, start: usize, end: usize) -> Self {
        Problem {
            message,
            start,
            end,
            severity: Severity::Error,
        }
    }
}

/// The problems in `source`: a lex, parse or compile error, or else every
/// name read but never defined and what the static checks find (wrong
/// argument counts, unknown methods on `this`, literals that do not match
/// their annotation), in source order.
fn check_source(source: &str) -> Vec<Problem> {
    let tokens = match pleno_lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(err) => return vec![Problem::error(err.message, err.span.start, err.span.end)],
    };
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => return vec![Problem::error(err.message, err.span.start, err.span.end)],
    };
    // Compiling may change the program in place, so check it first.
    let static_problems = pleno_compiler::check::check_program(&program);
    // Unused functions and code after a `return` are checked too.
    let mut compiler = pleno_compiler::Compiler::new(source.to_string()).keeping_unused_code();
    if let Err(err) = compiler.compile(&mut program) {
        let span = err
            .diagnostic
            .labels
            .first()
            .map(|label| (label.span.start, label.span.end))
            .unwrap_or((0, 0));
        return vec![Problem::error(err.diagnostic.message, span.0, span.1)];
    }

    // Built-in globals are whatever the standard library registers.
    let mut vm = pleno_vm::VM::new();
    pleno_stdlib::register(&mut vm);
    let mut problems: Vec<Problem> = compiler
        .undefined_globals(|name| vm.globals.contains_key(name))
        .into_iter()
        .map(|(name, span)| {
            Problem::error(format!("undefined variable '{name}'"), span.start, span.end)
        })
        .collect();
    problems.extend(static_problems.into_iter().map(|d| Problem {
        message: d.message,
        start: d.span.start,
        end: d.span.end,
        severity: match d.severity {
            pleno_compiler::check::Severity::Error => Severity::Error,
            pleno_compiler::check::Severity::Warning => Severity::Warning,
        },
    }));
    problems.sort_by_key(|p| (p.start, p.end));
    problems
}

/// The 1-based line and UTF-16 column of a byte offset.
fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &source[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line = before.matches('\n').count() + 1;
    let column = before[line_start..].encode_utf16().count() + 1;
    (line, column)
}

fn watch_file(path: &str) {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    let path = PathBuf::from(path);
    if !path.exists() {
        eprintln!("Error: File '{}' not found", path.display());
        process::exit(1);
    }

    println!("Watching {} for changes...", path.display());
    if !path.to_string_lossy().ends_with(".pln") {
        eprintln!("Error: Only .pln format file extensions are supported");
        process::exit(1);
    }
    run_file(&path.to_string_lossy(), None, BuildProfile::Debug);

    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Config::default()).unwrap();

    let watch_target = if path.is_dir() {
        path.clone()
    } else {
        path.parent().unwrap_or(&path).to_path_buf()
    };

    if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
        eprintln!("Error watching file: {}", e);
        process::exit(1);
    }

    loop {
        match rx.recv() {
            Ok(Ok(_event)) => {
                while rx.try_recv().is_ok() {}
                print!("\x1B[2J\x1B[1;1H"); // Clear screen
                println!("File changed. Re-running...");
                run_file(&path.to_string_lossy(), None, BuildProfile::Debug);
            }
            Ok(Err(e)) => println!("watch error: {:?}", e),
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

fn compile_file(path: &str, profile: BuildProfile) {
    if !path.ends_with(".pln") {
        eprintln!("Error: Only .pln format file extensions are supported");
        process::exit(1);
    }
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", path);
            process::exit(1);
        }
    };

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            eprintln!(
                "Lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens, &source);
    let mut program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!(
                "Parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            process::exit(1);
        }
    };

    let compiler = pleno_compiler::Compiler::new(source);
    let chunk = match compiler.with_profile(profile).compile(&mut program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!("Compile error: {}", err.diagnostic.message);
            process::exit(1);
        }
    };

    let output_path = std::path::Path::new(path).with_extension("plc");
    // The file starts with the `PLNC` magic and the bytecode format version, so a
    // `pleno` that reads another format rejects it instead of misreading it.
    let bytes = match pleno_bytecode::encode_chunk(&chunk) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("Failed to write bytecode: {}", err);
            process::exit(1);
        }
    };

    match fs::write(&output_path, bytes) {
        Ok(()) => {
            println!("Compiled {} -> {}", path, output_path.display());
        }
        Err(err) => {
            eprintln!("Failed to write {}: {}", output_path.display(), err);
            process::exit(1);
        }
    }
}

/// Runs a `.plc` file written by `pleno compile`.
fn run_bytecode_file(path: &str, config: Option<pleno_value::SandboxConfig>) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("Failed to read {path}: {err}");
            process::exit(1);
        }
    };
    let chunk = match pleno_bytecode::decode_chunk(&bytes) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!("{} {path}: {err}", style::red(Stream::Stderr, "Error:"));
            process::exit(1);
        }
    };
    execute_main_chunk(path, chunk, config, None);
}

fn handle_init(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let name = args.next().unwrap_or_else(|| {
        env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-app".to_string())
    });

    if Path::new("pleno.toml").exists() {
        eprintln!(
            "{} pleno.toml already exists here; run plm init in an empty folder.",
            style::red(Stream::Stderr, "Error:")
        );
        process::exit(1);
    }
    let existed: Vec<bool> = INIT_FILES
        .iter()
        .map(|path| Path::new(path).exists())
        .collect();

    let mut manifest = Manifest::new(&name, env!("CARGO_PKG_VERSION"));

    // Add standard scripts
    manifest
        .scripts
        .insert("dev".to_string(), "pleno run src/main.pln".to_string());
    manifest
        .scripts
        .insert("test".to_string(), "plm test".to_string());
    manifest
        .scripts
        .insert("build".to_string(), "plm build".to_string());
    manifest
        .scripts
        .insert("check".to_string(), "pleno check src/main.pln".to_string());
    manifest
        .scripts
        .insert("fmt".to_string(), "pleno format .".to_string());
    manifest
        .scripts
        .insert("lint".to_string(), "pleno lint .".to_string());
    manifest
        .scripts
        .insert("repl".to_string(), "pleno repl".to_string());

    if let Err(e) = manifest.save_to_file("pleno.toml") {
        eprintln!("Failed to create pleno.toml: {}", e);
        process::exit(1);
    }

    // Create src directory and main.pln
    let src_dir = Path::new("src");
    if !src_dir.exists() {
        fs::create_dir_all(src_dir).expect("Failed to create src directory");
    }
    let main_pln = src_dir.join("main.pln");
    if !main_pln.exists() {
        let content = r#"/// This is the entry point of your Pleno application.
///
/// You can run this file using `plm run dev` (defined in pleno.toml)
/// or explicitly with `pleno run src/main.pln`.

fn main() {
    // Single-line comment: Define a variable
    let world = "world!";

    /*
     * Multi-line comment:
     * Print the hello message to the standard output.
     */
    print("Hello,", world);
}

// Run the main function
main();
"#;
        fs::write(main_pln, content).expect("Failed to create src/main.pln");
    }

    // Create tests directory and integration_test.pln
    let tests_dir = Path::new("tests");
    if !tests_dir.exists() {
        fs::create_dir_all(tests_dir).expect("Failed to create tests directory");
    }
    let integration_test = tests_dir.join("integration_test.pln");
    if !integration_test.exists() {
        let content = r#"// Run the tests with `plm test`.
import { describe, test, assert } from "std:test";

describe("math", fn() {
    test("adds numbers", fn() {
        assert.equal(1 + 1, 2);
    });

    test("compares numbers", fn() {
        let x = 10;
        let y = 20;
        assert.true(x < y);
    });
});
"#;
        fs::write(integration_test, content).expect("Failed to create tests/integration_test.pln");
    }

    // Create README.md
    let readme = Path::new("README.md");
    // Create .gitignore
    let gitignore = Path::new(".gitignore");
    if !gitignore.exists() {
        let content = r#"# Pleno
.pleno_cache/
packages/
.logs/
target/

# System
.DS_Store
Thumbs.db

# IDEs
.vscode/*
!.vscode/settings.json
!.vscode/tasks.json
!.vscode/launch.json
!.vscode/extensions.json
.idea/
.project
.settings/
"#;
        fs::write(gitignore, content).expect("Failed to create .gitignore");
    }

    // Create .editorconfig
    let editorconfig = Path::new(".editorconfig");
    if !editorconfig.exists() {
        let content = r#"root = true

[*]
indent_style = space
indent_size = 4
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

[*.{js,json,yml,yaml}]
indent_size = 2
"#;
        fs::write(editorconfig, content).expect("Failed to create .editorconfig");
    }

    if !readme.exists() {
        let content = format!(
            r#"# {}

Welcome to your new Pleno project!

## About Pleno

Pleno is a dynamic, type-safe programming language designed for production. It combines modern features with a focus on stability and ease of use.

## Getting Started

The entry point of this project is [`src/main.pln`](./src/main.pln).

### Prerequisites

Ensure you have the Pleno toolchain installed. You can verify this by running:

```bash
pleno --version
plm --version
```

### Commands

This project includes a `pleno.toml` file with standard scripts for your convenience:

- **Run in Development**:
  ```bash
  plm run dev
  ```
  Or using the default task:
  ```bash
  plm run
  ```

- **Build**:
  ```bash
  plm build          # Default (Debug)
  plm build --debug  # Debug profile
  plm build --release # Release profile
  ```

- **Test**:
  ```bash
  plm test
  ```
  Runs all tests in the project (including the one in `src/main.pln`).

- **Check**:
  ```bash
  plm run check
  ```
  Checks the syntax and semantics of `src/main.pln`.

- **REPL**:
  ```bash
  plm run repl
  ```
  Starts the Pleno Read-Eval-Print Loop.

## Documentation

For more information about the Pleno programming language, please visit the [official documentation](https://pleno.mohirlab.uz/).
"#,
            name
        );
        fs::write(readme, content).expect("Failed to create README.md");
    }

    // Create LICENSE (MIT)
    let license = Path::new("LICENSE");
    if !license.exists() {
        let content = format!(
            r#"MIT License

Copyright (c) {} <Your Name/Organization>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#,
            chrono::Local::now().year()
        );
        fs::write(license, content).expect("Failed to create LICENSE");
    }

    // Create CONTRIBUTING.md
    let contributing = Path::new("CONTRIBUTING.md");
    if !contributing.exists() {
        let content = r#"# Contributing to this Project

We love your input! We want to make contributing to this project as easy and transparent as possible, whether it's:

- Reporting a bug
- Discussing the current state of the code
- Submitting a fix
- Proposing new features
- Becoming a maintainer

## We Use [Github Flow](https://guides.github.com/introduction/flow/index.html), So All Code Changes Happen Through Pull Requests

Pull requests are the best way to propose changes to the codebase (we use [Github Flow](https://guides.github.com/introduction/flow/index.html)). We actively welcome your pull requests:

1. Fork the repo and create your branch from `main`.
2. If you've added code that should be tested, add tests.
3. If you've changed APIs, update the documentation.
4. Ensure the test suite passes.
5. Make sure your code lints.
6. Issue that pull request!

## Any contributions you make will be under the MIT Software License

In short, when you submit code changes, your submissions are understood to be under the same [MIT License](LICENSE) that covers the project. Feel free to contact the maintainers if that's a concern.

## Report bugs using Github's [issue tracker](https://github.com/path/to/repo/issues)

We use GitHub issues to track public bugs. Report a bug by [opening a new issue](); it's that easy!
"#;
        fs::write(contributing, content).expect("Failed to create CONTRIBUTING.md");
    }

    // Create SECURITY.md
    let security = Path::new("SECURITY.md");
    if !security.exists() {
        let content = r#"# Security Policy

## Supported Versions

Use this section to tell people about which versions of your project are currently being supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

We take the security of our software seriously. If you believe you have found a security vulnerability in this project, please report it to us as described below.

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to [security@example.com](mailto:security@example.com).

You should receive a response within 24 hours. If for some reason you do not, please follow up via email to ensure we received your original message.
"#;
        fs::write(security, content).expect("Failed to create SECURITY.md");
    }

    // Create CHANGELOG.md
    let changelog = Path::new("CHANGELOG.md");
    if !changelog.exists() {
        let content = r#"# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project setup.
"#;
        fs::write(changelog, content).expect("Failed to create CHANGELOG.md");
    }

    // Create CODE_OF_CONDUCT.md
    let coc = Path::new("CODE_OF_CONDUCT.md");
    if !coc.exists() {
        let content = r#"# Contributor Covenant Code of Conduct

## Our Pledge

In the interest of fostering an open and welcoming environment, we as
contributors and maintainers pledge to making participation in our project and
our community a harassment-free experience for everyone, regardless of age, body
size, disability, ethnicity, sex characteristics, gender identity and expression,
level of experience, education, socio-economic status, nationality, personal
appearance, race, religion, or sexual identity and orientation.

## Our Standards

Examples of behavior that contributes to creating a positive environment
include:

* Using welcoming and inclusive language
* Being respectful of differing viewpoints and experiences
* Gracefully accepting constructive criticism
* Focusing on what is best for the community
* Showing empathy towards other community members

## Enforcement

Instances of abusive, harassing, or otherwise unacceptable behavior may be
reported by contacting the project team at [email@example.com]. All
complaints will be reviewed and investigated and will result in a response that
is deemed necessary and appropriate to the circumstances. The project team is
obligated to maintain confidentiality with regard to the reporter of an incident.
"#;
        fs::write(coc, content).expect("Failed to create CODE_OF_CONDUCT.md");
    }

    // Create .vscode directory and configs
    let vscode_dir = Path::new(".vscode");
    if !vscode_dir.exists() {
        fs::create_dir_all(vscode_dir).expect("Failed to create .vscode directory");
    }

    // tasks.json
    let tasks_json = vscode_dir.join("tasks.json");
    if !tasks_json.exists() {
        let content = r#"{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "plm: plm build --debug",
      "type": "shell",
      "command": "plm",
      "args": ["build", "--debug"],
      "group": "build",
      "presentation": {
        "reveal": "always",
        "panel": "new"
      },
      "problemMatcher": []
    },
    {
      "label": "plm: plm build --release",
      "type": "shell",
      "command": "plm",
      "args": ["build", "--release"],
      "group": "build",
      "presentation": {
        "reveal": "always",
        "panel": "new"
      },
      "problemMatcher": []
    },
    {
      "label": "plm: plm test",
      "type": "shell",
      "command": "plm",
      "args": ["test"],
      "group": "test",
      "presentation": {
        "reveal": "always",
        "panel": "new"
      },
      "problemMatcher": []
    },
    {
      "label": "pleno: pleno format .",
      "type": "shell",
      "command": "pleno",
      "args": ["format", "."],
      "presentation": {
        "reveal": "silent",
        "panel": "shared"
      },
      "problemMatcher": []
    },
    {
      "label": "plm: plm build",
      "type": "shell",
      "command": "plm",
      "args": ["build"],
      "group": {
        "kind": "build",
        "isDefault": true
      },
      "presentation": {
        "reveal": "always",
        "panel": "shared"
      },
      "problemMatcher": []
    },
    {
      "label": "plm: plm run dev",
      "type": "shell",
      "command": "plm",
      "args": ["run", "dev"],
      "presentation": {
        "reveal": "always",
        "panel": "shared"
      },
      "problemMatcher": []
    }
  ]
}"#;
        fs::write(tasks_json, content).expect("Failed to create .vscode/tasks.json");
    }

    // settings.json
    let settings_json = vscode_dir.join("settings.json");
    if !settings_json.exists() {
        let content = r#"{
  "files.autoSave": "afterDelay",
  "files.autoSaveDelay": 0,
  "files.autoSaveWhenNoErrors": true,
  "files.autoSaveWorkspaceFilesOnly": true,
  "files.refactoring.autoSave": true,
  "editor.formatOnSave": true,
  "editor.formatOnSaveMode": "file",
  "files.associations": {
    ".pln": "pleno"
  },
  "explorer.fileNesting.enabled": true,
  "explorer.fileNesting.expand": false,
  "explorer.fileNesting.patterns": {
    "*.ts": "${capture}.js",
    "*.js": "${capture}.js.map, ${capture}.min.js, ${capture}.d.ts",
    "*.jsx": "${capture}.js",
    "*.tsx": "${capture}.ts",
    "tsconfig.json": "tsconfig.*.json",
    "package.json": "package-lock.json, yarn.lock, pnpm-lock.yaml, bun.lockb, bun.lock",
    "pubspec.yaml": "pubspec.lock,pubspec_overrides.yaml,.packages,.flutter-plugins,.flutter-plugins-dependencies,.metadata",
    "*.dart": "${capture}.g.dart",
    "Cargo.toml": "Cargo.lock",
    "pleno.toml": "pleno.lock",
    "README.md": "CONTRIBUTING.*, LICENSE, LICENCE, LICENSE.*, LICENCE.*, COPYING, AUTHORS, MODERATORS, CONTRIBUTORS, SECURITY.*, CHANGELOG, CHANGELOG.*, CODE_OF_CONDUCT.*"
  }
}"#;
        fs::write(settings_json, content).expect("Failed to create .vscode/settings.json");
    }

    // launch.json
    let launch_json = vscode_dir.join("launch.json");
    if !launch_json.exists() {
        let content = r#"{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "pleno",
            "request": "launch",
            "name": "Launch main.pln",
            "program": "src/main.pln",
            "stopOnEntry": false
        }
    ]
}"#;
        fs::write(launch_json, content).expect("Failed to create .vscode/launch.json");
    }

    // extensions.json
    let extensions_json = vscode_dir.join("extensions.json");
    if !extensions_json.exists() {
        let content = r#"{
    "recommendations": [
        "pleno.pleno-language"
    ]
}"#;
        fs::write(extensions_json, content).expect("Failed to create .vscode/extensions.json");
    }

    let out = Stream::Stdout;
    println!();
    println!(
        "  {}  {}",
        style::gradient(out, "Pleno", &style::BRAND),
        style::dim(out, &format!("created {name}"))
    );
    println!();
    for (path, existed) in INIT_FILES.iter().zip(existed) {
        if !existed && Path::new(path).exists() {
            println!("  {} {path}", style::green(out, "\u{2714}"));
        }
    }
    println!();
    println!("  {}", style::bold(out, "Done. Now run:"));
    println!();
    println!("    {}", style::cyan(out, "plm run dev"));
    println!("    {}", style::cyan(out, "plm test"));
    println!();
}

/// Where `pleno test` and `plm test` look without a path: `tests/`, as
/// `plm init` creates it, or the older `sources/tests/`.
fn default_test_dir() -> String {
    ["tests", "sources/tests"]
        .into_iter()
        .find(|dir| Path::new(dir).is_dir())
        .unwrap_or("tests")
        .to_string()
}

/// The files `plm init` creates, listed when it finishes.
const INIT_FILES: &[&str] = &[
    "pleno.toml",
    "src/main.pln",
    "tests/integration_test.pln",
    "README.md",
    "LICENSE",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    ".gitignore",
    ".editorconfig",
    ".vscode/settings.json",
    ".vscode/tasks.json",
    ".vscode/launch.json",
    ".vscode/extensions.json",
];

fn handle_install(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let mut pkg_name = None;
    let mut features = Vec::new();
    let mut offline = false;
    let mut is_dev = false;

    let iter = args.by_ref();
    while let Some(arg) = iter.next() {
        if arg == "--features" {
            if let Some(f_list) = iter.next() {
                for f in f_list.split(',') {
                    features.push(f.to_string());
                }
            }
        } else if arg == "--offline" {
            offline = true;
        } else if arg == "--dev" || arg == "--save-dev" {
            is_dev = true;
        } else if pkg_name.is_none() && !arg.starts_with('-') {
            pkg_name = Some(arg);
        }
    }

    let mut manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };

    // Without PLENO_REGISTRY, packages come from local, path and Git sources.
    let mut manager = pleno_package::PackageManager::new("packages").with_offline(offline);
    if let Ok(registry_url) = std::env::var("PLENO_REGISTRY") {
        manager = manager.with_registry(registry_url);
    }

    if let Some(pkg) = pkg_name {
        let (name, source) = if let Some(pos) = pkg.rfind('@') {
            let (n, s) = pkg.split_at(pos);
            let version = s[1..].to_string();
            if n.starts_with("github.com/") {
                (n.to_string(), pkg.clone()) // Keep shorthand for source
            } else if !n.starts_with("git+") && !n.starts_with("http") && !n.contains('/') {
                (n.to_string(), version)
            } else {
                (
                    n.split('/').next_back().unwrap().replace(".git", ""),
                    pkg.clone(),
                )
            }
        } else if pkg.starts_with("github.com/") {
            (pkg.clone(), pkg.clone())
        } else if pkg.contains('/') || pkg.starts_with("git+") {
            let name = pkg.split('/').next_back().unwrap().replace(".git", "");
            (name, pkg.clone())
        } else {
            (pkg.clone(), "*".to_string())
        };

        let dep = if source.starts_with("github.com/") {
            let (git_url, version) = if let Some(pos) = source.rfind('@') {
                (
                    source[..pos].to_string(),
                    Some(source[pos + 1..].to_string()),
                )
            } else {
                (source.clone(), None)
            };
            Dependency::Detailed(DependencyDetail {
                git: Some(format!("https://{}", git_url)),
                version: None,
                path: None,
                branch: None,
                tag: version,
                rev: None,
                optional: None,
                features: None,
            })
        } else if source.contains('/') || source.starts_with("git+") || source.starts_with("http") {
            Dependency::Detailed(DependencyDetail {
                git: Some(source.clone()),
                version: None,
                path: None,
                branch: None,
                tag: None,
                rev: None,
                optional: None,
                features: None,
            })
        } else if source.starts_with("./") || source.starts_with("../") || source.starts_with("/") {
            Dependency::Detailed(DependencyDetail {
                path: Some(source.clone()),
                version: None,
                git: None,
                branch: None,
                tag: None,
                rev: None,
                optional: None,
                features: None,
            })
        } else {
            Dependency::Version(source.clone())
        };

        if is_dev {
            manifest.dev_dependencies.insert(name.clone(), dep);
        } else {
            manifest.dependencies.insert(name.clone(), dep);
        }
        if let Err(e) = manifest.save_to_file("pleno.toml") {
            eprintln!("Failed to save pleno.toml: {}", e);
            process::exit(1);
        }
        println!("Added {} to dependencies", name);

        if let Err(e) = manager.install_all(&manifest, &features) {
            eprintln!("Installation failed: {}", e);
            process::exit(1);
        }
    } else {
        println!("Installing dependencies...");
        if let Err(e) = manager.install_all(&manifest, &features) {
            eprintln!("Installation failed: {}", e);
            process::exit(1);
        }
    }

    println!("Installation successful.");
}

fn handle_update(_args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let mut manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };
    let manager = pleno_package::PackageManager::new("packages");
    if let Err(e) = manager.update(&mut manifest) {
        eprintln!("Update failed: {}", e);
        process::exit(1);
    }
}

fn handle_remove(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let name = args.next().unwrap_or_else(|| {
        eprintln!("Error: 'remove' requires a package name");
        process::exit(1);
    });
    let mut manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };
    let manager = pleno_package::PackageManager::new("packages");
    if let Err(e) = manager.remove(&mut manifest, &name) {
        eprintln!("Remove failed: {}", e);
        process::exit(1);
    }
}

fn handle_plm_run(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let script_name = args.next().unwrap_or_else(|| {
        eprintln!("Error: 'run' requires a script name");
        process::exit(1);
    });
    let manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };
    let manager = pleno_package::PackageManager::new("packages");
    if let Err(e) = manager.run_script(&manifest, &script_name) {
        eprintln!("Run failed: {}", e);
        process::exit(1);
    }
}

fn handle_plm_publish(_args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };
    let registry_url = required_registry_url();
    let manager = pleno_package::PackageManager::new("packages").with_registry(registry_url);
    if let Err(e) = manager.publish(&manifest) {
        eprintln!("Publish failed: {}", e);
        process::exit(1);
    }
    println!(
        "Published {} v{}",
        manifest.package.name, manifest.package.version
    );
}

/// The registry URL from `PLENO_REGISTRY`. There is no default: no public
/// registry exists yet, and a guessed domain could be taken by anyone.
fn required_registry_url() -> String {
    std::env::var("PLENO_REGISTRY").unwrap_or_else(|_| {
        eprintln!(
            "{} no package registry is configured. Set PLENO_REGISTRY to the registry's URL.",
            style::red(Stream::Stderr, "Error:")
        );
        process::exit(1);
    })
}

fn handle_plm_search(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let query = args.next();
    let registry_url = required_registry_url();
    let url = format!("{}/packages", registry_url);

    println!("Searching for packages...");

    let resp = match reqwest::blocking::get(&url) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to connect to registry: {}", e);
            process::exit(1);
        }
    };

    if !resp.status().is_success() {
        eprintln!("Registry returned error: {}", resp.status());
        process::exit(1);
    }

    let packages: Vec<serde_json::Value> = match resp.json() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse registry response: {}", e);
            process::exit(1);
        }
    };

    let filtered: Vec<_> = packages
        .into_iter()
        .filter(|pkg| {
            if let Some(q) = &query {
                let name = pkg["name"].as_str().unwrap_or("").to_lowercase();
                let desc = pkg["description"].as_str().unwrap_or("").to_lowercase();
                let q_lower = q.to_lowercase();
                name.contains(&q_lower) || desc.contains(&q_lower)
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        println!("No packages found.");
        return;
    }

    println!("{:<20} {:<10} {:<40}", "NAME", "VERSION", "DESCRIPTION");
    println!("{:-<20} {:-<10} {:-<40}", "", "", "");
    for pkg in filtered {
        let name = pkg["name"].as_str().unwrap_or("unknown");
        let version = pkg["version"].as_str().unwrap_or("?.?.?");
        let desc = pkg["description"].as_str().unwrap_or("");
        println!("{:<20} {:<10} {:<40}", name, version, desc);
    }
}

fn handle_build(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let mut target = None;
    // let mut debug = false;
    let mut release = false;
    let mut test_mode = false;

    for arg in args.by_ref() {
        if arg == "--debug" {
            // debug = true;
        } else if arg == "--release" {
            release = true;
        } else if arg == "--test" {
            test_mode = true;
        } else if target.is_none() {
            target = Some(arg);
        }
    }

    let build_profile = if release {
        BuildProfile::Release
    } else if test_mode {
        BuildProfile::Test
    } else {
        BuildProfile::Debug
    };

    println!(
        "Building for {}...",
        if release {
            "release"
        } else if test_mode {
            "test"
        } else {
            "debug"
        }
    );

    match target {
        Some(t) if t.ends_with(".pln") => {
            compile_file(&t, build_profile);
        }
        Some(t) => {
            if let Ok(manifest) = Manifest::load_from_file("pleno.toml") {
                let entry = &manifest.package.main;
                compile_file(entry, build_profile);
            } else {
                eprintln!("Error: No pleno.toml found and '{}' is not a .pln file", t);
                process::exit(1);
            }
        }
        None => {
            if let Ok(manifest) = Manifest::load_from_file("pleno.toml") {
                let entry = &manifest.package.main;
                compile_file(entry, build_profile);
            } else {
                eprintln!(
                    "Error: 'build' requires a target (file or package) and no pleno.toml found"
                );
                process::exit(1);
            }
        }
    }
}

fn handle_plm_info(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let json_output = args.peek().is_some_and(|arg| arg == "--json");
    if json_output {
        args.next();
    }

    match Manifest::load_from_file("pleno.toml") {
        Ok(manifest) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
            } else {
                println!("Package: {}", manifest.package.name);
                println!("Version: {}", manifest.package.version);
                println!("Main:    {}", manifest.package.main);

                if !manifest.scripts.is_empty() {
                    println!("\nScripts:");
                    let mut keys: Vec<_> = manifest.scripts.keys().collect();
                    keys.sort();
                    for name in keys {
                        println!("  {:10} {}", name, manifest.scripts[name]);
                    }
                }

                if !manifest.dependencies.is_empty() {
                    println!("\nDependencies:");
                    let mut keys: Vec<_> = manifest.dependencies.keys().collect();
                    keys.sort();
                    for name in keys {
                        println!("  {:10} {:?}", name, manifest.dependencies[name]);
                    }
                }
            }
        }
        Err(e) => {
            if json_output {
                println!(r#"{{"error": "{}"}}"#, e);
            } else {
                eprintln!("Error loading pleno.toml: {}", e);
            }
            process::exit(1);
        }
    }
}

fn handle_run(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let mut target = None;
    let mut max_memory = 0;
    let mut max_instructions = 0;
    let mut allow_io = true;
    let mut allow_network = true;
    let mut allowed_read_paths = Vec::new();
    let mut allowed_write_paths = Vec::new();
    let mut sandbox_active = false;
    let mut debug_mode = false;

    let iter = args.by_ref();
    while let Some(arg) = iter.next() {
        if arg == "--max-memory" {
            if let Some(m) = iter.next() {
                max_memory = m.parse::<usize>().unwrap_or(0);
                sandbox_active = true;
            }
        } else if arg == "--max-instructions" {
            if let Some(m) = iter.next() {
                max_instructions = m.parse::<u64>().unwrap_or(0);
                sandbox_active = true;
            }
        } else if arg == "--allow-io" {
            if let Some(m) = iter.next() {
                allow_io = m.parse::<bool>().unwrap_or(true);
                sandbox_active = true;
            }
        } else if arg == "--allow-network" {
            if let Some(m) = iter.next() {
                allow_network = m.parse::<bool>().unwrap_or(true);
                sandbox_active = true;
            }
        } else if arg == "--allow-read" {
            if let Some(m) = iter.next() {
                allowed_read_paths.push(m);
                sandbox_active = true;
            }
        } else if arg == "--allow-write" {
            if let Some(m) = iter.next() {
                allowed_write_paths.push(m);
                sandbox_active = true;
            }
        } else if arg == "--debug" {
            debug_mode = true;
        } else if target.is_none() {
            target = Some(arg);
        }
    }

    match target {
        Some(t) => {
            let config = if sandbox_active {
                Some(pleno_value::SandboxConfig {
                    max_memory,
                    max_instructions,
                    allow_io,
                    allow_network,
                    allowed_read_paths,
                    allowed_write_paths,
                })
            } else {
                None
            };

            if t.ends_with(".pln") {
                if debug_mode {
                    handle_dap_for_file(&t);
                } else {
                    run_file(&t, config, BuildProfile::Debug);
                }
            } else if t.ends_with(".plc") {
                run_bytecode_file(&t, config);
            } else {
                match Manifest::load_from_file("pleno.toml") {
                    Ok(manifest) => {
                        let manager = pleno_package::PackageManager::new("packages");
                        if let Err(_e) = manager.run_script(&manifest, &t) {
                            eprintln!(
                                "Script '{}' not found in pleno.toml, and file '{}' does not exist.",
                                t, t
                            );
                            process::exit(1);
                        }
                    }
                    Err(_) => {
                        eprintln!("File '{}' not found.", t);
                        process::exit(1);
                    }
                }
            }
        }
        None => {
            match Manifest::load_from_file("pleno.toml") {
                Ok(manifest) => {
                    let manager = pleno_package::PackageManager::new("packages");
                    // Default script to run if none provided? 'start' is a common default.
                    let script = "start";
                    if let Err(_e) = manager.run_script(&manifest, script) {
                        eprintln!(
                            "Error: No target provided and 'start' script not found in pleno.toml"
                        );
                        process::exit(1);
                    }
                }
                Err(_) => {
                    eprintln!("Error: 'run' requires a file path or script name");
                    process::exit(1);
                }
            }
        }
    }
}

fn handle_doc(path: Option<&str>, output_dir: Option<&str>) {
    let source_path = PathBuf::from(path.unwrap_or("."));
    let output_path = PathBuf::from(output_dir.unwrap_or("docs_out"));

    let mut generated_files = Vec::new();
    if let Err(e) = generate_docs_recursive(&source_path, &output_path, &mut generated_files) {
        eprintln!("Documentation generation failed: {}", e);
        process::exit(1);
    }

    if let Err(e) = generate_index(&output_path, &generated_files) {
        eprintln!("Warning: Failed to generate index: {}", e);
    }

    println!("Documentation generated in: {}", output_path.display());
}

fn generate_docs_recursive(
    source_path: &Path,
    output_path: &Path,
    generated_files: &mut Vec<String>,
) -> Result<(), String> {
    if source_path.is_dir() {
        for entry in fs::read_dir(source_path)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                // Skip common hidden dirs or build artifacts
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == "target" || s == ".git")
                {
                    continue;
                }
                generate_docs_recursive(&path, output_path, generated_files)?;
            } else if path.extension().is_some_and(|ext| ext == "pln")
                && let Some(file_name) = generate_docs_for_file(&path, output_path)?
            {
                generated_files.push(file_name);
            }
        }
    } else if source_path.extension().is_some_and(|ext| ext == "pln")
        && let Some(file_name) = generate_docs_for_file(source_path, output_path)?
    {
        generated_files.push(file_name);
    }
    Ok(())
}

fn generate_docs_for_file(path: &Path, output_dir: &Path) -> Result<Option<String>, String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file_display_name = path.file_name().unwrap().to_string_lossy();

    // Only generate if there's actual content or doc comments
    if !source.contains("///") && !source.contains("class") && !source.contains("fn") {
        return Ok(None);
    }

    let docs = pleno_doc::generate_docs(&source, &file_display_name)?;

    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    }

    let mut output_file = output_dir.to_path_buf();
    let file_stem = path.file_stem().unwrap().to_string_lossy();
    output_file.push(format!("{}.md", file_stem));

    fs::write(&output_file, docs).map_err(|e| e.to_string())?;
    Ok(Some(format!("{}.md", file_stem)))
}

fn generate_index(output_dir: &Path, files: &[String]) -> Result<(), String> {
    let mut index = String::new();
    index.push_str("# Project Documentation\n\n");
    index.push_str("## Modules\n\n");

    for file in files {
        let title = file.trim_end_matches(".md");
        index.push_str(&format!("- [{}]({})\n", title, file));
    }

    let mut index_path = output_dir.to_path_buf();
    index_path.push("README.md");
    fs::write(index_path, index).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_dap(_args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let stdin = std::io::stdin();
    let input = std::io::BufReader::new(stdin);
    let output = std::io::stdout();
    let mut server = pleno_dap::server::DapServer::new(input, output);
    if let Err(e) = server.run() {
        eprintln!("DAP server error: {e}");
        std::process::exit(1);
    }
}

fn handle_dap_for_file(_path: &str) {
    let stdin = std::io::stdin();
    let input = std::io::BufReader::new(stdin);
    let output = std::io::stdout();
    let mut server = pleno_dap::server::DapServer::new(input, output);
    if let Err(e) = server.run() {
        eprintln!("DAP server error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod check_tests {
    use super::*;

    fn summary(source: &str) -> Vec<(Severity, String, (usize, usize))> {
        check_source(source)
            .into_iter()
            .map(|p| (p.severity, p.message, line_and_column(source, p.start)))
            .collect()
    }

    #[test]
    fn check_reports_static_problems_in_source_order() {
        let source = "fn greet(name, greeting = \"Hi\") {}\n\
                      greet(1, 2, 3);\n\
                      let x: Int = \"hi\";\n\
                      print(missing);\n\
                      class A { fn m() { this.nope(); } }\n";
        let error = |message: &str, at| (Severity::Error, message.to_string(), at);
        assert_eq!(
            summary(source),
            vec![
                error("greet expects 1 to 2 arguments, got 3", (2, 1)),
                error("'x' is declared Int but given a String literal", (3, 14)),
                error("undefined variable 'missing'", (4, 7)),
                (
                    Severity::Warning,
                    "A has no method 'nope'".to_string(),
                    (5, 20)
                ),
            ]
        );
    }

    #[test]
    fn check_passes_valid_calls() {
        let source = "fn f(a, b = 1, ...rest) { return a; }\nprint(f(1), f(1, 2, 3, 4));\n";
        assert_eq!(summary(source), Vec::new());
    }
}

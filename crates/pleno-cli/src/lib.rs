use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use pleno_package::{Dependency, DependencyDetail, Manifest};

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
            println!("pleno {VERSION}", VERSION = env!("CARGO_PKG_VERSION"));
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
        "format" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'format' requires a file or directory path");
                process::exit(1);
            });
            format_file(&path);
        }
        "lint" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'lint' requires a file or directory path");
                process::exit(1);
            });
            lint_file(&path);
        }
        "check" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'check' requires a file path");
                process::exit(1);
            });
            check_file(&path);
        }
        "test" => {
            let mut test_path = "sources/tests".to_string();
            let mut json_output = false;
            for arg in args.by_ref() {
                if arg == "--json" {
                    json_output = true;
                } else {
                    test_path = arg;
                }
            }
            run_tests(&test_path, json_output);
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
            compile_file(&path);
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
        _ => {
            run_file(&command, None);
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
        "build" => handle_build(args),
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
            println!("  publish          Publish the current package");
            println!("  build            Build the current package");
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
    println!(
        "Pleno Programming Language CLI\n\n\
        Usage: pleno <command> [arguments]\n\n\
        Commands:\n\
          run <file.pln>     Run a Pleno file\n\
          plm <command>      Package management (init, install, run)\n\
          format <path>      Format Pleno files\n\
          lint <path>        Lint Pleno files\n\
          check <file.pln>   Type check a Pleno file\n\
          test <path>        Run tests in file or directory\n\
          doc [path] [out]   Generate documentation\n\
          repl               Start the interactive REPL\n\
          lsp                Start the Language Server\n\n\
        Package Commands (plm):\n\
          init [name]        Initialize a new Pleno package\n\
          install [url]      Install dependencies or a specific package\n\
          update             Update all dependencies\n\
          remove <name>      Remove a dependency\n\
          run <script>       Run a script defined in pleno.toml\n\n\
        Options:\n\
          -h, --help                Show this help message\n\
          -v, --version             Show version information\n\
          --max-memory <bytes>      Set memory limit in bytes\n\
          --max-instructions <num> Set instruction count limit\n\
          --allow-io <bool>         Enable/disable IO access\n\
          --allow-network <bool>    Enable/disable network access\n\n\
        Examples:\n\
          pleno run main.pln\n\
          pleno plm install\n\
          pleno plm update\n\
          pleno plm remove http-client\n\
          pleno plm run dev"
    );
}

fn run_tests(path: &str, json_output: bool) {
    let path_buf = PathBuf::from(path);
    let mut stats = TestStats::default();

    if path_buf.is_dir() {
        run_tests_in_dir(&path_buf, &mut stats, json_output);
    } else {
        run_single_test(&path_buf, &mut stats, json_output);
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
        println!("\nTest Summary:");
        println!("  Passed: {}", stats.passed);
        println!("  Failed: {}", stats.failed);
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

fn run_tests_in_dir(dir: &PathBuf, stats: &mut TestStats, json_output: bool) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                run_tests_in_dir(&path, stats, json_output);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                run_single_test(&path, stats, json_output);
            }
        }
    }
}

fn run_single_test(path: &PathBuf, stats: &mut TestStats, json_output: bool) {
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
                println!("FAILED (Lex Error)");
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
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            if !json_output {
                println!("FAILED (Parse Error)");
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

    let is_expected_compile_error = source.contains("// EXPECT_COMPILE_ERROR");
    let is_expected_runtime_error = source.contains("// EXPECT_RUNTIME_ERROR");

    let mut compiler = pleno_compiler::Compiler::new(source);
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => {
            if is_expected_compile_error {
                if !json_output {
                    println!("FAILED (Expected Compile Error but succeeded)");
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
                    println!("PASSED (Expected Compile Error)");
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
                println!("FAILED (Compile Error)");
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
    pleno_stdlib::register(&mut vm);
    match vm.execute(chunk) {
        Ok(_) => {
            if is_expected_runtime_error {
                if !json_output {
                    println!("FAILED (Expected Runtime Error but succeeded)");
                }
                stats.failed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "failed",
                    "message": "Expected runtime error but succeeded"
                }));
            } else {
                if !json_output {
                    println!("PASSED");
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
                    println!("PASSED (Expected Runtime Error)");
                }
                stats.passed += 1;
                stats.results.push(serde_json::json!({
                    "file": path.display().to_string(),
                    "status": "passed",
                    "message": "Expected runtime error occurred"
                }));
            } else {
                if !json_output {
                    println!("FAILED (Runtime Error)");
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

fn run_file(path: &str, config: Option<pleno_value::SandboxConfig>) {
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
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            process::exit(1);
        }
    };

    let mut compiler = pleno_compiler::Compiler::new(source.clone());
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!(
                "Compile error: {}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            process::exit(1);
        }
    };

    let mut vm = pleno_vm::VM::new();
    if let Some(cfg) = config {
        vm.apply_config(cfg);
    }
    pleno_stdlib::register(&mut vm);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| vm.execute(chunk)));

    match result {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );

            // Manual stack trace printing for now as Diagnostic doesn't support it fully yet
            if !err.stack_trace.is_empty() {
                eprintln!("\nStack backtrace:");
                for frame in err.stack_trace {
                    if let Some(span) = frame.span {
                        // Fallback line calculation if outside main file?
                        // For now assume main file or just print span
                        if span.end <= source.len() {
                            let (line, col) = get_line_col(&source, span.start);
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

fn format_file(path: &str) {
    let path_buf = PathBuf::from(path);
    if path_buf.is_dir() {
        format_directory(&path_buf);
    } else {
        if !path.ends_with(".pln") {
            eprintln!("Error: Only .pln format file extensions are supported");
            process::exit(1);
        }
        format_single_file(&path_buf);
    }
}

fn format_single_file(path: &PathBuf) {
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

    let mut formatter = pleno_formatter::Formatter::new();
    let formatted = formatter.format_program(&program, 0);

    if let Err(err) = fs::write(path, formatted) {
        eprintln!("Failed to write to {}: {err}", path.display());
        process::exit(1);
    }
    println!("Formatted {}", path.display());
}

fn format_directory(dir: &PathBuf) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                format_directory(&path);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                format_single_file(&path);
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

fn check_file(path: &str) {
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

    let mut compiler = pleno_compiler::Compiler::new(source);
    match compiler.compile(&program) {
        Ok(_) => {
            println!("Type check passed for {}", path);
        }
        Err(err) => {
            eprintln!("Compile error: {}", err.diagnostic.message);
            process::exit(1);
        }
    }
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
    run_file(&path.to_string_lossy(), None);

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
                run_file(&path.to_string_lossy(), None);
            }
            Ok(Err(e)) => println!("watch error: {:?}", e),
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

fn compile_file(path: &str) {
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

    let mut compiler = pleno_compiler::Compiler::new(source);
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!("Compile error: {}", err.diagnostic.message);
            process::exit(1);
        }
    };

    let output_path = std::path::Path::new(path).with_extension("plc");
    let file = fs::File::create(&output_path).expect("failed to create file");

    match bincode::serialize_into(file, &chunk) {
        Ok(_) => {
            println!("Compiled {} -> {}", path, output_path.display());
        }
        Err(err) => {
            eprintln!("Failed to write bytecode: {}", err);
            process::exit(1);
        }
    }
}

fn handle_init(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let name = args.next().unwrap_or_else(|| {
        env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-app".to_string())
    });
    let manifest = Manifest::new(&name, "0.1.0");
    if let Err(e) = manifest.save_to_file("pleno.toml") {
        eprintln!("Failed to create pleno.toml: {}", e);
        process::exit(1);
    }
    println!("Created pleno.toml for {}", name);
}

fn handle_install(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let mut pkg_name = None;
    let mut features = Vec::new();
    let mut offline = false;
    let mut is_dev = false;

    while let Some(arg) = args.next() {
        if arg == "--features" {
            if let Some(f_list) = args.next() {
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

    let registry_url = std::env::var("PLENO_REGISTRY")
        .unwrap_or_else(|_| "https://registry.pleno.dev".to_string());
    let manager = pleno_package::PackageManager::new("packages")
        .with_registry(registry_url)
        .with_offline(offline);

    if let Some(pkg) = pkg_name {
        let (name, source) = if let Some(pos) = pkg.rfind('@') {
            if pos > 0 && !pkg.starts_with("git+") && !pkg.starts_with("http") {
                let (n, s) = pkg.split_at(pos);
                (n.to_string(), s[1..].to_string())
            } else {
                (pkg.clone(), "*".to_string())
            }
        } else if pkg.contains('/') || pkg.starts_with("git+") {
            let name = pkg.split('/').next_back().unwrap().replace(".git", "");
            (name, pkg.clone())
        } else {
            (pkg.clone(), "*".to_string())
        };

        let dep = if source.contains('/')
            || source.starts_with("git+")
            || source.starts_with("http")
        {
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
    let manager = pleno_package::PackageManager::new("packages");
    if let Err(e) = manager.publish(&manifest) {
        eprintln!("Publish failed: {}", e);
        process::exit(1);
    }
}

fn handle_build(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let target = args.next();

    match target {
        Some(t) if t.ends_with(".pln") => {
            compile_file(&t);
        }
        Some(t) => {
            if let Ok(manifest) = Manifest::load_from_file("pleno.toml") {
                let entry = &manifest.package.main;
                compile_file(entry);
            } else {
                eprintln!("Error: No pleno.toml found and '{}' is not a .pln file", t);
                process::exit(1);
            }
        }
        None => {
            if let Ok(manifest) = Manifest::load_from_file("pleno.toml") {
                let entry = &manifest.package.main;
                compile_file(entry);
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

    while let Some(arg) = args.next() {
        if arg == "--max-memory" {
            if let Some(m) = args.next() {
                max_memory = m.parse::<usize>().unwrap_or(0);
                sandbox_active = true;
            }
        } else if arg == "--max-instructions" {
            if let Some(m) = args.next() {
                max_instructions = m.parse::<u64>().unwrap_or(0);
                sandbox_active = true;
            }
        } else if arg == "--allow-io" {
            if let Some(m) = args.next() {
                allow_io = m.parse::<bool>().unwrap_or(true);
                sandbox_active = true;
            }
        } else if arg == "--allow-network" {
            if let Some(m) = args.next() {
                allow_network = m.parse::<bool>().unwrap_or(true);
                sandbox_active = true;
            }
        } else if arg == "--allow-read" {
            if let Some(m) = args.next() {
                allowed_read_paths.push(m);
                sandbox_active = true;
            }
        } else if arg == "--allow-write" {
            if let Some(m) = args.next() {
                allowed_write_paths.push(m);
                sandbox_active = true;
            }
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
                run_file(&t, config);
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

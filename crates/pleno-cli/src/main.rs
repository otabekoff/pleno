use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

// CLI imports
use pleno_package::{Bundle, Dependency, Manifest};

fn main() {
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
            println!("pleno {}", env!("CARGO_PKG_VERSION"));
        }
        "init" => {
            handle_init(&mut args);
        }
        "install" => {
            handle_install(&mut args);
        }
        "build" => {
            handle_build(&mut args);
        }
        "plm" => {
            let sub_command = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'plm' requires a command (init, install, run)");
                process::exit(1)
            });
            match sub_command.as_str() {
                "init" => handle_init(&mut args),
                "install" => handle_install(&mut args),
                "run" => handle_plm_run(&mut args),
                "publish" => handle_plm_publish(&mut args),
                "build" => handle_build(&mut args),
                _ => {
                    eprintln!("Error: Unknown plm command '{}'", sub_command);
                    process::exit(1);
                }
            }
        }
        "run" => {
            let target = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'run' requires a file path or script name");
                process::exit(1);
            });
            handle_run(&target);
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
            let path = args.next();
            let path = path.as_deref().unwrap_or("sources/tests");
            run_tests(path);
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
                eprintln!("LSP server error: {}", e);
                process::exit(1);
            }
        }
        "repl" => {
            if let Err(e) = pleno_repl::start() {
                eprintln!("REPL error: {}", e);
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
            // Default: treat as file path and run it
            run_file(&command);
        }
    }
}

// Fallback if no args provided: run REPL
fn start_default() {
    if let Err(e) = pleno_repl::start() {
        eprintln!("REPL error: {}", e);
        process::exit(1);
    }
}

fn run_tests(path: &str) {
    let path_buf = PathBuf::from(path);
    let mut stats = TestStats::default();

    if path_buf.is_dir() {
        run_tests_in_dir(&path_buf, &mut stats);
    } else {
        run_single_test(&path_buf, &mut stats);
    }

    println!("\nTest Summary:");
    println!("  Passed: {}", stats.passed);
    println!("  Failed: {}", stats.failed);
    println!("  Total:  {}", stats.passed + stats.failed);

    if stats.failed > 0 {
        process::exit(1);
    }
}

#[derive(Default)]
struct TestStats {
    passed: usize,
    failed: usize,
}

fn run_tests_in_dir(dir: &PathBuf, stats: &mut TestStats) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                run_tests_in_dir(&path, stats);
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                run_single_test(&path, stats);
            }
        }
    } else {
        eprintln!("Failed to read directory: {}", dir.display());
    }
}

fn run_single_test(path: &PathBuf, stats: &mut TestStats) {
    let source = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Failed to read {}: {err}", path.display());
            stats.failed += 1;
            return;
        }
    };

    // Print running message
    print!("Running {} ... ", path.display());
    // Flush stdout to ensure message appears before test output
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            println!("FAILED (Lex Error)");
            eprintln!(
                "  Lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            stats.failed += 1;
            return;
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            println!("FAILED (Parse Error)");
            eprintln!(
                "  Parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            stats.failed += 1;
            return;
        }
    };

    let is_expected_compile_error = source.contains("// EXPECT_COMPILE_ERROR");
    let is_expected_runtime_error = source.contains("// EXPECT_RUNTIME_ERROR");

    let mut compiler = pleno_compiler::Compiler::new(source);
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => {
            if is_expected_compile_error {
                println!("FAILED (Expected Compile Error but succeeded)");
                stats.failed += 1;
                return;
            }
            chunk
        }
        Err(err) => {
            if is_expected_compile_error {
                println!("PASSED (Expected Compile Error)");
                stats.passed += 1;
                return;
            }
            println!("FAILED (Compile Error)");
            eprintln!("  Compile error: {}", err.message);
            stats.failed += 1;
            return;
        }
    };

    let mut vm = pleno_vm::VM::new();
    pleno_stdlib::register(&mut vm);
    // Temporarily verify simply by running without error
    match vm.execute(chunk) {
        Ok(_) => {
            if is_expected_runtime_error {
                println!("FAILED (Expected Runtime Error but succeeded)");
                stats.failed += 1;
            } else {
                println!("PASSED");
                stats.passed += 1;
            }
        }
        Err(err) => {
            if is_expected_runtime_error {
                println!("PASSED (Expected Runtime Error)");
                stats.passed += 1;
            } else {
                println!("FAILED (Runtime Error)");
                eprintln!("  Runtime error: {}", err.message);
                stats.failed += 1;
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

    let mut parser = pleno_parser::Parser::new(tokens);
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

fn run_file(path: &str) {
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
            let diagnostic = pleno_diagnostics::Diagnostic::new(&err.message, err.span);
            eprintln!(
                "{}",
                pleno_diagnostics::DiagnosticReporter::report(&diagnostic, &source)
            );
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            let diagnostic = pleno_diagnostics::Diagnostic::new(&err.message, err.span);
            eprintln!(
                "{}",
                pleno_diagnostics::DiagnosticReporter::report(&diagnostic, &source)
            );
            process::exit(1);
        }
    };

    let mut compiler = pleno_compiler::Compiler::new(source.clone());
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!("Compile error: {}", err.message);
            process::exit(1);
        }
    };

    let mut vm = pleno_vm::VM::new();
    pleno_stdlib::register(&mut vm);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| vm.execute(chunk)));

    match result {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            if let Some(span) = err.span {
                let diagnostic = pleno_diagnostics::Diagnostic::new(&err.message, span);
                eprintln!(
                    "{}",
                    pleno_diagnostics::DiagnosticReporter::report(&diagnostic, &source)
                );
            } else {
                eprintln!("Runtime error: {}", err.message);
            }

            if !err.stack_trace.is_empty() {
                eprintln!("\nStack backtrace:");
                for frame in err.stack_trace {
                    if let Some(span) = frame.span {
                        let (line, col) = get_line_col(&source, span.start);
                        eprintln!("  at {} ({}:{})", frame.function_name, line, col);
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
        // Format all .pln files in directory
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

    // Basic formatter: just re-parse and output (minimal formatting)
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

    let mut parser = pleno_parser::Parser::new(tokens);
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
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Failed to read directory {}: {err}", dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            format_directory(&path);
        } else if path.extension().is_some_and(|ext| ext == "pln") {
            format_single_file(&path);
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

    // Use pleno_linter crate
    let issues = pleno_linter::lint(&source);

    if issues.is_empty() {
        println!("No linting issues found in {}", path.display());
    } else {
        println!("{}", pleno_linter::format_diagnostics(&issues));
        process::exit(1);
    }
}

fn lint_directory(dir: &PathBuf) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Failed to read directory {}: {err}", dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            lint_directory(&path);
        } else if path.extension().is_some_and(|ext| ext == "pln") {
            lint_single_file(&path);
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

    let mut parser = pleno_parser::Parser::new(tokens);
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
            eprintln!("Compile error: {}", err.message);
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
    run_file(&path.to_string_lossy());

    let (tx, rx) = channel();

    // Create a watcher object, delivering debounced events.
    // The notification back-end is selected based on the platform.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Config::default()).unwrap();

    // Watch the parent directory to catch atomic saves/renames
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
                // Determine if we should clear screen and run
                // Simple debounce: drain channel
                // Ideally check event paths, but for now re-run on any folder change
                // if it matches our file.
                // Assuming simple workflow for now: clear, run.

                // Read all pending events to debounce
                while rx.try_recv().is_ok() {}

                print!("\x1B[2J\x1B[1;1H"); // Clear screen
                println!("File changed. Re-running...");
                run_file(&path.to_string_lossy());
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

    let mut parser = pleno_parser::Parser::new(tokens);
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
            eprintln!("Compile error: {}", err.message);
            process::exit(1);
        }
    };

    let output_path = std::path::Path::new(path).with_extension("plc");
    let file = match fs::File::create(&output_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "Failed to create output file {}: {err}",
                output_path.display()
            );
            process::exit(1);
        }
    };

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

fn print_help() {
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
          run <script>       Run a script defined in package.pln\n\n\
        Options:\n\
          -h, --help         Show this help message\n\
          -v, --version      Show version information\n\n\
        Examples:\n\
          pleno run main.pln\n\
          pleno plm install\n\
          pleno doc sources/\n\
          pleno plm run dev"
    );
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

    while let Some(arg) = args.next() {
        if arg == "--features" {
            if let Some(f_list) = args.next() {
                for f in f_list.split(',') {
                    features.push(f.to_string());
                }
            }
        } else if arg == "--offline" {
            offline = true;
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
        let (name, source) = if let Some(pos) = pkg.find('@') {
            let (n, s) = pkg.split_at(pos);
            (n.to_string(), s[1..].to_string())
        } else if pkg.contains('/') || pkg.starts_with("git+") {
            let name = pkg.split('/').next_back().unwrap().replace(".git", "");
            (name, pkg.clone())
        } else {
            eprintln!("Error: Package source must be 'name@version', a Git URL, or local path.");
            process::exit(1);
        };

        let dep = if source.contains('/')
            || source.starts_with("git+")
            || source.starts_with("http")
        {
            Dependency::Detailed(pleno_package::DependencyDetail {
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
            Dependency::Detailed(pleno_package::DependencyDetail {
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

        manifest.dependencies.insert(name.clone(), dep);
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

fn handle_build(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let entry_file = args.next();

    let manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };

    let manager = pleno_package::PackageManager::new("packages");
    println!("Building project {}...", manifest.package.name);

    if let Err(e) = manager.build_project(&manifest, entry_file.as_deref()) {
        eprintln!("Build failed: {}", e);
        process::exit(1);
    }
    println!("Build successful: {}.plb", manifest.package.name);
}

fn handle_plm_publish(_args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let manifest = match Manifest::load_from_file("pleno.toml") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load pleno.toml: {}", e);
            process::exit(1);
        }
    };

    let registry_url = std::env::var("PLENO_REGISTRY")
        .unwrap_or_else(|_| "https://registry.pleno.dev".to_string());

    let manager = pleno_package::PackageManager::new("packages").with_registry(registry_url);

    println!(
        "Publishing {} v{}...",
        manifest.package.name, manifest.package.version
    );
    if let Err(e) = manager.publish(&manifest) {
        eprintln!("Publish failed: {}", e);
        process::exit(1);
    }
    println!("Successfully published!");
}

fn handle_plm_run(args: &mut std::iter::Peekable<std::iter::Skip<std::env::Args>>) {
    let target = args.next().unwrap_or_else(|| {
        eprintln!("Error: 'plm run' requires a script name");
        process::exit(1);
    });

    match Manifest::load_from_file("pleno.toml") {
        Ok(manifest) => {
            if let Some(script) = manifest.scripts.get(&target) {
                run_shell_script(script);
            } else {
                eprintln!("Script '{}' not found in pleno.toml", target);
                process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("Error: pleno.toml not found.");
            process::exit(1);
        }
    }
}

fn run_bundle(path: &str) {
    let bytes = fs::read(path).unwrap_or_else(|e| {
        eprintln!("Failed to read bundle {}: {}", path, e);
        process::exit(1);
    });
    let bundle: Bundle = bincode::deserialize(&bytes).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize bundle {}: {}", path, e);
        process::exit(1);
    });
    let chunk = bundle.modules.get(&bundle.main_module).unwrap_or_else(|| {
        eprintln!("Main module {} not found in bundle", bundle.main_module);
        process::exit(1);
    });

    let mut vm = pleno_vm::VM::new();
    pleno_stdlib::register(&mut vm);
    if let Err(e) = vm.execute(chunk.clone()) {
        eprintln!("VM error: {}", e.message);
        process::exit(1);
    }
}

fn handle_run(target: &str) {
    if PathBuf::from(target).exists() {
        if target.ends_with(".plb") {
            run_bundle(target);
        } else if target.ends_with(".pln") {
            run_file(target);
        } else {
            eprintln!("Error: Only .pln and .plb format file extensions are supported");
            process::exit(1);
        }
    } else {
        match Manifest::load_from_file("pleno.toml") {
            Ok(manifest) => {
                if let Some(script) = manifest.scripts.get(target) {
                    run_shell_script(script);
                } else {
                    eprintln!(
                        "Script '{}' not found in pleno.toml, and file '{}' does not exist.",
                        target, target
                    );
                    process::exit(1);
                }
            }
            Err(_) => {
                eprintln!("File '{}' not found.", target);
                process::exit(1);
            }
        }
    }
}

fn run_shell_script(script: &str) {
    let status = if cfg!(windows) {
        process::Command::new("cmd").arg("/C").arg(script).status()
    } else {
        process::Command::new("sh").arg("-c").arg(script).status()
    };

    match status {
        Ok(s) => {
            if !s.success() {
                process::exit(s.code().unwrap_or(1));
            }
        }
        Err(e) => {
            eprintln!("Failed to execute script: {}", e);
            process::exit(1);
        }
    }
}

fn handle_doc(path: Option<&str>, output_dir: Option<&str>) {
    let source_path = PathBuf::from(path.unwrap_or("."));
    let output_path = PathBuf::from(output_dir.unwrap_or("docs"));

    // Create output directory
    if let Err(e) = fs::create_dir_all(&output_path) {
        eprintln!("Error creating output directory: {}", e);
        process::exit(1);
    }

    let mut files_processed = 0;
    let mut all_items: Vec<(String, Vec<pleno_doc::DocItem>)> = Vec::new();

    if source_path.is_file() {
        if let Some(items) = process_doc_file(&source_path) {
            let name = source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();
            all_items.push((name, items));
            files_processed += 1;
        }
    } else if source_path.is_dir() {
        process_doc_directory(&source_path, &mut all_items, &mut files_processed);
    } else {
        eprintln!(
            "Error: '{}' is not a valid file or directory",
            source_path.display()
        );
        process::exit(1);
    }

    // Generate Markdown files
    let renderer = pleno_doc::MarkdownRenderer::new();

    for (name, items) in &all_items {
        if items.is_empty() {
            continue;
        }
        let md_content = renderer.render(items, name);
        let md_path = output_path.join(format!("{}.md", name));
        if let Err(e) = fs::write(&md_path, md_content) {
            eprintln!("Error writing {}: {}", md_path.display(), e);
        } else {
            println!("Generated: {}", md_path.display());
        }
    }

    // Generate index.md
    if !all_items.is_empty() {
        let mut index = String::from("# API Documentation\n\n");
        index.push_str("## Modules\n\n");
        for (name, items) in &all_items {
            if !items.is_empty() {
                index.push_str(&format!("- [{}]({}.md)\n", name, name));
            }
        }
        let index_path = output_path.join("index.md");
        if let Err(e) = fs::write(&index_path, index) {
            eprintln!("Error writing index: {}", e);
        } else {
            println!("Generated: {}", index_path.display());
        }
    }

    println!(
        "\nDocumentation generated: {} files processed",
        files_processed
    );
}

fn process_doc_file(path: &PathBuf) -> Option<Vec<pleno_doc::DocItem>> {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            return None;
        }
    };

    // Lex
    let tokens = match pleno_lexer::lex(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lex error in {}: {}", path.display(), e.message);
            return None;
        }
    };

    // Parse
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error in {}: {}", path.display(), e.message);
            return None;
        }
    };

    // Extract documentation
    let generator = pleno_doc::DocGenerator::new();
    Some(generator.extract(&program))
}

fn process_doc_directory(
    dir: &PathBuf,
    all_items: &mut Vec<(String, Vec<pleno_doc::DocItem>)>,
    files_processed: &mut usize,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", dir.display(), e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            process_doc_directory(&path, all_items, files_processed);
        } else if path.extension().is_some_and(|e| e == "pln")
            && let Some(items) = process_doc_file(&path)
        {
            if !items.is_empty() {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string();
                all_items.push((name, items));
            }
            *files_processed += 1;
        }
    }
}

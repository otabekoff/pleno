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
            println!("pleno {}", env!("CARGO_PKG_VERSION"));
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
            run_file(&command);
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
          run <script>       Run a script defined in package.pln\n\n\
        Options:\n\
          -h, --help         Show this help message\n\
          -v, --version      Show version information\n\n\
        Examples:\n\
          pleno run main.pln\n\
          pleno plm install\n\
          pleno plm update\n\
          pleno plm remove http-client\n\
          pleno plm run dev"
    );
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

    print!("Running {} ... ", path.display());
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
    if let Err(e) = manifest.save_to_file("package.pln") {
        eprintln!("Failed to create package.pln: {}", e);
        process::exit(1);
    }
    println!("Created package.pln for {}", name);
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

    let mut manifest = match Manifest::load_from_file("package.pln") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load package.pln: {}", e);
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
        if let Err(e) = manifest.save_to_file("package.pln") {
            eprintln!("Failed to save package.pln: {}", e);
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
    let mut manifest = match Manifest::load_from_file("package.pln") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load package.pln: {}", e);
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
    let mut manifest = match Manifest::load_from_file("package.pln") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load package.pln: {}", e);
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
    let manifest = match Manifest::load_from_file("package.pln") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load package.pln: {}", e);
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
    let manifest = match Manifest::load_from_file("package.pln") {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load package.pln: {}", e);
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
    let target = args.next().unwrap_or_else(|| {
        eprintln!("Error: 'build' requires a target (file or package)");
        process::exit(1);
    });

    if target.ends_with(".pln") {
        compile_file(&target);
    } else {
        if let Ok(manifest) = Manifest::load_from_file("package.pln") {
            let entry = &manifest.package.main;
            compile_file(entry);
        } else {
            eprintln!(
                "Error: No package.pln found and '{}' is not a .pln file",
                target
            );
            process::exit(1);
        }
    }
}

fn handle_run(target: &str) {
    if target.ends_with(".pln") {
        run_file(target);
    } else {
        match Manifest::load_from_file("package.pln") {
            Ok(manifest) => {
                let manager = pleno_package::PackageManager::new("packages");
                if let Err(_e) = manager.run_script(&manifest, target) {
                    eprintln!(
                        "Script '{}' not found in package.pln, and file '{}' does not exist.",
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

fn handle_doc(path: Option<&str>, output_dir: Option<&str>) {
    let source_path = PathBuf::from(path.unwrap_or("."));
    let output_path = PathBuf::from(output_dir.unwrap_or("docs_out"));

    if let Err(e) = generate_docs_recursive(&source_path, &output_path) {
        eprintln!("Documentation generation failed: {}", e);
        process::exit(1);
    }
}

fn generate_docs_recursive(source_path: &Path, output_path: &Path) -> Result<(), String> {
    if source_path.is_dir() {
        for entry in fs::read_dir(source_path)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                generate_docs_recursive(&path, output_path)?;
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                generate_docs_for_file(&path, output_path)?;
            }
        }
    } else if source_path.extension().is_some_and(|ext| ext == "pln") {
        generate_docs_for_file(source_path, output_path)?;
    }
    Ok(())
}

fn generate_docs_for_file(path: &Path, output_dir: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file_name = path.file_name().unwrap().to_string_lossy();
    let docs = pleno_doc::generate_docs(&source, &file_name)?;

    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    }

    let mut output_file = output_dir.to_path_buf();
    output_file.push(path.file_name().unwrap());
    output_file.set_extension("md");

    fs::write(output_file, docs).map_err(|e| e.to_string())?;
    Ok(())
}

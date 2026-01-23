use std::env;
use std::fs;
// CLI imports
use std::path::PathBuf;
use std::process;

fn main() {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        print_help();
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
        "run" => {
            let path = args.next().unwrap_or_else(|| {
                eprintln!("Error: 'run' requires a file path");
                process::exit(1);
            });
            run_file(&path);
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
        _ => {
            // Default: treat as file path and run it
            run_file(&command);
        }
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

    let mut compiler = pleno_compiler::Compiler::new();
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
    // Temporarily verify simply by running without error
    match vm.execute(chunk) {
        Ok(_) => {
            println!("PASSED");
            stats.passed += 1;
        }
        Err(err) => {
            println!("FAILED (Runtime Error)");
            eprintln!("  Runtime error: {}", err.message);
            stats.failed += 1;
        }
    }
}

fn debug_ast(path: &str) {
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

    let mut compiler = pleno_compiler::Compiler::new();
    let chunk = match compiler.compile(&program) {
        Ok(chunk) => chunk,
        Err(err) => {
            eprintln!("Compile error: {}", err.message);
            process::exit(1);
        }
    };

    let mut vm = pleno_vm::VM::new();
    match vm.execute(chunk) {
        Ok(_) => {}
        Err(err) => {
            eprintln!("Runtime error: {}", err.message);
            process::exit(1);
        }
    }
}

fn format_file(path: &str) {
    let path_buf = PathBuf::from(path);
    if path_buf.is_dir() {
        // Format all .pln files in directory
        format_directory(&path_buf);
    } else {
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

    let mut issues = Vec::new();

    // Lex and check for issues
    let tokens = match pleno_lexer::lex(&source) {
        Ok(tokens) => tokens,
        Err(err) => {
            issues.push(format!(
                "Lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            ));
            // Continue to try parsing
            vec![]
        }
    };

    if tokens.is_empty() && !issues.is_empty() {
        for issue in issues {
            eprintln!("{}", issue);
        }
        process::exit(1);
    }

    // Parse and check for issues
    let mut parser = pleno_parser::Parser::new(tokens);
    match parser.parse_program() {
        Ok(_program) => {
            // Basic linting checks
            // TODO: Add more linting rules
            if issues.is_empty() {
                println!("No linting issues found in {}", path.display());
            } else {
                for issue in issues {
                    eprintln!("{}", issue);
                }
                process::exit(1);
            }
        }
        Err(err) => {
            issues.push(format!(
                "Parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            ));
            for issue in issues {
                eprintln!("{}", issue);
            }
            process::exit(1);
        }
    }
}

fn lint_directory(_dir: &PathBuf) {
    // TODO: Recursively lint all .pln files
    println!("Directory linting not yet fully implemented");
}

fn check_file(path: &str) {
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

    let mut compiler = pleno_compiler::Compiler::new();
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

fn print_help() {
    println!(
        "Pleno Programming Language CLI\n\n\
        Usage: pleno <command> [arguments]\n\n\
        Commands:\n\
          run <file.pln>     Run a Pleno file\n\
          format <path>      Format Pleno files\n\
          lint <path>        Lint Pleno files\n\
          check <file.pln>   Type check a Pleno file\n\
          test <path>        Run tests in file or directory\n\n\
        Options:\n\
          -h, --help         Show this help message\n\
          -v, --version      Show version information\n\n\
        Examples:\n\
          pleno run main.pln\n\
          pleno format src/\n\
          pleno lint main.pln\n\
          pleno check main.pln\n\
          pleno test sources/tests"
    );
}

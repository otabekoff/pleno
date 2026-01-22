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
            return;
        }
        "-v" | "--version" => {
            println!("pleno {}", env!("CARGO_PKG_VERSION"));
            return;
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
        _ => {
            // Default: treat as file path and run it
            run_file(&command);
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
            eprintln!("Lex error at {}..{}: {}", err.span.start, err.span.end, err.message);
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!("Parse error at {}..{}: {}", err.span.start, err.span.end, err.message);
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
        Ok(value) => println!("{value:?}"),
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
            eprintln!("Lex error at {}..{}: {}", err.span.start, err.span.end, err.message);
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    match parser.parse_program() {
        Ok(_) => {
            // For now, just verify it parses correctly
            // TODO: Implement actual formatting (indentation, spacing, etc.)
            println!("File {} is valid (formatting not yet fully implemented)", path.display());
        }
        Err(err) => {
            eprintln!("Parse error at {}..{}: {}", err.span.start, err.span.end, err.message);
            process::exit(1);
        }
    }
}

fn format_directory(_dir: &PathBuf) {
    // TODO: Recursively format all .pln files
    println!("Directory formatting not yet fully implemented");
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
            issues.push(format!("Lex error at {}..{}: {}", err.span.start, err.span.end, err.message));
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
            issues.push(format!("Parse error at {}..{}: {}", err.span.start, err.span.end, err.message));
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
            eprintln!("Lex error at {}..{}: {}", err.span.start, err.span.end, err.message);
            process::exit(1);
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(err) => {
            eprintln!("Parse error at {}..{}: {}", err.span.start, err.span.end, err.message);
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
          check <file.pln>   Type check a Pleno file\n\n\
        Options:\n\
          -h, --help         Show this help message\n\
          -v, --version      Show version information\n\n\
        Examples:\n\
          pleno run main.pln\n\
          pleno format src/\n\
          pleno lint main.pln\n\
          pleno check main.pln"
    );
}

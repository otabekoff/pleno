use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("Usage: pleno <file.pln>");
        process::exit(1);
    };

    let source = match fs::read_to_string(&path) {
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

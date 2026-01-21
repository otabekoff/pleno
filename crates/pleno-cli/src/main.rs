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
    match parser.parse_program() {
        Ok(program) => {
            println!("Parsed {} statements.", program.statements.len());
        }
        Err(err) => {
            eprintln!("Parse error at {}..{}: {}", err.span.start, err.span.end, err.message);
            process::exit(1);
        }
    }
}

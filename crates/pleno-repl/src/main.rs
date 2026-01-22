use std::io::{self, Write};

fn main() {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    println!("Pleno REPL (type :quit to exit)");

    loop {
        input.clear();
        print!("pleno> ");
        if stdout.flush().is_err() {
            break;
        }
        if stdin.read_line(&mut input).is_err() {
            break;
        }

        let line = input.trim_end();
        if line == ":quit" || line == ":q" {
            break;
        }
        if line.is_empty() {
            continue;
        }

        match pleno_lexer::lex(line) {
            Ok(tokens) => {
                let mut parser = pleno_parser::Parser::new(tokens);
                match parser.parse_program() {
                    Ok(program) => {
                        println!("ok: {} statement(s)", program.statements.len());
                    }
                    Err(err) => {
                        eprintln!("parse error at {}..{}: {}", err.span.start, err.span.end, err.message);
                    }
                }
            }
            Err(err) => {
                eprintln!("lex error at {}..{}: {}", err.span.start, err.span.end, err.message);
            }
        }
    }
}

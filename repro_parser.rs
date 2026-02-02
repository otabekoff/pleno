use pleno_lexer::lex;
use pleno_parser::Parser;
use std::fs;

fn main() {
    let source = fs::read_to_string("async_server_test.pln").expect("Failed to read file");
    match lex(&source) {
        Ok(tokens) => {
            let mut parser = Parser::new(tokens, &source);
            match parser.parse_program() {
                Ok(_) => println!("Successfully parsed!"),
                Err(err) => {
                    eprintln!("Parse error: {}", err.message);
                    eprintln!("Span: {}..{}", err.span.start, err.span.end);
                }
            }
        }
        Err(err) => {
            eprintln!("Lex error: {}", err.message);
            eprintln!("Span: {}..{}", err.span.start, err.span.end);
        }
    }
}

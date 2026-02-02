#![no_main]
use libfuzzer_sys::fuzz_target;
use pleno_lexer::lex;
use pleno_parser::Parser;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data)
        && let Ok(tokens) = lex(s)
    {
        let mut parser = Parser::new(tokens, s);
        let _ = parser.parse_program();
    }
});

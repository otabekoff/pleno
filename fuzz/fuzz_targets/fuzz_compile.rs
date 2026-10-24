#![no_main]
use libfuzzer_sys::fuzz_target;
use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;

// Any program the parser accepts must compile or fail with an error, not panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data)
        && let Ok(tokens) = lex(source)
        && let Ok(mut program) = Parser::new(tokens, source).parse_program()
    {
        let _ = Compiler::new(source.to_string()).compile(&mut program);
    }
});

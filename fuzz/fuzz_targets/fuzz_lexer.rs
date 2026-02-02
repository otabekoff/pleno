#![no_main]
use libfuzzer_sys::fuzz_target;
use pleno_lexer::lex;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = lex(s);
    }
});

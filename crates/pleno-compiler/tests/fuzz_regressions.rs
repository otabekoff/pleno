//! Inputs found by `cargo fuzz run fuzz_compile`: the compiler must report
//! an error or succeed, never panic.

use pleno_compiler::Compiler;

fn compile(source: &str) {
    let Ok(tokens) = pleno_lexer::lex(source) else {
        return;
    };
    let Ok(mut program) = pleno_parser::Parser::new(tokens, source).parse_program() else {
        return;
    };
    let _ = Compiler::new(source.to_string()).compile(&mut program);
}

#[test]
fn unbalanced_generic_in_a_union_type() {
    // crash-16db6b7f0c40f7127c56484018438ed4436ba860
    compile("cls<xs<xkKKs<xkKKKKea<hhhhKKKKea<h,hhhhnfor>>|KhKKea<h,hhhhnfKKKInfor>>");
}

#[test]
fn malformed_generic_annotations_are_errors() {
    for source in [
        "let a: X<Y | Z<W> = 1;",
        "let a: X<> = 1;",
        "fn f(a: A<B|C<D>) {}",
    ] {
        compile(source);
    }
}

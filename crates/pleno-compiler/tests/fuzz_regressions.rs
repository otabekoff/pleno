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

#[test]
fn more_than_256_constants_in_one_function() {
    // crash-bdb3e6d158d16387f18357b9190bfeac41e696e8 (fuzz_compile, 12 minutes into a long run)
    compile(
        r#"2+{}+O+OO0O+3+{is0OO}+O+OO0O+O4+O+OOis0OO4+O+O+{}+O+OO1O+2+{is0O+{}+O+OO1O+2+{is0OO}+Os0OO4+O+O+{}+O+OO1i*o??foO}+Os0OO4+O+O+{}+O+OO1i*o??fo??false..lafo??fo??o??tr??rO+2+{is0OO}+sO+O4+O+OOis0Ois0O"#,
    );
}

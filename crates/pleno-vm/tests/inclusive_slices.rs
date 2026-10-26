//! `[..=end]` slices include the end, like `[start..=end]`.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .unwrap_or_else(|e| panic!("{source}: {}", e.diagnostic.message));
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .unwrap_or_else(|e| panic!("{source}: {}", e.message))
        .to_string()
}

#[test]
fn array_slice_without_start_includes_the_end() {
    assert_eq!(eval("[1, 2, 3][..=1];"), "[1, 2]");
    assert_eq!(eval("[1, 2, 3][..=0];"), "[1]");
    assert_eq!(eval("let n = 1; [1, 2, 3][..=n];"), "[1, 2]");
    // An end past the last element stops there.
    assert_eq!(eval("[1, 2, 3][..=5];"), "[1, 2, 3]");
    // The exclusive forms are unchanged.
    assert_eq!(eval("[1, 2, 3][..1];"), "[1]");
    assert_eq!(eval("[1, 2, 3][:1];"), "[1]");
    assert_eq!(eval("[1, 2, 3][0..=1];"), "[1, 2]");
}

#[test]
fn negative_inclusive_ends_count_from_the_back() {
    assert_eq!(eval("[1, 2, 3][..=-1];"), "[1, 2, 3]");
    assert_eq!(eval("[1, 2, 3][..=-2];"), "[1, 2]");
    assert_eq!(eval("[1, 2, 3][..-1];"), "[1, 2]");
}

#[test]
fn strings_and_tuples_slice_the_same_way() {
    assert_eq!(eval(r#""abc"[..=1];"#), "ab");
    assert_eq!(eval(r#""héllo"[..=1];"#), "hé");
    assert_eq!(eval(r#""abc"[..=-1];"#), "abc");
    assert_eq!(eval("(1, 2, 3)[..=1];"), "(1, 2)");
}

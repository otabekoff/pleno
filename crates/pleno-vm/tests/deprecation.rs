//! `@deprecated` warns on the first call to a function or method, once per
//! name, and the function keeps working; `pleno check` reports the calls.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run(source: &str) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap();
    vm.output.take().unwrap_or_default()
}

#[test]
fn deprecated_functions_warn_once_and_keep_working() {
    let output = run("@deprecated(\"use add instead\")\n\
         fn plus(a, b) { return a + b; }\n\
         @deprecated\n\
         fn old() { return 1; }\n\
         class Shape { @deprecated(\"use area()\") fn size() { return 4; } }\n\
         print(plus(1, 2), plus(3, 4), old());\n\
         print([1, 2].map(fn(x) => plus(x, 1)));\n\
         print(Shape().size());");
    assert_eq!(
        output,
        "warning: plus is deprecated: use add instead\n\
         warning: old is deprecated\n\
         3 7 1\n\
         [2, 3]\n\
         warning: size is deprecated: use area()\n\
         4\n"
    );
}

#[test]
fn check_reports_calls_to_deprecated_functions() {
    let source =
        "@deprecated(\"use add instead\")\nfn plus(a, b) { return a + b; }\nplus(1, 2);\nplus(1);";
    let tokens = pleno_lexer::lex(source).unwrap();
    let program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let messages: Vec<String> = pleno_compiler::check::check_program(&program)
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(
        messages
            .iter()
            .filter(|m| *m == "'plus' is deprecated: use add instead")
            .count(),
        2,
        "{messages:?}"
    );
    // The argument count is still checked.
    assert!(
        messages.iter().any(|m| m.contains("argument")),
        "{messages:?}"
    );
}

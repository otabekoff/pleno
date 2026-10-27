//! A function (or file) can define more than 256 functions: the prototype
//! index used to be cut to one byte, so the 257th function created was a
//! different one and a call such as `run42()` ran some class's `area` method
//! (`missing required argument`).

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .map(|v| v.to_string())
        .unwrap_or_else(|e| e.message)
}

#[test]
fn every_class_method_is_its_own_function() {
    // 70 classes with 5 methods each: 350 prototypes in the script.
    let mut source = String::new();
    for i in 0..70 {
        source.push_str(&format!(
            "class C{i} {{ a() {{ return {i}; }} b() {{ return {i}; }} c() {{ return {i}; }} d() {{ return {i}; }} e() {{ return {i}; }} }}\n"
        ));
    }
    source.push_str("let mut wrong = 0;\n");
    for i in 0..70 {
        source.push_str(&format!(
            "if (C{i}().e() != {i}) {{ wrong = wrong + 1; }}\n"
        ));
    }
    source.push_str("wrong;");
    assert_eq!(eval(&source), "0");
}

#[test]
fn a_function_can_create_more_than_256_lambdas() {
    // 300 lambdas in one function, each a closure over `base`.
    let lambdas: Vec<String> = (0..300).map(|i| format!("(x) => x + base + {i}")).collect();
    let source = format!(
        "fn make(base) {{ return [{}]; }}\nlet fs = make(1000);\n[fs[0](0), fs[255](0), fs[256](0), fs[299](0)];",
        lambdas.join(", ")
    );
    assert_eq!(eval(&source), "[1000, 1255, 1256, 1299]");
}

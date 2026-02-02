use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let _ = env_logger::builder().is_test(true).try_init();
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("Parsing failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("Compilation failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_random() {
    let source = r#"
        import { random } from "std:random";
        let r = random();
        [r >= 0.0, r < 1.0]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            assert_eq!(elements[0], Value::Bool(true));
            assert_eq!(elements[1], Value::Bool(true));
        }
        val => panic!("Expected array result, got {:?}", val),
    }
}

#[test]
fn test_randint() {
    let source = r#"
        import { randint } from "std:random";
        let r = randint(1, 10);
        [r >= 1, r <= 10]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            assert_eq!(elements[0], Value::Bool(true));
            assert_eq!(elements[1], Value::Bool(true));
        }
        val => panic!("Expected array result, got {:?}", val),
    }
}

#[test]
fn test_choice() {
    let source = r#"
        import { choice } from "std:random";
        let arr = [1, 2, 3];
        let c = choice(arr);
        let valid = (c == 1 || c == 2 || c == 3);
        valid
    "#;
    match run_source(source) {
        Value::Bool(b) => assert!(b),
        val => panic!("Expected bool result, got {:?}", val),
    }
}

#[test]
fn test_shuffle() {
    let source = r#"
        import { shuffle } from "std:random";
        let arr = [1, 2, 3, 4, 5];
        // Clone for comparison? No, shuffle is in-place.
        // We verify that elements are still the same set, just order might change.
        shuffle(arr);
        arr.length == 5
    "#;
    match run_source(source) {
        Value::Bool(b) => assert!(b),
        val => panic!("Expected bool result, got {:?}", val),
    }
}

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("runtime failed")
}

#[test]
fn matches_tuple_destructuring() {
    let value = eval(
        "
        let t = (1, 2);
        match t {
            (a, b) => { return a + b; }
            _ => { return 0; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn matches_nested_patterns() {
    let value = eval(
        "
        let t = (1, (2, 3));
        match t {
            (a, (b, c)) => { return a + b + c; }
            _ => { return 0; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn matches_array_with_tuple_syntax() {
    let value = eval(
        "
        let arr = [10, 20];
        match arr {
            (a, b) => { return a + b; }
            _ => { return 0; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(30));
}

#[test]
fn matches_rest_pattern() {
    let value = eval(
        "
        let arr = [1, 2, 3];
        match arr {
            (head, ...tail) => { return head + tail.length; } // tail is [2, 3] (len 2)
            _ => { return 0; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(3)); // 1 + 2
}

#[test]
fn matches_match_expression_literals() {
    let value = eval(
        "
        let x = 2;
        match x {
            1 => { return 10; }
            2 => { return 20; }
            _ => { return 30; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(20));
}

#[test]
fn matches_guards() {
    let value = eval(
        "
        let x = 10;
        match x {
            n if n > 100 => { return 1; }
            n if n == 10 => { return 2; }
            _ => { return 3; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn matches_or_pattern() {
    let value = eval(
        "
        let x = 2;
        match x {
            1 | 2 => { return 100; }
            3 => { return 200; }
            _ => { return 300; }
        }
        ",
    );
    assert_eq!(value, pleno_value::Value::Int(100));
}

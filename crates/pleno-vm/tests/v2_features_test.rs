use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("Compilation failed");
    let mut vm = VM::new();
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_null_coalesce() {
    let source = "let a = null; let b = 10; a ?? b";
    assert_eq!(run_source(source), Value::Int(10));

    let source = "let a = 5; let b = 10; a ?? b";
    assert_eq!(run_source(source), Value::Int(5));
}

#[test]
fn test_safe_property() {
    let source = r#"
class User { name: String; constructor(n: String) { this.name = n; } }
let u = User("Alice");
let n1 = u?.name;
let u2 = null;
let n2 = u2?.name;
(n1, n2)
"#;
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0].as_str().unwrap(), "Alice");
        assert_eq!(e[1], Value::Null);
    } else {
        panic!("Expected tuple");
    }
}

#[test]
fn test_if_expr() {
    let source = "let x = 10; let y = if (x > 5) 100 else 200; y";
    assert_eq!(run_source(source), Value::Int(100));
}

#[test]
fn test_if_no_else() {
    let source = "let x = 1; let y = if (x > 0) 10; let z = if (x < 0) 20; (y, z)";
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0], Value::Int(10));
        assert_eq!(e[1], Value::Null);
    } else {
        panic!("Expected tuple");
    }
}

#[test]
fn test_match_expr() {
    let source = r#"
let x = 2;
let y = match (x) {
    1 => "one",
    2 => {
        let res = "two";
        res
    },
    _ => "many"
};
y
"#;
    assert_eq!(run_source(source).as_str().unwrap(), "two");
}

#[test]
fn test_safe_method_call() {
    let source = r#"
class User {
    fn greet() { return "hi"; }
}
let u = User();
let r1 = u?.greet();
let u2 = null;
let r2 = u2?.greet();
(r1, r2)
"#;
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0].as_str().unwrap(), "hi");
        assert_eq!(e[1], Value::Null);
    } else {
        panic!("Expected tuple");
    }
}

#[test]
fn test_safe_index() {
    let source = "let a = [1, 2, 3]; let v1 = a?.[1]; let b = null; let v2 = b?.[1]; (v1, v2)";
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0], Value::Int(2));
        assert_eq!(e[1], Value::Null);
    } else {
        panic!("Expected tuple");
    }
}

#[test]
fn test_complex_optional_chain() {
    let source = r#"
class Address { street: String; constructor(s: String) { this.street = s; } }
class Profile { address: Address; constructor(a: Address) { this.address = a; } }
class User { profile: Profile; constructor(p: Profile) { this.profile = p; } }

let u1 = User(Profile(Address("Main St")));
let s1 = u1?.profile.address.street;
let u2 = null;
let s2 = u2?.profile.address.street;
(s1, s2)
"#;
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0].as_str().unwrap(), "Main St");
        assert_eq!(e[1], Value::Null);
    } else {
        panic!("Expected tuple");
    }
}

#[test]
fn test_non_exhaustive_match() {
    let source = "let x = 10; match (x) { 1 => \"one\", 2 => \"two\" }";
    assert_eq!(run_source(source), Value::Null);
}

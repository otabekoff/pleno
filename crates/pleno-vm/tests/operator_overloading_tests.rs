use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

#[test]
fn test_operator_overloading_basic() {
    let source = r#"
class Vector {
    x: Float; y: Float;

    constructor(x: Float, y: Float) {
        this.x = x;
        this.y = y;
    }

    operator +(other: Vector): Vector {
        return Vector(this.x + other.x, this.y + other.y);
    }

    operator -(): Vector {
        return Vector(-this.x, -this.y);
    }

    operator ==(other: Vector): Bool {
        return this.x == other.x && this.y == other.y;
    }
}

let v1 = Vector(10.0, 20.0);
let v2 = Vector(5.0, 5.0);
let v3 = v1 + v2;
let v4 = -v1;
let eq = (v1 == Vector(10.0, 20.0));
let neq = (v1 == v2);

(v3.x, v3.y, v4.x, v4.y, eq, neq)
"#;

    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).unwrap();
    let mut vm = VM::new();
    let result = vm.execute(chunk).unwrap();

    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0], Value::Float(15.0));
        assert_eq!(e[1], Value::Float(25.0));
        assert_eq!(e[2], Value::Float(-10.0));
        assert_eq!(e[3], Value::Float(-20.0));
        assert_eq!(e[4], Value::Bool(true));
        assert_eq!(e[5], Value::Bool(false));
    } else {
        panic!("Expected tuple result, got {:?}", result);
    }
}

#[test]
fn test_operator_overloading_indexing() {
    let source = r#"
class Container {
    data: Map;
    constructor() {
        this.data = {};
    }
    operator [](key: String): Any {
        return this.data[key];
    }
    operator []=(key: String, value: Any) {
        this.data[key] = value;
    }
}

let c = Container();
c["hello"] = "world";
let val = c["hello"];
val
"#;

    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).unwrap();
    let mut vm = VM::new();
    let result = vm.execute(chunk).unwrap();

    assert_eq!(result.as_str().unwrap(), "world");
}

#[test]
fn test_operator_overloading_comparisons() {
    let source = r#"
class Box {
    val: Int;
    constructor(v: Int) { this.val = v; }
    operator <(other: Box): Bool { return this.val < other.val; }
    operator <=(other: Box): Bool { return this.val <= other.val; }
}

let b1 = Box(10);
let b2 = Box(20);
(b1 < b2, b2 < b1, b1 <= b1, b2 <= b1)
"#;

    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).unwrap();
    let mut vm = VM::new();
    let result = vm.execute(chunk).unwrap();

    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        assert_eq!(e[0], Value::Bool(true));
        assert_eq!(e[1], Value::Bool(false));
        assert_eq!(e[2], Value::Bool(true));
        assert_eq!(e[3], Value::Bool(false));
    } else {
        panic!("Expected tuple result, got {:?}", result);
    }
}

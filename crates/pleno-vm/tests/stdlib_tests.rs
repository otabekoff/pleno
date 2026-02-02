use pleno_compiler::Compiler;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("runtime failed")
}

#[test]
fn test_math_refinements() {
    // Test constants
    let pi = run_source("import Math from \"std:math\"; Math.PI");
    if let Value::Float(f) = pi {
        assert!((f - std::f64::consts::PI).abs() < 1e-10);
    } else {
        panic!("Expected float for PI, got {:?}", pi);
    }

    let sqrt2 = run_source("import Math from \"std:math\"; Math.SQRT2");
    if let Value::Float(f) = sqrt2 {
        assert!((f - std::f64::consts::SQRT_2).abs() < 1e-10);
    } else {
        panic!("Expected float for SQRT2, got {:?}", sqrt2);
    }

    // Test atan2
    let atan2 = run_source("import Math from \"std:math\"; Math.atan2(1, 1)");
    if let Value::Float(f) = atan2 {
        assert!((f - 1.0f64.atan2(1.0)).abs() < 1e-10);
    } else {
        panic!("Expected float for atan2, got {:?}", atan2);
    }

    // Test trunc
    let trunc = run_source("import Math from \"std:math\"; Math.trunc(3.7)");
    assert_eq!(trunc, Value::Int(3));

    // Test sign
    let sign_pos = run_source("import Math from \"std:math\"; Math.sign(5)");
    assert_eq!(sign_pos, Value::Int(1));
    let sign_neg = run_source("import Math from \"std:math\"; Math.sign(-5.5)");
    assert_eq!(sign_neg, Value::Int(-1));
    let sign_zero = run_source("import Math from \"std:math\"; Math.sign(0)");
    assert_eq!(sign_zero, Value::Int(0));
}

#[test]
fn test_json_robust_stringify() {
    // Test nested structure
    let code = r#"
        import _json from "std:json";
        _json.JSON.stringify({
            "name": "Pleno",
            "version": 1,
            "features": ["async", "concurrency"],
            "meta": { "stable": false }
        })
    "#;
    let json_str = run_source(code);
    if let Value::String(s) = json_str {
        println!("JSON String: {}", s);
        assert!(s.contains("\"name\":\"Pleno\""));
        assert!(s.contains("\"version\":1"));
        assert!(s.contains("\"features\":[\"async\",\"concurrency\"]"));
        assert!(s.contains("\"meta\":{\"stable\":false}"));
    } else {
        panic!("Expected string for JSON.stringify, got {:?}", json_str);
    }
}

#[test]
fn test_os_module() {
    let platform = run_source("import _os from \"std:os\"; _os.platform()");
    assert!(matches!(platform, Value::String(_)));

    let arch = run_source("import _os from \"std:os\"; _os.arch()");
    assert!(matches!(arch, Value::String(_)));

    let tmpdir = run_source("import _os from \"std:os\"; _os.tmpdir()");
    assert!(matches!(tmpdir, Value::String(_)));
}

#[test]
fn test_array_functional_methods() {
    // Map
    let map_res = run_source(
        "
        let arr = [1, 2, 3];
        arr.map(|x| => x * 2)
    ",
    );
    if let Value::Array(a) = map_res {
        let b = a.borrow();
        assert_eq!(b.elements.len(), 3);
        assert_eq!(b.elements[0], Value::Int(2));
        assert_eq!(b.elements[1], Value::Int(4));
        assert_eq!(b.elements[2], Value::Int(6));
    } else {
        panic!("Expected array from map, got {:?}", map_res);
    }

    // Filter
    let filter_res = run_source(
        "
        let arr = [1, 2, 3, 4, 5];
        arr.filter(|x| => x % 2 == 0)
    ",
    );
    if let Value::Array(a) = filter_res {
        let b = a.borrow();
        assert_eq!(b.elements.len(), 2);
        assert_eq!(b.elements[0], Value::Int(2));
        assert_eq!(b.elements[1], Value::Int(4));
    } else {
        panic!("Expected array from filter, got {:?}", filter_res);
    }

    // Reduce with initial
    let reduce_init = run_source(
        "
        let arr = [1, 2, 3, 4];
        arr.reduce(|acc, x| => acc + x, 10)
    ",
    );
    assert_eq!(reduce_init, Value::Int(20));

    // Reduce without initial
    let reduce_no_init = run_source(
        "
        let arr = [1, 2, 3, 4];
        arr.reduce(|acc, x| => acc + x)
    ",
    );
    assert_eq!(reduce_no_init, Value::Int(10));

    // Chain
    let chain_res = run_source(
        "
        [1, 2, 3, 4, 5]
            .filter(|x| => x % 2 != 0)
            .map(|x| => x * x)
            .reduce(|acc, x| => acc + x, 0)
    ",
    );
    // 1, 3, 5 -> 1, 9, 25 -> sum = 35
    assert_eq!(chain_res, Value::Int(35));
}

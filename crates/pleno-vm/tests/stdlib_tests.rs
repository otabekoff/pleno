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
        import JSON from "std:json";
        JSON.stringify({
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
    let platform = run_source("import OS from \"std:os\"; OS.platform()");
    assert!(matches!(platform, Value::String(_)));

    let arch = run_source("import OS from \"std:os\"; OS.arch()");
    assert!(matches!(arch, Value::String(_)));

    let tmpdir = run_source("import OS from \"std:os\"; OS.tmpdir()");
    assert!(matches!(tmpdir, Value::String(_)));
}

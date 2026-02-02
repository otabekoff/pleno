use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let tokens = lex(source).unwrap();
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().unwrap();
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("Compilation failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_base64() {
    let source = r#"
        import { encode, decode } from "std:base64";
        let original = "Hello, World!";
        let encoded = encode(original);
        let decoded = decode(encoded);
        decoded
    "#;
    let result = run_source(source);
    match result {
        Value::String(s) => assert_eq!(s.as_str(), "Hello, World!"),
        _ => panic!("Expected string, got {:?}", result),
    }
}

#[test]
fn test_url_parse() {
    let source = r#"
        import { URL } from "std:url";
        let url = URL.parse("https://user:pass@example.com:8080/path?query=param");
        (url.protocol, url.host, url.port, url.path, url.query)
    "#;
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        // protocol: "https"
        assert_eq!(e[0].as_str().unwrap(), "https");
        // host: "example.com"
        assert_eq!(e[1].as_str().unwrap(), "example.com");
        // port: 8080
        assert_eq!(e[2].as_int().unwrap(), 8080);
        // path: "/path"
        assert_eq!(e[3].as_str().unwrap(), "/path");
        // query: "query=param"
        assert_eq!(e[4].as_str().unwrap(), "query=param");
    } else {
        panic!("Expected tuple, got {:?}", result);
    }
}

#[test]
fn test_csv_parse() {
    let source = r#"
        import { parse } from "std:csv";
        let data = "name,age\nAlice,30\nBob,25";
        let rows = parse(data);
        (rows[0][0], rows[0][1], rows[1][0])
    "#;
    let result = run_source(source);
    if let Value::Tuple(t) = result {
        let e = &t.borrow().elements;
        // "Alice"
        assert_eq!(e[0].as_str().unwrap(), "Alice");
        // "30"
        assert_eq!(e[1].as_str().unwrap(), "30");
        // "Bob"
        assert_eq!(e[2].as_str().unwrap(), "Bob");
    } else {
        panic!("Expected tuple, got {:?}", result);
    }
}

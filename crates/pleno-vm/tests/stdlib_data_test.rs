use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let _ = env_logger::builder().is_test(true).try_init();
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("Parsing failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("Compilation failed");
    println!("DEBUG_CHUNK: {:?}", chunk);
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_uuid() {
    let source = r#"
        import { v4, v7, parse } from "std:uuid";
        print("v4 is:", v4);
        let id4 = v4(0);
        let id7 = v7(0);
        let parsed = parse(id4);
        [id4 == parsed, id4.length > 0, id7.length > 0]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            assert_eq!(elements[0], Value::Bool(true)); // id4 == parsed
            assert_eq!(elements[1], Value::Bool(true)); // id4.length > 0
            assert_eq!(elements[2], Value::Bool(true)); // id7.length > 0
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_yaml() {
    let source = r#"
        import { parse, stringify } from "std:yaml";
        let yaml_str = "
name: Pleno
version: 2
features:
  - fast
  - dynamic
";
        let parsed = parse(yaml_str);
        print("YAML parsed:", parsed); 
        let name = parsed["name"];
        let feat = parsed["features"][0];
        
        // Test stringify roundtrip (roughly)
        let back = stringify(parsed);
        print("YAML back:", back);
        let parsed_again = parse(back);
        print("Parsed again:", parsed_again);
        
        [name, feat, parsed_again["version"]]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            match &elements[0] {
                Value::String(s) => assert_eq!(s.as_str(), "Pleno"),
                _ => panic!("Expected string Pleno"),
            }
            match &elements[1] {
                Value::String(s) => assert_eq!(s.as_str(), "fast"),
                _ => panic!("Expected string fast"),
            }
            match &elements[2] {
                Value::Int(i) => assert_eq!(*i, 2),
                val => panic!("Expected int 2, got {:?}", val),
            }
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_toml() {
    let source = r#"
        import { parse, stringify } from "std:toml";
        let toml_str = "
[package]
name = 'pleno'
version = 1
";
        let parsed = parse(toml_str);
        print("TOML parsed:", parsed);
        print("Type of parsed:", typeof(parsed));
        // Debug keys
        let keys = parsed.keys();
        print("Keys:", keys);
        for k in keys {
            print("Key:", k, "Len:", k.len);
        }
        let name = parsed.package.name;
        
        let back = stringify(parsed);
        let parsed_again = parse(back);
        
        [name, parsed_again.package.version]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            match &elements[0] {
                Value::String(s) => assert_eq!(s.as_str(), "pleno"),
                _ => panic!("Expected string pleno"),
            }
            match &elements[1] {
                Value::Int(i) => assert_eq!(*i, 1),
                _ => panic!("Expected int 1"),
            }
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_xml() {
    let source = r#"
        import { parse } from "std:xml";
        let xml_str = "<root attr='val'><child>Hello</child></root>";
        let parsed = parse(xml_str);
        // Structure: Map { name: "root", attributes: Map, children: Array }
        let name = parsed.name;
        let attr = parsed.attributes["attr"];
        let child_text = parsed.children[0].children[0]; // child -> text node
        
        [name, attr, child_text]
    "#;

    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            match &elements[0] {
                Value::String(s) => assert_eq!(s.as_str(), "root"),
                _ => panic!("Expected string root"),
            }
            match &elements[1] {
                Value::String(s) => assert_eq!(s.as_str(), "val"),
                _ => panic!("Expected string val"),
            }
            match &elements[2] {
                Value::String(s) => assert_eq!(s.as_str(), "Hello"),
                _ => panic!("Expected string Hello"),
            }
        }
        _ => panic!("Expected array"),
    }
}

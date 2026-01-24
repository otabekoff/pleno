use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    vm.execute(chunk).expect("runtime failed")
}

#[test]
fn test_path_basic_properties() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.separator;
    "#,
    );

    match result {
        pleno_value::Value::String(s) => {
            assert_eq!(s.as_str(), std::path::MAIN_SEPARATOR.to_string());
        }
        _ => panic!("Expected string separator"),
    }
}

#[test]
fn test_path_join() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.join("p1", "p2");
    "#,
    );

    // We can't rely on exact string since it's OS dependent, but it should contain both parts
    let s = result.as_str().unwrap();
    assert!(s.contains("p1"));
    assert!(s.contains("p2"));
}

#[test]
fn test_path_dirname() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.dirname("foo/bar.txt");
    "#,
    );

    match result {
        pleno_value::Value::String(s) => {
            // "foo" on Unix, "foo" or "foo\" on Windows depending on implementation
            assert!(s.as_str().contains("foo"));
        }
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_path_basename() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.basename("foo/bar.txt");
    "#,
    );

    assert_eq!(result.as_str(), Some("bar.txt"));
}

#[test]
fn test_path_extname() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.extname("foo/bar.txt");
    "#,
    );

    assert_eq!(result.as_str(), Some(".txt"));
}

#[test]
fn test_path_is_file() {
    // Current file should be a file
    let result = eval(
        r#"
        import Path from "std:path";
        Path.isFile("Cargo.toml");
    "#,
    );

    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn test_path_exists() {
    let result = eval(
        r#"
        import Path from "std:path";
        Path.exists("Cargo.toml");
    "#,
    );

    assert_eq!(result.as_bool(), Some(true));
}

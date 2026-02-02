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
fn test_db_sqlite_basic() {
    let source = r#"
        import { Database } from "std:db";
        
        let db = Database.connect("sqlite::memory:");
        
        db.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)");
        db.execute("INSERT INTO users (name, age) VALUES (?, ?)", ["Alice", 30]);
        db.execute("INSERT INTO users (name, age) VALUES (?, ?)", ["Bob", 25]);
        
        let rows = db.query("SELECT * FROM users ORDER BY age DESC");
        
        if (rows.length != 2) return false;
        if (rows[0]["name"] != "Alice") return false;
        if (rows[1]["name"] != "Bob") return false;
        if (rows[0]["age"] != 30) return false;
        
        db.close();
        true
    "#;
    match run_source(source) {
        Value::Bool(b) => assert!(b),
        val => panic!("Expected bool(true), got {:?}", val),
    }
}

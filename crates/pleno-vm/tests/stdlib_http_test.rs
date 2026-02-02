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
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_http_router_registration() {
    let source = r#"
        import { Router } from "std:http";
        let r = Router();
        r.get("/", fn(req, res) { return "Home"; });
        r.post("/api", fn(req, res) { return "API"; });
        true
    "#;
    match run_source(source) {
        Value::Bool(b) => assert!(b),
        val => panic!("Expected bool, got {:?}", val),
    }
}
// Note: We don't test Server.listen() here as it blocks.

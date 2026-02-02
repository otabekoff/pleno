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
fn test_crypto_hash() {
    let source = r#"
        import { hash } from "std:crypto";
        let h = hash("sha256", "hello");
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        h
    "#;
    match run_source(source) {
        Value::String(s) => assert_eq!(
            s.as_str(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        ),
        val => panic!("Expected string hash, got {:?}", val),
    }
}

#[test]
fn test_crypto_hmac() {
    let source = r#"
        import { hmac } from "std:crypto";
        // key="key", data="The quick brown fox jumps over the lazy dog"
        let h = hmac("sha256", "key", "The quick brown fox jumps over the lazy dog");
        h
    "#;
    match run_source(source) {
        Value::String(s) => assert_eq!(
            s.as_str(),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        ),
        val => panic!("Expected string hmac, got {:?}", val),
    }
}

#[test]
fn test_crypto_aes() {
    let source = r#"
        import { encrypt, decrypt } from "std:crypto";
        // key 32 bytes hex, iv 16 bytes hex
        let key = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let iv = "000102030405060708090a0b0c0d0e0f";
        let data = "Secret Message";
        
        let enc = encrypt("aes-256-cbc", key, iv, data);
        let dec = decrypt("aes-256-cbc", key, iv, enc);
        
        [enc != data, dec == data]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            assert_eq!(elements[0], Value::Bool(true));
            assert_eq!(elements[1], Value::Bool(true));
        }
        val => panic!("Expected array result, got {:?}", val),
    }
}

#[test]
fn test_crypto_jwt() {
    let source = r#"
        import { jwtSign, jwtVerify } from "std:crypto";
        let payload = { sub: "123", name: "John Doe" };
        let secret = "supersecret";
        
        let token = jwtSign(payload, secret);
        let verified = jwtVerify(token, secret);
        
        [token.length > 0, verified.sub == "123", verified.name == "John Doe"]
    "#;
    match run_source(source) {
        Value::Array(arr) => {
            let elements = &arr.borrow().elements;
            assert_eq!(elements[0], Value::Bool(true));
            assert_eq!(elements[1], Value::Bool(true));
            assert_eq!(elements[2], Value::Bool(true));
        }
        val => panic!("Expected array result, got {:?}", val),
    }
}

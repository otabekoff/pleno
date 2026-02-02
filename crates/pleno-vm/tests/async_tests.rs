use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    // We assume parser/compiler works as in pattern_matching_tests
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("compile failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("runtime failed")
}

#[test]
fn test_async_awaits() {
    let value = eval(
        r#"
        import Time from "std:time";
        
        // Helper to delay value
        fn resolve_after(ms, val) {
            return new Promise(|resolve, reject| => {
                Time.setTimeout(| | => {
                    resolve(val);
                }, ms); 
            });
        }

        async fn main() {
            let val = await resolve_after(10, 42);
            return val + 1;
        }
        
        let p = main();
        p // Return the promise from the script
        "#,
    );

    // Check if value is a Promise
    if let Value::Map(map) = value {
        let borrowed = map.borrow();
        let variant = borrowed.get("variant").expect("Promise missing variant");
        let val = borrowed.get("value").expect("Promise missing value");

        if let Value::String(s) = variant {
            assert_eq!(s.as_str(), "Resolved", "Promise should be Resolved");
        } else {
            panic!("Promise variant not a string");
        }

        assert_eq!(*val, Value::Int(43), "Promise should resolve to 43");
    } else {
        panic!("Result was not a Promise/Map: {:?}", value);
    }
}

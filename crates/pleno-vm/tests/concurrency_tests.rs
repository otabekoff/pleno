use pleno_compiler::Compiler;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;
use std::rc::Rc;

fn get_promise_result(val: Value) -> Value {
    if let Value::Map(map) = val {
        let borrowed = map.borrow();
        let variant = borrowed.get("variant").expect("Promise missing variant");
        let result_val = borrowed.get("value").expect("Promise missing value");

        if let Value::String(s) = variant
            && s.as_str() != "Resolved"
        {
            panic!("Promise was not resolved: {:?}", s);
        }
        result_val.clone()
    } else {
        panic!("Result was not a Promise/Map: {:?}", val);
    }
}

#[test]
fn test_mutex_concurrency() {
    let source = r#"
        import { Mutex } from "std:concurrency";
        import { sleep } from "std:time";

        let counter = 0;
        let m = Mutex();

        async fn increment(by, delay) {
            await m.lock();
            let current = counter;
            await sleep(delay);
            counter = current + by;
            m.unlock();
        }

        async fn main() {
            let p1 = spawn(increment, 1, 10);
            let p2 = spawn(increment, 2, 5);
            let p3 = spawn(increment, 3, 1);
            
            await Promise.all([p1, p2, p3]);
            return counter;
        }

        let p_final = main();
        p_final;
    "#;

    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    let result = vm.execute(chunk).expect("runtime failed");

    let resolved = get_promise_result(result);
    assert_eq!(resolved, Value::Int(6));
}

#[test]
fn test_channel_buffered() {
    let source = r#"
        import { Channel } from "std:concurrency";
        import { sleep } from "std:time";

        let ch = Channel(2);

        async fn producer() {
            await ch.send("A");
            await ch.send("B");
            await ch.send("C");
        }

        async fn main() {
            let p = spawn(producer);
            
            await sleep(10);
            let v1 = await ch.recv();
            let v2 = await ch.recv();
            let v3 = await ch.recv();
            
            await p;
            return [v1, v2, v3];
        }

        let p_final = main();
        p_final;
    "#;

    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    let result = vm.execute(chunk).expect("runtime failed");

    let resolved = get_promise_result(result);
    println!("Channel Resolved Result: {:?}", resolved);

    // Result should be ["A", "B", "C"]
    if let Value::Array(arr) = resolved {
        let arr = arr.borrow();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], Value::String(Rc::new("A".to_string())));
        assert_eq!(arr[1], Value::String(Rc::new("B".to_string())));
        assert_eq!(arr[2], Value::String(Rc::new("C".to_string())));
    } else {
        panic!("resolved result is not an array: {:?}", resolved);
    }
}

#[test]
fn test_channel_close() {
    let source = r#"
        import { Channel } from "std:concurrency";

        let ch = Channel(1);

        async fn main() {
            await ch.send(1);
            ch.close();
            
            try {
                await ch.send(2);
                return "fail: sent on closed channel";
            } catch (e) {
                // Expected
            }
            
            let v1 = await ch.recv();
            let v2 = await ch.recv();
            
            return [v1, v2];
        }

        let p_final = main();
        p_final;
    "#;

    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    let result = vm.execute(chunk).expect("runtime failed");

    let resolved = get_promise_result(result);
    println!("Channel Close Result: {:?}", resolved);

    // Should be [1, Missing]
    if let Value::Array(arr) = resolved {
        let arr = arr.borrow();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::Int(1));
        assert_eq!(arr[1], Value::Missing);
    } else {
        panic!("resolved result is not an array: {:?}", resolved);
    }
}

#[test]
fn test_channel_iteration() {
    let source = r#"
        import { Channel } from "std:concurrency";

        let ch = Channel(5);

        async fn producer() {
            await ch.send(1);
            await ch.send(2);
            await ch.send(3);
            ch.close();
        }

        async fn main() {
            spawn(producer);
            let results = [];
            for (val in ch) {
                results.push(val);
            }
            return results;
        }

        let p_final = main();
        p_final;
    "#;

    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    let result = vm.execute(chunk).expect("runtime failed");

    let resolved = get_promise_result(result);
    println!("Channel Iteration Result: {:?}", resolved);

    // Should be [1, 2, 3]
    if let Value::Array(arr) = resolved {
        let arr = arr.borrow();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], Value::Int(1));
        assert_eq!(arr[1], Value::Int(2));
        assert_eq!(arr[2], Value::Int(3));
    } else {
        panic!("resolved result is not an array: {:?}", resolved);
    }
}

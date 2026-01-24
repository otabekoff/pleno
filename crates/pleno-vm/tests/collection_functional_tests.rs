use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("runtime failed")
}

// Map functional method tests

#[test]
fn test_map_map_method() {
    let result = eval(
        r#"
        let ages = { "Alice": 30, "Bob": 25 };
        let doubled = ages.map(|k, v| => v * 2);
        doubled.get("Alice");
    "#,
    );
    assert_eq!(result.as_int(), Some(60));
}

#[test]
fn test_map_filter_method() {
    let result = eval(
        r#"
        let ages = { "Alice": 30, "Bob": 25, "Charlie": 35 };
        let filtered = ages.filter(|k, v| => v > 26);
        filtered.size();
    "#,
    );
    assert_eq!(result.as_int(), Some(2)); // Alice and Charlie
}

#[test]
fn test_map_foreach_method() {
    // forEach returns null and executes callback for each entry
    let result = eval(
        r#"
        let ages = { "Alice": 30, "Bob": 25 };
        ages.forEach(|k, v| => {
            println(k);
        });
    "#,
    );
    assert!(result.is_null());
}

// Set functional method tests

#[test]
fn test_set_to_array_method() {
    let result = eval(
        r#"
        let nums = {1, 2, 3};
        let arr = nums.toArray();
        arr.length;
    "#,
    );
    assert_eq!(result.as_int(), Some(3));
}

#[test]
fn test_set_map_method() {
    let result = eval(
        r#"
        let nums = {1, 2, 3};
        let doubled = nums.map(|x| => x * 2);
        doubled.length;
    "#,
    );
    // Returns array, not set
    assert_eq!(result.as_int(), Some(3));
}

#[test]
fn test_set_filter_method() {
    let result = eval(
        r#"
        let nums = {1, 2, 3, 4, 5};
        let filtered = nums.filter(|x| => x > 2);
        filtered.size();
    "#,
    );
    assert_eq!(result.as_int(), Some(3)); // 3, 4, 5
}

#[test]
fn test_set_foreach_method() {
    // forEach returns null and executes callback for each element
    let result = eval(
        r#"
        let nums = {10, 20, 30};
        nums.forEach(|x| => {
            println(x);
        });
    "#,
    );
    assert!(result.is_null());
}

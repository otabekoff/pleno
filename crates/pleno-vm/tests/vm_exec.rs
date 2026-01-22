use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> pleno_value::Value {
    let tokens = pleno_lexer::lex(source).expect("lex failed");
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&program).expect("compile failed");
    let mut vm = VM::new();
    vm.execute(chunk).expect("runtime failed")
}

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new();
    let chunk = compiler.compile(&program).map_err(|e| e.message)?;
    let mut vm = VM::new();
    vm.execute(chunk).map_err(|e| e.message)
}

#[test]
fn evaluates_arithmetic() {
    let value = eval("1 + 2 * 3;");
    assert_eq!(value, pleno_value::Value::Int(7));
}

#[test]
fn evaluates_unary() {
    let value = eval("-(4 + 1);");
    assert_eq!(value, pleno_value::Value::Int(-5));
}

#[test]
fn evaluates_comparisons() {
    let value = eval("1 < 2;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_if_else() {
    let value = eval("if (1 < 2) { 10; } else { 20; } 30;");
    assert_eq!(value, pleno_value::Value::Int(30));
}

#[test]
fn evaluates_while_loop() {
    let value = eval("while (false) { 1; } 5;");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_logical_ops() {
    let value = eval("true && false; false || true;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_nested_blocks() {
    let value = eval("{ { 1; } 2; }");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_chained_comparisons() {
    let value = eval("1 < 2 && 2 < 3;");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_mutating_while() {
    let value = eval("let mut x = 0; while (x < 3) { x = x + 1; } x;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_for_range() {
    let value = eval("let mut sum = 0; for (i in 0..=3) { sum = sum + i; } sum;");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_for_open_start_range() {
    let value = eval("let mut sum = 0; for (i in ..3) { sum = sum + i; } sum;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_for_open_end_range_with_break() {
    let value = eval("let mut sum = 0; for (i in 1..) { if (i > 3) { break; } sum = sum + i; } sum;");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_class_instance_property() {
    let value = eval("class Point {} let p = Point(); p.x = 4; p.x;");
    assert_eq!(value, pleno_value::Value::Int(4));
}

#[test]
fn evaluates_native_call() {
    let value = eval("id(7);");
    assert_eq!(value, pleno_value::Value::Int(7));
}

#[test]
fn evaluates_index_assignment() {
    let value = eval("let a = [1, 2]; a[0] = 5; a[0];");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_user_function_call() {
    let value = eval("fn add(a, b) { return a + b; } add(2, 3);");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_method_with_this() {
    let value = eval("class Counter { fn inc() { this.x = this.x + 1; } fn get() { return this.x; } } let c = Counter(); c.x = 0; c.inc(); c.get();");
    assert_eq!(value, pleno_value::Value::Int(1));
}

#[test]
fn evaluates_constructor() {
    let value = eval("class Point { fn constructor(x, y) { this.x = x; this.y = y; } fn sum() { return this.x + this.y; } } let p = Point(2, 3); p.sum();");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_inheritance_and_super() {
    let value = eval("class A { fn value() { return 1; } } class B extends A { fn value() { return super.value() + 1; } } let b = B(); b.value();");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_static_method() {
    let value = eval("class Util { static fn val() { return 7; } } Util.val();");
    assert_eq!(value, pleno_value::Value::Int(7));
}

#[test]
fn evaluates_closure_capture() {
    let value = eval("fn make(x) { fn inner(y) { return x + y; } return inner; } let f = make(2); f(3);");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_default_args() {
    let value = eval("fn add(a, b = 3) { return a + b; } add(2);");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_named_args() {
    let value = eval("fn sum(a, b, c = 1) { return a + b + c; } sum(b = 2, a = 1);");
    assert_eq!(value, pleno_value::Value::Int(4));
}

#[test]
fn evaluates_named_args_on_method() {
    let value = eval("class Calc { fn add(a, b = 1) { return a + b; } } let c = Calc(); c.add(b = 3, a = 2);");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_named_args_on_closure() {
    let value = eval("fn make() { fn inner(a, b = 2) { return a + b; } return inner; } let f = make(); f(b = 3, a = 1);");
    assert_eq!(value, pleno_value::Value::Int(4));
}

#[test]
fn evaluates_static_field() {
    let value = eval("class C { static fn make() { return 1; } } C.value = 9; C.value;");
    assert_eq!(value, pleno_value::Value::Int(9));
}

#[test]
fn denies_private_field_access() {
    let result = eval_result("class A { private secret; fn set(v) { this.secret = v; } } let a = A(); a.set(3); a.secret;");
    assert!(result.unwrap_err().contains("field access denied"));
}

#[test]
fn allows_private_field_in_method() {
    let value = eval("class A { private secret; fn set(v) { this.secret = v; } fn get() { return this.secret; } } let a = A(); a.set(4); a.get();");
    assert_eq!(value, pleno_value::Value::Int(4));
}

#[test]
fn denies_private_static_field_access() {
    let result = eval_result("class A { private static secret; static fn set(v) { A.secret = v; } } A.set(3); A.secret;");
    assert!(result.unwrap_err().contains("static field access denied"));
}

#[test]
fn denies_private_method_access() {
    let result = eval_result("class A { private fn secret() { return 1; } fn reveal() { return this.secret(); } } let a = A(); a.secret();");
    assert!(result.unwrap_err().contains("access denied"));
}

#[test]
fn allows_protected_method_in_subclass() {
    let value = eval("class A { protected fn val() { return 1; } } class B extends A { fn get() { return this.val(); } } let b = B(); b.get();");
    assert_eq!(value, pleno_value::Value::Int(1));
}

#[test]
fn evaluates_import_module() {
    let path = format!("{}/../../sources/module_simple.pln", env!("CARGO_MANIFEST_DIR"));
    let source = format!("import Module from \"{}\"; Module.value;", path);
    let value = eval(&source);
    assert_eq!(value, pleno_value::Value::Int(42));
}

#[test]
fn evaluates_structured_imports() {
    let path = format!(
        "{}/../../sources/module_structured.pln",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = format!(
        "import {{ value, secret as hidden }} from \"{}\"; value + hidden;",
        path
    );
    let value = eval(&source);
    assert_eq!(value, pleno_value::Value::Int(49));
}

#[test]
fn evaluates_module_namespace_import() {
    let path = format!(
        "{}/../../sources/module_structured.pln",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = format!("import * as Mod from \"{}\"; Mod.secret;", path);
    let value = eval(&source);
    assert_eq!(value, pleno_value::Value::Int(7));
}

#[test]
fn denies_internal_field_outside_module() {
    let path = format!(
        "{}/../../sources/module_internal.pln",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = format!(
        "import Mod from \"{}\"; let b = Mod.Box(); b.secret;",
        path
    );
    let result = eval_result(&source);
    assert!(result.unwrap_err().contains("field access denied"));
}

#[test]
fn allows_internal_field_in_module_method() {
    let path = format!(
        "{}/../../sources/module_internal.pln",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = format!(
        "import Mod from \"{}\"; let b = Mod.Box(); b.set(5); b.get();",
        path
    );
    let value = eval(&source);
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn evaluates_getter_setter() {
    let value = eval("class Counter { private value; get current() { return this.value; } set current(v) { this.value = v; } } let c = Counter(); c.current = 3; c.current;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_variadic_args() {
    let value = eval("fn sum(...nums) { let mut total = 0; for (n in nums) { total = total + n; } return total; } sum(1, 2, 3);");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_keyword_only_args() {
    let value = eval("fn add(a, *, b) { return a + b; } add(2, b = 3);");
    assert_eq!(value, pleno_value::Value::Int(5));
}

#[test]
fn rejects_keyword_only_positional() {
    let result = eval_result("fn add(a, *, b) { return a + b; } add(2, 3);");
    assert!(result.unwrap_err().contains("keyword-only argument"));
}

#[test]
fn evaluates_break_in_while() {
    let value = eval("let mut x = 0; while (true) { if (x == 3) { break; } x = x + 1; } x;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_continue_in_for() {
    let value = eval("let mut sum = 0; for (i in 0..5) { if (i == 2) { continue; } sum = sum + i; } sum;");
    assert_eq!(value, pleno_value::Value::Int(8));
}

#[test]
fn evaluates_for_array() {
    let value = eval("let mut sum = 0; for (x in [1, 2, 3]) { sum = sum + x; } sum;");
    assert_eq!(value, pleno_value::Value::Int(6));
}

#[test]
fn evaluates_for_map_keys() {
    let value = eval("let mut count = 0; let m = {\"a\": 1, \"b\": 2}; for (k in m) { count = count + 1; } count;");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_for_set() {
    let value = eval("let mut count = 0; for (x in {1, 2, 3}) { count = count + 1; } count;");
    assert_eq!(value, pleno_value::Value::Int(3));
}

#[test]
fn evaluates_range_expression() {
    let value = eval("1..=3;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: Some(1),
            end: Some(3),
            inclusive: true
        })
    );
}

#[test]
fn evaluates_open_ended_ranges() {
    let value = eval("..10;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: None,
            end: Some(10),
            inclusive: false
        })
    );
    let value = eval("5..;");
    assert_eq!(
        value,
        pleno_value::Value::Range(pleno_value::Range {
            start: Some(5),
            end: None,
            inclusive: false
        })
    );
}

#[test]
fn evaluates_map_property_access() {
    let value = eval("let m = {\"a\": 1, \"b\": 2}; m.a;");
    assert_eq!(value, pleno_value::Value::Int(1));
}

#[test]
fn property_access_on_non_map_errors() {
    let err = eval_result("let a = 1; a.b;").unwrap_err();
    assert!(err.contains("non-object"));
}

#[test]
fn evaluates_array_literal_and_index() {
    let value = eval("let a = [1, 2, 3]; a[1];");
    assert_eq!(value, pleno_value::Value::Int(2));
}

#[test]
fn evaluates_map_index() {
    let value = eval("let m = {\"x\": 10}; m[\"x\"];");
    assert_eq!(value, pleno_value::Value::Int(10));
}

#[test]
fn evaluates_set_literal_and_has() {
    let value = eval("let s = {1, 2}; s[1];");
    assert_eq!(value, pleno_value::Value::Bool(true));
}

#[test]
fn evaluates_map_property_access_via_object() {
    let value = eval("let o = {\"x\": 1}; o.x;");
    assert_eq!(value, pleno_value::Value::Int(1));
}

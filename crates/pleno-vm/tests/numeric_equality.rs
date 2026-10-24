//! Ints and Floats compare by value: `5.0 == 5`.

use pleno_compiler::Compiler;
use pleno_value::{Value, ValueKey};
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

fn eval_bool(source: &str) -> bool {
    match eval_result(source) {
        Ok(Value::Bool(value)) => value,
        other => panic!("{source}: {other:?}"),
    }
}

#[test]
fn int_and_float_with_the_same_value_are_equal() {
    assert!(eval_bool("5.0 == 5;"));
    assert!(eval_bool("5 == 5.0;"));
    assert!(eval_bool("10 / 2 == 5;"));
    assert!(eval_bool("-0.0 == 0;"));
    assert!(!eval_bool("5.5 == 5;"));
    assert!(!eval_bool("5.0 != 5;"));
    assert!(eval_bool("5.5 != 5;"));
    assert!(eval_bool("let a = 2.0; let b = 2; a == b && !(a != b);"));
}

#[test]
fn equality_is_exact_for_large_ints() {
    // 2^53 + 1 is not the Float 2^53 it would round to.
    assert!(!eval_bool("9007199254740993 == 9007199254740992.0;"));
    assert!(eval_bool("9007199254740992 == 9007199254740992.0;"));
    // Ints outside the inline range are boxed; equal ones still compare equal.
    assert!(eval_bool(
        "let a = 9007199254740993; let b = 9007199254740992 + 1; a == b;"
    ));
    assert!(!eval_bool("1.0e300 == 9223372036854775807;"));
}

#[test]
fn ordering_still_works_across_int_and_float() {
    assert!(eval_bool("3 < 3.5 && 4.0 >= 4 && 4.0 <= 4 && 5 > 4.5;"));
}

#[test]
fn other_types_are_not_numbers() {
    assert!(!eval_bool("1 == true;"));
    assert!(!eval_bool("0.0 == null;"));
    assert!(!eval_bool("\"5\" == 5;"));
}

#[test]
fn match_literals_compare_by_value() {
    let value = eval_result("match (2.0) { case 2 => \"two\", case _ => \"other\" };").unwrap();
    assert_eq!(value, Value::String("two".to_string().into()));
}

#[test]
fn sets_and_arrays_use_numeric_equality() {
    assert!(eval_bool("Set([5, 5.0]).size() == 1;"));
    assert!(eval_bool(
        "Set([5]).contains(5.0) && Set([5.0]).contains(5);"
    ));
    assert!(!eval_bool("Set([5]).contains(5.5);"));
    assert!(eval_bool("Set([0.0]).contains(-0.0);"));
    assert!(eval_bool(
        "[1, 2, 3].contains(2.0) && [1, 2.0].indexOf(2) == 1;"
    ));
}

#[test]
fn map_index_with_a_float_reads_the_key_it_prints_as() {
    assert!(eval_bool(
        "let m = {\"5\": 1, \"5.5\": 2}; m[5.0] == 1 && m[5.5] == 2;"
    ));
}

#[test]
fn assert_equal_treats_int_and_float_alike() {
    let source = "import { assert } from \"std:test\"; assert.equal(10 / 2, 5); assert.equal([1.0, 2], [1, 2.0]); assert.notEqual(5.5, 5); true;";
    assert_eq!(eval_result(source).unwrap(), Value::Bool(true));
    let err =
        eval_result("import { assert } from \"std:test\"; assert.equal(5.5, 5);").unwrap_err();
    assert!(err.contains("5.5"), "{err}");
}

#[test]
fn value_keys_hash_numbers_by_value() {
    use std::collections::HashSet;
    let int = ValueKey::try_from_value(&Value::Int(5)).unwrap();
    let float = ValueKey::try_from_value(&Value::Float(5.0)).unwrap();
    let half = ValueKey::try_from_value(&Value::Float(5.5)).unwrap();
    assert_eq!(int, float);
    assert_ne!(int, half);
    let set: HashSet<ValueKey> = [int, float, half].into_iter().collect();
    assert_eq!(set.len(), 2);
}

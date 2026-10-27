use num_traits::FromPrimitive;
use pleno_bytecode::{Chunk, Instruction, Opcode};
use pleno_value::Value;
use pleno_vm::VM;
use proptest::prelude::*;

/// Every opcode, so new opcodes are fuzzed without updating this file.
fn all_opcodes() -> Vec<Opcode> {
    (0..=u8::MAX)
        .filter_map(<Opcode as FromPrimitive>::from_u8)
        .collect()
}

fn arb_opcode() -> impl Strategy<Value = Opcode> {
    prop::sample::select(all_opcodes())
}

prop_compose! {
    fn arb_instruction()(
        op in arb_opcode(),
        a in 0u8..255,
        b in 0u8..255,
        c in 0u8..255,
    ) -> Instruction {
        Instruction::new(op, a, b, c)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn test_random_opcodes_dont_panic(instructions in prop::collection::vec(arb_instruction(), 1..50)) {
        let mut vm = VM::new();
        let mut chunk = Chunk::new();
        chunk.code = instructions;
        chunk.max_registers = 256;

        // Add some constants to avoid immediate OOB
        for _ in 0..256 {
            chunk.constants.push(Value::Null);
            chunk.functions.push(pleno_bytecode::FunctionProto {
                name: "dummy".to_string(),
                chunk: Chunk::new(),
                arity: 0,
                params: Vec::new(),
                is_async: false,
                is_pure: false,
                is_inline: false,
                is_hot: false,
                no_optimize: false,
                type_params: Vec::new(),
                type_constraints: indexmap::IndexMap::new(),
                annotations: Vec::new(),
                captured_type_params: Vec::new(),
            });
        }

        // We don't care about the result, only that it doesn't panic
        let _ = vm.execute(chunk);
    }
}

/// Runs a Pleno program and returns the value of its last expression.
fn eval(source: &str) -> Value {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = pleno_compiler::Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap()
}

// Pleno's operators must agree with the equivalent Rust operations.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn integer_arithmetic_matches_rust(a in -1_000_000i64..1_000_000, b in -1_000_000i64..1_000_000) {
        prop_assert_eq!(eval(&format!("{a} + {b};")), Value::Int(a + b));
        prop_assert_eq!(eval(&format!("{a} - {b};")), Value::Int(a - b));
        prop_assert_eq!(eval(&format!("{a} * {b};")), Value::Int(a * b));
        if b != 0 {
            // `/` always produces a Float; `%` keeps the sign of the dividend.
            prop_assert_eq!(eval(&format!("{a} / {b};")), Value::Float(a as f64 / b as f64));
            prop_assert_eq!(eval(&format!("{a} % {b};")), Value::Int(a % b));
        }
    }

    #[test]
    fn comparisons_match_rust(a in -1000i64..1000, b in -1000i64..1000) {
        prop_assert_eq!(eval(&format!("{a} < {b};")), Value::Bool(a < b));
        prop_assert_eq!(eval(&format!("{a} <= {b};")), Value::Bool(a <= b));
        prop_assert_eq!(eval(&format!("{a} > {b};")), Value::Bool(a > b));
        prop_assert_eq!(eval(&format!("{a} >= {b};")), Value::Bool(a >= b));
        prop_assert_eq!(eval(&format!("{a} == {b};")), Value::Bool(a == b));
        prop_assert_eq!(eval(&format!("{a} != {b};")), Value::Bool(a != b));
    }

    #[test]
    fn compound_assignment_matches_plain_assignment(start in -1000i64..1000, step in 1i64..50) {
        let compound = eval(&format!("let mut x = {start}; x += {step}; x *= 3; x -= {step}; x;"));
        prop_assert_eq!(compound, Value::Int((start + step) * 3 - step));
    }

    #[test]
    fn string_concatenation_preserves_contents(a in "[a-z]{0,20}", b in "[a-z]{0,20}") {
        let joined = eval(&format!("\"{a}\" + \"{b}\";"));
        let expected = format!("{a}{b}");
        prop_assert_eq!(joined.as_str(), Some(expected.as_str()));
    }

    #[test]
    fn array_push_and_length_agree(items in prop::collection::vec(-100i64..100, 0..20)) {
        let pushes: String = items.iter().map(|i| format!("xs.push({i}); ")).collect();
        let len = eval(&format!("let mut xs = []; {pushes} xs.length;"));
        prop_assert_eq!(len, Value::Int(items.len() as i64));
        if let Some(last) = items.last() {
            prop_assert_eq!(eval(&format!("let mut xs = []; {pushes} xs[{}];", items.len() - 1)), Value::Int(*last));
        }
    }
}

fn eval_result(source: &str) -> Result<Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .map_err(|e| e.message)?;
    let chunk = pleno_compiler::Compiler::new(source.to_string())
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

// Int is a full 64-bit integer: results are exact, and overflow is an error.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn full_range_integer_arithmetic(a in any::<i64>(), b in any::<i64>()) {
        // Pass the operands through variables so the VM, not the constant
        // folder, does the arithmetic.
        for (op, expected) in [("+", a.checked_add(b)), ("-", a.checked_sub(b)), ("*", a.checked_mul(b))] {
            let source = format!("let a = {a}; let b = {b}; a {op} b;");
            match (eval_result(&source), expected) {
                (Ok(value), Some(expected)) => prop_assert_eq!(value, Value::Int(expected)),
                (Err(err), None) => prop_assert!(err.contains("overflow"), "{}", err),
                (got, expected) => prop_assert!(false, "{source}: got {got:?}, expected {expected:?}"),
            }
        }
    }

    #[test]
    fn large_integers_round_trip_and_compare(a in any::<i64>()) {
        prop_assert_eq!(eval(&format!("let x = {a}; x;")), Value::Int(a));
        prop_assert_eq!(eval(&format!("let x = {a}; let y = {a}; x == y;")), Value::Bool(true));
    }
}

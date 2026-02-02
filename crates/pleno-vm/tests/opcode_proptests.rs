use num_traits::FromPrimitive;
use pleno_bytecode::{Chunk, Instruction, Opcode};
use pleno_value::Value;
use pleno_vm::VM;
use proptest::prelude::*;

prop_compose! {
    fn arb_opcode()(op in 0u8..116) -> Opcode {
        <Opcode as FromPrimitive>::from_u8(op).unwrap()
    }
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
            });
        }

        // We don't care about the result, only that it doesn't panic
        let _ = vm.execute(chunk);
    }
}

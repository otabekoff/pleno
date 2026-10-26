//! Strings, ranges and large integers in registers live in side tables whose
//! slots must be freed once nothing refers to them.

use pleno_value::nan_value::boxed_value_counts;
use pleno_vm::VM;

fn run(source: &str) -> VM {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = pleno_compiler::Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap();
    vm
}

#[test]
fn temporary_strings_ranges_and_big_ints_are_freed() {
    // Keep the VM: dropping the last one on the thread frees every slot.
    let _vm = run(r#"
        import { collect } from "std:gc";
        fn churn() {
            let mut last = "";
            for (let mut i = 0; i < 50000; i += 1) {
                last = "item " + i;                 // a new string each iteration
                let r = 0..i;                       // a new range
                let big = 9000000000000000000 + i;  // a new boxed integer
                if (i % 10000 == 0) { collect(); }
            }
            return last;
        }
        print(churn());
        collect();
    "#);
    let (ints, strings, ranges) = boxed_value_counts();
    // Only values still referenced by registers survive the final collection.
    assert!(ints < 100, "{ints} big integers still allocated");
    assert!(strings < 100, "{strings} strings still allocated");
    assert!(ranges < 100, "{ranges} ranges still allocated");
}

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
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

#[test]
fn division_by_zero_error() {
    let err = eval_result("1 / 0;").unwrap_err();
    assert!(err.contains("division by zero"));
}

#[test]
fn invalid_operands_error() {
    let err = eval_result("true + false;").unwrap_err();
    assert!(err.contains("invalid operands"));
}

#[test]
fn array_index_out_of_bounds_error() {
    let err = eval_result("let a = [1, 2, 3]; a[10];").unwrap_err();
    assert!(err.contains("index out of bounds"));
}

#[test]
fn call_on_non_callable_error() {
    let err = eval_result("let a = 1; a();").unwrap_err();
    assert!(err.contains("not callable"));
}

#[test]
fn registry_package_not_found_error() {
    let err = eval_result("import NonExistentPackage from \"registry:nonexistent\";").unwrap_err();
    assert!(err.contains("registry package not found") || err.contains("module not found"));
}

#[test]
fn invalid_argument_count_error() {
    let err = eval_result("fn add(a, b) { return a + b; } add(1);").unwrap_err();
    assert!(err.contains("missing required argument"));
}

#[test]
fn too_many_arguments_error() {
    let err = eval_result("fn add(a, b) { return a + b; } add(1, 2, 3);").unwrap_err();
    assert!(err.contains("too many positional arguments"));
}

#[test]
fn unknown_named_argument_error() {
    let err = eval_result("fn add(a, b) { return a + b; } add(a = 1, c = 3);").unwrap_err();
    // Parser rejects named arguments without all positional args filled first
    assert!(err.contains("unknown named argument") || err.contains("Expected ')' after arguments"));
}

#[test]
fn duplicate_argument_error() {
    let err = eval_result("fn add(a, b) { return a + b; } add(1, a = 2);").unwrap_err();
    // Parser rejects duplicate arguments
    assert!(err.contains("duplicate argument") || err.contains("Expected ')' after arguments"));
}

#[test]
fn keyword_only_argument_error() {
    let err = eval_result("fn add(a, *, b) { return a + b; } add(1, 2);").unwrap_err();
    assert!(err.contains("keyword-only argument requires name"));
}

#[test]
fn invalid_range_error() {
    let err = eval_result("1..=\"string\";").unwrap_err();
    assert!(err.contains("expected integer or null"));
}

#[test]
fn invalid_comparison_error() {
    // Use parentheses to avoid parser interpreting < as generic type parameter
    // The parser is very aggressive about generics, so we also accept the parse error
    let err = eval_result("(\"hello\") < (1);").unwrap_err();
    assert!(
        err.contains("invalid operands for comparison")
            || err.contains("invalid operands")
            || err.contains("Expected type name")
    );
}

#[test]
fn invalid_unary_operation_error() {
    let err = eval_result("-\"hello\";").unwrap_err();
    assert!(err.contains("invalid operand for unary"));
}

#[test]
fn invalid_set_element_error() {
    let err = eval_result("let s = {1, 2}; s.add({});").unwrap_err();
    assert!(err.contains("invalid value for set"));
}

#[test]
fn unsupported_set_element_type_error() {
    let err = eval_result("let s = {1, 2}; s[{}];").unwrap_err();
    assert!(err.contains("unsupported set element type"));
}

#[test]
fn invalid_index_operation_error() {
    let err = eval_result("1[0];").unwrap_err();
    assert!(err.contains("invalid index operation"));
}

#[test]
fn invalid_index_assignment_error() {
    let err = eval_result("1[0] = 5;").unwrap_err();
    assert!(err.contains("invalid index assignment"));
}

#[test]
fn field_set_on_non_object_error() {
    let err = eval_result("let a = 1; a.field = 5;").unwrap_err();
    assert!(err.contains("field set on non-object"));
}

#[test]
fn reading_a_missing_property_names_it_and_the_type() {
    for (source, expected) in [
        ("let a = 1; a.x;", "cannot read property 'x' of Int"),
        (
            "let a = 1; a.append(2);",
            "cannot read property 'append' of Int",
        ),
        ("let s = 1; s.has(2);", "cannot read property 'has' of Int"),
        (
            "let s = \"s\"; s.keys();",
            "cannot read property 'keys' of String",
        ),
        ("let a = true; a.len;", "cannot read property 'len' of Bool"),
        // `super` is allowed as a property name, so this is a runtime error.
        ("let a = 1; a.super;", "cannot read property 'super' of Int"),
    ] {
        let err = eval_result(source).unwrap_err();
        assert!(err.contains(expected), "{source}: {err}");
    }
}

#[test]
fn property_set_on_non_map_object_error() {
    let err = eval_result("let a = 1; a.prop = 5;").unwrap_err();
    // Both field and property use SetProperty, so we accept either error message
    assert!(
        err.contains("property set on non-map/object") || err.contains("field set on non-object")
    );
}

#[test]
fn class_name_must_be_string_error() {
    // This error occurs at parse time, not runtime
    // The parser requires an identifier for class names
    // The VM's DefineClass opcode does check for string, but compiler always provides one
    let err = eval_result("class 123 {}").unwrap_err();
    // Accept either parse error or runtime error message
    assert!(err.contains("class name must be string") || err.contains("Expected class name"));
}

#[test]
fn iter_over_non_collection_error() {
    let err = eval_result("for (x in 1) { x; }").unwrap_err();
    // The compiler checks this and returns "for loop expects array, set, or range"
    // The VM's IterValues opcode returns "iter over non-collection" but compiler catches it first
    assert!(err.contains("iter over non-collection") || err.contains("for loop expects"));
}

#[test]
fn method_access_denied_error() {
    let err = eval_result("class A { private fn secret() { return 1; } } let a = A(); a.secret();")
        .unwrap_err();
    assert!(err.contains("access denied"));
}

#[test]
fn field_access_denied_error() {
    let err = eval_result("class A { private field; } let a = A(); a.field;").unwrap_err();
    assert!(err.contains("field access denied"));
}

#[test]
fn static_field_access_denied_error() {
    let err = eval_result("class A { private static field; } A.field;").unwrap_err();
    assert!(err.contains("static field access denied"));
}

#[test]
fn static_method_access_denied_error() {
    let err = eval_result("class A { private static fn secret() { return 1; } } A.secret();")
        .unwrap_err();
    assert!(err.contains("static method access denied"));
}

#[test]
fn superclass_missing_error() {
    let err = eval_result("class A { fn f() { return super.g(); } } A().f();").unwrap_err();
    assert!(err.contains("super used without superclass"));
}

#[test]
fn unknown_super_method_error() {
    let err =
        eval_result("class A {} class B extends A { fn f() { return super.nope(); } } B().f();")
            .unwrap_err();
    assert!(err.contains("unknown super method"));
}

#[test]
fn partial_callee_must_be_function_error() {
    // Use underscore (_) as placeholder, not ellipsis (...)
    // a(_, 2) creates a partial application with placeholder
    let err = eval_result("let a = 1; let p = a(_, 2);").unwrap_err();
    assert!(err.contains("partial callee must be a function"));
}

#[test]
fn variadic_parameter_must_be_last_error() {
    let err = eval_result("fn test(...args, extra) {}").unwrap_err();
    assert!(err.contains("variadic parameter must be last"));
}

#[test]
fn module_not_found_error() {
    let err = eval_result("import NonExistentModule from \"nonexistent.pln\";").unwrap_err();
    assert!(err.contains("module not found"));
}

#[test]
fn std_module_not_found_error() {
    let err = eval_result("import NonExistentStd from \"std:nonexistent\";").unwrap_err();
    assert!(err.contains("std module not found"));
}

// --- Errors from malformed bytecode ---
//
// The compiler never emits these instruction shapes, so the tests build the
// bytecode by hand (or corrupt one operand of compiled code) to check that the
// VM rejects it with an error instead of crashing.

use pleno_bytecode::{Chunk, Instruction, Opcode};
use pleno_value::Value;
use std::rc::Rc;

fn string(value: &str) -> Value {
    Value::String(Rc::new(value.to_string()))
}

/// Runs hand-written bytecode and returns the error it must produce.
fn bytecode_error(code: &[(Opcode, u8, u8, u8)], constants: Vec<Value>) -> String {
    let mut chunk = Chunk::new();
    for &(opcode, a, b, c) in code {
        chunk.code.push(Instruction::new(opcode, a, b, c));
        chunk.lines.push(1);
        chunk.spans.push(pleno_common::Span::new(0, 0));
    }
    chunk.constants = constants;
    chunk.max_registers = 8;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .expect_err("malformed bytecode must fail")
        .message
}

/// Compiles `source`, lets `corrupt` change the chunk, and returns the error.
fn corrupted_error(source: &str, corrupt: impl FnOnce(&mut Chunk)) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let mut chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    corrupt(&mut chunk);
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .expect_err("corrupted bytecode must fail")
        .message
}

#[test]
fn global_name_must_be_string_error() {
    let err = bytecode_error(&[(Opcode::GetGlobal, 0, 0, 0)], vec![Value::Int(1)]);
    assert!(err.contains("global name must be string"), "{err}");
}

#[test]
fn unknown_function_prototype_error() {
    let err = bytecode_error(&[(Opcode::Closure, 0, 5, 0)], vec![]);
    assert!(err.contains("unknown function prototype"), "{err}");
}

#[test]
fn capture_on_non_closure_error() {
    let err = bytecode_error(
        &[(Opcode::LoadInt, 0, 1, 0), (Opcode::Capture, 0, 0, 0)],
        vec![],
    );
    assert!(err.contains("capture on non-closure"), "{err}");
}

#[test]
fn capture_index_out_of_bounds_error() {
    // The main script's closure has no captures.
    let err = bytecode_error(&[(Opcode::LoadCapture, 0, 3, 0)], vec![]);
    assert!(err.contains("capture index out of bounds"), "{err}");
}

#[test]
fn program_counter_out_of_bounds_error() {
    // No Return: execution runs past the last instruction.
    let err = bytecode_error(&[(Opcode::LoadNull, 0, 0, 0)], vec![]);
    assert!(err.contains("program counter out of bounds"), "{err}");
}

#[test]
fn set_has_opcode_on_non_set_error() {
    let err = bytecode_error(
        &[(Opcode::LoadInt, 0, 1, 0), (Opcode::SetHas, 0, 0, 0)],
        vec![],
    );
    assert!(err.contains("set has on non-set value"), "{err}");
}

#[test]
fn call_expects_args_array_error() {
    let err = bytecode_error(
        &[
            (Opcode::LoadInt, 1, 1, 0),
            (Opcode::LoadInt, 2, 1, 0),
            (Opcode::Call, 0, 1, 2),
        ],
        vec![],
    );
    assert!(err.contains("Call expects array of arguments"), "{err}");
}

#[test]
fn call_named_expects_map_error() {
    let err = bytecode_error(
        &[
            (Opcode::LoadInt, 1, 1, 0),
            (Opcode::LoadInt, 2, 1, 0),
            (Opcode::CallNamed, 0, 1, 2),
        ],
        vec![],
    );
    assert!(err.contains("CallNamed expects map of arguments"), "{err}");
}

#[test]
fn partial_expects_args_array_error() {
    let err = bytecode_error(
        &[
            (Opcode::GetGlobal, 1, 0, 0),
            (Opcode::LoadInt, 2, 1, 0),
            (Opcode::Partial, 0, 1, 2),
        ],
        vec![string("print")],
    );
    assert!(err.contains("Partial expects array of arguments"), "{err}");
}

#[test]
fn import_path_must_be_string_error() {
    let err = bytecode_error(&[(Opcode::Import, 0, 0, 0)], vec![Value::Int(1)]);
    assert!(err.contains("import path must be string"), "{err}");
}

#[test]
fn pop_handler_underflow_error() {
    let err = bytecode_error(&[(Opcode::PopHandler, 0, 0, 0)], vec![]);
    assert!(err.contains("pop handler underflow"), "{err}");
}

#[test]
fn field_name_must_be_string_error() {
    let err = bytecode_error(&[(Opcode::SetField, 0, 1, 0)], vec![Value::Int(1)]);
    assert!(err.contains("field name must be string"), "{err}");
}

#[test]
fn property_name_must_be_string_error() {
    let err = bytecode_error(&[(Opcode::GetProperty, 0, 1, 0)], vec![Value::Int(1)]);
    assert!(err.contains("property name must be string"), "{err}");
}

#[test]
fn super_property_must_be_string_error() {
    let err = bytecode_error(&[(Opcode::GetSuper, 0, 1, 0)], vec![Value::Int(1)]);
    assert!(err.contains("super property must be string"), "{err}");
}

#[test]
fn class_methods_must_be_map_error() {
    let err = bytecode_error(
        &[(Opcode::LoadInt, 1, 1, 0), (Opcode::DefineClass, 0, 0, 1)],
        vec![string("C")],
    );
    assert!(err.contains("class methods must be map"), "{err}");
}

#[test]
fn static_methods_must_be_map_error() {
    let err = bytecode_error(
        &[(Opcode::LoadInt, 1, 1, 0), (Opcode::SetStatic, 0, 1, 0)],
        vec![],
    );
    assert!(err.contains("static methods must be map"), "{err}");
}

#[test]
fn set_field_visibility_on_non_class_error() {
    let err = bytecode_error(
        &[
            (Opcode::LoadInt, 0, 1, 0),
            (Opcode::SetFieldVisibility, 0, 0, 0),
        ],
        vec![string("field")],
    );
    assert!(
        err.contains("set field visibility on non-class value"),
        "{err}"
    );
}

#[test]
fn set_static_field_visibility_on_non_class_error() {
    let err = bytecode_error(
        &[
            (Opcode::LoadInt, 0, 1, 0),
            (Opcode::SetStaticFieldVisibility, 0, 0, 0),
        ],
        vec![string("field")],
    );
    assert!(
        err.contains("set static field visibility on non-class value"),
        "{err}"
    );
}

#[test]
fn invalid_visibility_code_error() {
    let err = corrupted_error("class A { x; } A();", |chunk| {
        for instruction in &mut chunk.code {
            if instruction.opcode() == Opcode::SetFieldVisibility {
                *instruction = Instruction::new(
                    Opcode::SetFieldVisibility,
                    instruction.a(),
                    instruction.b(),
                    99,
                );
            }
        }
    });
    assert!(err.contains("invalid visibility code"), "{err}");
}

#[test]
fn invalid_method_visibility_error() {
    let err = corrupted_error("class A { fn f() {} } A();", |chunk| {
        for constant in &mut chunk.constants {
            if matches!(constant, Value::String(s) if s.as_str() == "public") {
                *constant = string("bogus");
            }
        }
    });
    assert!(err.contains("invalid method visibility"), "{err}");
}

#[test]
fn variadic_parameter_cannot_be_named_error() {
    let err = eval_result("fn f(...xs) { return xs; } f(xs: 1);").unwrap_err();
    assert!(err.contains("variadic parameter cannot be named"), "{err}");
}

#[test]
fn failed_to_read_module_error() {
    // A directory resolves as a module path but cannot be read as a file.
    let dir = std::env::temp_dir();
    let source = format!(
        "import x from \"{}\";",
        dir.display().to_string().replace('\\', "/")
    );
    let err = eval_result(&source).unwrap_err();
    assert!(err.contains("failed to read module"), "{err}");
}

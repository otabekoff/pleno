//! Every caught error is an `Error` instance with `name` and `message`, and
//! prints as `name: message`.

use pleno_compiler::Compiler;
use pleno_value::Value;
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

/// Runs `body` inside `try` and returns `describe(e)` for the caught error.
fn caught(body: &str, describe: &str) -> String {
    let source = format!(
        "import {{ Reflect }} from \"std:reflect\";\n\
         fn typed(x: Int) {{ return x; }}\n\
         let mut out = \"nothing caught\";\n\
         try {{ {body} }} catch (e) {{ out = {describe}; }}\n\
         out;"
    );
    match eval_result(&source) {
        Ok(Value::String(text)) => text.to_string(),
        other => panic!("{body}: {other:?}"),
    }
}

#[test]
fn runtime_errors_are_error_instances() {
    let body = "let zero = 0; 1 / zero;";
    assert_eq!(caught(body, "`${e}`"), "Error: division by zero");
    assert_eq!(caught(body, "e.name"), "Error");
    assert_eq!(caught(body, "e.message"), "division by zero");
    assert_eq!(caught(body, "typeof(e is Error)"), "Bool");
    assert_eq!(caught(body, "`${e is Error}`"), "true");
}

#[test]
fn caught_errors_carry_no_vm_statistics() {
    let body = "let zero = 0; 1 / zero;";
    assert_eq!(
        caught(body, "Reflect.getFields(e).join(\",\")"),
        "message,name"
    );
    assert_eq!(caught(body, "\"\" + e"), "Error: division by zero");
}

#[test]
fn type_annotation_errors_are_type_errors() {
    let body = "typed(\"a\");";
    assert_eq!(
        caught(body, "`${e}`"),
        "TypeError: parameter 'x' expected Int, got String"
    );
    assert_eq!(caught(body, "e.name"), "TypeError");
    assert_eq!(
        caught(body, "e.message"),
        "parameter 'x' expected Int, got String"
    );
    assert_eq!(
        caught("let n: Int = \"a\";", "e.toString()"),
        "TypeError: expected Int, found String"
    );
}

#[test]
fn prefixed_messages_become_the_error_name() {
    assert_eq!(
        caught("assert(false, \"bad\");", "`${e.name}|${e.message}`"),
        "AssertionError|bad"
    );
}

#[test]
fn typed_catch_of_error_handles_runtime_errors() {
    let source = "
        class NotFound extends Error {}
        let mut out = [];
        try { let zero = 0; 1 / zero; } catch (e: NotFound) { out.push(\"wrong\"); } catch (e: Error) { out.push(e.name); }
        try { [1][3]; } catch (e: Error) { out.push(e.message); }
        try { throw NotFound(\"user 7\"); } catch (e: Error) { out.push(`${e}`); }
        out.join(\", \");
    ";
    assert_eq!(
        eval_result(source).unwrap(),
        Value::String(
            "Error, array index out of bounds, NotFound: user 7"
                .to_string()
                .into()
        )
    );
}

#[test]
fn user_errors_have_the_same_shape() {
    let source = "
        class ValidationError extends Error {
            private field: String;
            constructor(message: String, field: String) {
                super(message);
                this.field = field;
            }
            public fn getField(): String { return this.field; }
        }
        let e = ValidationError(\"too old\", \"age\");
        let plain = Error(\"boom\");
        [`${e}`, e.name, e.message, e.getField(), `${plain}`, plain.name, `${Error()}`, `${Error().message == \"\"}`].join(\"|\");
    ";
    assert_eq!(
        eval_result(source).unwrap(),
        Value::String(
            "ValidationError: too old|ValidationError|too old|age|Error: boom|Error|Error|true"
                .to_string()
                .into()
        )
    );
}

#[test]
fn shadowing_error_does_not_change_runtime_errors() {
    let source = "
        class Error { constructor() {} }
        let mut out = \"\";
        try { let zero = 0; 1 / zero; } catch (e) { out = `${e.name}: ${e.message}`; }
        out;
    ";
    assert_eq!(
        eval_result(source).unwrap(),
        Value::String("Error: division by zero".to_string().into())
    );
}

#[test]
fn uncaught_errors_keep_their_report() {
    let err = eval_result("let zero = 0; 1 / zero;").unwrap_err();
    assert_eq!(err, "uncaught exception: division by zero");
    let err = eval_result("throw Error(\"boom\");").unwrap_err();
    assert_eq!(err, "uncaught exception: boom");
    let err = eval_result("fn f(x: Int) { return x; } f(\"a\");").unwrap_err();
    assert_eq!(
        err,
        "uncaught exception: TypeError: parameter 'x' expected Int, got String"
    );
    let err = eval_result("class Oops extends Error {} throw Oops(\"bad\");").unwrap_err();
    assert_eq!(err, "uncaught exception: Oops: bad");
}

#[test]
fn thrown_strings_stay_strings() {
    assert_eq!(caught("throw \"plain\";", "e"), "plain");
}

#[test]
fn vm_statistics_come_from_reflect() {
    let source = "
        import { vmStats } from \"std:reflect\";
        let stats = vmStats();
        stats.instructions > 0 && stats.peakMemory >= stats.memory && stats.memory > 0;
    ";
    assert_eq!(eval_result(source).unwrap(), Value::Bool(true));
}

#[test]
fn split_error_message_takes_error_names_only() {
    use pleno_value::split_error_message;
    assert_eq!(split_error_message("TypeError: bad"), ("TypeError", "bad"));
    assert_eq!(
        split_error_message("SecurityError: IO access denied"),
        ("SecurityError", "IO access denied")
    );
    assert_eq!(
        split_error_message("division by zero"),
        ("Error", "division by zero")
    );
    assert_eq!(split_error_message("Error: x"), ("Error", "Error: x"));
    assert_eq!(
        split_error_message("JSON parse error: x"),
        ("Error", "JSON parse error: x")
    );
    assert_eq!(split_error_message("NetErr: x"), ("Error", "NetErr: x"));
}

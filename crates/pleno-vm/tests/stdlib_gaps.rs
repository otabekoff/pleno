//! Regressions for standard library gaps fixed before 1.0: regex flags, CSV
//! headers, JWT claims, JSON for class instances and sets, XML errors and
//! map literal keys.

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

fn eval(source: &str) -> Value {
    eval_result(source).unwrap_or_else(|err| panic!("{err}\n--- source:\n{source}"))
}

fn eval_string(source: &str) -> String {
    match eval(source) {
        Value::String(s) => s.to_string(),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn eval_error(source: &str) -> String {
    match eval_result(source) {
        Ok(value) => panic!("expected an error, got {value:?}\n--- source:\n{source}"),
        Err(message) => message,
    }
}

#[test]
fn regex_flags_are_applied() {
    let source = r#"
        import { Regex } from "std:regex";
        [
            Regex("abc", "i").test("ABC"),
            Regex("abc").test("ABC"),
            Regex("(?i)abc").test("ABC"),
            Regex("^b$", "m").test("a\nb\nc"),
            Regex("^b$").test("a\nb\nc"),
            Regex("a.b", "s").test("a\nb"),
            Regex("a.b").test("a\nb"),
            Regex("a b c # letters", "x").test("abc"),
            Regex("^B", "im").findAll("a\nb\nB").length == 2,
            Regex("A", "i").replace("aAa", "-") == "---",
            Regex(",", "x").split("a,b").length == 2,
            Regex("x", "si").flags == "si",
            Regex("x").flags == "",
        ]
    "#;
    let Value::Array(results) = eval(source) else {
        panic!("expected an array");
    };
    let results: Vec<bool> = results
        .borrow()
        .elements
        .iter()
        .map(|v| v == &Value::Bool(true))
        .collect();
    assert_eq!(
        results,
        [
            true, false, true, true, false, true, false, true, true, true, true, true, true
        ]
    );
}

#[test]
fn regex_rejects_unknown_flags() {
    assert_eq!(
        eval_string(r#"import { Regex } from "std:regex"; Regex("abc", "mi").flags;"#),
        "mi"
    );
    let message = eval_error(r#"import { Regex } from "std:regex"; Regex("abc", "g");"#);
    assert!(
        message.contains("invalid regex flag 'g'"),
        "unexpected message: {message}"
    );
    let message = eval_error(r#"import { Regex } from "std:regex"; Regex("abc", 1);"#);
    assert!(message.contains("flags must be a string"), "{message}");
}

#[test]
fn csv_header_option() {
    let source = r#"
        import { CSV } from "std:csv";
        let text = "name,age\nAnn,30\n";
        [
            CSV.parse(text).length,
            CSV.parse(text)[0][0],
            CSV.parse(text, { header: true }).length,
            CSV.parse(text, { header: false }).length,
            CSV.parse(text, { header: false })[0][0],
            CSV.parse(text, header: false)[1][1],
            CSV.parse("only,row").length,
            CSV.parse("only,row", { header: false })[0][1],
        ].map(|v| => v.toString()).join(" ")
    "#;
    assert_eq!(eval_string(source), "1 Ann 1 2 name 30 0 row");
}

#[test]
fn csv_errors_have_one_prefix() {
    let message = eval_error(r#"import { CSV } from "std:csv"; CSV.parse("a,b\n1\n");"#);
    assert!(message.contains("CSV error: record 1"), "{message}");
    assert!(!message.contains("CSV error: CSV error"), "{message}");
    let message =
        eval_error(r#"import { CSV } from "std:csv"; CSV.parse("a", { headers: false });"#);
    assert!(message.contains("unknown option 'headers'"), "{message}");
}

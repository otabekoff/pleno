//! Runtime errors for a misspelled name end with "did you mean ...?" when a
//! close name exists, and say nothing extra otherwise.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn error_of(source: &str) -> String {
    let tokens = pleno_lexer::lex(source).expect("lexes");
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("parses");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("compiles");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    match vm.execute(chunk) {
        Ok(value) => panic!("expected an error, got {value:?}"),
        Err(err) => err.message,
    }
}

/// Asserts that running `source` fails with `message`, exactly at its end.
fn assert_error(source: &str, message: &str) {
    let error = error_of(source);
    assert!(
        error.ends_with(message),
        "{error:?} should end with {message:?}"
    );
}

#[test]
fn undefined_variables_suggest_a_local() {
    assert_error(
        "let length = 3; print(length); print(lenght);",
        "undefined variable 'lenght'; did you mean 'length'?",
    );
    assert_error(
        "fn f(items) { let count = 1; print(count); return cuont; } f(1);",
        "undefined variable 'cuont'; did you mean 'count'?",
    );
    assert_error(
        "fn f(items) { return itmes; } f(1);",
        "undefined variable 'itmes'; did you mean 'items'?",
    );
}

#[test]
fn undefined_variables_suggest_a_global_or_builtin() {
    assert_error(
        "fn helper() { return 1; } print(helper()); helpr();",
        "undefined variable 'helpr'; did you mean 'helper'?",
    );
    assert_error(
        "pirnt(1);",
        "undefined variable 'pirnt'; did you mean 'print'?",
    );
}

#[test]
fn locals_out_of_scope_are_not_suggested() {
    assert_error(
        "{ let length = 1; print(length); } print(lenght);",
        "undefined variable 'lenght'",
    );
    assert_error(
        "print(lenght); let length = 1; print(length);",
        "undefined variable 'lenght'",
    );
}

#[test]
fn nothing_close_suggests_nothing() {
    assert_error("print(zzqx);", "undefined variable 'zzqx'");
}

#[test]
fn built_in_methods_and_properties() {
    assert_error(
        "let a = [1]; a.pushh(2);",
        "cannot read property 'pushh' of Array; did you mean 'push'?",
    );
    assert_error(
        "print(\"abc\".lenght);",
        "cannot read property 'lenght' of String; did you mean 'length'?",
    );
    assert_error(
        "print(\"abc\".toUppercase());",
        "cannot read property 'toUppercase' of String; did you mean 'toUpperCase'?",
    );
    assert_error(
        "let m = {\"a\": 1}; m.contains(\"a\");",
        "Map has no method 'contains' (and no key 'contains'); did you mean 'containsKey'?",
    );
    assert_error(
        "let m = {\"total\": 1}; m.totl();",
        "Map has no method 'totl' (and no key 'totl'); did you mean 'total'?",
    );
}

#[test]
fn methods_added_with_extend_are_suggested() {
    assert_error(
        "extend String { fn shout() { return this; } } print(\"a\".shuot());",
        "cannot read property 'shuot' of String; did you mean 'shout'?",
    );
}

#[test]
fn module_namespaces() {
    assert_error(
        "import Math from \"std:math\"; print(Math.flor(2.5));",
        "Map has no method 'flor' (and no key 'flor'); did you mean 'floor'?",
    );
    assert_error(
        "import json from \"std:json\"; json.parsee(\"1\");",
        "Map has no method 'parsee' (and no key 'parsee'); did you mean 'parse'?",
    );
}

#[test]
fn class_members() {
    let class = "class P { public total: Int; fn constructor() { this.total = 1; } \
                 fn run() { return 1; } get size() { return 2; } }";
    assert_error(
        &format!("{class} P().og();"),
        "cannot read property 'og' of Object",
    );
    assert_error(
        &format!("{class} P().rnu();"),
        "cannot read property 'rnu' of Object; did you mean 'run'?",
    );
    assert_error(
        &format!("{class} print(P().totl);"),
        "cannot read property 'totl' of Object; did you mean 'total'?",
    );
    assert_error(
        &format!("{class} print(P().szie);"),
        "cannot read property 'szie' of Object; did you mean 'size'?",
    );
}

#[test]
fn inherited_members_are_suggested() {
    assert_error(
        "class A { fn hello() { return 1; } } class B extends A {} B().helo();",
        "cannot read property 'helo' of Object; did you mean 'hello'?",
    );
}

#[test]
fn private_members_are_suggested_only_where_visible() {
    let class = "class A { private secret: Int; public other: Int; \
                 fn constructor() { this.secret = 1; this.other = 2; } \
                 fn peek() { return this.secre; } }";
    // Outside the class the private field is not suggested.
    assert_error(
        &format!("{class} print(A().secre);"),
        "cannot read property 'secre' of Object",
    );
    // Inside it, it is.
    assert_error(
        &format!("{class} print(A().peek());"),
        "cannot read property 'secre' of Object; did you mean 'secret'?",
    );
    // A subclass does not see its superclass's private members.
    assert_error(
        "class A { private fn hidden() { return 1; } } \
         class B extends A { fn peek() { return this.hiden(); } } B().peek();",
        "cannot read property 'hiden' of Object",
    );
}

#[test]
fn static_members_and_enum_variants() {
    assert_error(
        "class K { static fn make() { return 1; } } print(K.mkae());",
        "cannot read property 'mkae' of Class; did you mean 'make'?",
    );
    assert_error(
        "enum Color { Red, Green } print(Color.Gren);",
        "Enum 'Color' has no variant or static method 'Gren'; did you mean 'Green'?",
    );
}

#[test]
fn unknown_named_arguments() {
    assert_error(
        "fn f(name: String) { print(name); } f(nmae: \"x\");",
        "unknown named argument 'nmae'; did you mean 'name'?",
    );
    assert_error(
        "fn f(name: String) { print(name); } f(colour: \"x\");",
        "unknown named argument 'colour'",
    );
}

#[test]
fn unknown_std_modules() {
    assert_error(
        "import { parse } from \"std:jsn\"; print(parse);",
        "std module not found: jsn; did you mean 'std:json'?",
    );
    assert_error(
        "import { exit } from \"std:proces\"; exit(0);",
        "std module not found: proces; did you mean 'std:process'?",
    );
    assert_error(
        "import x from \"std:nothinglikeit\"; print(x);",
        "std module not found: nothinglikeit",
    );
}

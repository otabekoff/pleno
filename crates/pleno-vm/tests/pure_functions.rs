use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval(source: &str) -> Result<String, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .map(|v| v.to_string())
        .map_err(|e| e.message)
}

#[test]
fn pure_functions_build_literals() {
    let cases = [
        (
            "@pure\nfn f(x) { return [x, 1, ...[2]]; }\nf(0);",
            "[0, 1, 2]",
        ),
        (
            "@pure\nfn f(k) { return {\"a\": k, ...{\"b\": 2}}; }\nf(1);",
            "{\"a\": 1, \"b\": 2}",
        ),
        ("@pure\nfn f(x) { return Set([x, 1]); }\nf(3);", "{3, 1}"),
        (
            "@pure\nfn f(x) { return (x, [x], {\"x\": x}); }\nf(4);",
            "(4, [4], {\"x\": 4})",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(eval(source).as_deref(), Ok(expected), "{source}");
    }
}

#[test]
fn pure_functions_still_reject_assignments() {
    for source in [
        "@pure\nfn f(m) { m.x = 1; return m; }\nf({});",
        "@pure\nfn f(a) { a[0] = 1; return a; }\nf([0]);",
        "@pure\nfn f() { print(1); }\nf();",
    ] {
        let err = eval(source).unwrap_err();
        assert!(
            err.contains("pure function cannot have side effects"),
            "{source}: {err}"
        );
    }
}

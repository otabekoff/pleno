//! `s = s + x` appends to the string in place when nothing else can see the
//! old value. Every way of keeping the old value must still see it unchanged.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn output(source: &str) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap_or_else(|e| panic!("{}", e.diagnostic.message));
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .unwrap_or_else(|e| panic!("{}\n{source}", e.message));
    vm.output.take().unwrap_or_default()
}

#[test]
fn aliases_keep_the_old_string() {
    let source = r#"
        let mut s = "ab";
        let a = s;
        s = s + "x";
        print(a, s);
        let arr = [];
        arr.push(s);
        s = s + "y";
        print(arr, s);
        let mut m = "m";
        let mp = {"k": m};
        m = m + "3";
        let mut t = "t";
        let tup = (t, 1);
        t = t + "t";
        print(mp, m, tup, t);
        let mut c = "c";
        let old = c;
        let f = () => old;
        let g = () => c;
        c = c + "2";
        print(f(), c, g());
    "#;
    // `g` shares the variable `c`, so it sees the new string; `old` keeps the
    // value it was given.
    assert_eq!(
        output(source),
        "ab abx\n[\"abx\"] abxy\n{\"k\": \"m\"} m3 (\"t\", 1) tt\nc c2 c2\n"
    );
}

#[test]
fn arguments_and_callers_keep_the_old_string() {
    let source = r#"
        fn keep(t) { let mut u = t; u = u + "!"; return u; }
        fn inner(x) { let mut y = x; y = y + "-in"; return x; }
        fn param(p) { p = p + "?"; return p; }
        let mut s = "s";
        s = s + "0";
        print(keep(s), s, inner(s), s, param(s), s);
    "#;
    assert_eq!(output(source), "s0! s0 s0 s0 s0? s0\n");
}

#[test]
fn appending_operands_of_every_kind() {
    let source = r#"
        let mut d = "d";
        d = d + d;
        d = d + 1 + 2.5 + true + 'z' + null;
        let mut parts = [];
        let mut built = "";
        for i in 0..5 {
            built = built + i;
            if i % 2 == 0 { parts.push(built); }
        }
        let mut big = "";
        for i in 0..2000 { big = big + "ab"; }
        print(d, built, parts, big.length);
    "#;
    assert_eq!(
        output(source),
        "dd12.5trueznull 01234 [\"0\", \"012\", \"01234\"] 4000\n"
    );
}

#[test]
fn sandboxed_code_builds_strings_without_touching_the_callers() {
    let source = r#"
        import { Sandbox } from "std:security";
        let mut outer = "o";
        outer = outer + "1";
        let box = Sandbox({ maxInstructions: 1000000 });
        let inside = box.run(fn() {
            let mut s = outer;
            for i in 0..3 { s = s + i; }
            return s;
        });
        print(inside, outer);
    "#;
    assert_eq!(output(source), "o1012 o1\n");
}

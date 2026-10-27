//! `==` compares arrays, tuples, maps and sets by contents.

use pleno_compiler::Compiler;
use pleno_vm::VM;

/// Runs `source` and returns the value of its last line, an expression.
fn eval(source: &str) -> String {
    // A statement cannot start with `[` or `{`, so the last line is bound first.
    let (body, last) = source.rsplit_once('\n').unwrap_or(("", source));
    let source = &format!("{body}\nlet result = {last}\nresult;");
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).unwrap().to_string()
}

#[test]
fn containers_compare_by_contents() {
    for (source, expected) in [
        ("[1, [2, 3]] == [1, [2, 3]];", "true"),
        ("[1, 2] == [2, 1];", "false"),
        ("[1] != [1];", "false"),
        ("[1] == [1.0];", "true"),
        ("(1, \"x\") == (1, \"x\");", "true"),
        ("(x: 1, y: 2) == (x: 1, y: 2);", "true"),
        ("(x: 1, y: 2) == (1, 2);", "false"),
        ("{\"a\": 1, \"b\": 2} == {\"b\": 2, \"a\": 1};", "true"),
        ("{\"a\": [1]} == {\"a\": [2]};", "false"),
        ("{\"a\": 1} == {\"a\": 1, \"b\": 2};", "false"),
        ("Set([1, 2]) == Set([2, 1]);", "true"),
        ("[1] == (1,);", "false"),
        ("[[1]].contains([1]);", "true"),
        ("[[0], [1]].indexOf([1]);", "1"),
    ] {
        assert_eq!(eval(source), expected, "{source}");
    }
}

#[test]
fn instances_use_operator_eq_inside_containers() {
    let classes = "class V { constructor(x) { this.x = x; } operator ==(o) { return this.x == o.x; } }\n\
                   class P { constructor(x) { this.x = x; } }\n";
    for (source, expected) in [
        ("[V(1)] == [V(1)];", "true"),
        ("[V(1)] == [V(2)];", "false"),
        ("[P(1)] == [P(1)];", "false"),
        ("let p = P(1);\n[p] == [p];", "true"),
        ("{\"v\": V(3)} == {\"v\": V(3)};", "true"),
    ] {
        assert_eq!(eval(&format!("{classes}{source}")), expected, "{source}");
    }
}

#[test]
fn self_containing_and_deep_values_compare_without_overflow() {
    let source = "let a = [1];\na.push(a);\nlet b = [1];\nb.push(b);\na == b;";
    assert_eq!(eval(source), "true");
    let deep = "let mut x = [];\nlet mut y = [];\n\
                for (i in 0..100000) { x = [x]; y = [y]; }\nx == y;";
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || eval(deep))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(result, "true");
}

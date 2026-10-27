//! Behavior settled in the 1.0 review of the language reference: how values
//! print, slices with negative positions, string escapes, `typeof` and `%`.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run(source: &str) -> Result<String, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)?;
    Ok(vm.output.take().unwrap_or_default())
}

fn output(source: &str) -> String {
    run(source).unwrap_or_else(|e| panic!("{e}\n{source}"))
}

#[test]
fn strings_inside_containers_are_quoted() {
    assert_eq!(
        output(r#"print("s", ["s", 1, "1", null], {"k": "v"}, ("a", 'c'), Set(["x"]));"#),
        "s [\"s\", 1, \"1\", null] {\"k\": \"v\"} (\"a\", 'c') {\"x\"}\n"
    );
    // Quotes, backslashes and control characters are escaped.
    assert_eq!(
        output(r#"print(["a\"b", "c\\d", "e\nf", '\''], {"q\"": 1});"#),
        "[\"a\\\"b\", \"c\\\\d\", \"e\\nf\", '\\''] {\"q\\\"\": 1}\n"
    );
    // On its own, or in a string, a string is not quoted.
    assert_eq!(output(r#"print("x" + ["y"]);"#), "x[\"y\"]\n");
}

#[test]
fn one_element_tuples_and_results_print_as_written() {
    assert_eq!(
        output(r#"print((1,), (x: 1), Ok(1), Err("no"), Some([2]), None);"#),
        "(1,) (x: 1) Ok(1) Err(\"no\") Some([2]) None\n"
    );
}

#[test]
fn floats_always_print_as_floats() {
    assert_eq!(
        output("print(3.0, 10 / 2, 2.5, -0.0, 0.1 + 0.2, 1e21, 1e-7, 1e16, 1e15);"),
        "3.0 5.0 2.5 -0.0 0.30000000000000004 1e21 1e-7 1e16 1000000000000000.0\n"
    );
    assert_eq!(
        output(r#"print("n=" + 4.0, `${2.0}`, [1.0], (1.5).toString());"#),
        "n=4.0 2.0 [1.0] 1.5\n"
    );
    // A whole Float still names the key of the Int it equals.
    assert_eq!(
        output(r#"let m = {1.0: "a"}; m[2.0] = "b"; print(m, m[1], m["2"]);"#),
        "{\"1\": \"a\", \"2\": \"b\"} a b\n"
    );
}

#[test]
fn containers_that_contain_themselves_print_without_looping() {
    assert_eq!(
        output("let a = [1]; a.push(a); let m = {}; m.self = m; print(a, m);"),
        "[1, [...]] {\"self\": {...}}\n"
    );
    // The same value twice side by side is not a cycle.
    assert_eq!(output("let x = [1]; print([x, x]);"), "[[1], [1]]\n");
    // Very deep nesting is cut off instead of overflowing the stack.
    let deep = output(
        "let mut v = []; for (i in 0..5000) { v = [v]; } let s = v.toString(); print(s.length < 1000);",
    );
    assert_eq!(deep, "true\n");
}

#[test]
fn negative_slice_positions_count_from_the_end() {
    assert_eq!(
        output(
            "let a = [0, 1, 2, 3, 4];
             print(a[-2..], a[1..-1], a[-3..=-2], a[..-1], a[..=-1], a[-9..2], a[3..1]);"
        ),
        "[3, 4] [1, 2, 3] [2, 3] [0, 1, 2, 3] [0, 1, 2, 3, 4] [0, 1] []\n"
    );
    assert_eq!(
        output(r#"let s = "héllo"; print(s[-3..], s[1..-1], s[..=-2]);"#),
        "llo éll héll\n"
    );
    assert_eq!(output("print((1, 2, 3)[-2..]);"), "(2, 3)\n");
    assert_eq!(
        output("let a = [0, 1, 2, 3, 4]; print(a.slice(-2), a.slice(1, -1), a.slice(3, 1));"),
        "[3, 4] [1, 2, 3] []\n"
    );
    // A single index stays checked: no counting from the end there.
    let err = run("print([1, 2][-1]);").unwrap_err();
    assert!(err.contains("array index out of bounds"), "{err}");
}

#[test]
fn string_escapes() {
    assert_eq!(
        output(r#"print("a\tb\0c\u{e9}\u{1F600}\'\"\\", '\u{41}', `\${x}`);"#).as_bytes(),
        "a\tb\0cé😀'\"\\ A ${x}\n".as_bytes()
    );
    for bad in [
        r#""\d""#,
        r#""C:\new\x""#,
        r#""\u{110000}""#,
        r#""\u41""#,
        r#"'\q'"#,
    ] {
        let err = run(&format!("print({bad});")).unwrap_err();
        assert!(err.contains("escape"), "{bad}: {err}");
    }
    // Raw strings take backslashes as they are.
    assert_eq!(output(r#"print(r"\d+");"#), "\\d+\n");
}

#[test]
fn typeof_names_classes_and_functions() {
    assert_eq!(
        output(
            "class P {} class E2 extends Error {} fn f() {}
             print(typeof(P()), typeof(E2(\"x\")), typeof(Error(\"x\")), typeof(f), typeof(fn() => 1), typeof(print), typeof(P));"
        ),
        "P E2 Error Function Function Function Class\n"
    );
}

#[test]
fn math_min_and_max_take_every_argument_and_keep_ints() {
    assert_eq!(
        output(
            r#"import { Math } from "std:math";
               print(Math.max(1, 5, 3), Math.max(1, 3, 5), Math.min(4, 2.5, 9), Math.max([7, 2]), Math.min(3));"#
        ),
        "5 5 2.5 7 3\n"
    );
    let err = run(r#"import { Math } from "std:math"; Math.max();"#).unwrap_err();
    assert!(err.contains("at least one number"), "{err}");
}

#[test]
fn functions_see_later_assignments_to_the_variables_they_use() {
    // Before, a function kept the value a variable had when the function was
    // created, unless the function itself assigned the variable.
    assert_eq!(
        output(
            r#"let mut debug = false;
               fn log(m) { if (debug) { print(m); } }
               debug = true;
               log("on");
               let mut items = [];
               let read = fn() => items;
               items = [1];
               print(read());
               class K { fn flag() { return debug; } }
               debug = false;
               print(K().flag());
               fn outer() { let mut v = 1; fn inner() { return v; } v = 2; return inner(); }
               print(outer());"#
        ),
        "on\n[1]\nfalse\n2\n"
    );
}

#[test]
fn closures_share_parameters_and_loop_variables() {
    assert_eq!(
        output(
            r#"fn p1(x) { let g = fn() => x; x = 5; return g(); }
               fn p2(x) { let g = fn() { x = 5; }; g(); return x; }
               fn p3(x = 1) { let g = fn() => x; x += 1; return g(); }
               print(p1(1), p2(1), p3(), p3(10));
               let fs = [];
               for (k in [1, 2]) { fs.push(fn() => k); k = k * 10; }
               print(fs.map(|f| => f()));
               let gs = [];
               for (let i = 0; i < 3; i++) { gs.push(fn() => i); }
               print(gs.map(|f| => f()));
               let mut w = 0;
               let seen = fn() => w;
               w++;
               ++w;
               print(seen());"#
        ),
        "5 5 2 11\n[10, 20]\n[0, 1, 2]\n2\n"
    );
}

#[test]
fn an_exported_shared_variable_exports_its_value() {
    let dir = std::env::temp_dir().join(format!("pleno-spec-freeze-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("counter.pln"),
        "export let mut count = 1;\nexport fn current() { return count; }\ncount = 2;\n",
    )
    .unwrap();
    let main = dir.join("main.pln");
    let source = "import { count, current } from \"./counter.pln\";\nprint(count, current());\n";
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.output = Some(String::new());
    vm.main_path = Some(main);
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk)
        .unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(vm.output.take().unwrap_or_default(), "2 2\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn integer_remainder_has_the_sign_of_the_dividend() {
    assert_eq!(
        output("let m = -9223372036854775807 - 1; print(7 % 3, -7 % 3, 7 % -3, -7.5 % 2, m % -1);"),
        "1 -1 1 -1.5 0\n"
    );
}

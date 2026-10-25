//! String positions and lengths count Unicode scalar values (Rust `char`s),
//! never UTF-8 bytes, and no position makes the VM panic. Every program runs
//! with and without the small interpreter loop and must print the same.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run_once(source: &str, fast_loop: bool) -> (Result<String, String>, String) {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap_or_else(|e| panic!("{}", e.diagnostic.message));
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.fast_loop = fast_loop;
    let result = vm
        .execute(chunk)
        .map(|value| value.to_string())
        .map_err(|e| e.message);
    (result, vm.output.take().unwrap_or_default())
}

/// Runs `source` both ways, checks they agree and returns the output.
fn output(source: &str) -> String {
    let fast = run_once(source, true);
    let general = run_once(source, false);
    assert_eq!(fast, general, "{source}");
    let (result, output) = fast;
    assert!(result.is_ok(), "{source}: {result:?}");
    output
}

/// Runs `source` both ways and returns the error message.
fn error(source: &str) -> String {
    let fast = run_once(source, true);
    let general = run_once(source, false);
    assert_eq!(fast.0, general.0, "{source}");
    fast.0.expect_err(source)
}

/// One line per expression, so a failure shows which one differs.
fn lines(prelude: &str, source_lines: &[&str]) -> Vec<String> {
    let mut source = format!("{prelude}\n");
    for line in source_lines {
        source.push_str(&format!("print({line});\n"));
    }
    output(&source).lines().map(str::to_string).collect()
}

fn check(cases: &[(&str, &str)]) {
    check_with("", cases);
}

/// `check` with `prelude` (such as an import) run first.
fn check_with(prelude: &str, cases: &[(&str, &str)]) {
    let exprs: Vec<&str> = cases.iter().map(|(expr, _)| *expr).collect();
    let got = lines(prelude, &exprs);
    assert_eq!(got.len(), cases.len(), "{got:?}");
    for ((expr, want), got) in cases.iter().zip(got) {
        assert_eq!(&got, want, "{expr}");
    }
}

#[test]
fn length_counts_characters() {
    check(&[
        (r#""".length"#, "0"),
        (r#""hello".length"#, "5"),
        (r#""héllo".length"#, "5"),
        (r#""€uro".length"#, "4"),
        (r#""a😀b".length"#, "3"),
        // `e` followed by U+0301 COMBINING ACUTE ACCENT: two scalar values.
        ("\"e\u{301}\".length", "2"),
        (r#""日本語".length"#, "3"),
    ]);
    assert_eq!(output("let s = \"日本語\"; print(s.length);"), "3\n");
}

#[test]
fn indexing_counts_characters() {
    check(&[
        (r#""héllo"[0]"#, "h"),
        (r#""héllo"[1]"#, "é"),
        (r#""héllo"[2]"#, "l"),
        (r#""héllo"[4]"#, "o"),
        (r#""a€b"[1]"#, "€"),
        (r#""a€b"[2]"#, "b"),
        (r#""😀x"[0]"#, "😀"),
        (r#""😀x"[1]"#, "x"),
        ("\"e\u{301}x\"[1]", "\u{301}"),
        ("\"e\u{301}x\"[2]", "x"),
    ]);
    for source in [
        r#""héllo"[5];"#,
        r#""héllo"[6];"#,
        r#""héllo"[-1];"#,
        r#""abc"[3];"#,
        r#"""[0];"#,
        r#""😀"[9223372036854775807];"#,
    ] {
        assert!(
            error(source).contains("string index out of bounds"),
            "{source}"
        );
    }
}

#[test]
fn range_slices_count_characters() {
    check(&[
        (r#""héllo"[1..3]"#, "él"),
        (r#""héllo"[1..=3]"#, "éll"),
        (r#""héllo"[2..]"#, "llo"),
        (r#""héllo"[0..0]"#, ""),
        (r#""héllo"[0..5]"#, "héllo"),
        (r#""héllo"[0..99]"#, "héllo"),
        (r#""héllo"[4..2]"#, ""),
        (r#""héllo"[9..]"#, ""),
        (r#""a€😀b"[1..3]"#, "€😀"),
        (r#""a€😀b"[2..=2]"#, "😀"),
        (r#""abc"[0..9223372036854775807]"#, "abc"),
        (r#""abc"[0..=9223372036854775807]"#, "abc"),
        (r#"""[0..3]"#, ""),
        ("\"e\u{301}x\"[1..2]", "\u{301}"),
    ]);
    assert_eq!(output("let i = -2; print(\"héllo\"[i..2]);"), "hé\n");
    assert_eq!(output("let i = -5; print(\"héllo\"[0..i]);"), "\n");
}

#[test]
fn slice_syntax_counts_characters() {
    assert_eq!(
        output(
            r#"
            let s = "a€😀bé";
            print(s[1:3]);
            print(s[-2:]);
            print(s[..2]);
            print(s[3:]);
            print(s[2:99]);
            print(""[..1]);
            "#
        ),
        "€😀\nbé\na€\nbé\n😀bé\n\n"
    );
}

#[test]
fn substring_counts_characters_and_clamps() {
    check(&[
        (r#""héllo".substring(0, 2)"#, "hé"),
        (r#""héllo".substring(1, 2)"#, "é"),
        (r#""héllo".substring(2)"#, "llo"),
        (r#""héllo".substring(0)"#, "héllo"),
        (r#""héllo".substring(5)"#, ""),
        (r#""héllo".substring(0, 99)"#, "héllo"),
        (r#""héllo".substring(99, 0)"#, "héllo"),
        (r#""héllo".substring(3, 1)"#, "él"),
        (r#""héllo".substring(-3, 2)"#, "hé"),
        (r#""héllo".substring(2, -1)"#, "hé"),
        (r#""a€😀b".substring(1, 3)"#, "€😀"),
        (r#""a€😀b".substring(2, 3)"#, "😀"),
        (r#""".substring(0, 1)"#, ""),
        ("\"e\u{301}\".substring(0, 1)", "e"),
        (r#""hello".substring(1, 4)"#, "ell"),
    ]);
}

#[test]
fn char_at_counts_characters() {
    check(&[
        (r#""héllo".charAt(0)"#, "h"),
        (r#""héllo".charAt(1)"#, "é"),
        (r#""héllo".charAt(4)"#, "o"),
        (r#""héllo".charAt(5)"#, "null"),
        (r#""héllo".charAt(-1)"#, "null"),
        (r#""a€😀".charAt(1)"#, "€"),
        (r#""a€😀".charAt(2)"#, "😀"),
        (r#""".charAt(0)"#, "null"),
        ("\"e\u{301}\".charAt(1)", "\u{301}"),
        (r#""hello".charAt(9223372036854775807)"#, "null"),
    ]);
}

#[test]
fn index_of_counts_characters() {
    check(&[
        (r#""héllo".indexOf("l")"#, "2"),
        (r#""héllo".indexOf("o")"#, "4"),
        (r#""héllo".indexOf("é")"#, "1"),
        (r#""héllo".indexOf("x")"#, "-1"),
        (r#""héllo".indexOf("")"#, "0"),
        (r#""a€😀b".indexOf("b")"#, "3"),
        (r#""a€😀b".indexOf("😀")"#, "2"),
        (r#""".indexOf("a")"#, "-1"),
        ("\"e\u{301}x\".indexOf(\"x\")", "2"),
        (r#""hello".indexOf("lo")"#, "3"),
    ]);
}

#[test]
fn positions_agree_with_each_other() {
    // indexOf gives a position that substring, charAt and [i] read back.
    for text in ["hello world", "héllo wörld", "€ 5 😀 ok", "e\u{301} x"] {
        let source = format!(
            r#"
            let s = "{text}";
            let i = s.indexOf(" ");
            print(s[i] == " ", s.charAt(i) == " ", s.substring(i, i + 1) == " ");
            print(s.substring(0, i) + s.substring(i) == s);
            print(s[0..i] + s[i..] == s);
            print(s.split("").length == s.length);
            "#
        );
        assert_eq!(
            output(&source),
            "true true true\ntrue\ntrue\ntrue\n",
            "{text}"
        );
    }
}

#[test]
fn split_with_an_empty_separator_gives_characters() {
    check(&[
        (r#""abc".split("")"#, "[a, b, c]"),
        (r#""abc".split("").length"#, "3"),
        (r#""héllo".split("")"#, "[h, é, l, l, o]"),
        (r#""a€😀".split("")"#, "[a, €, 😀]"),
        (r#""".split("")"#, "[]"),
        (r#""".split("").length"#, "0"),
        ("\"e\u{301}\".split(\"\").length", "2"),
        (r#""a,b,,c".split(",")"#, "[a, b, , c]"),
        (r#""é,€".split(",")"#, "[é, €]"),
        (r#""".split(",").length"#, "1"),
    ]);
}

#[test]
fn methods_without_positions_handle_unicode() {
    check(&[
        (r#""ab".repeat(3)"#, "ababab"),
        (r#""é😀".repeat(2)"#, "é😀é😀"),
        (r#""é".repeat(0)"#, ""),
        (r#""".repeat(5)"#, ""),
        (r#""é😀".repeat(2).length"#, "4"),
        (r#""héllo".startsWith("hé")"#, "true"),
        (r#""héllo".endsWith("llo")"#, "true"),
        (r#""héllo".contains("él")"#, "true"),
        (r#""héllo".replace("é", "e")"#, "hello"),
        (r#""héllo".toUpperCase()"#, "HÉLLO"),
        (r#""  é  ".trim().length"#, "1"),
    ]);
    assert!(error(r#""ab".repeat(-1);"#).contains("must not be negative"));
    assert!(error(r#""ab".repeat(9223372036854775807);"#).contains("too long"));
}

#[test]
fn regex_match_index_counts_characters() {
    let import = r#"import { Regex } from "std:regex";"#;
    check_with(
        import,
        &[
            (r#"Regex("l+").match("héllo").index"#, "2"),
            (r#"Regex("o").match("héllo").index"#, "4"),
            (r#"Regex("h").match("héllo").index"#, "0"),
            (r#"Regex("b").match("a€😀b").index"#, "3"),
            (r#"Regex("x").match("abc")"#, "null"),
        ],
    );
    assert_eq!(
        output(
            r#"
            import { Regex } from "std:regex";
            let all = Regex("[a-z]+").findAll("é ab € cd 😀 ef");
            for (m in all) { print(m.value, m.index); }
            "#
        ),
        "ab 2\ncd 7\nef 12\n"
    );
}

#[test]
fn assert_length_of_counts_characters() {
    assert_eq!(
        output(
            r#"
            import { assert } from "std:test";
            assert.lengthOf("héllo", 5);
            assert.lengthOf("😀", 1);
            print("ok");
            "#
        ),
        "ok\n"
    );
}

#[test]
fn loops_over_characters() {
    // The loop body runs in the small interpreter loop when it is on.
    let source = r#"
        fn reverse(s) {
            let mut out = "";
            let mut i = s.length - 1;
            while (i >= 0) {
                out = out + s[i];
                i = i - 1;
            }
            return out;
        }
        fn count(s, c) {
            let mut n = 0;
            for (i in 0..s.length) {
                if (s.charAt(i) == c) { n = n + 1; }
            }
            return n;
        }
        print(reverse("héllo €😀"));
        print(count("€a€b€", "€"));
        print(reverse(""));
        let mut long = "";
        for (i in 0..200) { long = long + "é"; }
        print(long.length, count(long, "é"), long.substring(198).length);
    "#;
    assert_eq!(output(source), "😀€ olléh\n3\n\n200 200 2\n");
}

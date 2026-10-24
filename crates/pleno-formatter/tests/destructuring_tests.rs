//! Formatting destructuring `let` and tuple type annotations.

use pleno_formatter::{FormatOptions, format_code};

fn format(source: &str) -> String {
    format_code(source, &FormatOptions::default()).unwrap()
}

#[test]
fn formatted_destructuring_round_trips() {
    let source = "\
let (a, b) = pair;
let mut (x, (y, _)) = nested;
let [first, ...rest] = items;
let [head, ...] = items;
let (single,) = one;
let ([p, q], r): ((Int, Int), String) = value;
let point: (Int, Int) = (1, 2);
fn swap(pair: (Int, String)): (String, Int) {
    let (n, s) = pair;
    return (s, n);
}
";
    assert_eq!(format(source), source);
}

#[test]
fn messy_patterns_are_normalized() {
    let source = "let(a,b)=pair;\nlet mut [ x ,y, ..rest ]=items;\nlet ( z , ) = t;\n";
    let expected = "let (a, b) = pair;\nlet mut [x, y, ...rest] = items;\nlet (z,) = t;\n";
    let formatted = format(source);
    assert_eq!(formatted, expected);
    assert_eq!(format(&formatted), formatted, "formatting is idempotent");
}

#[test]
fn comments_around_destructuring_are_kept() {
    let source = "\
/// The two halves.
let (left, right) = split(); // both sides
";
    let formatted = format(source);
    assert!(formatted.contains("/// The two halves."), "{formatted}");
    assert!(
        formatted.contains("let (left, right) = split(); // both sides"),
        "{formatted}"
    );
}

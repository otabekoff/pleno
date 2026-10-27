//! Formatting number, string and character literals keeps their values and types.

use pleno_formatter::{FormatOptions, format_code};

fn format(source: &str) -> String {
    format_code(source, &FormatOptions::default()).unwrap()
}

#[test]
fn float_literals_stay_floats() {
    // `1e21` used to become 22 digits, an Int literal too large to parse, and
    // `1e17` an Int.
    assert_eq!(
        format("let a = 3.0;\nlet b = 1e21;\nlet c = 1e17;\nlet d = 1e-7;\nlet e = 2.50;\n"),
        "let a = 3.0;\nlet b = 1e21;\nlet c = 1e17;\nlet d = 1e-7;\nlet e = 2.5;\n"
    );
}

#[test]
fn nul_escapes_are_written_back() {
    assert_eq!(
        format("let s = \"a\\0b\";\nlet c = '\\0';\n"),
        "let s = \"a\\0b\";\nlet c = '\\0';\n"
    );
}

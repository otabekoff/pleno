use pleno_formatter::{FormatOptions, format_code};

#[test]
fn test_preserves_trailing_comment() {
    let input = "let x = 1; // comment";
    let expected = "let x = 1; // comment\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_preserves_blank_lines() {
    let input = "let x = 1;\n\nlet y = 2;";
    let expected = "let x = 1;\n\nlet y = 2;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_preserves_multiple_blank_lines_capped() {
    // Parser likely caps or formatter caps blank lines (AST logic caps at 255 but let's test regular usage)
    let input = "let x = 1;\n\n\nlet y = 2;";
    // Depending on logic, it might preserve all or normalize.
    // Our logic creates leading_blank_lines based on start line difference.
    // Parser logic: start_line - last_stmt_end_line - 1.
    // 1; on line 1.
    // \n
    // \n
    // let on line 4.
    // 4 - 1 - 1 = 2 blank lines.
    let expected = "let x = 1;\n\nlet y = 2;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_preserves_comments_in_with_statement() {
    let input = "// open both\nwith (a = open(\"a\"), b = open(\"b\")) {\n    // copy\n    b.write(a.read()); // all of it\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, input);
}

#[test]
fn test_preserves_comments_between_with_resources() {
    let input = "with (\n    // the source\n    a = open(\"a\"),\n    b = open(\"b\"), // the copy\n) {\n    b.write(a.read());\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, input);
}

// `x op= value` was written as `x = x op value` without parentheses, which
// changed what the program computed: `n *= 3 + 1` became `n = n * 3 + 1`.
#[test]
fn compound_assignments_are_kept_as_written() {
    let input = "\
out += i == 1 ? \"a\" : \"b\";
n *= 3 + 1;
n -= 1 - 1;
n += (2);
n /= a ?? b;
n %= 3;
m[\"k\"] *= 2 + 3;
o.v -= 4 - 1;
x = x + 1;
";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, input);
}

// Parsed without the formatter, the value keeps its grouping, so any printer
// of the tree gets it right.
#[test]
fn a_compound_assignment_value_keeps_its_grouping() {
    let source = "n *= 3 + 1;";
    let tokens = pleno_lexer::lex(source).unwrap();
    let program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let dump = format!("{:?}", program.statements[0].kind);
    assert!(
        dump.contains("op: Multiply, right: Expr { kind: Paren("),
        "{dump}"
    );
}

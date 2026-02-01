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

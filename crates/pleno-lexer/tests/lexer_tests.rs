//! Comprehensive tests for the Pleno lexer.
//!
//! These tests cover:
//! - Basic token recognition (identifiers, numbers, strings)
//! - Operators and punctuation
//! - Keywords
//! - Comments
//! - Error cases
//! - Edge cases

use pleno_ast::CommentKind;
use pleno_lexer::{Token, TokenKind, lex};

// ============================================================================
// Helper Functions
// ============================================================================

fn lex_ok(source: &str) -> Vec<Token> {
    lex(source).expect("lexing should succeed")
}

fn kinds(tokens: &[Token]) -> Vec<TokenKind> {
    tokens.iter().map(|t| t.kind.clone()).collect()
}

// ============================================================================
// Basic Token Tests
// ============================================================================

#[test]
fn test_empty_source() {
    let tokens = lex_ok("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
}

#[test]
fn test_whitespace_only() {
    let tokens = lex_ok("   \t\n\r\n   ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Eof);
}

#[test]
fn test_identifier_simple() {
    let tokens = lex_ok("foo");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Ident("foo".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_identifier_with_underscore() {
    let tokens = lex_ok("foo_bar_baz");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Ident("foo_bar_baz".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_identifier_with_numbers() {
    let tokens = lex_ok("foo123");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Ident("foo123".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_identifier_starting_with_underscore() {
    let tokens = lex_ok("_private");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Ident("_private".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_underscore_alone() {
    let tokens = lex_ok("_");
    assert_eq!(kinds(&tokens), vec![TokenKind::Underscore, TokenKind::Eof]);
}

// ============================================================================
// Number Tests
// ============================================================================

#[test]
fn test_integer_simple() {
    let tokens = lex_ok("42");
    assert_eq!(kinds(&tokens), vec![TokenKind::Int(42), TokenKind::Eof]);
}

#[test]
fn test_integer_zero() {
    let tokens = lex_ok("0");
    assert_eq!(kinds(&tokens), vec![TokenKind::Int(0), TokenKind::Eof]);
}

#[test]
fn test_integer_large() {
    let tokens = lex_ok("999999999");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Int(999_999_999), TokenKind::Eof]
    );
}

#[test]
fn test_float_simple() {
    let tokens = lex_ok("123.45");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::Float(123.45), TokenKind::Eof]
    );
}

#[test]
fn test_float_with_exponent() {
    let tokens = lex_ok("1e10");
    assert_eq!(kinds(&tokens), vec![TokenKind::Float(1e10), TokenKind::Eof]);
}

#[test]
fn test_float_with_negative_exponent() {
    let tokens = lex_ok("1e-5");
    assert_eq!(kinds(&tokens), vec![TokenKind::Float(1e-5), TokenKind::Eof]);
}

#[test]
fn test_hex_integer() {
    let tokens = lex_ok("0xFF");
    assert_eq!(kinds(&tokens), vec![TokenKind::Int(255), TokenKind::Eof]);
}

#[test]
fn test_binary_integer() {
    let tokens = lex_ok("0b1010");
    assert_eq!(kinds(&tokens), vec![TokenKind::Int(10), TokenKind::Eof]);
}

#[test]
fn test_octal_integer() {
    let tokens = lex_ok("0o77");
    assert_eq!(kinds(&tokens), vec![TokenKind::Int(63), TokenKind::Eof]);
}

// ============================================================================
// String Tests
// ============================================================================

#[test]
fn test_string_simple() {
    let tokens = lex_ok(r#""hello""#);
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::String("hello".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_string_empty() {
    let tokens = lex_ok(r#""""#);
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::String("".to_string()), TokenKind::Eof]
    );
}

#[test]
fn test_string_with_escape_newline() {
    let tokens = lex_ok(r#""hello\nworld""#);
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::String("hello\nworld".to_string()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_string_with_escape_tab() {
    let tokens = lex_ok(r#""hello\tworld""#);
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::String("hello\tworld".to_string()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_string_with_escape_quote() {
    let tokens = lex_ok(r#""say \"hello\"""#);
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::String("say \"hello\"".to_string()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_string_with_escape_backslash() {
    let tokens = lex_ok(r#""path\\to\\file""#);
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::String("path\\to\\file".to_string()),
            TokenKind::Eof
        ]
    );
}

// ============================================================================
// Char Tests
// ============================================================================

#[test]
fn test_char_simple() {
    let tokens = lex_ok("'a'");
    assert_eq!(kinds(&tokens), vec![TokenKind::Char('a'), TokenKind::Eof]);
}

#[test]
fn test_char_escape_newline() {
    let tokens = lex_ok(r"'\n'");
    assert_eq!(kinds(&tokens), vec![TokenKind::Char('\n'), TokenKind::Eof]);
}

#[test]
fn test_char_escape_tab() {
    let tokens = lex_ok(r"'\t'");
    assert_eq!(kinds(&tokens), vec![TokenKind::Char('\t'), TokenKind::Eof]);
}

// ============================================================================
// Boolean and Null Tests
// ============================================================================

#[test]
fn test_bool_true() {
    let tokens = lex_ok("true");
    assert_eq!(kinds(&tokens), vec![TokenKind::Bool(true), TokenKind::Eof]);
}

#[test]
fn test_bool_false() {
    let tokens = lex_ok("false");
    assert_eq!(kinds(&tokens), vec![TokenKind::Bool(false), TokenKind::Eof]);
}

#[test]
fn test_null() {
    let tokens = lex_ok("null");
    assert_eq!(kinds(&tokens), vec![TokenKind::Null, TokenKind::Eof]);
}

// ============================================================================
// Keyword Tests
// ============================================================================

#[test]
fn test_keyword_let() {
    let tokens = lex_ok("let");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordLet, TokenKind::Eof]);
}

#[test]
fn test_keyword_mut() {
    let tokens = lex_ok("mut");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordMut, TokenKind::Eof]);
}

#[test]
fn test_keyword_const() {
    let tokens = lex_ok("const");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordConst, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_fn() {
    let tokens = lex_ok("fn");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordFn, TokenKind::Eof]);
}

#[test]
fn test_keyword_return() {
    let tokens = lex_ok("return");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordReturn, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_if() {
    let tokens = lex_ok("if");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordIf, TokenKind::Eof]);
}

#[test]
fn test_keyword_else() {
    let tokens = lex_ok("else");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordElse, TokenKind::Eof]);
}

#[test]
fn test_keyword_while() {
    let tokens = lex_ok("while");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordWhile, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_for() {
    let tokens = lex_ok("for");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordFor, TokenKind::Eof]);
}

#[test]
fn test_keyword_in() {
    let tokens = lex_ok("in");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordIn, TokenKind::Eof]);
}

#[test]
fn test_keyword_class() {
    let tokens = lex_ok("class");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordClass, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_interface() {
    let tokens = lex_ok("interface");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordInterface, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_enum() {
    let tokens = lex_ok("enum");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordEnum, TokenKind::Eof]);
}

#[test]
fn test_keyword_import() {
    let tokens = lex_ok("import");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordImport, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_from() {
    let tokens = lex_ok("from");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordFrom, TokenKind::Eof]);
}

#[test]
fn test_keyword_export() {
    let tokens = lex_ok("export");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordExport, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_async() {
    let tokens = lex_ok("async");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordAsync, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_await() {
    let tokens = lex_ok("await");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordAwait, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_try() {
    let tokens = lex_ok("try");
    assert_eq!(kinds(&tokens), vec![TokenKind::KeywordTry, TokenKind::Eof]);
}

#[test]
fn test_keyword_catch() {
    let tokens = lex_ok("catch");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordCatch, TokenKind::Eof]
    );
}

#[test]
fn test_keyword_throw() {
    let tokens = lex_ok("throw");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::KeywordThrow, TokenKind::Eof]
    );
}

// ============================================================================
// Operator Tests
// ============================================================================

#[test]
fn test_operator_plus() {
    let tokens = lex_ok("+");
    assert_eq!(kinds(&tokens), vec![TokenKind::Plus, TokenKind::Eof]);
}

#[test]
fn test_operator_minus() {
    let tokens = lex_ok("-");
    assert_eq!(kinds(&tokens), vec![TokenKind::Minus, TokenKind::Eof]);
}

#[test]
fn test_operator_star() {
    let tokens = lex_ok("*");
    assert_eq!(kinds(&tokens), vec![TokenKind::Star, TokenKind::Eof]);
}

#[test]
fn test_operator_slash() {
    let tokens = lex_ok("/");
    assert_eq!(kinds(&tokens), vec![TokenKind::Slash, TokenKind::Eof]);
}

#[test]
fn test_operator_percent() {
    let tokens = lex_ok("%");
    assert_eq!(kinds(&tokens), vec![TokenKind::Percent, TokenKind::Eof]);
}

#[test]
fn test_operator_equal() {
    let tokens = lex_ok("=");
    assert_eq!(kinds(&tokens), vec![TokenKind::Equal, TokenKind::Eof]);
}

#[test]
fn test_operator_equal_equal() {
    let tokens = lex_ok("==");
    assert_eq!(kinds(&tokens), vec![TokenKind::EqualEqual, TokenKind::Eof]);
}

#[test]
fn test_operator_bang() {
    let tokens = lex_ok("!");
    assert_eq!(kinds(&tokens), vec![TokenKind::Bang, TokenKind::Eof]);
}

#[test]
fn test_operator_bang_equal() {
    let tokens = lex_ok("!=");
    assert_eq!(kinds(&tokens), vec![TokenKind::BangEqual, TokenKind::Eof]);
}

#[test]
fn test_operator_less() {
    let tokens = lex_ok("<");
    assert_eq!(kinds(&tokens), vec![TokenKind::Less, TokenKind::Eof]);
}

#[test]
fn test_operator_less_equal() {
    let tokens = lex_ok("<=");
    assert_eq!(kinds(&tokens), vec![TokenKind::LessEqual, TokenKind::Eof]);
}

#[test]
fn test_operator_greater() {
    let tokens = lex_ok(">");
    assert_eq!(kinds(&tokens), vec![TokenKind::Greater, TokenKind::Eof]);
}

#[test]
fn test_operator_greater_equal() {
    let tokens = lex_ok(">=");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::GreaterEqual, TokenKind::Eof]
    );
}

#[test]
fn test_operator_and_and() {
    let tokens = lex_ok("&&");
    assert_eq!(kinds(&tokens), vec![TokenKind::AndAnd, TokenKind::Eof]);
}

#[test]
fn test_operator_or_or() {
    let tokens = lex_ok("||");
    assert_eq!(kinds(&tokens), vec![TokenKind::OrOr, TokenKind::Eof]);
}

#[test]
fn test_operator_arrow() {
    let tokens = lex_ok("->");
    assert_eq!(kinds(&tokens), vec![TokenKind::Arrow, TokenKind::Eof]);
}

#[test]
fn test_operator_fat_arrow() {
    let tokens = lex_ok("=>");
    assert_eq!(kinds(&tokens), vec![TokenKind::FatArrow, TokenKind::Eof]);
}

#[test]
fn test_operator_pipeline() {
    let tokens = lex_ok("|>");
    assert_eq!(kinds(&tokens), vec![TokenKind::Pipeline, TokenKind::Eof]);
}

#[test]
fn test_operator_question() {
    let tokens = lex_ok("?");
    assert_eq!(kinds(&tokens), vec![TokenKind::Question, TokenKind::Eof]);
}

#[test]
fn test_operator_question_dot() {
    let tokens = lex_ok("?.");
    assert_eq!(kinds(&tokens), vec![TokenKind::QuestionDot, TokenKind::Eof]);
}

#[test]
fn test_operator_null_coalesce() {
    let tokens = lex_ok("??");
    assert_eq!(
        kinds(&tokens),
        vec![TokenKind::NullCoalesce, TokenKind::Eof]
    );
}

#[test]
fn test_operator_plus_plus() {
    let tokens = lex_ok("++");
    assert_eq!(kinds(&tokens), vec![TokenKind::PlusPlus, TokenKind::Eof]);
}

#[test]
fn test_operator_minus_minus() {
    let tokens = lex_ok("--");
    assert_eq!(kinds(&tokens), vec![TokenKind::MinusMinus, TokenKind::Eof]);
}

// ============================================================================
// Punctuation Tests
// ============================================================================

#[test]
fn test_punctuation_lparen() {
    let tokens = lex_ok("(");
    assert_eq!(kinds(&tokens), vec![TokenKind::LParen, TokenKind::Eof]);
}

#[test]
fn test_punctuation_rparen() {
    let tokens = lex_ok(")");
    assert_eq!(kinds(&tokens), vec![TokenKind::RParen, TokenKind::Eof]);
}

#[test]
fn test_punctuation_lbrace() {
    let tokens = lex_ok("{");
    assert_eq!(kinds(&tokens), vec![TokenKind::LBrace, TokenKind::Eof]);
}

#[test]
fn test_punctuation_rbrace() {
    let tokens = lex_ok("}");
    assert_eq!(kinds(&tokens), vec![TokenKind::RBrace, TokenKind::Eof]);
}

#[test]
fn test_punctuation_lbracket() {
    let tokens = lex_ok("[");
    assert_eq!(kinds(&tokens), vec![TokenKind::LBracket, TokenKind::Eof]);
}

#[test]
fn test_punctuation_rbracket() {
    let tokens = lex_ok("]");
    assert_eq!(kinds(&tokens), vec![TokenKind::RBracket, TokenKind::Eof]);
}

#[test]
fn test_punctuation_comma() {
    let tokens = lex_ok(",");
    assert_eq!(kinds(&tokens), vec![TokenKind::Comma, TokenKind::Eof]);
}

#[test]
fn test_punctuation_semicolon() {
    let tokens = lex_ok(";");
    assert_eq!(kinds(&tokens), vec![TokenKind::Semicolon, TokenKind::Eof]);
}

#[test]
fn test_punctuation_colon() {
    let tokens = lex_ok(":");
    assert_eq!(kinds(&tokens), vec![TokenKind::Colon, TokenKind::Eof]);
}

#[test]
fn test_punctuation_dot() {
    let tokens = lex_ok(".");
    assert_eq!(kinds(&tokens), vec![TokenKind::Dot, TokenKind::Eof]);
}

#[test]
fn test_punctuation_dot_dot() {
    let tokens = lex_ok("..");
    assert_eq!(kinds(&tokens), vec![TokenKind::DotDot, TokenKind::Eof]);
}

#[test]
fn test_punctuation_dot_dot_dot() {
    let tokens = lex_ok("...");
    assert_eq!(kinds(&tokens), vec![TokenKind::Ellipsis, TokenKind::Eof]);
}

#[test]
fn test_punctuation_at() {
    let tokens = lex_ok("@");
    assert_eq!(kinds(&tokens), vec![TokenKind::At, TokenKind::Eof]);
}

#[test]
fn test_punctuation_hash() {
    let tokens = lex_ok("#");
    assert_eq!(kinds(&tokens), vec![TokenKind::Hash, TokenKind::Eof]);
}

// ============================================================================
// Comment Tests
// ============================================================================

#[test]
fn test_line_comment() {
    let tokens = lex_ok("// this is a comment");
    assert_eq!(tokens.len(), 2);
    assert!(
        matches!(&tokens[0].kind, TokenKind::Comment(text, CommentKind::Line) if text == " this is a comment")
    );
}

#[test]
fn test_block_comment() {
    let tokens = lex_ok("/* block comment */");
    assert_eq!(tokens.len(), 2);
    assert!(
        matches!(&tokens[0].kind, TokenKind::Comment(text, CommentKind::Block) if text == " block comment ")
    );
}

#[test]
fn test_doc_comment() {
    let tokens = lex_ok("/// doc comment");
    assert_eq!(tokens.len(), 2);
    assert!(
        matches!(&tokens[0].kind, TokenKind::Comment(text, CommentKind::Doc) if text == " doc comment")
    );
}

// ============================================================================
// Complex Expression Tests
// ============================================================================

#[test]
fn test_simple_expression() {
    let tokens = lex_ok("1 + 2");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::Int(1),
            TokenKind::Plus,
            TokenKind::Int(2),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_function_call() {
    let tokens = lex_ok("foo(x, y)");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::Ident("foo".to_string()),
            TokenKind::LParen,
            TokenKind::Ident("x".to_string()),
            TokenKind::Comma,
            TokenKind::Ident("y".to_string()),
            TokenKind::RParen,
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_variable_declaration() {
    let tokens = lex_ok("let x = 42");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::KeywordLet,
            TokenKind::Ident("x".to_string()),
            TokenKind::Equal,
            TokenKind::Int(42),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_function_definition() {
    let tokens = lex_ok("fn add(a, b) { return a + b }");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::KeywordFn,
            TokenKind::Ident("add".to_string()),
            TokenKind::LParen,
            TokenKind::Ident("a".to_string()),
            TokenKind::Comma,
            TokenKind::Ident("b".to_string()),
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::KeywordReturn,
            TokenKind::Ident("a".to_string()),
            TokenKind::Plus,
            TokenKind::Ident("b".to_string()),
            TokenKind::RBrace,
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_class_definition() {
    let tokens = lex_ok("class Foo extends Bar {}");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::KeywordClass,
            TokenKind::Ident("Foo".to_string()),
            TokenKind::KeywordExtends,
            TokenKind::Ident("Bar".to_string()),
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_array_literal() {
    let tokens = lex_ok("[1, 2, 3]");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::LBracket,
            TokenKind::Int(1),
            TokenKind::Comma,
            TokenKind::Int(2),
            TokenKind::Comma,
            TokenKind::Int(3),
            TokenKind::RBracket,
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_map_literal() {
    let tokens = lex_ok(r#"{ "a": 1 }"#);
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::LBrace,
            TokenKind::String("a".to_string()),
            TokenKind::Colon,
            TokenKind::Int(1),
            TokenKind::RBrace,
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_pipeline_expression() {
    let tokens = lex_ok("data |> map(f) |> filter(g)");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::Ident("data".to_string()),
            TokenKind::Pipeline,
            TokenKind::Ident("map".to_string()),
            TokenKind::LParen,
            TokenKind::Ident("f".to_string()),
            TokenKind::RParen,
            TokenKind::Pipeline,
            TokenKind::Ident("filter".to_string()),
            TokenKind::LParen,
            TokenKind::Ident("g".to_string()),
            TokenKind::RParen,
            TokenKind::Eof
        ]
    );
}

// ============================================================================
// Error Tests
// ============================================================================

#[test]
fn test_error_unterminated_string() {
    let result = lex(r#""hello"#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_incomplete());
}

#[test]
fn test_error_unterminated_char() {
    let result = lex("'a");
    assert!(result.is_err());
}

#[test]
fn test_error_invalid_char_literal() {
    let result = lex("''");
    assert!(result.is_err());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_adjacent_operators() {
    let tokens = lex_ok("a++b");
    // Should be: a, ++, b
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::Ident("a".to_string()),
            TokenKind::PlusPlus,
            TokenKind::Ident("b".to_string()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_multiline_code() {
    let tokens = lex_ok("let x = 1\nlet y = 2");
    assert_eq!(
        kinds(&tokens),
        vec![
            TokenKind::KeywordLet,
            TokenKind::Ident("x".to_string()),
            TokenKind::Equal,
            TokenKind::Int(1),
            TokenKind::KeywordLet,
            TokenKind::Ident("y".to_string()),
            TokenKind::Equal,
            TokenKind::Int(2),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_span_tracking() {
    let tokens = lex_ok("let x = 1");
    // Check that spans are correctly tracked
    assert_eq!(tokens[0].span.start, 0); // 'let' starts at 0
    assert_eq!(tokens[0].span.end, 3); // 'let' ends at 3
}

#[test]
fn test_unicode_identifier() {
    // Pleno should support unicode identifiers if configured
    let tokens = lex_ok("πValue");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].kind, TokenKind::Ident(name) if name == "πValue"));
}

#[test]
fn a_leading_byte_order_mark_is_ignored() {
    // Windows editors (Notepad, PowerShell's Out-File) can start a file with one.
    let tokens = pleno_lexer::lex("\u{feff}let x = 1;").unwrap();
    assert!(matches!(tokens[0].kind, pleno_lexer::TokenKind::KeywordLet));
    // Only at the start: elsewhere it is still an unexpected character.
    assert!(pleno_lexer::lex("let x = \u{feff}1;").is_err());
}

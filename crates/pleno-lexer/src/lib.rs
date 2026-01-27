use pleno_ast::CommentKind;
use pleno_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Null,
    KeywordLet,
    KeywordMut,
    KeywordConst,
    KeywordFn,
    KeywordReturn,
    KeywordIf,
    KeywordElse,
    KeywordWhile,
    KeywordFor,
    KeywordIn,
    KeywordBreak,
    KeywordContinue,
    KeywordExtends,
    KeywordStatic,
    KeywordPublic,
    KeywordPrivate,
    KeywordProtected,
    KeywordInternal,
    KeywordImport,
    KeywordFrom,
    KeywordAs,
    KeywordExport,
    KeywordModule,
    KeywordClass,
    KeywordThrow,
    KeywordTry,
    KeywordCatch,
    KeywordFinally,
    KeywordMatch,
    KeywordCase,
    KeywordNew,
    KeywordThis,
    KeywordSuper,
    KeywordAsync,
    KeywordAwait,
    KeywordDo,
    KeywordAbstract,
    KeywordInterface,
    KeywordImplements,
    KeywordProperty,
    KeywordGet,
    KeywordSet,
    KeywordDeep,
    KeywordEnum,
    KeywordType,
    KeywordExtend,
    KeywordIs,
    KeywordOperator,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    DotDot,
    DotDotEqual,
    Ellipsis,
    Plus,
    Minus,
    Star,
    Slash,
    AndAnd,
    OrOr,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    At,
    Underscore,
    Arrow,
    FatArrow,
    Pipe,
    Pipeline,
    Question,
    QuestionDot,
    NullCoalesce,
    Percent,
    Backtick,
    Dollar,
    Tilda,
    Caret,
    Hash,
    Comment(String, CommentKind),
    Ampersand,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl LexError {
    pub fn is_incomplete(&self) -> bool {
        self.message.contains("Unterminated")
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }

        let token = match ch {
            '(' => token_simple(TokenKind::LParen, start),
            ')' => token_simple(TokenKind::RParen, start),
            '{' => token_simple(TokenKind::LBrace, start),
            '}' => token_simple(TokenKind::RBrace, start),
            '[' => token_simple(TokenKind::LBracket, start),
            ']' => token_simple(TokenKind::RBracket, start),
            ',' => token_simple(TokenKind::Comma, start),
            ';' => token_simple(TokenKind::Semicolon, start),
            ':' => token_simple(TokenKind::Colon, start),
            '.' => {
                if matches!(chars.peek(), Some((_, '.'))) {
                    chars.next();
                    if matches!(chars.peek(), Some((_, '.'))) {
                        let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 3);
                        Token {
                            kind: TokenKind::Ellipsis,
                            span: Span::new(start, end),
                        }
                    } else if matches!(chars.peek(), Some((_, '='))) {
                        let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                        Token {
                            kind: TokenKind::DotDotEqual,
                            span: Span::new(start, end),
                        }
                    } else {
                        let end = chars.peek().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                        Token {
                            kind: TokenKind::DotDot,
                            span: Span::new(start, end),
                        }
                    }
                } else {
                    token_simple(TokenKind::Dot, start)
                }
            }
            '+' => token_simple(TokenKind::Plus, start),
            '-' => {
                if matches!(chars.peek(), Some((_, '>'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::Arrow,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Minus, start)
                }
            }
            '*' => token_simple(TokenKind::Star, start),
            '%' => token_simple(TokenKind::Percent, start),
            '/' => {
                if matches!(chars.peek(), Some((_, '/'))) {
                    chars.next();
                    let kind = if matches!(chars.peek(), Some((_, '/'))) {
                        chars.next();
                        CommentKind::Doc
                    } else {
                        CommentKind::Line
                    };
                    let mut content = String::new();
                    while let Some((_, next_ch)) = chars.peek() {
                        if *next_ch == '\n' {
                            break;
                        }
                        content.push(*next_ch);
                        chars.next();
                    }
                    let end = chars.peek().map(|(idx, _)| *idx).unwrap_or(source.len());
                    Token {
                        kind: TokenKind::Comment(content, kind),
                        span: Span::new(start, end),
                    }
                } else if matches!(chars.peek(), Some((_, '*'))) {
                    chars.next();
                    let mut content = String::new();
                    let mut terminated = false;
                    while let Some((_, next_ch)) = chars.next() {
                        if next_ch == '*' && matches!(chars.peek(), Some((_, '/'))) {
                            chars.next();
                            terminated = true;
                            break;
                        }
                        content.push(next_ch);
                    }
                    if !terminated {
                        return Err(LexError {
                            message: "Unterminated multi-line comment".to_string(),
                            span: Span::new(start, start + 2),
                        });
                    }
                    let end = chars.peek().map(|(idx, _)| *idx).unwrap_or(source.len());
                    Token {
                        kind: TokenKind::Comment(content, CommentKind::Block),
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Slash, start)
                }
            }
            '@' => token_simple(TokenKind::At, start),
            '|' => {
                if matches!(chars.peek(), Some((_, '|'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::OrOr,
                        span: Span::new(start, end),
                    }
                } else if matches!(chars.peek(), Some((_, '>'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::Pipeline,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Pipe, start)
                }
            }
            '`' => token_simple(TokenKind::Backtick, start),
            '$' => token_simple(TokenKind::Dollar, start),
            '~' => token_simple(TokenKind::Tilda, start),
            '^' => token_simple(TokenKind::Caret, start),
            '#' => token_simple(TokenKind::Hash, start),
            '&' => {
                if matches!(chars.peek(), Some((_, '&'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::AndAnd,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Ampersand, start)
                }
            }
            '=' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::EqualEqual,
                        span: Span::new(start, end),
                    }
                } else if matches!(chars.peek(), Some((_, '>'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::FatArrow,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Equal, start)
                }
            }
            '<' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 1);
                    Token {
                        kind: TokenKind::LessEqual,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Less, start)
                }
            }
            '>' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 1);
                    Token {
                        kind: TokenKind::GreaterEqual,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Greater, start)
                }
            }
            '!' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::BangEqual,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Bang, start)
                }
            }
            '?' => {
                if matches!(chars.peek(), Some((_, '.'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::QuestionDot,
                        span: Span::new(start, end),
                    }
                } else if matches!(chars.peek(), Some((_, '?'))) {
                    let end = chars.next().map(|(idx, _)| idx + 1).unwrap_or(start + 2);
                    Token {
                        kind: TokenKind::NullCoalesce,
                        span: Span::new(start, end),
                    }
                } else {
                    token_simple(TokenKind::Question, start)
                }
            }
            '"' => {
                let (value, end) = lex_string(&mut chars, start)?;
                Token {
                    kind: TokenKind::String(value),
                    span: Span::new(start, end),
                }
            }
            '\'' => {
                let (value, end) = lex_char(&mut chars, start)?;
                Token {
                    kind: TokenKind::Char(value),
                    span: Span::new(start, end),
                }
            }
            _ if ch.is_ascii_digit() => {
                let (kind, end) = lex_number(source, start, ch, &mut chars)?;
                Token {
                    kind,
                    span: Span::new(start, end),
                }
            }
            _ if ch == 'r' && matches!(chars.peek(), Some((_, '"'))) => {
                chars.next(); // consume '"'
                let mut value = String::new();
                let mut terminated = false;
                while let Some((idx, next_ch)) = chars.next() {
                    if next_ch == '"' {
                        terminated = true;
                        tokens.push(Token {
                            kind: TokenKind::String(value),
                            span: Span::new(start, idx + 1),
                        });
                        break;
                    }
                    value.push(next_ch);
                }
                if !terminated {
                    return Err(LexError {
                        message: "Unterminated raw string literal".to_string(),
                        span: Span::new(start, start + 1),
                    });
                }
                continue;
            }
            _ if is_ident_start(ch) => {
                let (ident, end) = lex_ident(source, start, ch, &mut chars);
                let kind = match ident.as_str() {
                    "let" => TokenKind::KeywordLet,
                    "mut" => TokenKind::KeywordMut,
                    "const" => TokenKind::KeywordConst,
                    "fn" => TokenKind::KeywordFn,
                    "return" => TokenKind::KeywordReturn,
                    "if" => TokenKind::KeywordIf,
                    "else" => TokenKind::KeywordElse,
                    "while" => TokenKind::KeywordWhile,
                    "for" => TokenKind::KeywordFor,
                    "in" => TokenKind::KeywordIn,
                    "break" => TokenKind::KeywordBreak,
                    "continue" => TokenKind::KeywordContinue,
                    "extends" => TokenKind::KeywordExtends,
                    "static" => TokenKind::KeywordStatic,
                    "public" => TokenKind::KeywordPublic,
                    "private" => TokenKind::KeywordPrivate,
                    "protected" => TokenKind::KeywordProtected,
                    "internal" => TokenKind::KeywordInternal,
                    "import" => TokenKind::KeywordImport,
                    "from" => TokenKind::KeywordFrom,
                    "as" => TokenKind::KeywordAs,
                    "export" => TokenKind::KeywordExport,
                    "module" => TokenKind::KeywordModule,
                    "class" => TokenKind::KeywordClass,
                    "throw" => TokenKind::KeywordThrow,
                    "try" => TokenKind::KeywordTry,
                    "catch" => TokenKind::KeywordCatch,
                    "finally" => TokenKind::KeywordFinally,
                    "match" => TokenKind::KeywordMatch,
                    "case" => TokenKind::KeywordCase,
                    "new" => TokenKind::KeywordNew,
                    "this" => TokenKind::KeywordThis,
                    "super" => TokenKind::KeywordSuper,
                    "async" => TokenKind::KeywordAsync,
                    "await" => TokenKind::KeywordAwait,
                    "do" => TokenKind::KeywordDo,
                    "abstract" => TokenKind::KeywordAbstract,
                    "enum" => TokenKind::KeywordEnum,
                    "type" => TokenKind::KeywordType,
                    "is" => TokenKind::KeywordIs,
                    "interface" => TokenKind::KeywordInterface,
                    "extend" => TokenKind::KeywordExtend,
                    "implements" => TokenKind::KeywordImplements,
                    "property" => TokenKind::KeywordProperty,
                    "get" => TokenKind::KeywordGet,
                    "set" => TokenKind::KeywordSet,
                    "deep" => TokenKind::KeywordDeep,
                    "operator" => TokenKind::KeywordOperator,
                    "true" => TokenKind::Bool(true),
                    "false" => TokenKind::Bool(false),
                    "null" => TokenKind::Null,
                    "_" => TokenKind::Underscore,
                    _ => TokenKind::Ident(ident),
                };
                Token {
                    kind,
                    span: Span::new(start, end),
                }
            }
            _ => {
                return Err(LexError {
                    message: format!("Unexpected character: {ch}"),
                    span: Span::new(start, start + ch.len_utf8()),
                });
            }
        };

        tokens.push(token);
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(source.len(), source.len()),
    });

    Ok(tokens)
}

fn token_simple(kind: TokenKind, start: usize) -> Token {
    Token {
        kind,
        span: Span::new(start, start + 1),
    }
}

fn lex_string(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    start: usize,
) -> Result<(String, usize), LexError> {
    let mut value = String::new();
    while let Some((idx, ch)) = chars.next() {
        if ch == '"' {
            return Ok((value, idx + 1));
        }
        if ch == '\\' {
            if let Some((_, esc)) = chars.next() {
                value.push(match esc {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
            } else {
                return Err(LexError {
                    message: "Unterminated string escape".to_string(),
                    span: Span::new(start, start + 1),
                });
            }
        } else {
            value.push(ch);
        }
    }

    Err(LexError {
        message: "Unterminated string literal".to_string(),
        span: Span::new(start, start + 1),
    })
}

fn lex_char(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    start: usize,
) -> Result<(char, usize), LexError> {
    let ch = match chars.next() {
        Some((_, ch)) => {
            if ch == '\\' {
                // Handle escape sequences
                match chars.next() {
                    Some((_, esc)) => match esc {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '\'' => '\'',
                        '\\' => '\\',
                        other => other,
                    },
                    None => {
                        return Err(LexError {
                            message: "Unterminated char escape".to_string(),
                            span: Span::new(start, start + 1),
                        });
                    }
                }
            } else {
                ch
            }
        }
        None => {
            return Err(LexError {
                message: "Unterminated char literal".to_string(),
                span: Span::new(start, start + 1),
            });
        }
    };

    match chars.next() {
        Some((end, '\'')) => Ok((ch, end + 1)),
        Some((_, other)) => Err(LexError {
            message: format!("Expected ' after char, found '{}'", other),
            span: Span::new(start, start + 1),
        }),
        None => Err(LexError {
            message: "Unterminated char literal".to_string(),
            span: Span::new(start, start + 1),
        }),
    }
}

fn lex_number(
    source: &str,
    start: usize,
    first: char,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<(TokenKind, usize), LexError> {
    let mut end = start + first.len_utf8();
    let mut has_dot = false;

    while let Some((idx, ch)) = chars.peek().copied() {
        if ch == '.' && !has_dot {
            let mut lookahead = chars.clone();
            lookahead.next();
            if matches!(lookahead.peek(), Some((_, '.'))) {
                break;
            }
            has_dot = true;
            chars.next();
            end = idx + 1;
            continue;
        }
        if ch.is_ascii_digit() {
            chars.next();
            end = idx + 1;
            continue;
        }
        break;
    }

    let slice = &source[start..end];
    if has_dot {
        let value = slice.parse::<f64>().map_err(|_| LexError {
            message: format!("Invalid float literal: {slice}"),
            span: Span::new(start, end),
        })?;
        Ok((TokenKind::Float(value), end))
    } else {
        let value = slice.parse::<i64>().map_err(|_| LexError {
            message: format!("Invalid integer literal: {slice}"),
            span: Span::new(start, end),
        })?;
        Ok((TokenKind::Int(value), end))
    }
}

fn lex_ident(
    source: &str,
    start: usize,
    first: char,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> (String, usize) {
    let mut end = start + first.len_utf8();
    while let Some((idx, ch)) = chars.peek().copied() {
        if is_ident_continue(ch) {
            chars.next();
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    (source[start..end].to_string(), end)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

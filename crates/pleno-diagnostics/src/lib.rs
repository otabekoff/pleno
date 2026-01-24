use pleno_common::Span;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "Error"),
            Severity::Warning => write!(f, "Warning"),
            Severity::Info => write!(f, "Info"),
            Severity::Hint => write!(f, "Hint"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub severity: Severity,
    pub code: Option<String>,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Error,
            code: None,
            hint: None,
        }
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

pub struct DiagnosticReporter;

impl DiagnosticReporter {
    pub fn report(diagnostic: &Diagnostic, source: &str) -> String {
        let (line_num, col_num) = Self::get_line_col(source, diagnostic.span.start);
        let line_content = Self::get_line_content(source, line_num);

        let mut output = String::new();

        // Header
        let severity_color = match diagnostic.severity {
            Severity::Error => "\x1b[31m",   // Red
            Severity::Warning => "\x1b[33m", // Yellow
            _ => "\x1b[34m",                 // Blue
        };
        let reset = "\x1b[0m";

        output.push_str(&format!(
            "{}{}{}: {}\n",
            severity_color, diagnostic.severity, reset, diagnostic.message
        ));

        if let Some(code) = &diagnostic.code {
            output.push_str(&format!("  --> [{}]\n", code));
        }

        // Location
        output.push_str(&format!("  --> per line {}:{}\n", line_num, col_num));
        output.push_str("   |\n");
        output.push_str(&format!(" {:<3} | {}\n", line_num, line_content));

        // Underline
        let line_num_str = format!("{}", line_num);
        let padding_width = std::cmp::max(3, line_num_str.len()) + 3 + col_num - 1;
        let padding = " ".repeat(padding_width);
        let length = diagnostic.span.end - diagnostic.span.start;
        let underline = "^".repeat(std::cmp::max(1, length));
        output.push_str(&format!(
            "{}{}{}{}\n",
            padding, severity_color, underline, reset
        ));

        if let Some(hint) = &diagnostic.hint {
            output.push_str(&format!("   = hint: {}\n", hint));
        }

        output
    }

    fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, c) in source.char_indices() {
            if i >= offset {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn get_line_content(source: &str, line_target: usize) -> String {
        source
            .lines()
            .nth(line_target - 1)
            .unwrap_or("")
            .to_string()
    }
}

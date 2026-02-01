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
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            code: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

pub struct DiagnosticReporter;

impl DiagnosticReporter {
    pub fn report(diagnostic: &Diagnostic, source: &str) -> String {
        let mut output = String::new();

        // Header colors
        let severity_color = match diagnostic.severity {
            Severity::Error => "\x1b[31;1m",   // Bold Red
            Severity::Warning => "\x1b[33;1m", // Bold Yellow
            Severity::Info => "\x1b[34;1m",    // Bold Blue
            Severity::Hint => "\x1b[36;1m",    // Bold Cyan
        };
        let reset = "\x1b[0m";

        // Pre-calculate line info and max width
        let mut label_infos = Vec::new();
        let mut max_width = 0;

        for label in &diagnostic.labels {
            let (line_num, col_num) = Self::get_line_col(source, label.span.start);
            let line_content = Self::get_line_content(source, line_num);
            let width = line_num.to_string().len();
            if width > max_width {
                max_width = width;
            }
            label_infos.push((line_num, col_num, line_content));
        }

        if max_width < 1 {
            max_width = 1;
        }

        // 1. Message Header
        output.push_str(&format!(
            "{}{}{}: {}{}\n",
            severity_color, diagnostic.severity, reset, diagnostic.message, reset
        ));

        // 2. Location Info (Arrow)
        if let Some(first_label) = label_infos.first() {
            let (line_num, col_num, _) = first_label;
            let arrow_indent = " ".repeat(max_width + 1);
            output.push_str(&format!(
                "{}-->{} {}:{}:{}\n",
                arrow_indent, reset, "file", line_num, col_num
            ));
        }

        // 3. Optional Error Code
        if let Some(code) = &diagnostic.code {
            let code_indent = " ".repeat(max_width + 1);
            output.push_str(&format!(
                "{}-->{} [{}] {}\n",
                code_indent, reset, code, reset
            ));
        }

        // 4. Labels and Source Code
        let gutter_indent = " ".repeat(max_width + 1);
        for (i, label) in diagnostic.labels.iter().enumerate() {
            let (line_num, col_num, line_content) = &label_infos[i];

            output.push_str(&format!("{} |\n", gutter_indent));
            output.push_str(&format!(
                " {:>width$} | {}\n",
                line_num,
                line_content,
                width = max_width
            ));

            // Underline
            let underline_padding = " ".repeat(*col_num - 1);
            let length = if label.span.end > label.span.start {
                label.span.end - label.span.start
            } else {
                1
            };

            let max_len = line_content.len().saturating_sub(col_num - 1);
            let length = std::cmp::min(length, max_len);
            let underline = "^".repeat(std::cmp::max(1, length));

            output.push_str(&format!(
                "{} | {}{}{} {}\n",
                gutter_indent, underline_padding, severity_color, underline, label.message
            ));
        }

        // 5. Notes
        if !diagnostic.notes.is_empty() {
            output.push_str(&format!("{} |\n", gutter_indent));
            for note in &diagnostic.notes {
                output.push_str(&format!("{} = note: {}\n", gutter_indent, note));
            }
        }

        output.push_str(reset);
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

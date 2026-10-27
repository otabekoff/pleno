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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    source: String,
    line_starts: Vec<usize>,
    pub source_name: String,
}

impl SourceMap {
    pub fn new(source_name: String, source: String) -> Self {
        let mut line_starts = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            source,
            line_starts,
            source_name,
        }
    }

    /// The whole source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn get_line_col(&self, offset: usize) -> (usize, usize) {
        // Binary search to find the line
        let line = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx + 1, // Exact match means start of next line, so it's that line index + 1
            Err(idx) => idx,    // Insertion point is the next line index, so current is idx
        };

        let line_start = self.line_starts[line - 1];
        let col = offset - line_start + 1;
        (line, col)
    }

    pub fn get_line_content(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line - 1];
        let end = if line == self.line_starts.len() {
            self.source.len()
        } else {
            self.line_starts[line] - 1 // Exclude newline
        };

        // Handle case where file might end with newline or not
        if start > self.source.len() {
            return Some("");
        }

        let slice = &self.source[start..std::cmp::min(end, self.source.len())];
        // clear carriage return if present
        Some(slice.trim_end_matches('\r'))
    }
}

pub struct DiagnosticReporter;

impl DiagnosticReporter {
    pub fn report(diagnostic: &Diagnostic, source_map: &SourceMap) -> String {
        let mut output = String::new();

        // Colored only when stderr, where reports are printed, is a terminal.
        let color = pleno_common::style::enabled(pleno_common::style::Stream::Stderr);
        let code = |escape: &'static str| if color { escape } else { "" };
        let severity_color = code(match diagnostic.severity {
            Severity::Error => "\x1b[31;1m",   // Bold Red
            Severity::Warning => "\x1b[33;1m", // Bold Yellow
            Severity::Info => "\x1b[34;1m",    // Bold Blue
            Severity::Hint => "\x1b[36;1m",    // Bold Cyan
        });
        let reset = code("\x1b[0m");
        let bold = code("\x1b[1m");
        let blue = code("\x1b[34m");

        // Pre-calculate line info and max width
        let mut label_infos = Vec::new();
        let mut max_line_num_width = 0;

        for label in &diagnostic.labels {
            let (line_num, col_num) = source_map.get_line_col(label.span.start);
            let line_content = source_map.get_line_content(line_num).unwrap_or("");
            let width = line_num.to_string().len();
            if width > max_line_num_width {
                max_line_num_width = width;
            }
            label_infos.push((line_num, col_num, line_content));
        }

        if max_line_num_width < 1 {
            max_line_num_width = 1;
        }

        // 1. Message Header
        output.push_str(&format!(
            "{}{}{}: {}{}{}\n",
            severity_color, diagnostic.severity, reset, bold, diagnostic.message, reset
        ));

        // 2. Location Info
        if let Some(first_label) = label_infos.first() {
            let (line_num, col_num, _) = first_label;
            let pad = " ".repeat(max_line_num_width);
            output.push_str(&format!(
                "{}{}-->{} {}:{}:{}\n",
                pad, blue, reset, source_map.source_name, line_num, col_num
            ));
        }

        // 3. Optional Error Code
        if let Some(code) = &diagnostic.code {
            let pad = " ".repeat(max_line_num_width);
            output.push_str(&format!("{}{} |{} [{}]\n", pad, blue, reset, code));
        }

        // 4. Labels and Source Code
        let gutter_indent = " ".repeat(max_line_num_width);

        // Group labels by line to handle multiple labels on same line (basic handling for now: just iterate)
        for (i, label) in diagnostic.labels.iter().enumerate() {
            let (line_num, col_num, line_content) = &label_infos[i];

            // Empty line before code
            output.push_str(&format!("{}{} |{}\n", gutter_indent, blue, reset));

            // Code line
            output.push_str(&format!(
                "{}{} |{} {}\n",
                line_num, blue, reset, line_content
            ));

            // Underline
            // Calculate underline length
            let length = if label.span.end > label.span.start {
                label.span.end - label.span.start
            } else {
                1
            };

            // Limit underline to line length primarily (simple handling spanning lines not fully implemented yet)
            let max_len = line_content.len().saturating_sub(col_num - 1);
            let length = std::cmp::min(length, max_len);

            let underline_padding = " ".repeat(*col_num - 1);
            let underline = "^".repeat(std::cmp::max(1, length));

            output.push_str(&format!(
                "{}{} |{} {}{}{} {} {}\n",
                gutter_indent,
                blue,
                reset,
                underline_padding,
                severity_color,
                underline,
                reset,
                label.message
            ));
        }

        // 5. Notes
        if !diagnostic.notes.is_empty() {
            output.push_str(&format!("{}{} |{}\n", gutter_indent, blue, reset));
            for note in &diagnostic.notes {
                output.push_str(&format!(
                    "{}{} = note:{} {}\n",
                    gutter_indent, blue, reset, note
                ));
            }
        }

        output
    }

    // Deprecated helpers removed in favor of SourceMap
}

pub trait ToDiagnostic {
    fn to_diagnostic(&self) -> Diagnostic;
}

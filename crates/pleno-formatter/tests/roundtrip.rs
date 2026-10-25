//! Formatting every Pleno file in the repository must keep it valid: the output
//! parses whenever the input does, it is the same program, formatting it again
//! changes nothing, and no line is longer than the limit unless a string or a
//! comment, which are never split, makes it so.

use pleno_formatter::{DEFAULT_MAX_LINE_LENGTH, FormatOptions, format_code};
use std::path::{Path, PathBuf};

fn parse(source: &str) -> Result<pleno_ast::Program, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .map_err(|e| e.message)
}

/// A value read back from a `{:?}` dump of the AST.
#[derive(Debug, PartialEq)]
enum Node {
    /// A number, a name such as `None`, or a quoted string or char.
    Atom(String),
    Struct(String, Vec<(String, Node)>),
    Tuple(String, Vec<Node>),
    List(Vec<Node>),
}

struct DumpParser<'a> {
    text: &'a [u8],
    pos: usize,
}

impl DumpParser<'_> {
    fn skip_space(&mut self) {
        while self.text.get(self.pos).is_some_and(u8::is_ascii_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.skip_space();
        if self.text.get(self.pos) == Some(&byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self) -> String {
        self.skip_space();
        let start = self.pos;
        match self.text.get(self.pos) {
            Some(&quote @ (b'"' | b'\'')) => {
                self.pos += 1;
                while let Some(&byte) = self.text.get(self.pos) {
                    self.pos += if byte == b'\\' { 2 } else { 1 };
                    if byte == quote {
                        break;
                    }
                }
            }
            _ => {
                while self
                    .text
                    .get(self.pos)
                    .is_some_and(|b| !b" \n\t{}()[],:".contains(b))
                {
                    self.pos += 1;
                }
            }
        }
        String::from_utf8_lossy(&self.text[start..self.pos]).into_owned()
    }

    fn items(&mut self, close: u8) -> Vec<Node> {
        let mut items = Vec::new();
        while !self.eat(close) {
            items.push(self.value());
            self.eat(b',');
        }
        items
    }

    fn value(&mut self) -> Node {
        if self.eat(b'[') {
            return Node::List(self.items(b']'));
        }
        let name = self.word();
        if self.eat(b'(') {
            return Node::Tuple(name, self.items(b')'));
        }
        if self.eat(b'{') {
            let mut fields = Vec::new();
            while !self.eat(b'}') {
                let field = self.word();
                self.eat(b':');
                fields.push((field, self.value()));
                self.eat(b',');
            }
            return Node::Struct(name, fields);
        }
        Node::Atom(name)
    }
}

/// Leaves out what only says how the program was laid out, and treats the
/// rewrites the formatter makes on purpose as no change: a single statement
/// put in braces, parentheses around a `match` subject, and the `public` of
/// a constructor, which is public anyway.
fn normalize(node: Node) -> Node {
    match node {
        Node::Struct(name, fields) => {
            let mut fields: Vec<(String, Node)> = fields
                .into_iter()
                .filter(|(field, _)| {
                    field != "span"
                        && !field.ends_with("_span")
                        && field != "leading_blank_lines"
                        && field != "trailing_comment"
                })
                .map(|(field, value)| match value {
                    // Trailing spaces in doc comments are trimmed with all others.
                    Node::Tuple(some, docs) if field == "doc" => {
                        let docs = docs
                            .into_iter()
                            .map(|doc| match doc {
                                Node::Atom(text) => {
                                    let mut text = text.replace(" \"", "\"");
                                    while text.contains(" \\n") {
                                        text = text.replace(" \\n", "\\n");
                                    }
                                    Node::Atom(text)
                                }
                                other => other,
                            })
                            .collect();
                        (field, Node::Tuple(some, docs))
                    }
                    value => (field, normalize(value)),
                })
                .collect();
            let field = |fields: &[(String, Node)], wanted: &str| {
                fields.iter().position(|(field, _)| field == wanted)
            };
            if name == "Stmt"
                && let Some(kind) = field(&fields, "kind")
                && let Node::Tuple(block, items) = &mut fields[kind].1
                && block == "Block"
                && let [Node::List(statements)] = items.as_mut_slice()
                && statements.len() == 1
            {
                return statements.remove(0);
            }
            if name == "Match"
                && let Some(subject) = field(&fields, "expr")
                && let Node::Struct(_, expr) = &mut fields[subject].1
                && let Some((_, Node::Tuple(paren, inner))) = expr.first_mut()
                && paren == "Paren"
                && inner.len() == 1
            {
                fields[subject].1 = inner.remove(0);
            }
            if name == "FunctionDecl"
                && fields.iter().any(|(field, value)| {
                    field == "name" && *value == Node::Atom("\"constructor\"".into())
                })
                && let Some(visibility) = field(&fields, "visibility")
            {
                fields[visibility].1 = Node::Atom("None".into());
            }
            Node::Struct(name, fields)
        }
        Node::Tuple(name, items) => Node::Tuple(name, items.into_iter().map(normalize).collect()),
        Node::List(items) => Node::List(items.into_iter().map(normalize).collect()),
        atom => atom,
    }
}

/// The program without source positions, blank lines and comments, so two
/// layouts of one program compare equal.
fn shape(program: &pleno_ast::Program) -> String {
    let dump = format!("{:?}", program.statements);
    let mut parser = DumpParser {
        text: dump.as_bytes(),
        pos: 0,
    };
    format!("{:?}", normalize(parser.value()))
}

fn pln_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            pln_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pln") {
            out.push(path);
        }
    }
}

/// Whether a line may be longer than the limit: it holds a string or a
/// comment, which the formatter never splits.
fn unsplittable(line: &str) -> bool {
    line.contains('"') || line.contains('`') || line.contains("//") || line.contains("/*")
}

#[test]
fn shape_ignores_layout_but_not_code() {
    let shape_of = |source: &str| shape(&parse(source).unwrap());
    assert_eq!(
        shape_of("let x = f(1, \"a, b\");  // c\n"),
        shape_of("\n\nlet x = f(\n    1,\n    \"a, b\",\n);\n")
    );
    assert_ne!(shape_of("let x = f(1, 2);"), shape_of("let x = f(2, 1);"));
    assert_ne!(shape_of("let x = \"a\";"), shape_of("let x = \"b\";"));
}

/// Formats `source` and records what goes wrong. Code that does not parse
/// (a test of syntax errors, a fragment in the docs) is skipped.
fn check(name: &str, source: &str, failures: &mut Vec<String>) {
    let options = FormatOptions::default();
    let Ok(original) = parse(source) else {
        return;
    };
    let formatted = match format_code(source, &options) {
        Ok(formatted) => formatted,
        Err(e) => {
            failures.push(format!("{name}: formatter failed: {e}"));
            return;
        }
    };
    let reparsed = match parse(&formatted) {
        Ok(program) => program,
        Err(e) => {
            failures.push(format!("{name}: formatted code does not parse: {e}"));
            return;
        }
    };
    let (before, after) = (shape(&original), shape(&reparsed));
    if before != after {
        let at = before
            .bytes()
            .zip(after.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(before.len().min(after.len()));
        let context = |text: &str| {
            let start = text.floor_char_boundary(at.saturating_sub(60));
            let end = text.floor_char_boundary((at + 60).min(text.len()));
            text[start..end].to_string()
        };
        failures.push(format!(
            "{name}: formatting changes the program\n  before: {}\n  after:  {}",
            context(&before),
            context(&after)
        ));
    }
    for (i, line) in formatted.lines().enumerate() {
        let width = line.chars().count();
        if width > DEFAULT_MAX_LINE_LENGTH && !unsplittable(line) {
            failures.push(format!("{name}:{}: line is {width} characters", i + 1));
        }
    }
    match format_code(&formatted, &options) {
        Ok(again) if again == formatted => {}
        Ok(_) => failures.push(format!("{name}: a second format changes the code")),
        Err(e) => failures.push(format!("{name}: second format failed: {e}")),
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn relative_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[test]
fn formatting_repository_files_keeps_them_valid_and_stable() {
    let root = repository_root();
    let mut files = Vec::new();
    for dir in ["tests", "examples", "dev-tests"] {
        pln_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 100, "found only {} files", files.len());

    let mut failures = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        check(&relative_name(&root, path), &source, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let skipped = path
            .file_name()
            .is_some_and(|name| name == "node_modules" || name == ".vitepress");
        if path.is_dir() && !skipped {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

#[test]
fn formatting_doc_examples_keeps_them_valid_and_stable() {
    let root = repository_root();
    let mut files = Vec::new();
    markdown_files(&root.join("docs"), &mut files);

    let mut failures = Vec::new();
    let mut blocks = 0;
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let mut lines = text.lines().enumerate();
        while let Some((start, line)) = lines.next() {
            if !(line.starts_with("```pleno") || line.starts_with("```pln")) {
                continue;
            }
            let mut block = String::new();
            for (_, line) in lines.by_ref() {
                if line.starts_with("```") {
                    break;
                }
                block.push_str(line);
                block.push('\n');
            }
            blocks += 1;
            let name = format!("{}:{}", relative_name(&root, path), start + 1);
            check(&name, &block, &mut failures);
        }
    }
    assert!(blocks > 100, "found only {blocks} code blocks");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

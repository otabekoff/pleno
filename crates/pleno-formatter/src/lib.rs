use pleno_ast::*;

#[derive(Debug, Clone)]
pub struct Formatter {
    indent_size: usize,
    use_tabs: bool,
    _max_line_length: usize,
    comments: Vec<Comment>,
    comment_index: usize,
    /// The formatted source, used to keep the author's blank lines. Empty
    /// when only an AST was given.
    source: String,
    line_starts: Vec<usize>,
    /// Set at the start of a block: a blank line right after `{` is dropped.
    at_block_start: bool,
}

/// A class or interface member, so members can be written in source order.
enum Member<'a> {
    Field(&'a FieldDecl),
    Method(&'a FunctionDecl),
    Property(&'a pleno_ast::PropertyDecl),
    Class(&'a Stmt),
}

impl Member<'_> {
    fn start(&self) -> usize {
        match self {
            Member::Field(field) => field.span.start,
            Member::Method(method) => method.span.start,
            Member::Property(property) => property.span.start,
            Member::Class(class) => class.span.start,
        }
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            indent_size: 4,
            use_tabs: false,
            _max_line_length: 80,
            comments: Vec::new(),
            comment_index: 0,
            source: String::new(),
            line_starts: Vec::new(),
            at_block_start: true,
        }
    }

    /// Gives the formatter the source the AST was parsed from, so blank lines
    /// between statements are kept.
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self.line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        self
    }

    /// Whether the code at `offset` starts its line and the line above it is
    /// blank. Always false right after an opening brace.
    fn blank_line_before(&mut self, offset: usize) -> bool {
        if std::mem::take(&mut self.at_block_start) || self.line_starts.is_empty() {
            return false;
        }
        let offset = offset.min(self.source.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        if line == 0
            || !self.source[self.line_starts[line]..offset]
                .trim()
                .is_empty()
        {
            return false;
        }
        self.source[self.line_starts[line - 1]..self.line_starts[line]]
            .trim()
            .is_empty()
    }

    pub fn format(&self, source: &str) -> Result<String, String> {
        let options = FormatOptions {
            indent_style: if self.use_tabs {
                IndentStyle::Tabs
            } else {
                IndentStyle::Spaces(self.indent_size)
            },
            max_line_length: Some(self._max_line_length),
            ..Default::default()
        };
        format_code(source, &options)
    }

    fn indent(&self, level: usize) -> String {
        if self.use_tabs {
            "\t".repeat(level)
        } else {
            " ".repeat(level * self.indent_size)
        }
    }

    fn emit_comments_before(&mut self, limit: usize, indent_level: usize) -> String {
        let mut result = String::new();
        while self.comment_index < self.comments.len() {
            let start = self.comments[self.comment_index].span.start;
            if start >= limit {
                break;
            }
            if self.blank_line_before(start) {
                result.push('\n');
            }
            let comment = &self.comments[self.comment_index];
            // Add indentation
            result.push_str(&self.indent(indent_level));
            match comment.kind {
                CommentKind::Line => {
                    result.push_str("//");
                    result.push_str(&comment.content);
                    result.push('\n');
                }
                CommentKind::Block => {
                    result.push_str("/*");
                    result.push_str(&comment.content);
                    result.push_str("*/\n");
                }
                CommentKind::Doc => {
                    result.push_str("///");
                    result.push_str(&comment.content);
                    result.push('\n');
                }
            }
            self.comment_index += 1;
        }
        result
    }

    fn emit_remaining_comments(&mut self, indent_level: usize) -> String {
        let mut result = String::new();
        while self.comment_index < self.comments.len() {
            let start = self.comments[self.comment_index].span.start;
            if self.blank_line_before(start) {
                result.push('\n');
            }
            let comment = &self.comments[self.comment_index];
            result.push_str(&self.indent(indent_level));
            match comment.kind {
                CommentKind::Line => {
                    result.push_str("//");
                    result.push_str(&comment.content);
                    result.push('\n');
                }
                CommentKind::Block => {
                    result.push_str("/*");
                    result.push_str(&comment.content);
                    result.push_str("*/\n");
                }
                CommentKind::Doc => {
                    result.push_str("///");
                    result.push_str(&comment.content);
                    result.push('\n');
                }
            }
            self.comment_index += 1;
        }
        result
    }

    fn format_annotations(&mut self, annotations: &[Annotation], indent_level: usize) -> String {
        let mut result = String::new();
        let indent = self.indent(indent_level);
        for annotation in annotations {
            result.push_str(&format!("{}@{}", indent, annotation.name));
            if !annotation.args.is_empty() {
                result.push('(');
                let args: Vec<String> = annotation
                    .args
                    .iter()
                    .map(|arg| self.format_expression(arg, indent_level))
                    .collect();
                result.push_str(&args.join(", "));
                result.push(')');
            }
            result.push('\n');
        }
        result
    }

    pub fn format_program(&mut self, program: &Program, indent_level: usize) -> String {
        self.comments = program.comments.clone();
        self.comment_index = 0;
        self.at_block_start = true;
        let mut result = String::new();

        for stmt in &program.statements {
            let formatted = self.format_statement(stmt, indent_level);
            result.push_str(&formatted);
            // Only add newline if statement doesn't already end with one
            if !formatted.ends_with('\n') {
                result.push('\n');
            }
        }

        // Emit remaining comments (trailing comments at end of file)
        result.push_str(&self.emit_remaining_comments(indent_level));

        result
    }

    fn format_statement(&mut self, stmt: &Stmt, indent_level: usize) -> String {
        let mut result = self.emit_comments_before(stmt.span.start, indent_level);

        // Keep at most one blank line before a statement.
        let blank = if self.line_starts.is_empty() {
            !std::mem::take(&mut self.at_block_start) && stmt.leading_blank_lines > 0
        } else {
            self.blank_line_before(stmt.span.start)
        };
        if blank {
            result.push('\n');
        }

        let current_indent = self.indent(indent_level);
        let mut stmt_str = match &stmt.kind {
            StmtKind::Module { name, body, .. } => {
                let mut result = self.format_annotations(&[], indent_level); // Modules don't have annotations yet
                if let Some(stmts) = body {
                    result.push_str(&format!("{}module {} {{\n", current_indent, name));
                    for stmt in stmts {
                        result.push_str(&self.format_statement(stmt, indent_level + 1));
                    }
                    result.push_str(&format!("{}}}\n", current_indent));
                    result
                } else {
                    format!("{}module {};\n", current_indent, name)
                }
            }
            StmtKind::Let {
                name,
                value,
                mutable,
                is_const,
                is_deep,
                ty,
                is_lazy,
                ..
            } => {
                let mut result = current_indent;
                if *is_const {
                    if *is_deep {
                        result.push_str("const deep ");
                    } else {
                        result.push_str("const ");
                    }
                } else {
                    result.push_str("let ");
                    if *is_lazy {
                        result.push_str("lazy ");
                    }
                    if *mutable {
                        result.push_str("mut ");
                    }
                }
                result.push_str(name);

                if let Some(type_name) = ty {
                    result.push_str(&format!(": {}", type_name));
                }

                if let Some(expr) = value {
                    result.push_str(" = ");
                    result.push_str(&self.format_expression(expr, indent_level));
                }

                result.push_str(";\n");
                result
            }
            StmtKind::Assign { name, value } => {
                format!(
                    "{}{} = {};\n",
                    current_indent,
                    name,
                    self.format_expression(value, indent_level)
                )
            }
            StmtKind::AssignProperty {
                object,
                name,
                value,
            } => {
                format!(
                    "{}{}.{} = {};\n",
                    current_indent,
                    self.format_expression(object, indent_level),
                    name,
                    self.format_expression(value, indent_level)
                )
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                format!(
                    "{}{}[{}] = {};\n",
                    current_indent,
                    self.format_expression(collection, indent_level),
                    self.format_expression(index, indent_level),
                    self.format_expression(value, indent_level)
                )
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let mut result = format!(
                    "{}if ({}) ",
                    current_indent,
                    self.format_expression(condition, indent_level)
                );

                // Format the then branch - if it's a block, format it directly
                match &then_branch.kind {
                    StmtKind::Block(_) => {
                        let then_formatted = self.format_statement(then_branch, indent_level);
                        result.push_str(then_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let then_formatted = self.format_statement(then_branch, indent_level + 1);
                        result.push_str(&then_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }

                if let Some(else_stmt) = else_branch {
                    // Remove trailing newline and add else on same line
                    result = result.trim_end().to_string();
                    result.push_str(" else ");
                    match &else_stmt.kind {
                        StmtKind::Block(_) => {
                            let else_formatted = self.format_statement(else_stmt, indent_level);
                            result.push_str(else_formatted.trim_start());
                        }
                        _ => {
                            result.push_str("{\n");
                            let else_formatted = self.format_statement(else_stmt, indent_level + 1);
                            result.push_str(&else_formatted);
                            result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                        }
                    }
                }

                result
            }
            StmtKind::While { condition, body } => {
                let mut result = format!(
                    "{}while ({}) ",
                    current_indent,
                    self.format_expression(condition, indent_level)
                );
                match &body.kind {
                    StmtKind::Block(_) => {
                        let body_formatted = self.format_statement(body, indent_level);
                        result.push_str(body_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let body_formatted = self.format_statement(body, indent_level + 1);
                        result.push_str(&body_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }
                result
            }
            StmtKind::For {
                label,
                names,
                iterable,
                body,
            } => {
                let names_str = names.join(", ");
                let mut result = current_indent.clone();
                if let Some(l) = label {
                    result.push_str(&format!("{}: ", l));
                }
                result.push_str(&format!(
                    "for ({} in {}) ",
                    names_str,
                    self.format_expression(iterable, indent_level)
                ));
                match &body.kind {
                    StmtKind::Block(_) => {
                        let body_formatted = self.format_statement(body, indent_level);
                        result.push_str(body_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let body_formatted = self.format_statement(body, indent_level + 1);
                        result.push_str(&body_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }
                result
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                let mut result = current_indent.clone();
                result.push_str("for (");
                if let Some(i) = init {
                    let formatted = self.format_statement(i, 0);
                    result.push_str(formatted.trim_end_matches([';', '\n', ' ']));
                }
                result.push_str("; ");
                if let Some(c) = condition {
                    result.push_str(&self.format_expression(c, indent_level));
                }
                result.push_str("; ");
                if let Some(s) = step {
                    let formatted = self.format_statement(s, 0);
                    result.push_str(formatted.trim_end_matches([';', '\n', ' ']));
                }
                result.push_str(") ");
                match &body.kind {
                    StmtKind::Block(_) => {
                        let body_formatted = self.format_statement(body, indent_level);
                        result.push_str(body_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let body_formatted = self.format_statement(body, indent_level + 1);
                        result.push_str(&body_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }
                result
            }
            StmtKind::DoWhile { body, condition } => {
                let mut result = format!("{}do ", current_indent);
                match &body.kind {
                    StmtKind::Block(_) => {
                        let body_formatted = self.format_statement(body, indent_level);
                        result.push_str(body_formatted.trim_start());
                        result = result.trim_end().to_string();
                    }
                    _ => {
                        result.push_str("{\n");
                        let body_formatted = self.format_statement(body, indent_level + 1);
                        result.push_str(&body_formatted);
                        result.push_str(&format!("{}}}", self.indent(indent_level)));
                    }
                }
                result.push_str(&format!(
                    " while ({});\n",
                    self.format_expression(condition, indent_level)
                ));
                result
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                let mut result = format!("{}try ", current_indent);

                // Format try branch as block
                match &try_branch.kind {
                    StmtKind::Block(_) => {
                        let try_formatted = self.format_statement(try_branch, indent_level);
                        result.push_str(try_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let try_formatted = self.format_statement(try_branch, indent_level + 1);
                        result.push_str(&try_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }

                for catch in catch_branches {
                    // Remove trailing newline from try block if present
                    result = result.trim_end().to_string();
                    result.push_str(" catch ");
                    if let Some(ty) = &catch.ty {
                        result.push_str(&format!("({} {}) ", ty, catch.name));
                    } else {
                        result.push_str(&format!("({}) ", catch.name));
                    }

                    match &catch.body.kind {
                        StmtKind::Block(_) => {
                            let catch_formatted = self.format_statement(&catch.body, indent_level);
                            result.push_str(catch_formatted.trim_start());
                        }
                        _ => {
                            result.push_str("{\n");
                            let catch_formatted =
                                self.format_statement(&catch.body, indent_level + 1);
                            result.push_str(&catch_formatted);
                            result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                        }
                    }
                }

                if let Some(finally) = finally_branch {
                    // Remove trailing newline from catch block if present
                    result = result.trim_end().to_string();
                    result.push_str(" finally ");
                    match &finally.kind {
                        StmtKind::Block(_) => {
                            let finally_formatted = self.format_statement(finally, indent_level);
                            result.push_str(finally_formatted.trim_start());
                        }
                        _ => {
                            result.push_str("{\n");
                            let finally_formatted =
                                self.format_statement(finally, indent_level + 1);
                            result.push_str(&finally_formatted);
                            result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                        }
                    }
                }

                result
            }
            StmtKind::Throw(expr) => {
                format!(
                    "{}throw {};\n",
                    current_indent,
                    self.format_expression(expr, indent_level)
                )
            }
            StmtKind::Import { path, kind } => match kind {
                ImportKind::Module { alias } => {
                    if let Some(alias_name) = alias {
                        format!(
                            "{}import {} from \"{}\";\n",
                            current_indent, alias_name, path
                        )
                    } else {
                        format!("{}import \"{}\";\n", current_indent, path)
                    }
                }
                ImportKind::Names(items) => {
                    let names: Vec<String> = items
                        .iter()
                        .map(|item| {
                            if let Some(alias) = &item.alias {
                                format!("{} as {}", item.name, alias)
                            } else {
                                item.name.clone()
                            }
                        })
                        .collect();
                    format!(
                        "{}import {{{}}} from \"{}\";\n",
                        current_indent,
                        names.join(", "),
                        path
                    )
                }
                ImportKind::Star { alias } => {
                    format!(
                        "{}import * as {} from \"{}\";\n",
                        current_indent, alias, path
                    )
                }
            },
            StmtKind::Export(inner_stmt) => {
                // Format the inner statement without extra indent, then prefix with export
                let inner = self.format_statement(inner_stmt, indent_level);
                format!("{}export {}", current_indent, inner.trim_start())
            }
            StmtKind::ExportList(items) => {
                let names: Vec<String> = items
                    .iter()
                    .map(|item| {
                        if let Some(alias) = &item.alias {
                            format!("{} as {}", item.name, alias)
                        } else {
                            item.name.clone()
                        }
                    })
                    .collect();
                format!("{}export {{{}}};\n", current_indent, names.join(", "))
            }
            StmtKind::Class {
                name,
                generics,
                fields,
                methods,
                properties,
                superclass,
                implements,
                is_abstract,
                annotations,
                nested_classes,
                ..
            } => {
                let mut result = self.format_annotations(annotations, indent_level);
                let mut prefix = current_indent.clone();
                if *is_abstract {
                    prefix.push_str("abstract ");
                }
                result.push_str(&format!("{}class ", prefix));
                result.push_str(name);
                result.push_str(&self.format_generics(generics));

                if let Some(superclass_name) = superclass {
                    result.push_str(&format!(" extends {}", superclass_name));
                }

                if !implements.is_empty() {
                    result.push_str(&format!(" implements {}", implements.join(", ")));
                }

                result.push_str(" {\n");
                let next_indent = indent_level + 1;

                let mut members: Vec<Member> = fields.iter().map(Member::Field).collect();
                members.extend(methods.iter().map(Member::Method));
                members.extend(properties.iter().map(Member::Property));
                members.extend(nested_classes.iter().map(Member::Class));
                members.sort_by_key(Member::start);

                self.at_block_start = true;
                for member in members {
                    match member {
                        Member::Field(field) => {
                            result.push_str(
                                &self.emit_comments_before(field.span.start, next_indent),
                            );
                            if self.blank_line_before(field.span.start) {
                                result.push('\n');
                            }
                            result.push_str(
                                &self.format_annotations(&field.annotations, next_indent),
                            );
                            let visibility = match &field.visibility {
                                Some(vis) => format!("{} ", format_visibility(vis)),
                                None => "".to_string(),
                            };
                            let static_keyword = if field.is_static { "static " } else { "" };
                            let const_keyword = if field.is_const { "const " } else { "" };

                            result.push_str(&self.indent(next_indent));
                            result.push_str(&format!(
                                "{}{}{}{}",
                                visibility, static_keyword, const_keyword, field.name
                            ));

                            if let Some(ty) = &field.ty {
                                result.push_str(": ");
                                result.push_str(ty);
                            }

                            if let Some(val) = &field.default {
                                result.push_str(" = ");
                                result.push_str(&self.format_expression(val, next_indent));
                            }

                            result.push_str(";\n");
                        }
                        Member::Method(method) => {
                            result.push_str(
                                &self.emit_comments_before(method.span.start, next_indent),
                            );
                            if self.blank_line_before(method.span.start) {
                                result.push('\n');
                            }
                            result.push_str(&self.format_function(method, next_indent, false));
                        }
                        Member::Property(property) => {
                            result.push_str(
                                &self.emit_comments_before(property.span.start, next_indent),
                            );
                            if self.blank_line_before(property.span.start) {
                                result.push('\n');
                            }
                            result.push_str(&self.format_property(property, next_indent));
                        }
                        Member::Class(class) => {
                            result.push_str(&self.format_statement(class, next_indent));
                        }
                    }
                }
                result.push_str(&self.emit_comments_before(stmt.span.end, next_indent));
                self.at_block_start = false;

                result.push_str(&format!("{}}}\n", self.indent(indent_level)));

                result
            }
            StmtKind::Interface {
                name,
                generics,
                methods,
                properties,
                extends,
                annotations,
                ..
            } => {
                let mut result = self.format_annotations(annotations, indent_level);
                result.push_str(&format!("{}interface {} ", current_indent, name));
                result.push_str(&self.format_generics(generics));
                if !extends.is_empty() {
                    result.push_str(&format!("extends {} ", extends.join(", ")));
                }
                result.push_str("{\n");
                let next_indent = indent_level + 1;
                let mut members: Vec<Member> = methods.iter().map(Member::Method).collect();
                members.extend(properties.iter().map(Member::Property));
                members.sort_by_key(Member::start);
                self.at_block_start = true;
                for member in members {
                    let start = member.start();
                    result.push_str(&self.emit_comments_before(start, next_indent));
                    if self.blank_line_before(start) {
                        result.push('\n');
                    }
                    match member {
                        Member::Method(method) => {
                            result.push_str(&self.format_function(method, next_indent, true))
                        }
                        Member::Property(property) => {
                            result.push_str(&self.format_property(property, next_indent))
                        }
                        Member::Field(_) | Member::Class(_) => {}
                    }
                }
                result.push_str(&self.emit_comments_before(stmt.span.end, next_indent));
                self.at_block_start = false;
                result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                result
            }
            StmtKind::Fn {
                name,
                generics,
                params,
                return_ty,
                body,
                annotations,
                is_async,
                ..
            } => {
                let mut result = self.format_annotations(annotations, indent_level);
                result.push_str(&current_indent);
                if *is_async {
                    result.push_str("async ");
                }
                result.push_str("fn ");
                result.push_str(name);
                result.push_str(&self.format_generics(generics));
                result.push('(');

                result.push_str(&self.format_params(params));
                result.push(')');
                if let Some(ret_ty) = return_ty {
                    result.push_str(": ");
                    result.push_str(ret_ty);
                }
                result.push(' ');
                match &body.kind {
                    StmtKind::Block(_) => {
                        let body_formatted = self.format_statement(body, indent_level);
                        result.push_str(body_formatted.trim_start());
                    }
                    _ => {
                        result.push_str("{\n");
                        let body_formatted = self.format_statement(body, indent_level + 1);
                        result.push_str(&body_formatted);
                        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                    }
                }

                result
            }
            StmtKind::Return(expr) => match expr {
                Some(e) => format!(
                    "{}return {};\n",
                    current_indent,
                    self.format_expression(e, indent_level)
                ),
                None => format!("{}return;\n", current_indent),
            },
            StmtKind::Break(label) => {
                let mut result = format!("{}break", current_indent);
                if let Some(l) = label {
                    result.push_str(&format!(" {}", l));
                }
                result.push_str(";\n");
                result
            }
            StmtKind::Continue(label) => {
                let mut result = format!("{}continue", current_indent);
                if let Some(l) = label {
                    result.push_str(&format!(" {}", l));
                }
                result.push_str(";\n");
                result
            }
            StmtKind::Labeled { label, stmt } => {
                let mut result = format!("{}{}: ", current_indent, label);
                // Try to format on same line if possible, or new line
                let inner = self.format_statement(stmt, indent_level);
                result.push_str(inner.trim_start());
                result
            }
            StmtKind::Block(statements) => {
                // Handle empty blocks
                let has_comments = self
                    .comments
                    .get(self.comment_index)
                    .is_some_and(|comment| comment.span.start < stmt.span.end);
                if statements.is_empty() && !has_comments {
                    let mut result = String::new();
                    if indent_level > 0 {
                        result.push_str(&self.indent(indent_level));
                    }
                    result.push_str("{}\n");
                    return result;
                }

                let mut result = String::new();
                // Only add indent if we're at a non-zero indent level
                if indent_level > 0 {
                    result.push_str(&self.indent(indent_level));
                }
                result.push_str("{\n");
                let next_indent = indent_level + 1;

                self.at_block_start = true;
                for stmt in statements {
                    result.push_str(&self.format_statement(stmt, next_indent));
                }
                result.push_str(&self.emit_comments_before(stmt.span.end, next_indent));
                self.at_block_start = false;

                result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                result
            }
            StmtKind::Extend {
                target,
                generics,
                methods,
                ..
            } => {
                let mut result = format!("{}extend ", current_indent);
                if !generics.is_empty() {
                    result.push_str(&self.format_generics(generics));
                    result.push(' ');
                }
                result.push_str(target);
                result.push_str(" {\n");
                for method in methods {
                    result.push_str(&self.format_function(method, indent_level + 1, false));
                }
                result.push_str(&format!("{}}}\n", current_indent));
                result
            }
            // A `match` used as a statement needs no `;`.
            StmtKind::Expr(
                expr @ Expr {
                    kind: ExprKind::Match { .. },
                    ..
                },
            ) => format!(
                "{}{}\n",
                current_indent,
                self.format_expression(expr, indent_level)
            ),
            StmtKind::Expr(expr) => {
                format!(
                    "{}{};\n",
                    current_indent,
                    self.format_expression(expr, indent_level)
                )
            }
            StmtKind::Enum(decl) => {
                let mut result = self.format_annotations(&decl.annotations, indent_level);
                result.push_str(&format!("{}enum {} {{\n", current_indent, decl.name));
                for (i, variant) in decl.variants.iter().enumerate() {
                    result.push_str(&format!("{}    {}", current_indent, variant.name));
                    if let Some(params) = &variant.params {
                        result.push('(');
                        let param_strs: Vec<String> =
                            params.iter().map(|p| self.format_param(p)).collect();
                        result.push_str(&param_strs.join(", "));
                        result.push(')');
                    }
                    if let Some(value) = &variant.value {
                        let value = self.format_expression(value, indent_level);
                        result.push_str(&format!(" = {value}"));
                    }
                    if i < decl.variants.len() - 1 {
                        result.push_str(",\n");
                    } else {
                        result.push('\n');
                    }
                }

                if !decl.methods.is_empty() {
                    result.push('\n');
                    for method in &decl.methods {
                        result.push_str(&self.format_function(method, indent_level + 1, false));
                    }
                }

                result.push_str(&format!("{}}}\n", current_indent));
                result
            }
            StmtKind::TypeAlias { name, ty, .. } => {
                format!("{}type {} = {};\n", current_indent, name, ty)
            }
            StmtKind::MacroDecl {
                name, params, body, ..
            } => {
                let mut result = format!("{}macro {}(", current_indent, name);
                result.push_str(&params.join(", "));
                result.push_str(") ");
                let body_str = self.format_statement(body, indent_level);
                result.push_str(body_str.trim_start());
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result
            }
        };

        // Append trailing comment if present
        if let Some(comment) = &stmt.trailing_comment {
            // Remove trailing newline if it exists
            if stmt_str.ends_with('\n') {
                stmt_str.pop();
            }
            stmt_str.push_str(" //");
            stmt_str.push_str(comment);
            stmt_str.push('\n');
        }

        result.push_str(&stmt_str);
        result
    }

    fn format_expression(&mut self, expr: &Expr, indent_level: usize) -> String {
        match &expr.kind {
            ExprKind::Literal(literal) => self.format_literal(literal),
            ExprKind::Ident(name) => name.clone(),
            ExprKind::This => "this".to_string(),
            ExprKind::Super => "super".to_string(),
            ExprKind::Unary { op, expr } => {
                let inner = self.format_expression(expr, indent_level);
                match op {
                    UnaryOp::Negate => format!("-{}", inner),
                    UnaryOp::Not => format!("!{}", inner),
                    UnaryOp::PreIncrement => format!("++{}", inner),
                    UnaryOp::PostIncrement => format!("{}++", inner),
                    UnaryOp::PreDecrement => format!("--{}", inner),
                    UnaryOp::PostDecrement => format!("{}--", inner),
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if let Some(eb) = else_branch {
                    format!(
                        "{} ? {} : {}",
                        self.format_expression(condition, indent_level),
                        self.format_expression(then_branch, indent_level),
                        self.format_expression(eb, indent_level)
                    )
                } else {
                    format!(
                        "if ({}) {}",
                        self.format_expression(condition, indent_level),
                        self.format_expression(then_branch, indent_level)
                    )
                }
            }
            ExprKind::Binary { left, op, right } => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Subtract => "-",
                    BinaryOp::Multiply => "*",
                    BinaryOp::Divide => "/",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    BinaryOp::Equal => "==",
                    BinaryOp::NotEqual => "!=",
                    BinaryOp::Less => "<",
                    BinaryOp::LessEqual => "<=",
                    BinaryOp::Greater => ">",
                    BinaryOp::GreaterEqual => ">=",
                    BinaryOp::Modulo => "%",
                    BinaryOp::NullCoalesce => "??",
                };
                format!(
                    "{} {} {}",
                    self.format_expression(left, indent_level),
                    op_str,
                    self.format_expression(right, indent_level)
                )
            }
            ExprKind::Property { object, name } => {
                format!("{}.{}", self.format_expression(object, indent_level), name)
            }
            ExprKind::SafeProperty { object, name } => {
                format!("{}?.{}", self.format_expression(object, indent_level), name)
            }
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let start_str = match start {
                    Some(expr) => self.format_expression(expr, indent_level),
                    None => "".to_string(),
                };
                let end_str = match end {
                    Some(expr) => self.format_expression(expr, indent_level),
                    None => "".to_string(),
                };
                let op = if *inclusive { "..=" } else { ".." };
                format!("{}{}{}", start_str, op, end_str)
            }
            ExprKind::MacroCall { name, args } => {
                let arg_list = args
                    .iter()
                    .map(|arg| match arg {
                        CallArg::Positional(expr) => self.format_expression(expr, indent_level),
                        CallArg::Named(name, expr) => {
                            format!("{}: {}", name, self.format_expression(expr, indent_level))
                        }
                        CallArg::Placeholder => "_".to_string(),
                        CallArg::Spread(expr) => {
                            format!("...{}", self.format_expression(expr, indent_level))
                        }
                    })
                    .collect::<Vec<_>>();
                format!("{}!({})", name, arg_list.join(", "))
            }
            ExprKind::MacroSplice(expr) => {
                format!("${{{}}}", self.format_expression(expr, indent_level))
            }
            ExprKind::MapLiteral(entries) => {
                let items: Vec<String> = entries
                    .iter()
                    .map(|item| match item {
                        MapItem::Entry(key, value) => format!(
                            "\"{}\": {}",
                            key,
                            self.format_expression(value, indent_level)
                        ),
                        MapItem::Spread(expr) => {
                            let expr_kind = &expr.kind;
                            if matches!(expr_kind, ExprKind::MapLiteral(_)) {
                                self.format_expression(expr, indent_level)
                            } else {
                                format!("...{}", self.format_expression(expr, indent_level))
                            }
                        }
                    })
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            ExprKind::Spread(expr) => {
                format!("...{}", self.format_expression(expr, indent_level))
            }
            ExprKind::Paren(expr) => {
                format!("({})", self.format_expression(expr, indent_level))
            }
            ExprKind::NamedTupleLiteral(entries) => {
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|(key, value)| {
                        format!("{}: {}", key, self.format_expression(value, indent_level))
                    })
                    .collect();
                format!("({})", pairs.join(", "))
            }
            ExprKind::Is { left, ty } => {
                format!("{} is {}", self.format_expression(left, indent_level), ty)
            }
            ExprKind::ArrayLiteral(items) => {
                let items_str: Vec<String> = items
                    .iter()
                    .map(|item| self.format_expression(item, indent_level))
                    .collect();
                format!("[{}]", items_str.join(", "))
            }
            ExprKind::TupleLiteral(items) => {
                let items_str: Vec<String> = items
                    .iter()
                    .map(|item| self.format_expression(item, indent_level))
                    .collect();
                // `(x,)` is a one-element tuple; `(x)` is just `x`.
                if items_str.len() == 1 {
                    format!("({},)", items_str[0])
                } else {
                    format!("({})", items_str.join(", "))
                }
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                annotations,
                is_async,
            } => {
                let mut head = String::new();
                for annotation in annotations {
                    head.push('@');
                    head.push_str(&annotation.name);
                    if !annotation.args.is_empty() {
                        let args: Vec<String> = annotation
                            .args
                            .iter()
                            .map(|arg| self.format_expression(arg, indent_level))
                            .collect();
                        head.push_str(&format!("({})", args.join(", ")));
                    }
                    head.push(' ');
                }
                // `(x) => ...` takes only bare names; anything more needs `fn(...)`.
                let arrow = !*is_async
                    && return_ty.is_none()
                    && params.iter().all(|param| {
                        param.ty.is_none()
                            && param.default.is_none()
                            && !param.is_variadic
                            && !param.is_keyword_only
                            && param.ownership == Ownership::None
                    });
                if *is_async {
                    head.push_str("async ");
                }
                if arrow {
                    head.push_str(&format!("({}) =>", self.format_params(params)));
                } else {
                    head.push_str(&format!("fn({})", self.format_params(params)));
                    if let Some(ty) = return_ty {
                        head.push_str(&format!(": {}", ty));
                    }
                }

                let body_str = match &body.kind {
                    ExprKind::Block(statements) => {
                        let mut block_str = String::from(
                            "{
",
                        );
                        self.at_block_start = true;
                        for stmt in statements {
                            block_str.push_str(&self.format_statement(stmt, indent_level + 1));
                        }
                        block_str
                            .push_str(&self.emit_comments_before(body.span.end, indent_level + 1));
                        self.at_block_start = false;
                        block_str.push_str(&format!("{}}}", self.indent(indent_level)));
                        block_str
                    }
                    _ if arrow => self.format_expression(body, indent_level),
                    _ => format!("=> {}", self.format_expression(body, indent_level)),
                };

                format!("{} {}", head, body_str)
            }
            ExprKind::Block(statements) => {
                let mut result = String::from("{ ");
                for stmt in statements {
                    result.push_str(&self.format_statement(stmt, indent_level));
                    result.push(' ');
                }
                result.push('}');
                result
            }
            ExprKind::Underscore => "_".to_string(),
            ExprKind::Index { collection, index } => {
                format!(
                    "{}[{}]",
                    self.format_expression(collection, indent_level),
                    self.format_expression(index, indent_level)
                )
            }
            ExprKind::SafeIndex { collection, index } => {
                format!(
                    "{}?.[{}]",
                    self.format_expression(collection, indent_level),
                    self.format_expression(index, indent_level)
                )
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                let start_str = match start {
                    Some(expr) => self.format_expression(expr, indent_level),
                    None => "".to_string(),
                };
                let end_str = match end {
                    Some(expr) => self.format_expression(expr, indent_level),
                    None => "".to_string(),
                };
                format!(
                    "{}[{}:{}]",
                    self.format_expression(collection, indent_level),
                    start_str,
                    end_str
                )
            }
            ExprKind::SetLiteral(items) => {
                let items_str: Vec<String> = items
                    .iter()
                    .map(|item| self.format_expression(item, indent_level))
                    .collect();
                format!("{{{}}}", items_str.join(", "))
            }
            ExprKind::Pipeline { left, right } => {
                format!(
                    "{} |> {}",
                    self.format_expression(left, indent_level),
                    self.format_expression(right, indent_level)
                )
            }
            ExprKind::Call { callee, args } => {
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|arg| match arg {
                        CallArg::Positional(expr) => self.format_expression(expr, indent_level),
                        CallArg::Named(name, expr) => {
                            format!("{}: {}", name, self.format_expression(expr, indent_level))
                        }
                        CallArg::Placeholder => "_".to_string(),
                        CallArg::Spread(expr) => {
                            format!("...{}", self.format_expression(expr, indent_level))
                        }
                    })
                    .collect();
                format!(
                    "{}({})",
                    self.format_expression(callee, indent_level),
                    arg_strs.join(", ")
                )
            }
            ExprKind::Match { expr, arms } => {
                // `match (x)` keeps its parentheses in the AST.
                let scrutinee = match &expr.kind {
                    ExprKind::Paren(inner) => inner,
                    _ => expr,
                };
                let mut result = format!(
                    "match ({}) {{\n",
                    self.format_expression(scrutinee, indent_level)
                );
                for arm in arms {
                    let guard_str = if let Some(guard) = &arm.guard {
                        format!(" if {}", self.format_expression(guard, indent_level))
                    } else {
                        "".to_string()
                    };
                    result.push_str(&format!(
                        "{}case {}{} => {},\n",
                        self.indent(indent_level + 1),
                        self.format_pattern(&arm.pattern),
                        guard_str,
                        self.format_expression(&arm.body, indent_level + 1)
                    ));
                }
                result.push_str(&self.indent(indent_level));
                result.push('}');
                result
            }
            ExprKind::Await { expr } => {
                format!("await {}", self.format_expression(expr, indent_level))
            }
            ExprKind::AssertNotNull(inner) => {
                format!("{}!", self.format_expression(inner, indent_level))
            }
            ExprKind::SafeCall { callee, args } => {
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|arg| match arg {
                        CallArg::Positional(expr) => self.format_expression(expr, indent_level),
                        CallArg::Named(name, expr) => {
                            format!("{}: {}", name, self.format_expression(expr, indent_level))
                        }
                        CallArg::Placeholder => "_".to_string(),
                        CallArg::Spread(expr) => {
                            format!("...{}", self.format_expression(expr, indent_level))
                        }
                    })
                    .collect();
                format!(
                    "{}?.({})",
                    self.format_expression(callee, indent_level),
                    arg_strs.join(", ")
                )
            }
            ExprKind::Specialize { left, type_args } => {
                format!(
                    "{}<{}>",
                    self.format_expression(left, indent_level),
                    type_args.join(", ")
                )
            }
            ExprKind::InterpolatedString(segments) => {
                let mut result = String::from("\"");
                for segment in segments {
                    match &segment.kind {
                        ExprKind::Literal(Literal::String(s)) => result.push_str(&escape_string(s)),
                        _ => {
                            result.push_str("${");
                            result.push_str(&self.format_expression(segment, indent_level));
                            result.push('}');
                        }
                    }
                }
                result.push('"');
                result
            }
        }
    }

    fn format_literal(&self, literal: &Literal) -> String {
        match literal {
            Literal::Int(i) => i.to_string(),
            // Keep the decimal point: `1.0` is a Float, `1` an Int.
            Literal::Float(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 => {
                format!("{:.1}", f)
            }
            Literal::Float(f) => f.to_string(),
            Literal::String(s) => format!("\"{}\"", escape_string(s)),
            Literal::Char(c) => match c {
                '\n' => "'\\n'".to_string(),
                '\r' => "'\\r'".to_string(),
                '\t' => "'\\t'".to_string(),
                '\\' => "'\\\\'".to_string(),
                '\'' => "'\\''".to_string(),
                _ => format!("'{}'", c),
            },
            Literal::Bool(b) => b.to_string(),
            Literal::Null => "null".to_string(),
        }
    }

    fn format_generics(&self, generics: &[GenericParam]) -> String {
        if generics.is_empty() {
            return String::new();
        }
        let params: Vec<String> = generics
            .iter()
            .map(|param| {
                let mut result = param.name.clone();
                if !param.constraints.is_empty() {
                    result.push_str(": ");
                    result.push_str(&param.constraints.join(" + "));
                }
                result
            })
            .collect();
        format!("<{}>", params.join(", "))
    }

    /// Formats a parameter list, writing the `*` marker once before the
    /// first keyword-only parameter.
    fn format_params(&mut self, params: &[Param]) -> String {
        let mut parts = Vec::new();
        let mut keyword_only = false;
        for param in params {
            if param.is_keyword_only && !keyword_only {
                keyword_only = true;
                parts.push("*".to_string());
            }
            parts.push(self.format_param(param));
        }
        parts.join(", ")
    }

    fn format_param(&mut self, param: &Param) -> String {
        let ownership = match param.ownership {
            Ownership::Move => "move ",
            Ownership::Borrow => "&",
            Ownership::BorrowMut => "&mut ",
            Ownership::None => "",
        };
        let mut result = format!("{}{}", ownership, param.name);
        if param.is_variadic {
            result = format!("...{}", result);
        }
        if let Some(ty) = &param.ty {
            result.push_str(&format!(": {}", ty));
        }
        if let Some(default) = &param.default {
            result.push_str(&format!(" = {}", self.format_expression(default, 0)));
        }
        result
    }

    fn format_pattern(&self, pattern: &Pattern) -> String {
        match &pattern.kind {
            PatternKind::Literal(literal) => self.format_literal(literal),
            PatternKind::Ident(name) => name.clone(),
            PatternKind::Tuple(patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                format!("({})", items.join(", "))
            }
            PatternKind::Underscore => "_".to_string(),
            PatternKind::Rest(name) => match name {
                Some(n) => format!("...{}", n),
                None => "...".to_string(),
            },
            PatternKind::Variant(name, patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                format!("{}({})", name, items.join(", "))
            }
            PatternKind::Object(props, rest) => {
                let mut items: Vec<String> = props
                    .iter()
                    .map(|(name, p)| {
                        if let PatternKind::Ident(ident_name) = &p.kind
                            && ident_name == name
                        {
                            return name.clone();
                        }
                        format!("{}: {}", name, self.format_pattern(p))
                    })
                    .collect();
                if let Some(r) = rest {
                    items.push(self.format_pattern(r));
                }
                format!("{{ {} }}", items.join(", "))
            }
            PatternKind::Type(ty, binding) => match binding {
                Some(b) => format!("{} {}", ty, b),
                None => ty.clone(),
            },
            PatternKind::Or(patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                items.join(", ")
            }
        }
    }

    fn format_function(
        &mut self,
        func: &FunctionDecl,
        indent_level: usize,
        in_interface: bool,
    ) -> String {
        let current_indent = self.indent(indent_level);
        let mut result = current_indent.clone();

        // Add visibility if present. Constructors are public by default, so
        // the parser records `public` even when it was not written.
        let is_constructor = func.name == "constructor";
        match &func.visibility {
            Some(Visibility::Public) if is_constructor => {}
            Some(visibility) => result.push_str(&format!("{} ", format_visibility(visibility))),
            None => {}
        }

        // Add static if applicable
        if func.is_static {
            result.push_str("static ");
        }

        // Add async if applicable
        if func.is_async {
            result.push_str("async ");
        }

        // Interface members are always abstract; the keyword is implied there.
        if func.is_abstract && !in_interface {
            result.push_str("abstract ");
        }

        match &func.kind {
            FunctionKind::Getter => result.push_str(&format!("get {}", func.name)),
            FunctionKind::Setter => result.push_str(&format!("set {}", func.name)),
            FunctionKind::Operator(symbol) => result.push_str(&format!("operator {}", symbol)),
            FunctionKind::Method => {
                result.push_str("fn ");
                result.push_str(&func.name);
            }
        }
        result.push_str(&self.format_generics(&func.generics));
        result.push('(');
        result.push_str(&self.format_params(&func.params));
        result.push(')');

        if let Some(ret_ty) = &func.return_ty {
            result.push_str(": ");
            result.push_str(ret_ty);
        }

        // Abstract members have no body.
        if func.is_abstract {
            result.push_str(";\n");
            return result;
        }

        result.push(' ');

        // Add body - format as block
        match &func.body.kind {
            StmtKind::Block(_) => {
                let body_formatted = self.format_statement(&func.body, indent_level);
                result.push_str(body_formatted.trim_start());
            }
            _ => {
                result.push_str("{\n");
                let body_formatted = self.format_statement(&func.body, indent_level + 1);
                result.push_str(&body_formatted);
                result.push_str(&format!("{}}}\n", self.indent(indent_level)));
            }
        }

        result
    }

    fn format_property(&mut self, prop: &pleno_ast::PropertyDecl, indent_level: usize) -> String {
        let mut result = self.indent(indent_level);

        // Add visibility if present
        if let Some(visibility) = &prop.visibility {
            result.push_str(&format!("{} ", format_visibility(visibility)));
        }

        // Add static if applicable
        if prop.is_static {
            result.push_str("static ");
        }

        result.push_str(&format!("property {} ", prop.name));

        if let Some(ty) = &prop.ty {
            result.push_str(&format!(": {} ", ty));
        }

        result.push_str("{\n");
        let next_indent = indent_level + 1;

        if let Some(getter) = &prop.getter {
            result.push_str(&self.indent(next_indent));
            result.push_str("get ");
            // Getter body - handle both block and expression if needed, but here it's likely a block
            match &getter.body.kind {
                StmtKind::Block(_) => {
                    let body_formatted = self.format_statement(&getter.body, next_indent);
                    result.push_str(body_formatted.trim_start());
                }
                _ => {
                    result.push_str("{\n");
                    let body_formatted = self.format_statement(&getter.body, next_indent + 1);
                    result.push_str(&body_formatted);
                    result.push_str(&format!("{}}}\n", self.indent(next_indent)));
                }
            }
        }

        if let Some(setter) = &prop.setter {
            result.push_str(&self.indent(next_indent));
            result.push_str("set(");
            if let Some(param) = setter.params.first() {
                result.push_str(&param.name);
            }
            result.push_str(") ");
            match &setter.body.kind {
                StmtKind::Block(_) => {
                    let body_formatted = self.format_statement(&setter.body, next_indent);
                    result.push_str(body_formatted.trim_start());
                }
                _ => {
                    result.push_str("{\n");
                    let body_formatted = self.format_statement(&setter.body, next_indent + 1);
                    result.push_str(&body_formatted);
                    result.push_str(&format!("{}}}\n", self.indent(next_indent)));
                }
            }
        }

        result.push_str(&format!("{}}}\n", self.indent(indent_level)));
        result
    }
}

/// Escapes a string's contents so the lexer reads back the same value.
fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            other => out.push(other),
        }
    }
    out
}

fn format_visibility(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Protected => "protected",
        Visibility::Internal => "internal",
    }
    .to_string()
}

// Public API function for easy formatting
pub fn format_code(source: &str, options: &FormatOptions) -> Result<String, String> {
    use pleno_lexer::lex;
    use pleno_parser::Parser;

    // Parse the source into an AST
    let tokens = lex(source).map_err(|e| e.message)?;
    let mut parser = Parser::new(tokens, source);
    let program = parser.parse_program().map_err(|e| e.message)?;

    // Create formatter with options
    let mut formatter = Formatter {
        indent_size: match options.indent_style {
            IndentStyle::Spaces(n) => n,
            IndentStyle::Tabs => 4, // Default to 4 for tabs
        },
        use_tabs: matches!(options.indent_style, IndentStyle::Tabs),
        _max_line_length: options.max_line_length.unwrap_or(80),
        comments: Vec::new(),
        comment_index: 0,
        source: String::new(),
        line_starts: Vec::new(),
        at_block_start: true,
    }
    .with_source(source);

    // Format the program
    let mut result = formatter.format_program(&program, 0);

    // Apply post-processing based on options
    if options.trim_trailing_whitespace {
        result = result
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");
    }

    if options.insert_final_newline && !result.ends_with('\n') {
        result.push('\n');
    }

    Ok(result)
}

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub indent_style: IndentStyle,
    pub max_line_length: Option<usize>,
    pub insert_final_newline: bool,
    pub trim_trailing_whitespace: bool,
}

#[derive(Debug, Clone)]
pub enum IndentStyle {
    Spaces(usize),
    Tabs,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_style: IndentStyle::Spaces(4),
            max_line_length: Some(80),
            insert_final_newline: true,
            trim_trailing_whitespace: true,
        }
    }
}

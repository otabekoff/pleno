use pleno_ast::*;

#[derive(Debug, Clone)]
pub struct Formatter {
    indent_size: usize,
    use_tabs: bool,
    _max_line_length: usize,
    comments: Vec<Comment>,
    comment_index: usize,
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
        }
    }

    pub fn format(&self, source: &str) -> Result<String, String> {
        // Since we're working with AST nodes directly, we'll create a new formatter instance
        let mut formatter = self.clone();
        Ok(formatter.format_source(source))
    }

    fn format_source(&mut self, source: &str) -> String {
        // This is a simplified version that would require the actual AST to be parsed
        // For now, we'll just return the source as-is since the AST parsing is done elsewhere
        source.to_string()
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
            let comment = &self.comments[self.comment_index];
            if comment.span.start >= limit {
                break;
            }
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

    pub fn format_program(&mut self, program: &Program, indent_level: usize) -> String {
        self.comments = program.comments.clone();
        self.comment_index = 0;
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
        let result = self.emit_comments_before(stmt.span.start, indent_level);
        let current_indent = self.indent(indent_level);
        let stmt_str = match &stmt.kind {
            StmtKind::Module { name, body } => {
                if let Some(stmts) = body {
                    let mut result = format!("{}module {} {{\n", current_indent, name);
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
            } => {
                let mut result = current_indent;
                if *is_const {
                    if *is_deep {
                        result.push_str("const deep ");
                    } else {
                        result.push_str("const ");
                    }
                } else if *mutable {
                    result.push_str("let mut ");
                } else {
                    result.push_str("let ");
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
                format!(
                    "{}export {}",
                    current_indent,
                    self.format_statement(inner_stmt, indent_level)
                )
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
                annotations: _,
                nested_classes,
            } => {
                let mut result = String::new();
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

                for field in fields {
                    let visibility = match &field.visibility {
                        Some(vis) => format!("{} ", format_visibility(vis)),
                        None => "".to_string(),
                    };
                    let static_keyword = if field.is_static { "static " } else { "" };
                    result.push_str(&format!(
                        "{}{}{}{}{};\n",
                        self.indent(next_indent),
                        current_indent,
                        visibility,
                        static_keyword,
                        field.name
                    ));
                }

                for method in methods {
                    result.push_str(&self.format_function(method, next_indent));
                }

                for property in properties {
                    result.push_str(&self.format_property(property, next_indent));
                }

                for nested_class in nested_classes {
                    // Recursively format nested classes
                    if let StmtKind::Class { .. } = &nested_class.kind {
                        // We need to re-invoke format_statement but adjust indentation?
                        // format_statement should handle it correctly if we pass next_indent
                        let class_str = self.format_statement(nested_class, next_indent);
                        result.push_str(&class_str);
                    }
                }

                result.push_str(&format!("{}}}\n", self.indent(indent_level)));

                result
            }
            StmtKind::Interface {
                name,
                generics,
                methods,
                extends,
                annotations: _,
            } => {
                let mut result = format!("{}interface {} ", current_indent, name);
                result.push_str(&self.format_generics(generics));
                if !extends.is_empty() {
                    result.push_str(&format!("extends {} ", extends.join(", ")));
                }
                result.push_str("{\n");
                let next_indent = indent_level + 1;
                for method in methods {
                    result.push_str(&self.format_function(method, next_indent));
                }
                result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                result
            }
            StmtKind::Fn {
                name,
                generics,
                params,
                body,
                annotations: _,
                is_async,
            } => {
                let mut result = current_indent.clone();
                if *is_async {
                    result.push_str("async ");
                }
                result.push_str("fn ");
                result.push_str(name);
                result.push_str(&self.format_generics(generics));
                result.push('(');

                let param_strs: Vec<String> = params
                    .iter()
                    .map(|param| self.format_param(param))
                    .collect();
                result.push_str(&param_strs.join(", "));

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
                if statements.is_empty() {
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

                for stmt in statements {
                    result.push_str(&self.format_statement(stmt, next_indent));
                }

                result.push_str(&format!("{}}}\n", self.indent(indent_level)));
                result
            }
            StmtKind::Expr(expr) => {
                format!(
                    "{}{};\n",
                    current_indent,
                    self.format_expression(expr, indent_level)
                )
            }
            StmtKind::Enum(decl) => {
                let mut result = format!("{}enum {} {{\n", current_indent, decl.name);
                for variant in &decl.variants {
                    result.push_str(&format!("{}    {},\n", current_indent, variant.name));
                }
                result.push_str(&format!("{}}}\n", current_indent));
                result
            }
            StmtKind::TypeAlias { name, ty } => {
                format!("{}type {} = {};\n", current_indent, name, ty)
            }
        };
        result + &stmt_str
    }

    fn format_expression(&mut self, expr: &Expr, indent_level: usize) -> String {
        match &expr.kind {
            ExprKind::Literal(literal) => self.format_literal(literal),
            ExprKind::Ident(name) => name.clone(),
            ExprKind::This => "this".to_string(),
            ExprKind::Super => "super".to_string(),
            ExprKind::Unary { op, expr } => {
                let op_str = match op {
                    UnaryOp::Negate => "-",
                    UnaryOp::Not => "!",
                };
                format!("{}{}", op_str, self.format_expression(expr, indent_level))
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                // Format as ternary style: condition ? then : else
                format!(
                    "{} ? {} : {}",
                    self.format_expression(condition, indent_level),
                    self.format_expression(then_branch, indent_level),
                    self.format_expression(else_branch, indent_level)
                )
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
                format!("#({})", pairs.join(", "))
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
                format!("({})", items_str.join(", "))
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty: _,
                annotations: _,
            } => {
                let param_strs: Vec<String> =
                    params.iter().map(|param| param.name.clone()).collect();

                // Format body - if it's a block, format it properly
                let body_str = match &body.kind {
                    ExprKind::Block(statements) => {
                        let mut block_str = String::from("{\n");
                        for stmt in statements {
                            block_str.push_str(&self.format_statement(stmt, indent_level + 1));
                        }
                        block_str.push_str(&format!("{}}}", self.indent(indent_level)));
                        block_str
                    }
                    _ => self.format_expression(body, indent_level),
                };

                // Use arrow syntax for lambdas
                format!("({}) => {}", param_strs.join(", "), body_str)
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
                let mut result = format!(
                    "match ({}) {{\n",
                    self.format_expression(expr, indent_level)
                );
                for arm in arms {
                    let guard_str = if let Some(guard) = &arm.guard {
                        format!(" if {}", self.format_expression(guard, indent_level))
                    } else {
                        "".to_string()
                    };
                    result.push_str(&format!(
                        "    case {}{} => {},\n",
                        self.format_pattern(&arm.pattern),
                        guard_str,
                        self.format_expression(&arm.body, indent_level)
                    ));
                }
                result.push('}');
                result
            }
            ExprKind::Await { expr } => {
                format!("await {}", self.format_expression(expr, indent_level))
            }
            ExprKind::Specialize { left, type_args } => {
                format!(
                    "{}<{}>",
                    self.format_expression(left, indent_level),
                    type_args.join(", ")
                )
            }
        }
    }

    fn format_literal(&self, literal: &Literal) -> String {
        match literal {
            Literal::Int(i) => i.to_string(),
            Literal::Float(f) => f.to_string(),
            Literal::String(s) => format!("\"{}\"", s),
            Literal::Char(c) => format!("'{}'", c),
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

    fn format_param(&mut self, param: &Param) -> String {
        let mut result = param.name.clone();
        if let Some(ty) = &param.ty {
            result.push_str(&format!(": {}", ty));
        }
        if let Some(default) = &param.default {
            result.push_str(&format!(" = {}", self.format_expression(default, 0)));
        }
        if param.is_variadic {
            result = format!("...{}", result);
        }
        if param.is_keyword_only {
            result = format!("*{}", result);
        }
        result
    }

    fn format_pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Literal(literal) => self.format_literal(literal),
            Pattern::Ident(name) => name.clone(),
            Pattern::Tuple(patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                format!("({})", items.join(", "))
            }
            Pattern::Underscore => "_".to_string(),
            Pattern::Rest(name) => match name {
                Some(n) => format!("...{}", n),
                None => "...".to_string(),
            },
            Pattern::Variant(name, patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                format!("{}({})", name, items.join(", "))
            }
            Pattern::Object(props, rest) => {
                let mut items: Vec<String> = props
                    .iter()
                    .map(|(name, p)| {
                        if let Pattern::Ident(ident_name) = p
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
            Pattern::Type(ty, binding) => match binding {
                Some(b) => format!("{} {}", ty, b),
                None => ty.clone(),
            },
            Pattern::Or(patterns) => {
                let items: Vec<String> = patterns.iter().map(|p| self.format_pattern(p)).collect();
                items.join(", ")
            }
        }
    }

    fn format_function(&mut self, func: &FunctionDecl, indent_level: usize) -> String {
        let current_indent = self.indent(indent_level);
        let mut result = current_indent.clone();

        // Add visibility if present
        if let Some(visibility) = &func.visibility {
            result.push_str(&format!("{} ", format_visibility(visibility)));
        }

        // Add static if applicable
        if func.is_static {
            result.push_str("static ");
        }

        // Add async if applicable
        if func.is_async {
            result.push_str("async ");
        }

        // Add abstract if applicable
        if func.is_abstract {
            result.push_str("abstract ");
        }

        // Add function keyword and name
        result.push_str("fn ");
        result.push_str(&func.name);
        result.push_str(&self.format_generics(&func.generics));
        result.push('(');

        // Add parameters
        let param_strs: Vec<String> = func
            .params
            .iter()
            .map(|param| self.format_param(param))
            .collect();
        result.push_str(&param_strs.join(", "));

        result.push_str(") ");

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

fn format_visibility(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public => "",
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
    let mut parser = Parser::new(tokens);
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
    };

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

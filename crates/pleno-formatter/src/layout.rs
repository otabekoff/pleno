//! Line breaking. A construct is printed on one line when that line fits in
//! the limit; otherwise the outermost breakable construct is broken, one
//! element per line, and its elements are laid out the same way.
//!
//! Every layout function takes the indent level of the line the construct
//! starts on, the column it starts at and the width of the text that follows
//! it on its last line (`suffix`, such as `;` or `) {`).

use crate::Formatter;
use pleno_ast::*;

/// One element of a bracketed list.
pub(crate) struct Item<'a> {
    /// Written before the element, such as `name: ` or `...`.
    pub prefix: String,
    pub expr: Option<&'a Expr>,
    /// The element itself when it is not an expression.
    pub text: String,
    /// Where the element is in the source, so comments around it stay there.
    pub span: Option<Span>,
    /// Whether the parser accepts a comma after this element when it is last.
    pub comma_ok: bool,
}

impl<'a> Item<'a> {
    pub fn expr(prefix: impl Into<String>, expr: &'a Expr) -> Self {
        Self {
            prefix: prefix.into(),
            expr: Some(expr),
            text: String::new(),
            span: Some(expr.span),
            comma_ok: true,
        }
    }

    pub fn text(text: String, span: Option<Span>) -> Self {
        Self {
            prefix: String::new(),
            expr: None,
            text,
            span,
            comma_ok: true,
        }
    }
}

/// Binding strength of a binary operator; operators of one level form a chain.
fn precedence(op: &BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 1,
        BinaryOp::NullCoalesce => 2,
        BinaryOp::And => 3,
        BinaryOp::Equal | BinaryOp::NotEqual => 4,
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => 5,
        BinaryOp::Add | BinaryOp::Subtract => 6,
        BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Modulo => 7,
    }
}

pub(crate) fn binary_op_str(op: &BinaryOp) -> &'static str {
    match op {
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
    }
}

/// Collects the operands and operators of a chain like `a + b - c`.
fn flatten_chain<'a>(expr: &'a Expr, operands: &mut Vec<&'a Expr>, ops: &mut Vec<&'static str>) {
    match &expr.kind {
        ExprKind::Binary { left, op, right } => {
            let level = precedence(op);
            match &left.kind {
                ExprKind::Binary { op: inner, .. } if precedence(inner) == level => {
                    flatten_chain(left, operands, ops)
                }
                _ => operands.push(left),
            }
            ops.push(binary_op_str(op));
            operands.push(right);
        }
        ExprKind::Pipeline { left, right } => {
            match &left.kind {
                ExprKind::Pipeline { .. } => flatten_chain(left, operands, ops),
                _ => operands.push(left),
            }
            ops.push("|>");
            operands.push(right);
        }
        _ => operands.push(expr),
    }
}

fn is_member(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Property { .. } | ExprKind::SafeProperty { .. }
    )
}

fn is_call(expr: &Expr) -> bool {
    matches!(expr.kind, ExprKind::Call { .. } | ExprKind::SafeCall { .. })
}

impl Formatter {
    /// Display width of `text`; a tab counts as one indent.
    pub(crate) fn width(&self, text: &str) -> usize {
        text.chars()
            .map(|ch| if ch == '\t' { self.indent_size } else { 1 })
            .sum()
    }

    /// The column after `text` when it starts at `col`.
    pub(crate) fn end_col(&self, text: &str, col: usize) -> usize {
        match text.rfind('\n') {
            Some(pos) => self.width(&text[pos + 1..]),
            None => col + self.width(text),
        }
    }

    /// Whether `text`, starting at `col` and followed by `suffix` columns,
    /// stays within the limit. Only the first and last line count for text
    /// that spans lines: the lines in between belong to nested blocks, which
    /// are laid out on their own.
    pub(crate) fn fits(&self, text: &str, col: usize, suffix: usize) -> bool {
        match (text.find('\n'), text.rfind('\n')) {
            (Some(first), Some(last)) => {
                col + self.width(&text[..first]) <= self.max_line_length
                    && self.width(&text[last + 1..]) + suffix <= self.max_line_length
            }
            _ => col + self.width(text) + suffix <= self.max_line_length,
        }
    }

    fn snapshot(&self) -> (usize, bool) {
        (self.comment_index, self.at_block_start)
    }

    fn restore(&mut self, (comment_index, at_block_start): (usize, bool)) {
        self.comment_index = comment_index;
        self.at_block_start = at_block_start;
    }

    fn line_of(&self, offset: usize) -> Option<usize> {
        if self.line_starts.is_empty() {
            return None;
        }
        Some(match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        })
    }

    fn same_line(&self, a: usize, b: usize) -> bool {
        matches!((self.line_of(a), self.line_of(b)), (Some(x), Some(y)) if x == y)
    }

    /// Whether a comment that is not yet written lies in `region` but outside
    /// every item, so the list holding the items cannot be printed flat.
    pub(crate) fn has_loose_comment(&self, region: Span, items: &[Item]) -> bool {
        self.comments[self.comment_index.min(self.comments.len())..]
            .iter()
            .take_while(|comment| comment.span.start < region.end)
            .filter(|comment| comment.span.start >= region.start)
            .any(|comment| {
                !items.iter().any(|item| {
                    item.span.is_some_and(|span| {
                        span.start <= comment.span.start && comment.span.start < span.end
                    })
                })
            })
    }

    /// Marks the current flat attempt as failed when `region` holds a comment
    /// outside `items`.
    pub(crate) fn check_loose_comment(&mut self, region: Span, items: &[Item]) {
        if self.has_loose_comment(region, items) {
            self.flat_failed = true;
        }
    }

    /// Writes the comments before `limit`, each on its own line at `indent`.
    pub(crate) fn own_line_comments(&mut self, limit: usize, indent: usize) -> String {
        let mut out = String::new();
        while let Some(comment) = self.comments.get(self.comment_index) {
            if comment.span.start >= limit {
                break;
            }
            out.push_str(&self.indent(indent));
            out.push_str(&comment_text(comment));
            out.push('\n');
            self.comment_index += 1;
        }
        out
    }

    /// Writes the comments after `after` (and before `before`) that are on
    /// the same source line as `after`, as ` // comment`.
    pub(crate) fn trailing_comments(&mut self, after: usize, before: usize) -> String {
        let mut out = String::new();
        while let Some(comment) = self.comments.get(self.comment_index) {
            let start = comment.span.start;
            if start < after || start >= before || !self.same_line(after, start) {
                break;
            }
            out.push(' ');
            out.push_str(&comment_text(comment));
            self.comment_index += 1;
            if comment.kind != CommentKind::Block {
                break;
            }
        }
        out
    }

    /// The comments before an element at `start`: block comments on its own
    /// line stay in front of it (added to `line`), the rest go on lines above.
    fn leading_comments(&mut self, start: usize, indent: usize, line: &mut String) -> String {
        let mut out = String::new();
        while let Some(comment) = self.comments.get(self.comment_index) {
            if comment.span.start >= start {
                break;
            }
            let text = comment_text(comment);
            if comment.kind == CommentKind::Block && self.same_line(comment.span.end, start) {
                line.push_str(&text);
                line.push(' ');
            } else {
                out.push_str(&self.indent(indent));
                out.push_str(&text);
                out.push('\n');
            }
            self.comment_index += 1;
        }
        out
    }

    /// The elements on one line, as `a, b, c`.
    pub(crate) fn flat_items(&mut self, items: &[Item], indent: usize) -> String {
        let mut parts = Vec::with_capacity(items.len());
        for item in items {
            parts.push(match item.expr {
                Some(expr) => format!("{}{}", item.prefix, self.format_expression(expr, indent)),
                None => format!("{}{}", item.prefix, item.text),
            });
        }
        parts.join(", ")
    }

    fn fit_item(&mut self, item: &Item, indent: usize, col: usize, suffix: usize) -> String {
        match item.expr {
            Some(expr) => {
                let col = col + self.width(&item.prefix);
                format!("{}{}", item.prefix, self.fit(expr, indent, col, suffix))
            }
            None => format!("{}{}", item.prefix, item.text),
        }
    }

    /// A list on one line when it fits, otherwise one element per line.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn list(
        &mut self,
        open: &str,
        items: &[Item],
        close: &str,
        trailing_comma: bool,
        region: Span,
        indent: usize,
        col: usize,
        suffix: usize,
    ) -> String {
        let snapshot = self.snapshot();
        let loose = self.has_loose_comment(region, items);
        let outer = std::mem::replace(&mut self.flat_failed, false);
        let flat = format!("{open}{}{close}", self.flat_items(items, indent));
        let failed = std::mem::replace(&mut self.flat_failed, outer) || loose;
        if !failed && self.fits(&flat, col, suffix) {
            return flat;
        }
        self.restore(snapshot);
        self.broken_list(open, items, close, trailing_comma, region.end, indent)
    }

    /// One element per line, indented one level, each followed by a comma
    /// (the last one only when `trailing_comma` is set and the parser takes
    /// it there). Comments between the elements stay next to them.
    pub(crate) fn broken_list(
        &mut self,
        open: &str,
        items: &[Item],
        close: &str,
        trailing_comma: bool,
        end: usize,
        indent: usize,
    ) -> String {
        let inner = indent + 1;
        let pad = self.indent(inner);
        let mut out = format!("{open}\n");
        for (i, item) in items.iter().enumerate() {
            let mut line = pad.clone();
            if let Some(span) = item.span {
                out.push_str(&self.leading_comments(span.start, inner, &mut line));
            }
            let last = i + 1 == items.len();
            let comma = !last || (trailing_comma && item.comma_ok);
            let col = self.width(&line);
            line.push_str(&self.fit_item(item, inner, col, usize::from(comma)));
            if comma {
                line.push(',');
            }
            if let Some(span) = item.span {
                let next = items[i + 1..]
                    .iter()
                    .find_map(|item| item.span)
                    .map_or(end, |span| span.start);
                line.push_str(&self.trailing_comments(span.end, next));
            }
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str(&self.own_line_comments(end, inner));
        out.push_str(&self.indent(indent));
        out.push_str(close);
        out
    }

    /// Formats `expr` on one line and returns it when it fits.
    fn try_flat(
        &mut self,
        expr: &Expr,
        indent: usize,
        col: usize,
        suffix: usize,
    ) -> Option<String> {
        let snapshot = self.snapshot();
        let outer = std::mem::replace(&mut self.flat_failed, false);
        let flat = self.format_expression(expr, indent);
        let failed = std::mem::replace(&mut self.flat_failed, outer);
        if !failed && self.fits(&flat, col, suffix) {
            return Some(flat);
        }
        self.restore(snapshot);
        None
    }

    /// Formats `expr`, breaking it over several lines when it does not fit.
    pub(crate) fn fit(&mut self, expr: &Expr, indent: usize, col: usize, suffix: usize) -> String {
        let breakable = match &expr.kind {
            ExprKind::ArrayLiteral(items) | ExprKind::SetLiteral(items) => !items.is_empty(),
            ExprKind::TupleLiteral(items) => !items.is_empty(),
            ExprKind::MapLiteral(items) => !items.is_empty(),
            ExprKind::NamedTupleLiteral(items) => !items.is_empty(),
            ExprKind::If { else_branch, .. } => else_branch.is_some(),
            ExprKind::Lambda { body, .. } => !matches!(body.kind, ExprKind::Block(_)),
            ExprKind::Call { .. }
            | ExprKind::SafeCall { .. }
            | ExprKind::MacroCall { .. }
            | ExprKind::Property { .. }
            | ExprKind::SafeProperty { .. }
            | ExprKind::Index { .. }
            | ExprKind::SafeIndex { .. }
            | ExprKind::AssertNotNull(_)
            | ExprKind::Binary { .. }
            | ExprKind::Pipeline { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Await { .. }
            | ExprKind::Spread(_)
            | ExprKind::Paren(_)
            | ExprKind::Is { .. } => true,
            _ => false,
        };
        if !breakable {
            return self.format_expression(expr, indent);
        }
        if let Some(flat) = self.try_flat(expr, indent, col, suffix) {
            return flat;
        }
        self.break_expr(expr, indent, col, suffix)
    }

    fn break_expr(&mut self, expr: &Expr, indent: usize, col: usize, suffix: usize) -> String {
        match &expr.kind {
            ExprKind::Call { .. }
            | ExprKind::SafeCall { .. }
            | ExprKind::Property { .. }
            | ExprKind::SafeProperty { .. }
            | ExprKind::Index { .. }
            | ExprKind::SafeIndex { .. }
            | ExprKind::AssertNotNull(_) => match self.break_chain(expr, indent, col, suffix) {
                Some(chain) => chain,
                None => self.break_postfix(expr, indent, col, suffix),
            },
            ExprKind::MacroCall { name, args } => {
                let head = format!("{name}!");
                let items = Self::call_items(args);
                let region = Span::new(expr.span.start + name.len(), expr.span.end);
                let col = col + self.width(&head);
                let args = self.list("(", &items, ")", true, region, indent, col, suffix);
                head + &args
            }
            ExprKind::Binary { .. } | ExprKind::Pipeline { .. } => {
                self.break_binary(expr, indent, col, suffix)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch: Some(else_branch),
            } => {
                let inner = indent + 1;
                let pad = self.indent(inner);
                let branch_col = self.width(&pad) + 2;
                let mut out = self.fit(condition, indent, col, 0);
                let then = self.fit(then_branch, inner, branch_col, 0);
                out.push_str(&format!("\n{pad}? {then}"));
                let otherwise = self.fit(else_branch, inner, branch_col, suffix);
                out.push_str(&format!("\n{pad}: {otherwise}"));
                out
            }
            ExprKind::ArrayLiteral(items) => {
                let items: Vec<Item> = items.iter().map(|e| Item::expr("", e)).collect();
                self.broken_list("[", &items, "]", true, expr.span.end, indent)
            }
            ExprKind::SetLiteral(items) => {
                let items: Vec<Item> = items.iter().map(|e| Item::expr("", e)).collect();
                self.broken_list("{", &items, "}", true, expr.span.end, indent)
            }
            ExprKind::TupleLiteral(items) => {
                let items: Vec<Item> = items.iter().map(|e| Item::expr("", e)).collect();
                self.broken_list("(", &items, ")", true, expr.span.end, indent)
            }
            ExprKind::NamedTupleLiteral(entries) => {
                let items = Self::named_tuple_items(entries);
                self.broken_list("(", &items, ")", true, expr.span.end, indent)
            }
            ExprKind::MapLiteral(entries) => {
                let items = Self::map_items(entries);
                self.broken_list("{", &items, "}", true, expr.span.end, indent)
            }
            ExprKind::Unary { op, expr: inner } => match op {
                UnaryOp::PostIncrement | UnaryOp::PostDecrement => {
                    let op = if *op == UnaryOp::PostIncrement {
                        "++"
                    } else {
                        "--"
                    };
                    self.fit(inner, indent, col, suffix + 2) + op
                }
                _ => {
                    let op = match op {
                        UnaryOp::Negate => "-",
                        UnaryOp::Not => "!",
                        UnaryOp::PreIncrement => "++",
                        _ => "--",
                    };
                    op.to_string() + &self.fit(inner, indent, col + op.len(), suffix)
                }
            },
            ExprKind::Await { expr: inner } => {
                "await ".to_string() + &self.fit(inner, indent, col + 6, suffix)
            }
            ExprKind::Spread(inner) => {
                "...".to_string() + &self.fit(inner, indent, col + 3, suffix)
            }
            ExprKind::Paren(inner) => format!("({})", self.fit(inner, indent, col + 1, suffix + 1)),
            ExprKind::Is { left, ty } => {
                let tail = format!(" is {ty}");
                let suffix = suffix + self.width(&tail);
                self.fit(left, indent, col, suffix) + &tail
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                annotations,
                is_async,
            } => {
                let (head, arrow) =
                    self.lambda_head(params, return_ty, annotations, *is_async, indent);
                let head = if arrow { head } else { head + " =>" };
                let body_col = col + self.width(&head) + 1;
                format!("{head} {}", self.fit(body, indent, body_col, suffix))
            }
            _ => self.format_expression(expr, indent),
        }
    }

    /// `a.b(x).c(y)` with two or more method calls, one call per line:
    ///
    /// ```text
    /// a
    ///     .b(x)
    ///     .c(y)
    /// ```
    fn break_chain(
        &mut self,
        expr: &Expr,
        indent: usize,
        col: usize,
        suffix: usize,
    ) -> Option<String> {
        // Each link of the chain with the expression it applies to, innermost first.
        let mut links: Vec<(&Expr, &Expr)> = Vec::new();
        let mut current = expr;
        loop {
            let operand = match &current.kind {
                ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => object,
                ExprKind::Call { callee, .. } | ExprKind::SafeCall { callee, .. } => callee,
                ExprKind::Index { collection, .. } | ExprKind::SafeIndex { collection, .. } => {
                    collection
                }
                ExprKind::AssertNotNull(inner) => inner,
                _ => break,
            };
            links.push((current, operand));
            current = operand;
        }
        links.reverse();

        // The chain starts at the first `.name(...)`; what comes before it
        // stays on the first line. A new group starts at each `.name` after a call.
        let first = (0..links.len().saturating_sub(1))
            .find(|&i| is_member(links[i].0) && is_call(links[i + 1].0))?;
        let mut groups: Vec<Vec<&Expr>> = Vec::new();
        for &(node, _) in &links[first..] {
            let new_group = is_member(node)
                && groups
                    .last()
                    .is_none_or(|group| group.iter().any(|node| is_call(node)));
            if new_group {
                groups.push(Vec::new());
            }
            groups.last_mut()?.push(node);
        }
        let calls = groups
            .iter()
            .filter(|group| group.iter().any(|node| is_call(node)))
            .count();
        if calls < 2 {
            return None;
        }

        let head = links[first].1;
        let mut out = self.fit(head, indent, col, 0);
        let inner = indent + 1;
        let pad = self.indent(inner);
        let mut previous_end = head.span.end;
        for (g, group) in groups.iter().enumerate() {
            let name_end = group[0].span.end;
            out.push_str(&self.trailing_comments(previous_end, name_end));
            out.push('\n');
            out.push_str(&self.own_line_comments(name_end, inner));
            let mut line = pad.clone();
            for (l, node) in group.iter().enumerate() {
                let last = g + 1 == groups.len() && l + 1 == group.len();
                let link_col = self.end_col(&line, 0);
                let link = self.link_text(node, inner, link_col, if last { suffix } else { 0 });
                line.push_str(&link);
            }
            out.push_str(&line);
            previous_end = group.last().map_or(previous_end, |node| node.span.end);
        }
        Some(out)
    }

    /// The part a postfix expression adds to its operand: `.name`, `(args)`,
    /// `[index]` or `!`.
    fn link_text(&mut self, node: &Expr, indent: usize, col: usize, suffix: usize) -> String {
        match &node.kind {
            ExprKind::Property { name, .. } => format!(".{name}"),
            ExprKind::SafeProperty { name, .. } => format!("?.{name}"),
            ExprKind::Call { callee, args } => {
                let items = Self::call_items(args);
                let region = Span::new(callee.span.end, node.span.end);
                self.list("(", &items, ")", true, region, indent, col, suffix)
            }
            ExprKind::SafeCall { callee, args } => {
                let items = Self::call_items(args);
                let region = Span::new(callee.span.end, node.span.end);
                let args = self.list("(", &items, ")", true, region, indent, col + 2, suffix);
                format!("?.{args}")
            }
            ExprKind::Index { index, .. } => format!("[{}]", self.format_expression(index, indent)),
            ExprKind::SafeIndex { index, .. } => {
                format!("?.[{}]", self.format_expression(index, indent))
            }
            ExprKind::AssertNotNull(_) => "!".to_string(),
            _ => String::new(),
        }
    }

    /// A postfix expression that is not a method chain: its operand is laid
    /// out first, then the arguments are broken if they still do not fit.
    fn break_postfix(&mut self, expr: &Expr, indent: usize, col: usize, suffix: usize) -> String {
        let (operand, tail_width) = match &expr.kind {
            ExprKind::Call { callee, .. } => (callee, 1),
            ExprKind::SafeCall { callee, .. } => (callee, 3),
            ExprKind::Property { object, name } => (object, name.len() + 1),
            ExprKind::SafeProperty { object, name } => (object, name.len() + 2),
            ExprKind::Index { collection, .. } => (collection, 2),
            ExprKind::SafeIndex { collection, .. } => (collection, 4),
            ExprKind::AssertNotNull(inner) => (inner, 1),
            _ => return self.format_expression(expr, indent),
        };
        let tail_suffix = if is_call(expr) {
            tail_width
        } else {
            suffix + tail_width
        };
        let head = self.fit(operand, indent, col, tail_suffix);
        // The call did not fit on one line; when its callee did, the
        // arguments are what is too long, so there is no use trying them flat.
        if !head.contains('\n')
            && let ExprKind::Call { args, .. } = &expr.kind
        {
            let items = Self::call_items(args);
            return head + &self.broken_list("(", &items, ")", true, expr.span.end, indent);
        }
        let link_col = self.end_col(&head, col);
        head + &self.link_text(expr, indent, link_col, suffix)
    }

    /// `a && b || c` broken before each operator of the outermost chain:
    ///
    /// ```text
    /// a && b
    ///     || c
    /// ```
    fn break_binary(&mut self, expr: &Expr, indent: usize, col: usize, suffix: usize) -> String {
        let mut operands = Vec::new();
        let mut ops = Vec::new();
        flatten_chain(expr, &mut operands, &mut ops);
        let inner = indent + 1;
        let pad = self.indent(inner);
        let mut out = self.fit(operands[0], indent, col, 0);
        for (i, op) in ops.iter().enumerate() {
            let operand = operands[i + 1];
            out.push_str(&self.trailing_comments(operands[i].span.end, operand.span.start));
            out.push('\n');
            out.push_str(&self.own_line_comments(operand.span.start, inner));
            let line = format!("{pad}{op} ");
            let last = i + 1 == ops.len();
            let operand_col = self.width(&line);
            let operand = self.fit(operand, inner, operand_col, if last { suffix } else { 0 });
            out.push_str(&line);
            out.push_str(&operand);
        }
        out
    }

    pub(crate) fn call_items(args: &[CallArg]) -> Vec<Item<'_>> {
        args.iter()
            .map(|arg| match arg {
                CallArg::Positional(expr) => Item::expr("", expr),
                CallArg::Named(name, expr) => Item::expr(format!("{name}: "), expr),
                CallArg::Placeholder => Item::text("_".to_string(), None),
                CallArg::Spread(expr) => Item::expr("...", expr),
            })
            .collect()
    }

    pub(crate) fn map_items(entries: &[MapItem]) -> Vec<Item<'_>> {
        entries
            .iter()
            .map(|entry| match entry {
                MapItem::Entry(key, value) => Item::expr(format!("\"{key}\": "), value),
                MapItem::Spread(expr) if matches!(expr.kind, ExprKind::MapLiteral(_)) => {
                    Item::expr("", expr)
                }
                MapItem::Spread(expr) => Item::expr("...", expr),
            })
            .collect()
    }

    pub(crate) fn named_tuple_items(entries: &[(String, Expr)]) -> Vec<Item<'_>> {
        entries
            .iter()
            .map(|(name, value)| Item::expr(format!("{name}: "), value))
            .collect()
    }
}

pub(crate) fn comment_text(comment: &Comment) -> String {
    match comment.kind {
        CommentKind::Line => format!("//{}", comment.content),
        CommentKind::Block => format!("/*{}*/", comment.content),
        CommentKind::Doc => format!("///{}", comment.content),
    }
}

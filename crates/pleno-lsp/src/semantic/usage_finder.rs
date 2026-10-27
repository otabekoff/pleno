use crate::semantic::ast_utils::{Node, find_node_at};
use pleno_ast::visitor::{self, Visitor};
use pleno_ast::{
    Expr, ExprKind, FunctionDecl, Pattern, PatternKind, Program, Span, Stmt, StmtKind,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Scope {
    parent: Option<usize>,
    definitions: HashMap<String, Span>,
    range: Span,
}

struct ScopeBuilder<'a> {
    scopes: Vec<Scope>,
    current_scope: usize,
    // We Map AST Node Spans to their scope index to handle searching
    scope_map: Vec<(Span, usize)>,
    phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> ScopeBuilder<'a> {
    fn new(program_span: Span) -> Self {
        let root = Scope {
            parent: None,
            definitions: HashMap::new(),
            range: program_span,
        };
        Self {
            scopes: vec![root],
            current_scope: 0,
            scope_map: Vec::new(),
            phantom: std::marker::PhantomData,
        }
    }

    fn enter_scope(&mut self, span: Span) {
        let new_scope = Scope {
            parent: Some(self.current_scope),
            definitions: HashMap::new(),
            range: span,
        };
        let index = self.scopes.len();
        self.scopes.push(new_scope);
        self.current_scope = index;
        self.scope_map.push((span, index));
    }

    fn exit_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    fn define(&mut self, name: &str, span: Span) {
        self.scopes[self.current_scope]
            .definitions
            .insert(name.to_string(), span);
    }
}

impl<'a> Visitor<'a> for ScopeBuilder<'a> {
    fn visit_function_decl(&mut self, decl: &'a FunctionDecl) {
        // Define function name in CURRENT scope (unless anonymous? Pleno fns always named in decl?)
        if decl.name_span.start != 0 {
            self.define(&decl.name, decl.name_span);
        }

        // Enter new scope for body
        self.enter_scope(decl.span);

        // Define params
        for param in &decl.params {
            self.define(&param.name, param.name_span);
        }

        // Visit body
        visitor::walk_function_decl(self, decl);

        self.exit_scope();
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let StmtKind::Block(stmts) = &stmt.kind {
            self.enter_scope(stmt.span);
            for s in stmts {
                self.visit_stmt(s);
            }
            self.exit_scope();
            return;
        }

        match &stmt.kind {
            StmtKind::Let {
                name,
                name_span,
                value,
                ..
            } => {
                // Define variable
                self.define(name, *name_span);
                // Visit initializer
                if let Some(expr) = value {
                    self.visit_expr(expr);
                }
            }
            StmtKind::LetPattern { pattern, value, .. } => {
                self.visit_expr(value);
                for (name, span) in pattern.bindings() {
                    self.define(name, span);
                }
            }
            // `for ((k, v) in pairs)`: the names live in the loop's scope.
            StmtKind::For {
                pattern: Some(pattern),
                iterable,
                body,
                ..
            } => {
                self.visit_expr(iterable);
                self.enter_scope(stmt.span);
                for (name, span) in pattern.bindings() {
                    self.define(name, span);
                }
                self.visit_stmt(body);
                self.exit_scope();
            }
            // `with (f = open(path)) { ... }`: the names live in the statement's
            // scope, visible in later resources and the body.
            StmtKind::With { resources, body } => {
                self.enter_scope(stmt.span);
                for resource in resources {
                    self.visit_expr(&resource.value);
                    self.define(&resource.name, resource.name_span);
                }
                self.visit_stmt(body);
                self.exit_scope();
            }
            // `fn` statements are not `FunctionDecl`s: define the name and
            // give the parameters a scope the same way.
            StmtKind::Fn {
                name,
                name_span,
                params,
                body,
                ..
            } => {
                self.define(name, *name_span);
                self.enter_scope(stmt.span);
                for param in params {
                    self.define(&param.name, param.name_span);
                    self.visit_param(param);
                }
                self.visit_stmt(body);
                self.exit_scope();
            }
            StmtKind::Class {
                name, name_span, ..
            }
            | StmtKind::Interface {
                name, name_span, ..
            }
            | StmtKind::Enum(pleno_ast::EnumDecl {
                name, name_span, ..
            }) => {
                self.define(name, *name_span);
                visitor::walk_stmt(self, stmt);
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        match &pattern.kind {
            PatternKind::Ident(name) => {
                self.define(name, pattern.span);
            }
            _ => visitor::walk_pattern(self, pattern),
        }
    }
}

fn build_scopes(program: &Program) -> ScopeBuilder<'_> {
    let mut builder = ScopeBuilder::new(pleno_ast::Span::new(0, 0)); // Program has no span? Use explicit?
    // Program doesn't have a span field in struct definition I saw
    // Use 0..max?
    // Let's assume root scope covers everything.
    builder.visit_program(program);
    builder
}

fn find_scope_for_pos(builder: &ScopeBuilder, pos: usize) -> usize {
    // Find the smallest scope range that contains pos
    let mut best_match = 0; // Root
    let mut smallest_len = usize::MAX;

    for (i, scope) in builder.scopes.iter().enumerate() {
        // Simple containment check.
        // For root scope (program), we might not have a Span.
        // If scope.range is default (0,0), assume it's root?
        // Or specific logic.
        let contains = if i == 0 {
            true
        } else {
            scope.range.contains(pos)
        };

        if contains {
            let len = scope.range.end - scope.range.start;
            // If len is 0 (default), treat as max (root)
            let len = if len == 0 { usize::MAX } else { len };

            if len < smallest_len {
                smallest_len = len;
                best_match = i;
            }
        }
    }
    best_match
}

fn resolve_in_scope(builder: &ScopeBuilder, start_scope: usize, name: &str) -> Option<Span> {
    let mut current = start_scope;
    loop {
        if let Some(span) = builder.scopes[current].definitions.get(name) {
            return Some(*span);
        }
        if let Some(parent) = builder.scopes[current].parent {
            current = parent;
        } else {
            break;
        }
    }
    None
}

/// Helper to get the name from a node if it is adaptable to a reference/definition
fn get_node_name(node: &Node, pos: usize) -> Option<String> {
    match node {
        Node::Expr(e) => match &e.kind {
            ExprKind::Ident(name) => Some(name.clone()),
            _ => None,
        },
        Node::Pattern(p) => match &p.kind {
            PatternKind::Ident(name) => Some(name.clone()),
            _ => None,
        },
        Node::Function(f) => Some(f.name.clone()),
        Node::Field(f) => Some(f.name.clone()),
        Node::Param(p) => Some(p.name.clone()),
        Node::EnumVariant(v) => Some(v.name.clone()),
        // Stmt declarations
        Node::Stmt(s) => match &s.kind {
            StmtKind::Class { name, .. } | StmtKind::Interface { name, .. } => Some(name.clone()),
            StmtKind::Enum(e) => Some(e.name.clone()),
            StmtKind::Let { name, .. } => Some(name.clone()),
            StmtKind::LetPattern { pattern, .. }
            | StmtKind::For {
                pattern: Some(pattern),
                ..
            } => binding_at(pattern, pos).map(|(name, _)| name),
            StmtKind::With { resources, .. } => {
                with_resource_at(resources, pos).map(|resource| resource.name.clone())
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn find_declaration(program: &Program, pos: usize) -> Option<Span> {
    let node = find_node_at(program, pos)?;
    let name = get_node_name(&node, pos)?;

    // Check if we are ON the definition
    if let Node::Function(f) = node
        && f.name_span.contains(pos)
    {
        return Some(f.name_span);
    }
    // ... params, classes, etc. check ...
    if let Node::Stmt(s) = node {
        match &s.kind {
            StmtKind::Let { name_span, .. }
            | StmtKind::Class { name_span, .. }
            | StmtKind::Interface { name_span, .. } => {
                if name_span.contains(pos) {
                    return Some(*name_span);
                }
            }
            StmtKind::Enum(e) if e.name_span.contains(pos) => {
                return Some(e.name_span);
            }
            StmtKind::LetPattern { pattern, .. }
            | StmtKind::For {
                pattern: Some(pattern),
                ..
            } => {
                if let Some((_, span)) = binding_at(pattern, pos) {
                    return Some(span);
                }
            }
            StmtKind::With { resources, .. } => {
                if let Some(resource) = with_resource_at(resources, pos) {
                    return Some(resource.name_span);
                }
            }
            _ => {}
        }
    }
    if let Node::Param(p) = node
        && p.name_span.contains(pos)
    {
        return Some(p.name_span);
    }

    // Build scopes
    let builder = build_scopes(program);
    let scope_idx = find_scope_for_pos(&builder, pos);

    resolve_in_scope(&builder, scope_idx, &name)
}

/// The resource of a `with` whose name is at `pos`.
fn with_resource_at(
    resources: &[pleno_ast::WithResource],
    pos: usize,
) -> Option<&pleno_ast::WithResource> {
    resources
        .iter()
        .find(|resource| resource.name_span.contains(pos))
}

/// The name a destructuring pattern binds at `pos`, with its span.
fn binding_at(pattern: &pleno_ast::BindingPattern, pos: usize) -> Option<(String, Span)> {
    pattern
        .bindings()
        .into_iter()
        .find(|(_, span)| span.contains(pos))
        .map(|(name, span)| (name.to_string(), span))
}

struct UsageVisitor<'a> {
    builder: &'a ScopeBuilder<'a>,
    target_decl_span: Span,
    target_name: String,
    usages: Vec<Span>,
}

impl<'a, 'b> Visitor<'a> for UsageVisitor<'b> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let ExprKind::Ident(name) = &expr.kind
            && name == &self.target_name
        {
            let scope_idx = find_scope_for_pos(self.builder, expr.span.start);
            if let Some(decl) = resolve_in_scope(self.builder, scope_idx, &self.target_name)
                && decl == self.target_decl_span
            {
                self.usages.push(expr.span);
            }
        }
        visitor::walk_expr(self, expr);
    }
}

pub fn find_usages(program: &Program, pos: usize) -> Vec<Span> {
    let decl_span = find_declaration(program, pos);
    if let Some(decl) = decl_span {
        let node = find_node_at(program, pos)
            .map(|node| (node, pos))
            .or_else(|| find_node_at(program, decl.start).map(|node| (node, decl.start)));

        if let Some((node, node_pos)) = node
            && let Some(name) = get_node_name(&node, node_pos)
        {
            let builder = build_scopes(program);
            let mut visitor = UsageVisitor {
                builder: &builder,
                target_decl_span: decl,
                target_name: name,
                usages: vec![decl], // Definition is also a "usage" or reference
            };
            visitor.visit_program(program);
            // remove duplicates if definition was added twice (once manually, once by visitor if define site is also an Ident expr?)
            // A definition (e.g. fn foo) is NOT an Expr::Ident.
            // A Let { name } is NOT an Expr::Ident.
            // So manually adding decl is correct.
            return visitor.usages;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Program {
        let tokens = pleno_lexer::lex(source).unwrap();
        pleno_parser::Parser::new(tokens, source)
            .parse_program()
            .unwrap()
    }

    #[test]
    fn fn_statement_names_and_params_have_usages() {
        let source = "fn area(r) {\n    return r * r;\n}\nprint(area(2));";
        let program = parse(source);
        let r_decl = source.find("(r)").unwrap() + 1;
        assert_eq!(find_usages(&program, r_decl).len(), 3);
        let area_use = source.rfind("area").unwrap();
        assert_eq!(
            find_usages(&program, area_use),
            vec![Span::new(3, 7), Span::new(area_use, area_use + 4)]
        );
    }

    #[test]
    fn destructured_names_resolve_to_their_pattern() {
        let source = "let (a, [b, ...rest]) = (1, [2, 3]);\nprint(a + b + rest.length);";
        let program = parse(source);
        let a_decl = source.find('a').unwrap();
        let a_use = source.rfind("a +").unwrap();
        let rest_decl = source.find("rest").unwrap();
        let rest_use = source.rfind("rest").unwrap();

        assert_eq!(
            find_declaration(&program, a_use),
            Some(Span::new(a_decl, a_decl + 1))
        );
        assert_eq!(
            find_declaration(&program, rest_use),
            Some(Span::new(rest_decl, rest_decl + 4))
        );
        // From the definition inside the pattern: the definition and the use.
        assert_eq!(
            find_declaration(&program, a_decl),
            Some(Span::new(a_decl, a_decl + 1))
        );
        let usages = find_usages(&program, a_decl);
        assert_eq!(
            usages,
            vec![Span::new(a_decl, a_decl + 1), Span::new(a_use, a_use + 1)]
        );
    }

    #[test]
    fn with_names_resolve_to_their_resource() {
        let source = "let f = 0;\nwith (f = open(), g = wrap(f)) {\n    print(f, g);\n}\nprint(f);";
        let program = parse(source);
        let f_decl = source.find("(f =").unwrap() + 1;
        let f_in_g = source.find("wrap(f)").unwrap() + 5;
        let f_use = source.find("print(f, g)").unwrap() + 6;
        let g_decl = source.find("g =").unwrap();
        let g_use = f_use + 3;
        let outer_f = source.find("let f").unwrap() + 4;
        let outer_use = source.rfind("f)").unwrap();

        assert_eq!(
            find_declaration(&program, f_use),
            Some(Span::new(f_decl, f_decl + 1))
        );
        assert_eq!(
            find_declaration(&program, g_use),
            Some(Span::new(g_decl, g_decl + 1))
        );
        // From the name in `with (...)`: the declaration itself.
        assert_eq!(
            find_declaration(&program, f_decl),
            Some(Span::new(f_decl, f_decl + 1))
        );
        // The names do not leak out of the statement.
        assert_eq!(
            find_declaration(&program, outer_use),
            Some(Span::new(outer_f, outer_f + 1))
        );
        assert_eq!(
            find_usages(&program, f_decl),
            vec![
                Span::new(f_decl, f_decl + 1),
                Span::new(f_in_g, f_in_g + 1),
                Span::new(f_use, f_use + 1)
            ]
        );
    }

    #[test]
    fn for_pattern_names_resolve_to_their_pattern() {
        let source = "let k = 0;\nfor ((k, v) in [(1, 2)]) {\n    print(k + v);\n}\nprint(k);";
        let program = parse(source);
        let k_decl = source.find("(k,").unwrap() + 1;
        let k_use = source.find("k + v").unwrap();
        let v_use = k_use + 4;
        let v_decl = source.find("v)").unwrap();
        let outer_k = source.find("let k").unwrap() + 4;
        let outer_use = source.rfind("k)").unwrap();

        assert_eq!(
            find_declaration(&program, k_use),
            Some(Span::new(k_decl, k_decl + 1))
        );
        assert_eq!(
            find_declaration(&program, v_use),
            Some(Span::new(v_decl, v_decl + 1))
        );
        // The loop's `k` does not leak out of the loop.
        assert_eq!(
            find_declaration(&program, outer_use),
            Some(Span::new(outer_k, outer_k + 1))
        );
        assert_eq!(
            find_usages(&program, k_decl),
            vec![Span::new(k_decl, k_decl + 1), Span::new(k_use, k_use + 1)]
        );
    }
}

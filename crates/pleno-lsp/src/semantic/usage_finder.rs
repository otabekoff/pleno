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
fn get_node_name(node: &Node) -> Option<String> {
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
            _ => None,
        },
        _ => None,
    }
}

pub fn find_declaration(program: &Program, pos: usize) -> Option<Span> {
    let node = find_node_at(program, pos)?;
    let name = get_node_name(&node)?;

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
            StmtKind::Enum(e) => {
                if e.name_span.contains(pos) {
                    return Some(e.name_span);
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
        let node = find_node_at(program, pos).or_else(|| find_node_at(program, decl.start));

        if let Some(node) = node
            && let Some(name) = get_node_name(&node)
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

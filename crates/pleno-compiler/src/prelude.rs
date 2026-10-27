//! Program-level analysis the compiler runs before compiling.

use pleno_ast::visitor::{
    Visitor, walk_annotation, walk_expr, walk_field_decl, walk_function_decl, walk_param, walk_stmt,
};
use pleno_ast::{Expr, ExprKind, FieldDecl, FunctionDecl, Param, Stmt, StmtKind};

fn declared_name(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Export(inner) => declared_name(inner),
        StmtKind::Class { name, .. } | StmtKind::Fn { name, .. } | StmtKind::Let { name, .. } => {
            Some(name)
        }
        _ => None,
    }
}

/// Finds the variables of a block that a nested function (a `fn`, a lambda,
/// or a method of a class, enum or `extend` declared in the block) can see
/// change: names used inside a nested function that are also assigned
/// somewhere in the block. Such a variable has to live in a shared cell, or the
/// function keeps the value it had when the function was created.
///
/// A parameter of the nested function shadows the outer name. A local the
/// nested function declares with `let` is not tracked, so such a name still
/// counts: that only costs a cell, never correctness.
#[derive(Default)]
pub struct CaptureScan {
    depth: usize,
    /// The parameters of each nested function being visited.
    params: Vec<Vec<String>>,
    used_in_nested: std::collections::HashSet<String>,
    assigned: std::collections::HashSet<String>,
}

impl CaptureScan {
    pub fn shared_names(stmts: &[Stmt]) -> std::collections::HashSet<String> {
        let mut scan = CaptureScan::default();
        for stmt in stmts {
            scan.visit_stmt(stmt);
        }
        scan.used_in_nested
            .intersection(&scan.assigned)
            .cloned()
            .collect()
    }

    fn note_use(&mut self, name: &str) {
        if self.depth > 0 && !self.params.iter().flatten().any(|param| param == name) {
            self.used_in_nested.insert(name.to_string());
        }
    }
}

impl<'ast> Visitor<'ast> for CaptureScan {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if let StmtKind::Assign { name, .. } = &stmt.kind {
            self.assigned.insert(name.clone());
            self.note_use(name);
        }
        let nested = matches!(
            stmt.kind,
            StmtKind::Fn { .. }
                | StmtKind::Class { .. }
                | StmtKind::Enum(_)
                | StmtKind::Extend { .. }
        );
        let params = match &stmt.kind {
            StmtKind::Fn { params, .. } => params.iter().map(|p| p.name.clone()).collect(),
            _ => Vec::new(),
        };
        self.depth += usize::from(nested);
        self.params.push(params);
        walk_stmt(self, stmt);
        self.params.pop();
        self.depth -= usize::from(nested);
    }

    fn visit_function_decl(&mut self, decl: &'ast FunctionDecl) {
        self.params
            .push(decl.params.iter().map(|p| p.name.clone()).collect());
        walk_function_decl(self, decl);
        self.params.pop();
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => self.note_use(name),
            ExprKind::Unary {
                op:
                    pleno_ast::UnaryOp::PreIncrement
                    | pleno_ast::UnaryOp::PostIncrement
                    | pleno_ast::UnaryOp::PreDecrement
                    | pleno_ast::UnaryOp::PostDecrement,
                expr: target,
            } => {
                if let ExprKind::Ident(name) = &target.kind {
                    self.assigned.insert(name.clone());
                }
            }
            _ => {}
        }
        let nested = matches!(expr.kind, ExprKind::Lambda { .. });
        let params = match &expr.kind {
            ExprKind::Lambda { params, .. } => params.iter().map(|p| p.name.clone()).collect(),
            _ => Vec::new(),
        };
        self.depth += usize::from(nested);
        self.params.push(params);
        walk_expr(self, expr);
        self.params.pop();
        self.depth -= usize::from(nested);
    }
}

/// Collects every name a program refers to, so unreferenced top-level
/// declarations can be skipped. Besides identifiers this counts names that
/// only appear in decorators, type annotations, `extends`/`implements`
/// clauses, `extend` targets and export lists.
#[derive(Default)]
pub struct ReferenceCollector {
    pub names: std::collections::HashSet<String>,
}

impl ReferenceCollector {
    /// Records each identifier in a type annotation such as `Map<String, User>`.
    fn add_type(&mut self, ty: &str) {
        for name in ty.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if !name.is_empty() {
                self.names.insert(name.to_string());
            }
        }
    }
}

impl<'ast> Visitor<'ast> for ReferenceCollector {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Assign { name, .. } => {
                self.names.insert(name.clone());
            }
            StmtKind::Export(inner) => {
                if let Some(name) = declared_name(inner) {
                    self.names.insert(name.to_string());
                }
            }
            StmtKind::ExportList(items) => {
                for item in items {
                    self.names.insert(item.name.clone());
                }
            }
            StmtKind::Let { ty: Some(ty), .. } | StmtKind::LetPattern { ty: Some(ty), .. } => {
                self.add_type(ty)
            }
            StmtKind::Fn { annotations, .. } => {
                for annotation in annotations {
                    self.names.insert(annotation.name.clone());
                }
            }
            StmtKind::Class {
                superclass,
                implements,
                annotations,
                ..
            } => {
                self.names.extend(superclass.iter().cloned());
                self.names.extend(implements.iter().cloned());
                for annotation in annotations {
                    self.names.insert(annotation.name.clone());
                }
            }
            StmtKind::Interface { extends, .. } => {
                self.names.extend(extends.iter().cloned());
            }
            StmtKind::Extend { target, .. } => {
                self.names.insert(target.clone());
            }
            StmtKind::TypeAlias { ty, .. } => self.add_type(ty),
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                self.names.insert(name.clone());
            }
            ExprKind::Is { ty, .. } => self.add_type(ty),
            ExprKind::Specialize { type_args, .. } => {
                for ty in type_args {
                    self.add_type(ty);
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_param(&mut self, param: &'ast Param) {
        if let Some(ty) = &param.ty {
            self.add_type(ty);
        }
        walk_param(self, param);
    }

    fn visit_field_decl(&mut self, field: &'ast FieldDecl) {
        if let Some(ty) = &field.ty {
            self.add_type(ty);
        }
        walk_field_decl(self, field);
    }

    fn visit_function_decl(&mut self, decl: &'ast FunctionDecl) {
        if let Some(ty) = &decl.return_ty {
            self.add_type(ty);
        }
        walk_function_decl(self, decl);
    }

    fn visit_annotation(&mut self, annotation: &'ast pleno_ast::Annotation) {
        self.names.insert(annotation.name.clone());
        walk_annotation(self, annotation);
    }
}

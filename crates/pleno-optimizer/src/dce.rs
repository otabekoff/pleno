use pleno_ast::visitor::{walk_expr, walk_stmt, Visitor};
use pleno_ast::*;
use std::collections::HashSet;

pub struct DeadCodeEliminator {
    used_names: HashSet<String>,
}

impl Default for DeadCodeEliminator {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadCodeEliminator {
    pub fn new() -> Self {
        Self {
            used_names: HashSet::new(),
        }
    }

    pub fn eliminate_program(&mut self, program: &mut Program) {
        // Pass 1: Collect usages
        self.used_names.clear();
        let mut collector = UsageCollector {
            used_names: &mut self.used_names,
        };
        collector.visit_program(program);

        // Pass 2: Eliminate
        self.eliminate_stmts(&mut program.statements);
    }

    fn eliminate_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        let mut terminator_index = None;

        while i < stmts.len() {
            let stmt = &mut stmts[i];

            // Special check for unused variable removal
            if let StmtKind::Let {
                name, value, ty, ..
            } = &stmt.kind
            {
                // A type annotation is checked at runtime, so a typed `let` must stay.
                if !self.used_names.contains(name)
                    && ty.is_none()
                    && value.as_ref().is_none_or(is_side_effect_free)
                {
                    stmts.remove(i);
                    continue;
                }
            }

            self.eliminate_stmt(stmt);

            if self.is_terminator(stmt) {
                terminator_index = Some(i);
                break;
            }
            i += 1;
        }

        if let Some(idx) = terminator_index {
            if idx + 1 < stmts.len() {
                stmts.truncate(idx + 1);
            }
        }

        // Filter out empty blocks produced by simplification
        stmts.retain(|s| !matches!(&s.kind, StmtKind::Block(inner) if inner.is_empty()));
    }

    fn is_terminator(&self, stmt: &Stmt) -> bool {
        matches!(
            &stmt.kind,
            StmtKind::Return(_) | StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Throw(_)
        )
    }

    fn eliminate_stmt(&mut self, stmt: &mut Stmt) {
        match &mut stmt.kind {
            StmtKind::Block(stmts) => self.eliminate_stmts(stmts),
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.eliminate_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.eliminate_stmt(eb);
                }

                // Simplify if condition is constant bool
                if let ExprKind::Literal(Literal::Bool(val)) = &condition.kind {
                    if *val {
                        let new_stmt = (**then_branch).clone();
                        *stmt = new_stmt;
                        self.eliminate_stmt(stmt);
                    } else if let Some(eb) = else_branch {
                        let new_stmt = (**eb).clone();
                        *stmt = new_stmt;
                        self.eliminate_stmt(stmt);
                    } else {
                        stmt.kind = StmtKind::Block(vec![]);
                    }
                }
            }
            StmtKind::While { condition, body } => {
                self.eliminate_stmt(body);
                if let ExprKind::Literal(Literal::Bool(false)) = &condition.kind {
                    stmt.kind = StmtKind::Block(vec![]);
                }
            }
            StmtKind::Fn {
                body, annotations, ..
            } => {
                if !annotations.iter().any(|a| a.name == "no_optimize") {
                    self.eliminate_stmt(body);
                }
            }
            StmtKind::Class {
                methods,
                annotations,
                ..
            } => {
                if annotations.iter().any(|a| a.name == "no_optimize") {
                    return;
                }
                for m in methods {
                    self.eliminate_stmt(&mut m.body);
                }
            }
            StmtKind::Enum(enum_decl) => {
                if !enum_decl
                    .annotations
                    .iter()
                    .any(|a| a.name == "no_optimize")
                {
                    for m in &mut enum_decl.methods {
                        self.eliminate_stmt(&mut m.body);
                    }
                }
            }
            StmtKind::Module {
                body: Some(body), ..
            } => {
                self.eliminate_stmts(body);
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.eliminate_stmt(try_branch);
                for catch in catch_branches {
                    self.eliminate_stmt(&mut catch.body);
                }
                if let Some(fb) = finally_branch {
                    self.eliminate_stmt(fb);
                }
            }
            StmtKind::With { body, .. } => {
                self.eliminate_stmt(body);
            }
            StmtKind::Export(inner) => {
                self.eliminate_stmt(inner);
            }
            StmtKind::Labeled { stmt: inner, .. } => {
                self.eliminate_stmt(inner);
            }
            _ => {}
        }
    }
}

/// Records every name that is read, assigned or exported anywhere in the program.
struct UsageCollector<'a> {
    used_names: &'a mut HashSet<String>,
}

impl<'ast> Visitor<'ast> for UsageCollector<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Assign { name, .. } => {
                self.used_names.insert(name.clone());
            }
            StmtKind::ExportList(names) => {
                for export in names {
                    self.used_names.insert(export.name.clone());
                }
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let ExprKind::Ident(name) = &expr.kind {
            self.used_names.insert(name.clone());
        }
        walk_expr(self, expr);
    }
}

/// An unused `let` can only be dropped when evaluating its initializer cannot
/// have observable effects.
fn is_side_effect_free(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Lambda { .. } => true,
        ExprKind::Paren(inner) => is_side_effect_free(inner),
        ExprKind::ArrayLiteral(items) | ExprKind::TupleLiteral(items) => {
            items.iter().all(is_side_effect_free)
        }
        _ => false,
    }
}

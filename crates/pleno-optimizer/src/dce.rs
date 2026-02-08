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
        for stmt in &program.statements {
            self.collect_usages_stmt(stmt);
        }

        // Pass 2: Eliminate
        self.eliminate_stmts(&mut program.statements);
    }

    fn collect_usages_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.collect_usages_stmt(s);
                }
            }
            StmtKind::Let { value: Some(v), .. } => {
                self.collect_usages_expr(v);
            }
            StmtKind::Assign { value, .. } => {
                self.collect_usages_expr(value);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_usages_expr(condition);
                self.collect_usages_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.collect_usages_stmt(eb);
                }
            }
            StmtKind::While { condition, body } => {
                self.collect_usages_expr(condition);
                self.collect_usages_stmt(body);
            }
            StmtKind::For { iterable, body, .. } => {
                self.collect_usages_expr(iterable);
                self.collect_usages_stmt(body);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_usages_stmt(init);
                }
                if let Some(cond) = condition {
                    self.collect_usages_expr(cond);
                }
                if let Some(step) = step {
                    self.collect_usages_stmt(step);
                }
                self.collect_usages_stmt(body);
            }
            StmtKind::Fn { body, .. } => {
                self.collect_usages_stmt(body);
            }
            StmtKind::Class {
                methods,
                properties,
                nested_classes,
                ..
            } => {
                for m in methods {
                    self.collect_usages_stmt(&m.body);
                }
                for p in properties {
                    if let Some(g) = &p.getter {
                        self.collect_usages_stmt(&g.body);
                    }
                    if let Some(s) = &p.setter {
                        self.collect_usages_stmt(&s.body);
                    }
                }
                for n in nested_classes {
                    self.collect_usages_stmt(n);
                }
            }
            StmtKind::Enum(enum_decl) => {
                for m in &enum_decl.methods {
                    self.collect_usages_stmt(&m.body);
                }
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.collect_usages_stmt(try_branch);
                for catch in catch_branches {
                    self.collect_usages_stmt(&catch.body);
                }
                if let Some(fb) = finally_branch {
                    self.collect_usages_stmt(fb);
                }
            }
            StmtKind::Throw(expr) | StmtKind::Expr(expr) | StmtKind::Return(Some(expr)) => {
                self.collect_usages_expr(expr);
            }
            StmtKind::Export(inner) => self.collect_usages_stmt(inner),
            StmtKind::Labeled { stmt: inner, .. } => self.collect_usages_stmt(inner),
            _ => {}
        }
    }

    fn collect_usages_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                self.used_names.insert(name.clone());
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_usages_expr(left);
                self.collect_usages_expr(right);
            }
            ExprKind::Unary { expr: inner, .. } => self.collect_usages_expr(inner),
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.collect_usages_expr(callee);
                for arg in args {
                    match arg {
                        CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) => {
                            self.collect_usages_expr(e);
                        }
                        CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
                self.collect_usages_expr(object);
            }
            ExprKind::Index { collection, index } | ExprKind::SafeIndex { collection, index } => {
                self.collect_usages_expr(collection);
                self.collect_usages_expr(index);
            }
            ExprKind::ArrayLiteral(items)
            | ExprKind::TupleLiteral(items)
            | ExprKind::SetLiteral(items) => {
                for item in items {
                    self.collect_usages_expr(item);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        MapItem::Entry(_, v) => {
                            self.collect_usages_expr(v);
                        }
                        MapItem::Spread(e) => self.collect_usages_expr(e),
                    }
                }
            }
            ExprKind::Lambda { body, .. } => {
                self.collect_usages_expr(body);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_usages_expr(condition);
                self.collect_usages_expr(then_branch);
                if let Some(eb) = else_branch {
                    self.collect_usages_expr(eb);
                }
            }
            ExprKind::Await { expr: inner }
            | ExprKind::Spread(inner)
            | ExprKind::Paren(inner)
            | ExprKind::AssertNotNull(inner) => {
                self.collect_usages_expr(inner);
            }
            ExprKind::Pipeline { left, right } => {
                self.collect_usages_expr(left);
                self.collect_usages_expr(right);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.collect_usages_expr(part);
                }
            }
            _ => {}
        }
    }

    fn eliminate_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        let mut terminator_index = None;

        while i < stmts.len() {
            let stmt = &mut stmts[i];

            // Special check for unused variable removal
            if let StmtKind::Let { name, .. } = &stmt.kind {
                if !self.used_names.contains(name) {
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

use pleno_ast::*;

pub struct DeadCodeEliminator;

impl Default for DeadCodeEliminator {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadCodeEliminator {
    pub fn new() -> Self {
        Self
    }

    pub fn eliminate_program(&mut self, program: &mut Program) {
        self.eliminate_stmts(&mut program.statements);
    }

    fn eliminate_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        let mut i = 0;
        let mut terminator_index = None;

        while i < stmts.len() {
            let stmt = &mut stmts[i];
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
                        // Replace if with then_branch
                        let new_stmt = (**then_branch).clone();
                        *stmt = new_stmt;
                        // Recurse again in case the new stmt can be further eliminated
                        self.eliminate_stmt(stmt);
                    } else if let Some(eb) = else_branch {
                        // Replace if with else_branch
                        let new_stmt = (**eb).clone();
                        *stmt = new_stmt;
                        // Recurse again
                        self.eliminate_stmt(stmt);
                    } else {
                        // Replace if with empty block
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
            StmtKind::Fn { body, .. } => {
                self.eliminate_stmt(body);
            }
            StmtKind::Class { methods, .. } => {
                for m in methods {
                    self.eliminate_stmt(&mut m.body);
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

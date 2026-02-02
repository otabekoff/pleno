use pleno_ast::*;

pub struct ConstantFolder;

impl Default for ConstantFolder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstantFolder {
    pub fn new() -> Self {
        Self
    }

    pub fn fold_program(&mut self, program: &mut Program) {
        for stmt in &mut program.statements {
            self.fold_stmt(stmt);
        }
    }

    pub fn fold_stmt(&mut self, stmt: &mut Stmt) {
        match &mut stmt.kind {
            StmtKind::Module {
                body: Some(body), ..
            } => {
                for stmt in body {
                    self.fold_stmt(stmt);
                }
            }
            StmtKind::Module { body: None, .. } => {}
            StmtKind::Let { value: Some(v), .. } => {
                self.fold_expr(v);
            }
            StmtKind::Let { value: None, .. } => {}
            StmtKind::MacroDecl { body, .. } => {
                self.fold_stmt(body);
            }
            StmtKind::Assign { value, .. } => {
                self.fold_expr(value);
            }
            StmtKind::AssignProperty { object, value, .. } => {
                self.fold_expr(object);
                self.fold_expr(value);
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.fold_expr(collection);
                self.fold_expr(index);
                self.fold_expr(value);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.fold_expr(condition);
                self.fold_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.fold_stmt(eb);
                }
            }
            StmtKind::While { condition, body } => {
                self.fold_expr(condition);
                self.fold_stmt(body);
            }
            StmtKind::For { iterable, body, .. } => {
                self.fold_expr(iterable);
                self.fold_stmt(body);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.fold_stmt(init);
                }
                if let Some(cond) = condition {
                    self.fold_expr(cond);
                }
                if let Some(step) = step {
                    self.fold_stmt(step);
                }
                self.fold_stmt(body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.fold_stmt(body);
                self.fold_expr(condition);
            }
            StmtKind::Export(inner) => {
                self.fold_stmt(inner);
            }
            StmtKind::Class {
                methods,
                properties,
                nested_classes,
                ..
            } => {
                for m in methods {
                    self.fold_stmt(&mut m.body);
                }
                for p in properties {
                    if let Some(g) = &mut p.getter {
                        self.fold_stmt(&mut g.body);
                    }
                    if let Some(s) = &mut p.setter {
                        self.fold_stmt(&mut s.body);
                    }
                }
                for n in nested_classes {
                    self.fold_stmt(n);
                }
            }
            StmtKind::Interface {
                methods,
                properties,
                ..
            } => {
                for m in methods {
                    self.fold_stmt(&mut m.body);
                }
                for p in properties {
                    if let Some(g) = &mut p.getter {
                        self.fold_stmt(&mut g.body);
                    }
                    if let Some(s) = &mut p.setter {
                        self.fold_stmt(&mut s.body);
                    }
                }
            }
            StmtKind::Fn { body, .. } => {
                self.fold_stmt(body);
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.fold_stmt(try_branch);
                for catch in catch_branches {
                    self.fold_stmt(&mut catch.body);
                }
                if let Some(finally) = finally_branch {
                    self.fold_stmt(finally);
                }
            }
            StmtKind::Throw(expr) => {
                self.fold_expr(expr);
            }
            StmtKind::Return(Some(expr)) => {
                self.fold_expr(expr);
            }
            StmtKind::Return(None) => {}
            StmtKind::Labeled { stmt, .. } => {
                self.fold_stmt(stmt);
            }
            StmtKind::Block(stmts) => {
                for stmt in stmts {
                    self.fold_stmt(stmt);
                }
            }
            StmtKind::Extend { methods, .. } => {
                for m in methods {
                    self.fold_stmt(&mut m.body);
                }
            }
            StmtKind::Expr(expr) => {
                self.fold_expr(expr);
            }
            _ => {}
        }
    }

    pub fn fold_expr(&mut self, expr: &mut Expr) {
        match &mut expr.kind {
            ExprKind::Unary { op, expr: inner } => {
                self.fold_expr(inner);
                if let Some(folded) = self.try_fold_unary(op, inner) {
                    expr.kind = folded;
                }
            }
            ExprKind::Binary { left, op, right } => {
                self.fold_expr(left);
                self.fold_expr(right);
                if let Some(folded) = self.try_fold_binary(left, op, right) {
                    expr.kind = folded;
                }
            }
            ExprKind::Property { object, .. } => {
                self.fold_expr(object);
            }
            ExprKind::SafeProperty { object, .. } => {
                self.fold_expr(object);
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.fold_expr(s);
                }
                if let Some(e) = end {
                    self.fold_expr(e);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        MapItem::Entry(_, value) => self.fold_expr(value),
                        MapItem::Spread(expr) => self.fold_expr(expr),
                    }
                }
            }
            ExprKind::NamedTupleLiteral(entries) => {
                for (_, value) in entries {
                    self.fold_expr(value);
                }
            }
            ExprKind::ArrayLiteral(items) => {
                for item in items {
                    self.fold_expr(item);
                }
            }
            ExprKind::TupleLiteral(items) => {
                for item in items {
                    self.fold_expr(item);
                }
            }
            ExprKind::Lambda { body, .. } => {
                self.fold_expr(body);
            }
            ExprKind::Block(stmts) => {
                for stmt in stmts {
                    self.fold_stmt(stmt);
                }
            }
            ExprKind::Index { collection, index } => {
                self.fold_expr(collection);
                self.fold_expr(index);
            }
            ExprKind::SafeIndex { collection, index } => {
                self.fold_expr(collection);
                self.fold_expr(index);
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                self.fold_expr(collection);
                if let Some(s) = start {
                    self.fold_expr(s);
                }
                if let Some(e) = end {
                    self.fold_expr(e);
                }
            }
            ExprKind::SetLiteral(items) => {
                for item in items {
                    self.fold_expr(item);
                }
            }
            ExprKind::Call { callee, args } => {
                self.fold_expr(callee);
                for arg in args {
                    match arg {
                        CallArg::Positional(e) => self.fold_expr(e),
                        CallArg::Named(_, e) => self.fold_expr(e),
                        CallArg::Spread(e) => self.fold_expr(e),
                        CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Pipeline { left, right } => {
                self.fold_expr(left);
                self.fold_expr(right);
            }
            ExprKind::AssertNotNull(inner) => {
                self.fold_expr(inner);
            }
            ExprKind::SafeCall { callee, args } => {
                self.fold_expr(callee);
                for arg in args {
                    match arg {
                        CallArg::Positional(e) => self.fold_expr(e),
                        CallArg::Named(_, e) => self.fold_expr(e),
                        CallArg::Spread(e) => self.fold_expr(e),
                        CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Match { expr: inner, arms } => {
                self.fold_expr(inner);
                for arm in arms {
                    if let Some(guard) = &mut arm.guard {
                        self.fold_expr(guard);
                    }
                    self.fold_expr(&mut arm.body);
                }
            }
            ExprKind::Await { expr: inner } => {
                self.fold_expr(inner);
            }
            ExprKind::Spread(inner) => {
                self.fold_expr(inner);
            }
            ExprKind::Paren(inner) => {
                self.fold_expr(inner);
                // Unwrap paren if it contains a literal or another paren we can unwrap
                match inner.kind {
                    ExprKind::Literal(_) | ExprKind::Paren(_) | ExprKind::Ident(_) => {
                        expr.kind = inner.kind.clone();
                    }
                    _ => {}
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.fold_expr(condition);
                self.fold_expr(then_branch);
                if let Some(eb) = else_branch {
                    self.fold_expr(eb);
                }
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.fold_expr(part);
                }
            }
            _ => {}
        }
    }

    fn try_fold_unary(&self, op: &UnaryOp, expr: &Expr) -> Option<ExprKind> {
        match (&expr.kind, op) {
            (ExprKind::Literal(Literal::Int(v)), UnaryOp::Negate) => {
                Some(ExprKind::Literal(Literal::Int(-v)))
            }
            (ExprKind::Literal(Literal::Float(v)), UnaryOp::Negate) => {
                Some(ExprKind::Literal(Literal::Float(-v)))
            }
            (ExprKind::Literal(Literal::Bool(v)), UnaryOp::Not) => {
                Some(ExprKind::Literal(Literal::Bool(!v)))
            }
            _ => None,
        }
    }

    fn try_fold_binary(&self, left: &Expr, op: &BinaryOp, right: &Expr) -> Option<ExprKind> {
        match (&left.kind, op, &right.kind) {
            // Int arithmetic
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Add,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Int(a + b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Subtract,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Int(a - b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Multiply,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Int(a * b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Divide,
                ExprKind::Literal(Literal::Int(b)),
            ) if *b != 0 => Some(ExprKind::Literal(Literal::Int(a / b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Modulo,
                ExprKind::Literal(Literal::Int(b)),
            ) if *b != 0 => Some(ExprKind::Literal(Literal::Int(a % b))),

            // Float arithmetic
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Add,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a + b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Subtract,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a - b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Multiply,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a * b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Divide,
                ExprKind::Literal(Literal::Float(b)),
            ) if *b != 0.0 => Some(ExprKind::Literal(Literal::Float(a / b))),

            // String comparisons
            (
                ExprKind::Literal(Literal::String(a)),
                BinaryOp::Equal,
                ExprKind::Literal(Literal::String(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a == b))),
            (
                ExprKind::Literal(Literal::String(a)),
                BinaryOp::NotEqual,
                ExprKind::Literal(Literal::String(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a != b))),

            // Mixed arithmetic (promoting Int to Float)
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Add,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(*a as f64 + b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Add,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a + *b as f64))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Subtract,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(*a as f64 - b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Subtract,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a - *b as f64))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Multiply,
                ExprKind::Literal(Literal::Float(b)),
            ) => Some(ExprKind::Literal(Literal::Float(*a as f64 * b))),
            (
                ExprKind::Literal(Literal::Float(a)),
                BinaryOp::Multiply,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Float(a * *b as f64))),

            // String concatenation
            (
                ExprKind::Literal(Literal::String(a)),
                BinaryOp::Add,
                ExprKind::Literal(Literal::String(b)),
            ) => {
                let mut res = a.clone();
                res.push_str(b);
                Some(ExprKind::Literal(Literal::String(res)))
            }

            // Boolean logic
            (
                ExprKind::Literal(Literal::Bool(a)),
                BinaryOp::And,
                ExprKind::Literal(Literal::Bool(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(*a && *b))),
            (
                ExprKind::Literal(Literal::Bool(a)),
                BinaryOp::Or,
                ExprKind::Literal(Literal::Bool(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(*a || *b))),

            // Comparisons
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Equal,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a == b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::NotEqual,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a != b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Less,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a < b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::LessEqual,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a <= b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::Greater,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a > b))),
            (
                ExprKind::Literal(Literal::Int(a)),
                BinaryOp::GreaterEqual,
                ExprKind::Literal(Literal::Int(b)),
            ) => Some(ExprKind::Literal(Literal::Bool(a >= b))),

            // Null coalescing
            (ExprKind::Literal(Literal::Null), BinaryOp::NullCoalesce, other) => {
                Some(other.clone())
            }
            (ExprKind::Literal(Literal::Int(_)), BinaryOp::NullCoalesce, _) => {
                Some(left.kind.clone())
            }
            (ExprKind::Literal(Literal::Float(_)), BinaryOp::NullCoalesce, _) => {
                Some(left.kind.clone())
            }
            (ExprKind::Literal(Literal::Bool(_)), BinaryOp::NullCoalesce, _) => {
                Some(left.kind.clone())
            }
            (ExprKind::Literal(Literal::String(_)), BinaryOp::NullCoalesce, _) => {
                Some(left.kind.clone())
            }

            _ => None,
        }
    }
}

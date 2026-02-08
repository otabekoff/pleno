use pleno_ast::*;
use std::collections::HashMap;

pub struct Inliner {
    inlinable_functions: HashMap<String, FunctionDecl>,
}

impl Default for Inliner {
    fn default() -> Self {
        Self::new()
    }
}

impl Inliner {
    pub fn new() -> Self {
        Self {
            inlinable_functions: HashMap::new(),
        }
    }

    pub fn inline_program(&mut self, program: &mut Program) {
        // Pass 1: Collect inlinable functions
        self.collect_inlinable_functions(program);

        // Pass 2: Inline calls
        for stmt in &mut program.statements {
            self.inline_stmt(stmt);
        }
    }

    fn collect_inlinable_functions(&mut self, program: &Program) {
        for stmt in &program.statements {
            if let StmtKind::Fn {
                name,
                body,
                annotations,
                generics,
                params,
                ..
            } = &stmt.kind
            {
                if annotations.iter().any(|a| a.name == "inline") {
                    // Only inline simple functions for now (single return statement)
                    if self.is_simple_function(body) {
                        let decl = FunctionDecl {
                            span: stmt.span,
                            name: name.clone(),
                            name_span: Span::new(0, 0),
                            generics: generics.clone(),
                            params: params.clone(),
                            return_ty: None,
                            body: body.clone(),
                            is_static: false,
                            is_abstract: false,
                            is_async: false,
                            visibility: None,
                            kind: FunctionKind::Method,
                            annotations: annotations.clone(),
                            doc: None,
                        };
                        self.inlinable_functions.insert(name.clone(), decl);
                    }
                }
            }
        }
    }

    fn is_simple_function(&self, body: &Stmt) -> bool {
        match &body.kind {
            StmtKind::Block(stmts) if stmts.len() == 1 => {
                matches!(&stmts[0].kind, StmtKind::Return(Some(_)))
            }
            StmtKind::Return(Some(_)) => true,
            _ => false,
        }
    }

    fn inline_stmt(&mut self, stmt: &mut Stmt) {
        match &mut stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.inline_stmt(s);
                }
            }
            StmtKind::Let { value: Some(v), .. } => {
                self.inline_expr(v);
            }
            StmtKind::Assign { value, .. } => {
                self.inline_expr(value);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.inline_expr(condition);
                self.inline_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.inline_stmt(eb);
                }
            }
            StmtKind::While { condition, body } => {
                self.inline_expr(condition);
                self.inline_stmt(body);
            }
            StmtKind::For { iterable, body, .. } => {
                self.inline_expr(iterable);
                self.inline_stmt(body);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.inline_stmt(init);
                }
                if let Some(cond) = condition {
                    self.inline_expr(cond);
                }
                if let Some(step) = step {
                    self.inline_stmt(step);
                }
                self.inline_stmt(body);
            }
            StmtKind::Fn { body, .. } => {
                self.inline_stmt(body);
            }
            StmtKind::Class { methods, .. } => {
                for m in methods {
                    self.inline_stmt(&mut m.body);
                }
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.inline_stmt(try_branch);
                for catch in catch_branches {
                    self.inline_stmt(&mut catch.body);
                }
                if let Some(fb) = finally_branch {
                    self.inline_stmt(fb);
                }
            }
            StmtKind::Expr(expr) | StmtKind::Return(Some(expr)) | StmtKind::Throw(expr) => {
                self.inline_expr(expr);
            }
            StmtKind::Export(inner) => self.inline_stmt(inner),
            _ => {}
        }
    }

    fn inline_expr(&mut self, expr: &mut Expr) {
        match &mut expr.kind {
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.inline_expr(callee);
                for arg in args.iter_mut() {
                    match arg {
                        CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) => {
                            self.inline_expr(e);
                        }
                        CallArg::Placeholder => {}
                    }
                }

                if let ExprKind::Ident(name) = &callee.kind {
                    if let Some(decl) = self.inlinable_functions.get(name).cloned() {
                        if let Some(inlined) = self.try_inline(&decl, args) {
                            expr.kind = inlined;
                        }
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.inline_expr(left);
                self.inline_expr(right);
            }
            ExprKind::Unary { expr: inner, .. } => self.inline_expr(inner),
            ExprKind::ArrayLiteral(items)
            | ExprKind::TupleLiteral(items)
            | ExprKind::SetLiteral(items) => {
                for item in items {
                    self.inline_expr(item);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        MapItem::Entry(_, v) => {
                            self.inline_expr(v);
                        }
                        MapItem::Spread(e) => self.inline_expr(e),
                    }
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.inline_expr(condition);
                self.inline_expr(then_branch);
                if let Some(eb) = else_branch {
                    self.inline_expr(eb);
                }
            }
            ExprKind::Await { expr: inner }
            | ExprKind::Spread(inner)
            | ExprKind::Paren(inner)
            | ExprKind::AssertNotNull(inner) => {
                self.inline_expr(inner);
            }
            ExprKind::Pipeline { left, right } => {
                self.inline_expr(left);
                self.inline_expr(right);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.inline_expr(part);
                }
            }
            _ => {}
        }
    }

    fn try_inline(&self, decl: &FunctionDecl, args: &[CallArg]) -> Option<ExprKind> {
        if decl.params.len() != args.len() {
            return None;
        }

        let mut arg_map = HashMap::new();
        for (i, param) in decl.params.iter().enumerate() {
            if let CallArg::Positional(arg_expr) = &args[i] {
                arg_map.insert(param.name.clone(), arg_expr.clone());
            } else {
                return None;
            }
        }

        let mut return_expr = match &decl.body.kind {
            StmtKind::Return(Some(expr)) => expr.clone(),
            StmtKind::Block(stmts) if stmts.len() == 1 => {
                if let StmtKind::Return(Some(expr)) = &stmts[0].kind {
                    expr.clone()
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        self.replace_params(&mut return_expr, &arg_map);

        Some(ExprKind::Paren(Box::new(return_expr)))
    }

    fn replace_params(&self, expr: &mut Expr, arg_map: &HashMap<String, Expr>) {
        match &mut expr.kind {
            ExprKind::Ident(name) => {
                if let Some(arg) = arg_map.get(name) {
                    *expr = arg.clone();
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.replace_params(left, arg_map);
                self.replace_params(right, arg_map);
            }
            ExprKind::Unary { expr: inner, .. } => self.replace_params(inner, arg_map),
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.replace_params(callee, arg_map);
                for arg in args {
                    if let CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) = arg
                    {
                        self.replace_params(e, arg_map);
                    }
                }
            }
            ExprKind::ArrayLiteral(items)
            | ExprKind::TupleLiteral(items)
            | ExprKind::SetLiteral(items) => {
                for item in items {
                    self.replace_params(item, arg_map);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        MapItem::Entry(_, v) => {
                            self.replace_params(v, arg_map);
                        }
                        MapItem::Spread(e) => self.replace_params(e, arg_map),
                    }
                }
            }
            ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
                self.replace_params(object, arg_map);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.replace_params(condition, arg_map);
                self.replace_params(then_branch, arg_map);
                if let Some(eb) = else_branch {
                    self.replace_params(eb, arg_map);
                }
            }
            _ => {}
        }
    }
}

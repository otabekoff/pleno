use pleno_ast::visitor::{walk_expr, walk_stmt, Visitor};
use pleno_ast::*;

/// The most AST nodes an unrolled loop may produce: iterations times the
/// size of its body. Without it, nested constant loops multiply: eleven
/// `for (let i = 0; i < 3; i = i + 1)` loops inside each other unrolled to
/// 3^11 copies of the innermost body and the compiler ran out of memory.
const MAX_UNROLLED_NODES: usize = 256;

/// What unrolling needs to know about a loop body: its size in AST nodes and
/// whether it assigns the loop variable (`i = ...`, `i++`), which the
/// unrolled `let` copies could not do.
struct BodyFacts<'a> {
    name: &'a str,
    nodes: usize,
    assigns_name: bool,
}

impl BodyFacts<'_> {
    fn of<'a>(name: &'a str, body: &Stmt) -> BodyFacts<'a> {
        let mut facts = BodyFacts {
            name,
            nodes: 0,
            assigns_name: false,
        };
        facts.visit_stmt(body);
        facts
    }

    /// Whether `count` copies of the body stay within the budget and keep
    /// the program's meaning.
    fn allow(&self, count: usize) -> bool {
        !self.assigns_name && self.nodes.saturating_mul(count) <= MAX_UNROLLED_NODES
    }
}

impl<'ast> Visitor<'ast> for BodyFacts<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        self.nodes += 1;
        if matches!(&stmt.kind, StmtKind::Assign { name, .. } if name == self.name) {
            self.assigns_name = true;
        }
        walk_stmt(self, stmt)
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        self.nodes += 1;
        if let ExprKind::Unary {
            op:
                UnaryOp::PreIncrement
                | UnaryOp::PostIncrement
                | UnaryOp::PreDecrement
                | UnaryOp::PostDecrement,
            expr: target,
        } = &expr.kind
        {
            if matches!(&target.kind, ExprKind::Ident(n) if n == self.name) {
                self.assigns_name = true;
            }
        }
        walk_expr(self, expr)
    }
}

/// Loop Unrolling Optimizer
/// Unrolls small, constant-bound loops to reduce loop overhead.
pub struct LoopUnroller {
    pub max_unroll_iterations: usize,
}

impl Default for LoopUnroller {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopUnroller {
    pub fn new() -> Self {
        Self {
            max_unroll_iterations: 8, // Reasonable default for small loops
        }
    }

    pub fn with_max_iterations(max: usize) -> Self {
        Self {
            max_unroll_iterations: max,
        }
    }

    pub fn unroll_program(&mut self, program: &mut Program) {
        for stmt in &mut program.statements {
            self.unroll_stmt(stmt);
        }
    }

    fn unroll_stmt(&mut self, stmt: &mut Stmt) {
        match &mut stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.unroll_stmt(s);
                }
            }
            StmtKind::For {
                names,
                pattern,
                iterable,
                body,
                ..
            } => {
                self.unroll_stmt(body);

                // Try to unroll if iterable is a small range literal. A
                // destructuring loop is left for the compiler to reject.
                if pattern.is_none() {
                    if let Some(unrolled) = self.try_unroll_range_for(names, iterable, body) {
                        stmt.kind = StmtKind::Block(unrolled);
                    }
                }
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(i) = init {
                    self.unroll_stmt(i);
                }
                if let Some(s) = step {
                    self.unroll_stmt(s);
                }
                self.unroll_stmt(body);

                // Try to unroll C-style for loops
                if let Some(unrolled) = self.try_unroll_c_for(
                    init.as_deref(),
                    condition.as_ref(),
                    step.as_deref(),
                    body,
                ) {
                    stmt.kind = StmtKind::Block(unrolled);
                }
            }
            StmtKind::While { condition, body } => {
                self.unroll_stmt(body);
                // Simple while(true) detection for infinite loop warning (no unroll)
                if let ExprKind::Literal(Literal::Bool(true)) = &condition.kind {
                    // Infinite loop, don't unroll
                }
            }
            StmtKind::Fn {
                body, annotations, ..
            } => {
                if !annotations.iter().any(|a| a.name == "no_optimize") {
                    self.unroll_stmt(body);
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
                    self.unroll_stmt(&mut m.body);
                }
            }
            StmtKind::Enum(enum_decl) => {
                if !enum_decl
                    .annotations
                    .iter()
                    .any(|a| a.name == "no_optimize")
                {
                    for m in &mut enum_decl.methods {
                        self.unroll_stmt(&mut m.body);
                    }
                }
            }
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.unroll_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.unroll_stmt(eb);
                }
            }
            StmtKind::Module {
                body: Some(body), ..
            } => {
                for s in body {
                    self.unroll_stmt(s);
                }
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.unroll_stmt(try_branch);
                for catch in catch_branches {
                    self.unroll_stmt(&mut catch.body);
                }
                if let Some(fb) = finally_branch {
                    self.unroll_stmt(fb);
                }
            }
            StmtKind::With { body, .. } => self.unroll_stmt(body),
            _ => {}
        }
    }

    /// Try to unroll a range-based for loop like: for i in 0..5 { ... }
    fn try_unroll_range_for(
        &self,
        names: &[String],
        iterable: &Expr,
        body: &Stmt,
    ) -> Option<Vec<Stmt>> {
        if names.len() != 1 {
            return None; // Only single variable loops
        }

        // Don't unroll if body contains break or continue
        if self.contains_loop_control(body) {
            return None;
        }

        let name = &names[0];

        // Check if iterable is a Range with integer literals
        if let ExprKind::Range {
            start: Some(start),
            end: Some(end),
            inclusive,
        } = &iterable.kind
        {
            let start_val = self.extract_int(start)?;
            let end_val = self.extract_int(end)?;

            let count = if *inclusive {
                (end_val - start_val + 1) as usize
            } else {
                (end_val - start_val) as usize
            };

            if count == 0
                || count > self.max_unroll_iterations
                || !BodyFacts::of(name, body).allow(count)
            {
                return None;
            }

            let mut unrolled = Vec::with_capacity(count);

            for i in 0..count {
                let val = start_val + i as i64;

                // Create: let <name> = <val>;
                let let_stmt = Stmt::new(
                    StmtKind::Let {
                        name: name.clone(),
                        name_span: Span::new(0, 0),
                        mutable: false,
                        ty: None,
                        value: Some(Expr::new(ExprKind::Literal(Literal::Int(val)), body.span)),
                        is_const: false,
                        is_deep: false,
                        is_lazy: false,
                        doc: None,
                    },
                    body.span,
                );

                // Wrap body in a block with the let statement
                let block = Stmt::new(StmtKind::Block(vec![let_stmt, body.clone()]), body.span);

                unrolled.push(block);
            }

            return Some(unrolled);
        }

        None
    }

    /// Try to unroll a C-style for loop like: for (let i = 0; i < 5; i++) { ... }
    fn try_unroll_c_for(
        &self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        step: Option<&Stmt>,
        body: &Stmt,
    ) -> Option<Vec<Stmt>> {
        // Don't unroll if body contains break or continue
        if self.contains_loop_control(body) {
            return None;
        }

        // Extract init: let <name> = <start>
        let (name, start_val) = self.extract_init(init?)?;

        // Extract condition: <name> < <end> or <name> <= <end>
        let (cond_name, end_val, inclusive) = self.extract_condition(condition?)?;
        if cond_name != name {
            return None;
        }

        // Extract step: <name>++ or <name> = <name> + 1
        let step_name = self.extract_step(step?)?;
        if step_name != name {
            return None;
        }

        let count = if inclusive {
            (end_val - start_val + 1) as usize
        } else {
            (end_val - start_val) as usize
        };

        if count == 0
            || count > self.max_unroll_iterations
            || !BodyFacts::of(&name, body).allow(count)
        {
            return None;
        }

        let mut unrolled = Vec::with_capacity(count);

        for i in 0..count {
            let val = start_val + i as i64;

            let let_stmt = Stmt::new(
                StmtKind::Let {
                    name: name.clone(),
                    name_span: Span::new(0, 0),
                    mutable: false,
                    ty: None,
                    value: Some(Expr::new(ExprKind::Literal(Literal::Int(val)), body.span)),
                    is_const: false,
                    is_deep: false,
                    is_lazy: false,
                    doc: None,
                },
                body.span,
            );

            let block = Stmt::new(StmtKind::Block(vec![let_stmt, body.clone()]), body.span);

            unrolled.push(block);
        }

        Some(unrolled)
    }

    fn extract_int(&self, expr: &Expr) -> Option<i64> {
        if let ExprKind::Literal(Literal::Int(v)) = &expr.kind {
            Some(*v)
        } else {
            None
        }
    }

    fn extract_init(&self, stmt: &Stmt) -> Option<(String, i64)> {
        if let StmtKind::Let {
            name,
            value: Some(expr),
            ..
        } = &stmt.kind
        {
            let val = self.extract_int(expr)?;
            return Some((name.clone(), val));
        }
        // Also handle expression statement with assignment
        if let StmtKind::Assign { name, value } = &stmt.kind {
            let val = self.extract_int(value)?;
            return Some((name.clone(), val));
        }
        None
    }

    fn extract_condition(&self, expr: &Expr) -> Option<(String, i64, bool)> {
        if let ExprKind::Binary { left, op, right } = &expr.kind {
            let name = if let ExprKind::Ident(n) = &left.kind {
                n.clone()
            } else {
                return None;
            };

            let end_val = self.extract_int(right)?;

            match op {
                BinaryOp::Less => Some((name, end_val, false)),
                BinaryOp::LessEqual => Some((name, end_val, true)),
                _ => None,
            }
        } else {
            None
        }
    }

    fn extract_step(&self, stmt: &Stmt) -> Option<String> {
        // Handle i++ as expression statement
        if let StmtKind::Expr(expr) = &stmt.kind {
            if let ExprKind::Unary { op, expr: inner } = &expr.kind {
                if matches!(op, pleno_ast::UnaryOp::PostIncrement) {
                    if let ExprKind::Ident(name) = &inner.kind {
                        return Some(name.clone());
                    }
                }
            }
        }

        // Handle i = i + 1
        if let StmtKind::Assign { name, value } = &stmt.kind {
            if let ExprKind::Binary { left, op, right } = &value.kind {
                if matches!(op, BinaryOp::Add) {
                    if let ExprKind::Ident(n) = &left.kind {
                        if n == name {
                            if let ExprKind::Literal(Literal::Int(1)) = &right.kind {
                                return Some(name.clone());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Check if a statement contains break or continue (not nested in inner loops)
    fn contains_loop_control(&self, stmt: &Stmt) -> bool {
        match &stmt.kind {
            StmtKind::Break(_) | StmtKind::Continue(_) => true,
            StmtKind::Block(stmts) => stmts.iter().any(|s| self.contains_loop_control(s)),
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.contains_loop_control(then_branch)
                    || else_branch
                        .as_ref()
                        .map(|eb| self.contains_loop_control(eb))
                        .unwrap_or(false)
            }
            StmtKind::Labeled { stmt: inner, .. } => self.contains_loop_control(inner),
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.contains_loop_control(try_branch)
                    || catch_branches
                        .iter()
                        .any(|c| self.contains_loop_control(&c.body))
                    || finally_branch
                        .as_ref()
                        .map(|fb| self.contains_loop_control(fb))
                        .unwrap_or(false)
            }
            StmtKind::With { body, .. } => self.contains_loop_control(body),
            // Don't recurse into inner loops - break/continue there is fine
            StmtKind::For { .. } | StmtKind::CFor { .. } | StmtKind::While { .. } => false,
            _ => false,
        }
    }
}

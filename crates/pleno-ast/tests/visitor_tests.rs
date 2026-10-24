use pleno_ast::visitor::{self, Visitor};
use pleno_ast::*;
use pleno_common::Span;

struct SymbolCollector {
    symbols: Vec<String>,
}

impl SymbolCollector {
    fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }
}

impl<'ast> Visitor<'ast, ()> for SymbolCollector {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, .. } => {
                self.symbols.push(name.clone());
            }
            StmtKind::Fn { name, .. } => {
                self.symbols.push(name.clone());
            }
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let ExprKind::Ident(name) = &expr.kind {
            self.symbols.push(name.clone());
        }
        visitor::walk_expr(self, expr);
    }

    fn visit_param(&mut self, param: &'ast Param) {
        self.symbols.push(param.name.clone());
        visitor::walk_param(self, param);
    }
}

#[test]
fn test_symbol_collection() {
    let span = Span::new(0, 0);

    // let x = 10;
    let stmt1 = Stmt::new(
        StmtKind::Let {
            name: "x".to_string(),
            name_span: span,
            mutable: false,
            ty: None,
            value: Some(Expr::new(ExprKind::Literal(Literal::Int(10)), span)),
            is_const: false,
            is_deep: false,
            is_lazy: false,
            doc: None,
        },
        span,
    );

    // fn foo(a) { return a + x; }
    let func_body = Stmt::new(
        StmtKind::Return(Some(Expr::new(
            ExprKind::Binary {
                left: Box::new(Expr::new(ExprKind::Ident("a".to_string()), span)),
                op: BinaryOp::Add,
                right: Box::new(Expr::new(ExprKind::Ident("x".to_string()), span)),
            },
            span,
        ))),
        span,
    );

    let stmt2 = Stmt::new(
        StmtKind::Fn {
            name: "foo".to_string(),
            name_span: span,
            generics: Vec::new(),
            params: vec![Param {
                span,
                name: "a".to_string(),
                name_span: span,
                ty: None,
                default: None,
                is_variadic: false,
                is_keyword_only: false,
                ownership: Ownership::None,
            }],
            return_ty: None,
            body: Box::new(func_body),
            annotations: Vec::new(),
            is_async: false,
            doc: None,
        },
        span,
    );

    let program = Program::new(vec![stmt1, stmt2], Vec::new());

    let mut collector = SymbolCollector::new();
    collector.visit_program(&program);

    assert_eq!(collector.symbols, vec!["x", "foo", "a", "a", "x"]);
}

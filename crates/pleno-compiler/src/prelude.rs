//! Program-level analysis and declarations the compiler adds on demand.

use pleno_ast::visitor::{
    Visitor, walk_annotation, walk_expr, walk_field_decl, walk_function_decl, walk_param, walk_stmt,
};
use pleno_ast::{
    BinaryOp, Expr, ExprKind, FieldDecl, FunctionDecl, FunctionKind, Literal, Ownership, Param,
    Program, Span, Stmt, StmtKind, Visibility,
};

/// The native `Error(message)` global is a function, so it cannot be a superclass.
/// When a program declares `class X extends Error` without defining `Error` itself,
/// this prepends an equivalent class:
///
/// ```pleno
/// class Error {
///     message; name;
///     constructor(message = null) { this.message = message; this.name = "Error"; }
///     fn toString() { return this.name + ": " + this.message; }
/// }
/// ```
pub fn add_error_class(program: &mut Program) {
    let extends_error = program.statements.iter().any(|stmt| {
        matches!(
            top_level_class(stmt),
            Some(StmtKind::Class { superclass: Some(name), .. }) if name == "Error"
        )
    });
    let defines_error = program
        .statements
        .iter()
        .any(|stmt| declared_name(stmt) == Some("Error"));
    if extends_error && !defines_error {
        program.statements.insert(0, error_class());
    }
}

fn top_level_class(stmt: &Stmt) -> Option<&StmtKind> {
    match &stmt.kind {
        StmtKind::Export(inner) => top_level_class(inner),
        kind @ StmtKind::Class { .. } => Some(kind),
        _ => None,
    }
}

fn declared_name(stmt: &Stmt) -> Option<&str> {
    match &stmt.kind {
        StmtKind::Export(inner) => declared_name(inner),
        StmtKind::Class { name, .. } | StmtKind::Fn { name, .. } | StmtKind::Let { name, .. } => {
            Some(name)
        }
        _ => None,
    }
}

fn error_class() -> Stmt {
    let field = |name: &str| FieldDecl {
        span: SPAN,
        name: name.to_string(),
        name_span: SPAN,
        ty: None,
        default: None,
        is_static: false,
        is_const: false,
        visibility: Some(Visibility::Public),
        annotations: Vec::new(),
        doc: None,
    };
    let set_field = |name: &str, value: Expr| {
        stmt(StmtKind::AssignProperty {
            object: expr(ExprKind::This),
            name: name.to_string(),
            value,
        })
    };
    let this_field = |name: &str| {
        expr(ExprKind::Property {
            object: Box::new(expr(ExprKind::This)),
            name: name.to_string(),
        })
    };
    let add = |left: Expr, right: Expr| {
        expr(ExprKind::Binary {
            left: Box::new(left),
            op: BinaryOp::Add,
            right: Box::new(right),
        })
    };

    let constructor = method(
        "constructor",
        vec![Param {
            span: SPAN,
            name: "message".to_string(),
            name_span: SPAN,
            ty: None,
            default: Some(expr(ExprKind::Literal(Literal::Null))),
            is_variadic: false,
            is_keyword_only: false,
            ownership: Ownership::None,
        }],
        vec![
            set_field("message", expr(ExprKind::Ident("message".to_string()))),
            set_field("name", string("Error")),
        ],
    );
    let to_string = method(
        "toString",
        Vec::new(),
        vec![stmt(StmtKind::Return(Some(add(
            add(this_field("name"), string(": ")),
            this_field("message"),
        ))))],
    );

    stmt(StmtKind::Class {
        name: "Error".to_string(),
        name_span: SPAN,
        generics: Vec::new(),
        fields: vec![field("message"), field("name")],
        methods: vec![constructor, to_string],
        properties: Vec::new(),
        superclass: None,
        implements: Vec::new(),
        is_abstract: false,
        annotations: Vec::new(),
        nested_classes: Vec::new(),
        doc: None,
    })
}

const SPAN: Span = Span { start: 0, end: 0 };

fn method(name: &str, params: Vec<Param>, body: Vec<Stmt>) -> FunctionDecl {
    FunctionDecl {
        span: SPAN,
        name: name.to_string(),
        name_span: SPAN,
        generics: Vec::new(),
        params,
        return_ty: None,
        body: Box::new(stmt(StmtKind::Block(body))),
        is_static: false,
        is_abstract: false,
        is_async: false,
        visibility: Some(Visibility::Public),
        kind: FunctionKind::Method,
        annotations: Vec::new(),
        doc: None,
    }
}

fn stmt(kind: StmtKind) -> Stmt {
    Stmt::new(kind, SPAN)
}

fn expr(kind: ExprKind) -> Expr {
    Expr::new(kind, SPAN)
}

fn string(value: &str) -> Expr {
    expr(ExprKind::Literal(Literal::String(value.to_string())))
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

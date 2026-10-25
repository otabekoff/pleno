//! Static checks for `pleno check` and the language server, run on the parsed
//! program before it is compiled:
//!
//! - calls with the wrong number of arguments (or unknown, repeated or missing
//!   named arguments) to functions and classes declared at the top level of
//!   the file, and to methods called on `this`;
//! - calls to methods on `this` that the class and its superclasses never
//!   declare (a warning);
//! - literals that cannot match a type annotation, as in `let x: Int = "hi";`
//!   or `f(1)` for `fn f(s: String)`.
//!
//! Every check is conservative: when a name may be rebound, a declaration may
//! be changed by a decorator, an `extend` block or another module, or a type is
//! anything but a built-in primitive, nothing is reported.

use pleno_ast::visitor::{self, Visitor};
use pleno_ast::{
    CallArg, CatchBlock, Expr, ExprKind, FunctionDecl, FunctionKind, ImportKind, Literal, Param,
    Pattern, PatternKind, Program, Span, Stmt, StmtKind, UnaryOp, Visibility,
};
use pleno_value::suggest::did_you_mean;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticDiagnostic {
    pub message: String,
    pub span: Span,
    pub severity: Severity,
}

/// The problems the static checks find in `program`, in source order.
pub fn check_program(program: &Program) -> Vec<StaticDiagnostic> {
    let mut facts = Facts::default();
    facts.visit_program(program);
    let tables = Tables::build(program, &facts);
    let mut checker = Checker {
        facts: &facts,
        tables: &tables,
        class: None,
        in_try: 0,
        returns: Vec::new(),
        diagnostics: Vec::new(),
    };
    checker.visit_program(program);
    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));
    diagnostics.dedup();
    diagnostics
}

/// Annotations that never change a function's signature; any other one may be
/// a decorator that replaces the function.
const SIGNATURE_PRESERVING: &[&str] = &["pure", "inline", "hot", "no_optimize", "deprecated"];

/// What the whole file says about names, gathered before checking.
#[derive(Default)]
struct Facts {
    /// How often each name is bound anywhere in the file, in any scope.
    bindings: HashMap<String, usize>,
    /// Property names assigned anywhere (`x.name = ...`): such a field may
    /// shadow or add a method.
    assigned_properties: HashSet<String>,
    /// String literals anywhere: a name built at runtime (`reflect.setField`).
    strings: HashSet<String>,
    /// Types named by `extend` blocks.
    extended: HashSet<String>,
    /// Classes some class in the file extends.
    subclassed: HashSet<String>,
    /// Names exported from the file.
    exported: HashSet<String>,
    /// Names of type aliases, classes, interfaces, enums and type parameters.
    type_names: HashSet<String>,
    /// The file uses reflection or assigns to `this[...]`.
    dynamic_members: bool,
}

impl Facts {
    fn bind(&mut self, name: &str) {
        *self.bindings.entry(name.to_string()).or_default() += 1;
    }

    fn is_unique(&self, name: &str) -> bool {
        self.bindings.get(name).copied() == Some(1)
    }
}

/// `Base<T>` names the class `Base`.
fn base_name(ty: &str) -> &str {
    ty.split('<').next().unwrap_or(ty).trim()
}

impl<'ast> Visitor<'ast> for Facts {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, .. } | StmtKind::Assign { name, .. } => self.bind(name),
            StmtKind::MacroDecl { name, params, .. } => {
                self.bind(name);
                for param in params {
                    self.bind(param);
                }
            }
            StmtKind::AssignProperty { name, .. } => {
                self.assigned_properties.insert(name.clone());
            }
            StmtKind::AssignIndex { collection, .. } => {
                if matches!(collection.kind, ExprKind::This) {
                    self.dynamic_members = true;
                }
            }
            StmtKind::For { names, .. } => {
                for name in names {
                    self.bind(name);
                }
            }
            StmtKind::Import { path, kind } => {
                if path.contains("reflect") {
                    self.dynamic_members = true;
                }
                match kind {
                    ImportKind::Module { alias } => {
                        let stem = path
                            .rsplit(['/', '\\', ':', '.'])
                            .find(|part| !part.is_empty() && *part != "pln")
                            .unwrap_or(path);
                        self.bind(alias.as_deref().unwrap_or(stem));
                    }
                    ImportKind::Star { alias } => self.bind(alias),
                    ImportKind::Names(items) => {
                        for item in items {
                            self.bind(item.alias.as_ref().unwrap_or(&item.name));
                        }
                    }
                }
            }
            StmtKind::Export(inner) => {
                if let StmtKind::Let { name, .. }
                | StmtKind::Fn { name, .. }
                | StmtKind::Class { name, .. } = &inner.kind
                {
                    self.exported.insert(name.clone());
                }
            }
            StmtKind::ExportList(items) => {
                for item in items {
                    self.exported.insert(item.name.clone());
                }
            }
            StmtKind::Class {
                name,
                generics,
                superclass,
                ..
            } => {
                self.bind(name);
                self.type_names.insert(name.clone());
                for generic in generics {
                    self.type_names.insert(generic.name.clone());
                }
                if let Some(superclass) = superclass {
                    self.subclassed.insert(base_name(superclass).to_string());
                }
            }
            StmtKind::Interface { name, generics, .. } => {
                self.bind(name);
                self.type_names.insert(name.clone());
                for generic in generics {
                    self.type_names.insert(generic.name.clone());
                }
            }
            StmtKind::Enum(decl) => {
                self.bind(&decl.name);
                self.type_names.insert(decl.name.clone());
                for variant in &decl.variants {
                    self.bind(&variant.name);
                }
            }
            StmtKind::Fn { name, generics, .. } => {
                self.bind(name);
                for generic in generics {
                    self.type_names.insert(generic.name.clone());
                }
            }
            StmtKind::TypeAlias { name, params, .. } => {
                self.bind(name);
                self.type_names.insert(name.clone());
                for param in params {
                    self.type_names.insert(param.clone());
                }
            }
            StmtKind::Extend {
                target, generics, ..
            } => {
                self.extended.insert(base_name(target).to_string());
                for generic in generics {
                    self.type_names.insert(generic.name.clone());
                }
            }
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Literal(Literal::String(value)) => {
                self.strings.insert(value.clone());
            }
            ExprKind::Ident(name) if name == "reflect" => self.dynamic_members = true,
            _ => {}
        }
        visitor::walk_expr(self, expr);
    }

    fn visit_function_decl(&mut self, decl: &'ast FunctionDecl) {
        for generic in &decl.generics {
            self.type_names.insert(generic.name.clone());
        }
        visitor::walk_function_decl(self, decl);
    }

    fn visit_param(&mut self, param: &'ast Param) {
        self.bind(&param.name);
        visitor::walk_param(self, param);
    }

    fn visit_pattern(&mut self, pattern: &'ast Pattern) {
        match &pattern.kind {
            PatternKind::Ident(name)
            | PatternKind::Rest(Some(name))
            | PatternKind::Type(_, Some(name)) => self.bind(name),
            PatternKind::Object(items, _) => {
                for (key, _) in items {
                    self.bind(key);
                }
            }
            _ => {}
        }
        visitor::walk_pattern(self, pattern);
    }

    fn visit_catch_block(&mut self, catch: &'ast CatchBlock) {
        self.bind(&catch.name);
        visitor::walk_catch_block(self, catch);
    }
}

/// A class declared at the top level of the file.
struct ClassInfo<'ast> {
    superclass: Option<&'ast str>,
    fields: Vec<&'ast str>,
    methods: &'ast [FunctionDecl],
    properties: Vec<&'ast str>,
    nested: Vec<&'ast str>,
    /// The members declared `private`, which subclasses cannot use.
    private: HashSet<&'ast str>,
    /// No decorator, `implements` or `extend` can change its members.
    plain: bool,
    is_abstract: bool,
}

/// What a name a call can resolve to.
struct Tables<'ast> {
    functions: HashMap<&'ast str, &'ast [Param]>,
    classes: HashMap<&'ast str, ClassInfo<'ast>>,
    /// Top-level functions marked `@deprecated`, with the note it gives.
    deprecated: HashMap<&'ast str, Option<&'ast str>>,
}

impl<'ast> Tables<'ast> {
    fn build(program: &'ast Program, facts: &Facts) -> Self {
        let mut tables = Tables {
            functions: HashMap::new(),
            classes: HashMap::new(),
            deprecated: HashMap::new(),
        };
        for stmt in &program.statements {
            let stmt = match &stmt.kind {
                StmtKind::Export(inner) => inner.as_ref(),
                _ => stmt,
            };
            if let StmtKind::Fn {
                name, annotations, ..
            } = &stmt.kind
                && facts.is_unique(name)
                && let Some(annotation) = annotations.iter().find(|a| a.name == "deprecated")
            {
                let note = annotation.args.first().and_then(|arg| match &arg.kind {
                    ExprKind::Literal(Literal::String(note)) => Some(note.as_str()),
                    _ => None,
                });
                tables.deprecated.insert(name, note);
            }
            match &stmt.kind {
                StmtKind::Fn {
                    name,
                    params,
                    annotations,
                    ..
                } if facts.is_unique(name)
                    && annotations
                        .iter()
                        .all(|a| SIGNATURE_PRESERVING.contains(&a.name.as_str())) =>
                {
                    tables.functions.insert(name, params);
                }
                StmtKind::Class {
                    name,
                    fields,
                    methods,
                    properties,
                    superclass,
                    implements,
                    is_abstract,
                    annotations,
                    nested_classes,
                    ..
                } if facts.is_unique(name) => {
                    let nested = nested_classes
                        .iter()
                        .filter_map(|s| match &s.kind {
                            StmtKind::Class { name, .. } => Some(name.as_str()),
                            _ => None,
                        })
                        .collect();
                    let private = fields
                        .iter()
                        .filter(|f| f.visibility == Some(Visibility::Private))
                        .map(|f| f.name.as_str())
                        .chain(
                            methods
                                .iter()
                                .filter(|m| m.visibility == Some(Visibility::Private))
                                .map(|m| m.name.as_str()),
                        )
                        .chain(
                            properties
                                .iter()
                                .filter(|p| p.visibility == Some(Visibility::Private))
                                .map(|p| p.name.as_str()),
                        )
                        .collect();
                    tables.classes.insert(
                        name,
                        ClassInfo {
                            superclass: superclass.as_deref().map(base_name),
                            fields: fields.iter().map(|f| f.name.as_str()).collect(),
                            methods,
                            properties: properties.iter().map(|p| p.name.as_str()).collect(),
                            nested,
                            private,
                            plain: annotations.is_empty()
                                && implements.is_empty()
                                && !facts.extended.contains(name.as_str()),
                            is_abstract: *is_abstract,
                        },
                    );
                }
                _ => {}
            }
        }
        tables
    }

    /// The parameters of the constructor `Class(...)` runs, if they are known.
    fn constructor(&self, class: &str) -> Option<&'ast [Param]> {
        let mut current = class;
        for _ in 0..64 {
            let info = self.classes.get(current)?;
            if !info.plain {
                return None;
            }
            if let Some(ctor) = info
                .methods
                .iter()
                .find(|m| m.name == "constructor" && !m.is_static)
            {
                return Some(&ctor.params);
            }
            match info.superclass {
                Some(superclass) => current = superclass,
                None => return Some(&[]),
            }
        }
        None
    }

    /// The chain of classes `this` may be in a method of `class`, when every
    /// member of every instance is known: the class and its superclasses are
    /// all declared here, unchanged by decorators or `extend`, and nothing else
    /// can subclass or extend them.
    fn closed_chain(&self, class: &str, facts: &Facts) -> Option<Vec<&ClassInfo<'ast>>> {
        if facts.dynamic_members || facts.subclassed.contains(class) {
            return None;
        }
        let mut chain = Vec::new();
        let mut current = class;
        loop {
            let info = self.classes.get(current)?;
            if !info.plain || facts.exported.contains(current) || chain.len() > 64 {
                return None;
            }
            chain.push(info);
            match info.superclass {
                Some(superclass) => current = superclass,
                None => return Some(chain),
            }
        }
    }
}

struct Checker<'a, 'ast> {
    facts: &'a Facts,
    tables: &'a Tables<'ast>,
    /// The class whose method is being checked, when its members are known.
    class: Option<(&'ast str, Vec<&'a ClassInfo<'ast>>)>,
    /// Inside the body of a `try` with a `catch`, where a failing call may be
    /// the point (tests do that), so nothing is reported.
    in_try: usize,
    /// For each enclosing function, its name and the return annotation its
    /// `return` literals are checked against (`None` when not checked).
    returns: Vec<Option<(String, String)>>,
    diagnostics: Vec<StaticDiagnostic>,
}

/// The annotation a function's returned values are checked against at
/// runtime, as the compiler reads it: `async` functions describe the resolved
/// value (`Promise<Int>` is `Int`), and `Void` is not checked.
fn checked_return_type(return_ty: Option<&String>, is_async: bool) -> Option<String> {
    let ty = return_ty?.trim();
    let ty = match ty
        .strip_prefix("Promise<")
        .and_then(|t| t.strip_suffix('>'))
    {
        Some(inner) if is_async => inner.trim(),
        _ => ty,
    };
    (ty != "Void").then(|| ty.to_string())
}

/// One argument of a call, after a pipeline has supplied its left side.
enum Arg<'e> {
    Positional(&'e Expr),
    Named(&'e str, &'e Expr),
}

/// "1 argument", "1 to 2 arguments", "at least 1 argument".
fn describe_range(min: usize, max: Option<usize>) -> String {
    let noun = |n: usize| if n == 1 { "argument" } else { "arguments" };
    match max {
        None => format!("at least {min} {}", noun(min)),
        Some(0) => "no arguments".to_string(),
        Some(max) if max == min => format!("{min} {}", noun(min)),
        Some(max) => format!("{min} to {max} {}", noun(max)),
    }
}

/// The type a literal has at runtime, and its family: a literal of one family
/// is never reported against an annotation that accepts the family. `Float`
/// and `Number` accept `Int` literals (an `Int` widens to a `Float`); `Int`
/// does not accept `Float` literals. `String` and `Char` share a family.
fn literal_type(expr: &Expr) -> Option<(&'static str, &'static str)> {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(_)) => Some(("Int", "int")),
        ExprKind::Literal(Literal::Float(_)) => Some(("Float", "float")),
        ExprKind::Literal(Literal::String(_)) | ExprKind::InterpolatedString(_) => {
            Some(("String", "text"))
        }
        ExprKind::Literal(Literal::Char(_)) => Some(("Char", "text")),
        ExprKind::Literal(Literal::Bool(_)) => Some(("Bool", "bool")),
        ExprKind::Literal(Literal::Null) => Some(("Null", "null")),
        ExprKind::Unary {
            op: UnaryOp::Negate,
            expr,
        } => match &expr.kind {
            ExprKind::Literal(Literal::Int(_)) => Some(("Int", "int")),
            ExprKind::Literal(Literal::Float(_)) => Some(("Float", "float")),
            _ => None,
        },
        ExprKind::Paren(inner) => literal_type(inner),
        _ => None,
    }
}

/// Whether a value of `family` can match the annotation `ty`: `Some(false)`
/// only when `ty` is made of built-in primitive names that all reject it.
fn literal_matches(ty: &str, family: &str, facts: &Facts) -> Option<bool> {
    let ty = ty.trim();
    if let Some(inner) = ty.strip_suffix('?') {
        if family == "null" {
            return Some(true);
        }
        return literal_matches(inner, family, facts);
    }
    let mut matches = false;
    for part in ty.split('|') {
        let part = part.trim();
        if facts.type_names.contains(part) {
            return None;
        }
        matches |= match part {
            "Int" => family == "int",
            "Float" | "Number" => matches!(family, "int" | "float"),
            "String" | "Char" => family == "text",
            "Bool" => family == "bool",
            "Null" | "Void" => family == "null",
            "Any" => return Some(true),
            _ => return None,
        };
    }
    Some(matches)
}

impl<'a, 'ast> Checker<'a, 'ast> {
    fn report(&mut self, severity: Severity, message: String, span: Span) {
        if self.in_try > 0 {
            return;
        }
        self.diagnostics.push(StaticDiagnostic {
            message,
            span,
            severity,
        });
    }

    fn check_literal(&mut self, ty: &str, value: &Expr, describe: impl FnOnce(&str) -> String) {
        if let Some((literal, family)) = literal_type(value)
            && literal_matches(ty, family, self.facts) == Some(false)
        {
            let message = describe(literal);
            self.report(Severity::Error, message, value.span);
        }
    }

    /// Checks `args` against `params` the way the VM binds them.
    fn check_call(&mut self, callee: &str, params: &[Param], args: &[Arg], span: Span) {
        let positional: Vec<&Expr> = args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Positional(expr) => Some(*expr),
                Arg::Named(..) => None,
            })
            .collect();
        let named: Vec<(&str, &Expr)> = args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Named(name, expr) => Some((*name, *expr)),
                Arg::Positional(_) => None,
            })
            .collect();

        for (name, expr) in &named {
            match params.iter().find(|p| p.name == *name) {
                None => {
                    let hint = did_you_mean(
                        name,
                        params
                            .iter()
                            .filter(|p| !p.is_variadic)
                            .map(|p| p.name.as_str()),
                    );
                    let message = format!("{callee} has no parameter named '{name}'{hint}");
                    self.report(Severity::Error, message, expr.span);
                    return;
                }
                Some(param) if param.is_variadic => return,
                Some(_) => {}
            }
        }

        let positional_params: Vec<&Param> = params
            .iter()
            .filter(|p| !p.is_variadic && !p.is_keyword_only)
            .collect();
        let min = positional_params
            .iter()
            .rposition(|p| p.default.is_none())
            .map_or(0, |i| i + 1);
        let positional_params = positional_params.len();
        let max = if params.iter().any(|p| p.is_variadic) {
            None
        } else {
            Some(positional_params)
        };
        let count_error = |got: usize| {
            let range = describe_range(min, max);
            let range = if named.is_empty() && !params.iter().any(|p| p.is_keyword_only) {
                range
            } else {
                range.replace("argument", "positional argument")
            };
            format!("{callee} expects {range}, got {got}")
        };

        let mut bound: Vec<(&Param, &Expr)> = Vec::new();
        let mut next = 0;
        for param in params {
            if param.is_variadic {
                next = positional.len();
                continue;
            }
            if param.is_keyword_only && next < positional.len() {
                let message = count_error(positional.len());
                self.report(Severity::Error, message, span);
                return;
            }
            if next < positional.len() {
                if named.iter().any(|(name, _)| *name == param.name) {
                    let message = format!("{callee} got more than one value for '{}'", param.name);
                    self.report(Severity::Error, message, span);
                    return;
                }
                bound.push((param, positional[next]));
                next += 1;
            } else if let Some((_, expr)) = named.iter().find(|(name, _)| *name == param.name) {
                bound.push((param, expr));
            } else if param.default.is_none() {
                let message = if param.is_keyword_only {
                    format!("{callee} is missing named argument '{}'", param.name)
                } else if named.is_empty() {
                    count_error(positional.len())
                } else {
                    format!("{callee} is missing argument '{}'", param.name)
                };
                self.report(Severity::Error, message, span);
                return;
            }
        }
        if next < positional.len() {
            let message = count_error(positional.len());
            self.report(Severity::Error, message, span);
            return;
        }

        for (param, expr) in bound {
            if let Some(ty) = &param.ty {
                self.check_literal(ty, expr, |literal| {
                    format!(
                        "{callee} expects {ty} for '{}', found {}",
                        param.name,
                        describe_literal(literal)
                    )
                });
            }
        }
    }

    /// Checks a call to `callee` with `args`, `piped` being the left side of
    /// `piped |> callee(args)`.
    fn check_callee(
        &mut self,
        callee: &'ast Expr,
        args: &'ast [CallArg],
        piped: Option<&'ast Expr>,
        span: Span,
    ) {
        let mut effective = Vec::new();
        let mut piped = piped;
        for arg in args {
            match arg {
                CallArg::Positional(expr) => effective.push(Arg::Positional(expr)),
                CallArg::Named(name, expr) => effective.push(Arg::Named(name, expr)),
                CallArg::Placeholder => match piped.take() {
                    Some(left) => effective.push(Arg::Positional(left)),
                    // `f(1, _)` is a partial application.
                    None => return,
                },
                CallArg::Spread(_) => return,
            }
        }
        if let Some(left) = piped {
            effective.insert(0, Arg::Positional(left));
        }

        if let ExprKind::Ident(name) = &callee.kind
            && let Some(note) = self.tables.deprecated.get(name.as_str())
        {
            let message = match note {
                Some(note) => format!("'{name}' is deprecated: {note}"),
                None => format!("'{name}' is deprecated"),
            };
            self.report(Severity::Warning, message, callee.span);
        }
        match &callee.kind {
            ExprKind::Ident(name) => {
                if let Some(params) = self.tables.functions.get(name.as_str()) {
                    self.check_call(name, params, &effective, span);
                } else if self.tables.classes.contains_key(name.as_str())
                    && !self.tables.classes[name.as_str()].is_abstract
                    && let Some(params) = self.tables.constructor(name)
                {
                    self.check_call(name, params, &effective, span);
                }
            }
            ExprKind::Property { object, name } if matches!(object.kind, ExprKind::This) => {
                self.check_this_call(name, &effective, callee.span, span);
            }
            _ => {}
        }
    }

    fn check_this_call(&mut self, method: &str, args: &[Arg], callee_span: Span, span: Span) {
        let Some((class, chain)) = &self.class else {
            return;
        };
        if method == "constructor"
            || self.facts.assigned_properties.contains(method)
            || self.facts.strings.contains(method)
        {
            return;
        }
        let class = *class;
        let mut found = None;
        for info in chain {
            if info.fields.contains(&method)
                || info.properties.contains(&method)
                || info.nested.contains(&method)
            {
                return;
            }
            if let Some(decl) = info.methods.iter().find(|m| m.name == method) {
                found = Some(decl);
                break;
            }
        }
        match found {
            None => {
                // Members the class itself declares, and those of its
                // superclasses that are not private.
                let members = chain.iter().enumerate().flat_map(|(depth, info)| {
                    info.fields
                        .iter()
                        .chain(&info.properties)
                        .chain(&info.nested)
                        .copied()
                        .chain(
                            info.methods
                                .iter()
                                .filter(|m| m.name != "constructor")
                                .map(|m| m.name.as_str()),
                        )
                        .filter(move |name| depth == 0 || !info.private.contains(name))
                });
                let hint = did_you_mean(method, members);
                let message = format!("{class} has no method '{method}'{hint}");
                self.report(Severity::Warning, message, callee_span);
            }
            Some(decl)
                if decl.kind == FunctionKind::Method
                    && !decl.is_static
                    && decl
                        .annotations
                        .iter()
                        .all(|a| SIGNATURE_PRESERVING.contains(&a.name.as_str())) =>
            {
                let name = format!("{class}.{method}");
                self.check_call(&name, &decl.params, args, span);
            }
            Some(_) => {}
        }
    }

    fn visit_args(&mut self, args: &'ast [CallArg]) {
        for arg in args {
            self.visit_call_arg(arg);
        }
    }
}

/// "an Int literal", "a String literal", "null".
fn describe_literal(type_name: &str) -> String {
    if type_name == "Null" {
        return "null".to_string();
    }
    let an = matches!(type_name.chars().next(), Some('A' | 'E' | 'I' | 'O' | 'U'));
    format!("{} {type_name} literal", if an { "an" } else { "a" })
}

impl<'a, 'ast> Visitor<'ast> for Checker<'a, 'ast> {
    fn visit_function_decl(&mut self, decl: &'ast FunctionDecl) {
        let checked = if decl.kind == FunctionKind::Setter
            || decl.name == "constructor"
            || decl.is_abstract
        {
            None
        } else {
            checked_return_type(decl.return_ty.as_ref(), decl.is_async)
        };
        let name = match decl.kind {
            FunctionKind::Getter => decl.name.strip_prefix("get_").unwrap_or(&decl.name),
            _ => decl.name.as_str(),
        };
        self.returns.push(checked.map(|ty| (name.to_string(), ty)));
        visitor::walk_function_decl(self, decl);
        self.returns.pop();
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Let {
                name,
                ty: Some(ty),
                value: Some(value),
                is_lazy: false,
                ..
            } => {
                self.check_literal(ty, value, |literal| {
                    format!(
                        "'{name}' is declared {ty} but given {}",
                        describe_literal(literal)
                    )
                });
                visitor::walk_stmt(self, stmt);
            }
            StmtKind::Class {
                name,
                fields,
                methods,
                properties,
                nested_classes,
                ..
            } => {
                // Only classes declared at the top level are in the tables, so a
                // nested class with the same name is never mistaken for one.
                let is_top_level = self
                    .tables
                    .classes
                    .get(name.as_str())
                    .is_some_and(|info| std::ptr::eq(info.methods, methods.as_slice()));
                let context = if is_top_level {
                    self.tables
                        .closed_chain(name, self.facts)
                        .map(|chain| (name.as_str(), chain))
                } else {
                    None
                };
                let saved = std::mem::replace(&mut self.class, context);
                for field in fields {
                    self.visit_field_decl(field);
                }
                for method in methods {
                    self.visit_function_decl(method);
                }
                for property in properties {
                    self.visit_property_decl(property);
                }
                self.class = None;
                for nested in nested_classes {
                    self.visit_stmt(nested);
                }
                self.class = saved;
            }
            StmtKind::Fn {
                name,
                return_ty,
                is_async,
                ..
            } => {
                // `this` is not the enclosing class's instance in a function.
                let saved = self.class.take();
                let checked = checked_return_type(return_ty.as_ref(), *is_async);
                self.returns.push(checked.map(|ty| (name.clone(), ty)));
                visitor::walk_stmt(self, stmt);
                self.returns.pop();
                self.class = saved;
            }
            StmtKind::Enum(_) | StmtKind::Extend { .. } | StmtKind::Interface { .. } => {
                // `this` is not the enclosing class's instance in these.
                let saved = self.class.take();
                visitor::walk_stmt(self, stmt);
                self.class = saved;
            }
            StmtKind::Return(Some(value)) => {
                if let Some(Some((name, ty))) = self.returns.last().cloned() {
                    self.check_literal(&ty, value, |literal| {
                        format!(
                            "{name} is declared to return {ty} but returns {}",
                            describe_literal(literal)
                        )
                    });
                }
                visitor::walk_stmt(self, stmt);
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                let caught = usize::from(!catch_branches.is_empty());
                self.in_try += caught;
                self.visit_stmt(try_branch);
                self.in_try -= caught;
                for catch in catch_branches {
                    self.visit_catch_block(catch);
                }
                if let Some(finally) = finally_branch {
                    self.visit_stmt(finally);
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                self.check_callee(callee, args, None, expr.span);
                self.visit_expr(callee);
                // `assert.throws(fn() { f(1, 2); })` expects the failure.
                let callee_name = match &callee.kind {
                    ExprKind::Ident(name) | ExprKind::Property { name, .. } => name.as_str(),
                    _ => "",
                };
                let expects_failure = usize::from(callee_name.to_lowercase().contains("throw"));
                self.in_try += expects_failure;
                self.visit_args(args);
                self.in_try -= expects_failure;
            }
            ExprKind::Pipeline { left, right } => match &right.kind {
                ExprKind::Call { callee, args } => {
                    self.check_callee(callee, args, Some(left), expr.span);
                    self.visit_expr(left);
                    self.visit_expr(callee);
                    self.visit_args(args);
                }
                _ => {
                    self.check_callee(right, &[], Some(left), expr.span);
                    visitor::walk_expr(self, expr);
                }
            },
            ExprKind::Lambda {
                return_ty,
                is_async,
                body,
                ..
            } => {
                let saved = self.class.take();
                let checked = checked_return_type(return_ty.as_ref(), *is_async);
                // `fn(): Int => "s"` returns its body.
                if let Some(ty) = &checked
                    && !matches!(body.kind, ExprKind::Block(_))
                {
                    self.check_literal(ty, body, |literal| {
                        format!(
                            "lambda is declared to return {ty} but returns {}",
                            describe_literal(literal)
                        )
                    });
                }
                self.returns
                    .push(checked.map(|ty| ("lambda".to_string(), ty)));
                visitor::walk_expr(self, expr);
                self.returns.pop();
                self.class = saved;
            }
            _ => visitor::walk_expr(self, expr),
        }
    }
}

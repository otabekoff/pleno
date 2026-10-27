//! Static checks for `pleno check` and the language server, run on the parsed
//! program before it is compiled:
//!
//! - calls with the wrong number of arguments (or unknown, repeated or missing
//!   named arguments) to functions and classes declared at the top level of
//!   the file, to methods called on `this` or on a value of a known class, to
//!   static methods, to lambdas bound with `let`, and to functions imported
//!   from other `.pln` files;
//! - calls to methods that the class and its superclasses never declare, on
//!   `this`, on a value of a known class, or on the class itself (a warning);
//! - names imported from a `.pln` file that the file does not export (a
//!   warning: the import gives `null`);
//! - values that cannot match a type annotation, as in `let x: Int = "hi";`,
//!   or `f(p)` for `fn f(s: String)` when `p` holds an instance of a class.
//!
//! Every check is conservative: when a name may be rebound, a declaration may
//! be changed by a decorator, an `extend` block or another module, or a type is
//! anything but a built-in primitive, nothing is reported.
//!
//! A value's type is known only when it cannot be anything else: a literal, a
//! call of a class declared here (an instance of exactly that class), a call
//! of a function declared here whose `Int`, `Float`, `String` or `Bool` return
//! annotation is checked when it returns, or a name bound once in the file, by
//! a `let` without `mut`, to one of these.

use pleno_ast::visitor::{self, Visitor};
use pleno_ast::{
    BindingPattern, BindingPatternKind, CallArg, CatchBlock, Expr, ExprKind, FunctionDecl,
    FunctionKind, ImportKind, Literal, Param, Pattern, PatternKind, Program, Span, Stmt, StmtKind,
    UnaryOp, Visibility,
};
use pleno_value::suggest::did_you_mean;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

/// The problems the static checks find in `program`, in source order. Calls
/// to imported functions are not checked: see [`check_program_in`].
pub fn check_program(program: &Program) -> Vec<StaticDiagnostic> {
    check_program_in(program, None)
}

/// The problems the static checks find in `program`, read from `file`, in
/// source order. The `.pln` files it imports with a relative path (`./m.pln`,
/// `../lib/m`) are read from beside `file`, as running it would, so that the
/// names it imports and the calls to their functions are checked too.
pub fn check_program_in(program: &Program, file: Option<&Path>) -> Vec<StaticDiagnostic> {
    let loaded = match file {
        Some(file) => load_modules(program, file),
        None => Vec::new(),
    };
    let mut facts = Facts::default();
    facts.visit_program(program);
    let modules: HashMap<&str, ModuleInfo> = loaded
        .iter()
        .map(|(path, module)| (path.as_str(), ModuleInfo::build(module)))
        .collect();
    let tables = Tables::build(program, &facts, modules);
    let mut checker = Checker {
        facts: &facts,
        tables: &tables,
        class: None,
        in_try: 0,
        returns: Vec::new(),
        scopes: vec![HashMap::new()],
        diagnostics: Vec::new(),
    };
    checker.check_imports(program);
    checker.visit_program(program);
    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by_key(|d| (d.span.start, d.span.end));
    diagnostics.dedup();
    diagnostics
}

/// Annotations that never change a function's signature; any other one may be
/// a decorator that replaces the function.
const SIGNATURE_PRESERVING: &[&str] = &["pure", "inline", "hot", "no_optimize", "deprecated"];

fn preserves_signature(annotations: &[pleno_ast::Annotation]) -> bool {
    annotations
        .iter()
        .all(|a| SIGNATURE_PRESERVING.contains(&a.name.as_str()))
}

/// Whether `path` names a file relative to the importing one, which is the
/// only kind of import the checks follow.
fn is_relative(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

/// The name `import "path"` binds when it gives no alias: the file's stem.
fn module_stem(path: &str) -> &str {
    path.rsplit(['/', '\\', ':', '.'])
        .find(|part| !part.is_empty() && *part != "pln")
        .unwrap_or(path)
}

/// Reads and parses the `.pln` files that `program`, read from `file`,
/// imports at its top level with a relative path, as the VM resolves them:
/// `path` beside `file`, else `path.pln`. Files that cannot be read or parsed
/// are left out, so nothing is checked against them.
fn load_modules(program: &Program, file: &Path) -> Vec<(String, Program)> {
    let dir = file.parent().unwrap_or(Path::new(""));
    let mut loaded: Vec<(String, Program)> = Vec::new();
    for stmt in &program.statements {
        let StmtKind::Import { path, .. } = &stmt.kind else {
            continue;
        };
        if !is_relative(path) || loaded.iter().any(|(p, _)| p == path) {
            continue;
        }
        let mut candidates = vec![dir.join(path)];
        if !path.ends_with(".pln") {
            candidates.push(dir.join(format!("{path}.pln")));
        }
        let Some(source) = candidates
            .iter()
            .filter(|candidate| candidate.is_file())
            .find_map(|candidate| std::fs::read_to_string(candidate).ok())
        else {
            continue;
        };
        let Ok(tokens) = pleno_lexer::lex(&source) else {
            continue;
        };
        if let Ok(module) = pleno_parser::Parser::new(tokens, &source).parse_program() {
            loaded.push((path.clone(), module));
        }
    }
    loaded
}

/// What an imported file exports.
struct ModuleInfo<'ast> {
    /// Each exported name, with its parameters when it is a function whose
    /// signature is known.
    exports: HashMap<&'ast str, Option<&'ast [Param]>>,
    /// Every `export` is at the top level, so `exports` lists every name.
    complete: bool,
}

impl<'ast> ModuleInfo<'ast> {
    fn build(program: &'ast Program) -> Self {
        let mut facts = Facts::default();
        facts.visit_program(program);
        // The checks read annotations as the importing file means them; a
        // module that gives a built-in type's name to its own type means
        // something else by them.
        let renames_builtin = ["Int", "Float", "Number", "String", "Char", "Bool", "Null"]
            .iter()
            .any(|name| facts.type_names.contains(*name));
        let mut functions: HashMap<&str, &[Param]> = HashMap::new();
        for stmt in &program.statements {
            let stmt = match &stmt.kind {
                StmtKind::Export(inner) => inner.as_ref(),
                _ => stmt,
            };
            if let StmtKind::Fn {
                name,
                params,
                annotations,
                ..
            } = &stmt.kind
                && facts.is_unique(name)
                && preserves_signature(annotations)
                && !renames_builtin
            {
                functions.insert(name, params);
            }
        }
        let mut exports = HashMap::new();
        let mut top_level = 0;
        for stmt in &program.statements {
            match &stmt.kind {
                StmtKind::Export(inner) => {
                    top_level += 1;
                    match &inner.kind {
                        StmtKind::Fn { name, .. } => {
                            exports.insert(name.as_str(), functions.get(name.as_str()).copied());
                        }
                        StmtKind::Let {
                            name,
                            mutable,
                            value,
                            ..
                        } => {
                            let params = match value.as_ref().map(|v| &v.kind) {
                                Some(ExprKind::Lambda {
                                    params,
                                    annotations,
                                    ..
                                }) if !mutable
                                    && !renames_builtin
                                    && facts.is_unique(name)
                                    && preserves_signature(annotations) =>
                                {
                                    Some(params.as_slice())
                                }
                                _ => None,
                            };
                            exports.insert(name.as_str(), params);
                        }
                        StmtKind::Class { name, .. } => {
                            exports.insert(name.as_str(), None);
                        }
                        _ => {}
                    }
                }
                StmtKind::ExportList(items) => {
                    top_level += 1;
                    for item in items {
                        let export = item.alias.as_deref().unwrap_or(&item.name);
                        exports.insert(export, functions.get(item.name.as_str()).copied());
                    }
                }
                _ => {}
            }
        }
        ModuleInfo {
            exports,
            complete: top_level == facts.export_statements,
        }
    }
}

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
    /// `export` statements anywhere, at the top level or not.
    export_statements: usize,
}

impl Facts {
    fn bind(&mut self, name: &str) {
        *self.bindings.entry(name.to_string()).or_default() += 1;
    }

    fn is_unique(&self, name: &str) -> bool {
        self.bindings.get(name).copied() == Some(1)
    }

    fn bind_pattern(&mut self, pattern: &BindingPattern) {
        match &pattern.kind {
            BindingPatternKind::Name(name) => self.bind(name),
            BindingPatternKind::Ignore => {}
            BindingPatternKind::Tuple(items) => {
                for item in items {
                    self.bind_pattern(item);
                }
            }
            BindingPatternKind::Array { elements, rest } => {
                for item in elements.iter().chain(rest.as_deref()) {
                    self.bind_pattern(item);
                }
            }
        }
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
            StmtKind::LetPattern { pattern, .. } => self.bind_pattern(pattern),
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
                        self.bind(alias.as_deref().unwrap_or(module_stem(path)));
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
                self.export_statements += 1;
                if let StmtKind::Let { name, .. }
                | StmtKind::Fn { name, .. }
                | StmtKind::Class { name, .. } = &inner.kind
                {
                    self.exported.insert(name.clone());
                }
            }
            StmtKind::ExportList(items) => {
                self.export_statements += 1;
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
    /// No decorator can replace the class.
    undecorated: bool,
    static_fields: Vec<&'ast str>,
    is_abstract: bool,
}

/// What a name a call can resolve to.
struct Tables<'ast> {
    functions: HashMap<&'ast str, &'ast [Param]>,
    /// Top-level functions whose result has a known built-in type: their
    /// return annotation is `Int`, `Float`, `String` or `Bool`, and the VM
    /// checks it on every return.
    returns: HashMap<&'ast str, ValueType<'ast>>,
    classes: HashMap<&'ast str, ClassInfo<'ast>>,
    /// Top-level functions marked `@deprecated`, with the note it gives.
    deprecated: HashMap<&'ast str, Option<&'ast str>>,
    /// Top-level functions that call one of their parameters inside a `try`
    /// with a `catch`: a function given to them may be meant to fail.
    catching: HashSet<&'ast str>,
    /// The files imported with a relative path, by that path.
    modules: HashMap<&'ast str, ModuleInfo<'ast>>,
    /// Functions imported by name from those files: the local name, and the
    /// function's parameters.
    imported: HashMap<&'ast str, &'ast [Param]>,
    /// Names bound to the exports of one of those files (`import m from
    /// "./m.pln"`, `import * as m from ...`), with the file's path.
    module_names: HashMap<&'ast str, &'ast str>,
}

/// Finds a call of one of `params` inside the body of a `try` with a `catch`.
struct CallsInTry<'ast> {
    params: HashSet<&'ast str>,
    in_try: usize,
    found: bool,
}

impl<'ast> Visitor<'ast> for CallsInTry<'ast> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match &stmt.kind {
            StmtKind::Try {
                try_branch,
                catch_branches,
                ..
            } if !catch_branches.is_empty() => {
                self.in_try += 1;
                self.visit_stmt(try_branch);
                self.in_try -= 1;
                for catch in catch_branches {
                    self.visit_catch_block(catch);
                }
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.in_try > 0
            && let ExprKind::Call { callee, .. } = &expr.kind
            && let ExprKind::Ident(name) = &callee.kind
            && self.params.contains(name.as_str())
        {
            self.found = true;
        }
        visitor::walk_expr(self, expr);
    }
}

/// The type of a value: a built-in one (its name and its family, see
/// [`literal_type`]) or an instance of a class declared in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueType<'ast> {
    Builtin(&'static str, &'static str),
    Instance(&'ast str),
}

impl ValueType<'_> {
    fn family(self) -> &'static str {
        match self {
            ValueType::Builtin(_, family) => family,
            ValueType::Instance(_) => "instance",
        }
    }

    /// The built-in type an annotation guarantees once the VM has checked a
    /// value against it: `Float` also holds an `Int` given to it, since the
    /// check widens it. Any other annotation guarantees nothing here.
    fn of_annotation(ty: &str, facts: &Facts) -> Option<Self> {
        let ty = ty.trim();
        if facts.type_names.contains(ty) {
            return None;
        }
        match ty {
            "Int" => Some(ValueType::Builtin("Int", "int")),
            "Float" => Some(ValueType::Builtin("Float", "float")),
            "String" => Some(ValueType::Builtin("String", "text")),
            "Bool" => Some(ValueType::Builtin("Bool", "bool")),
            _ => None,
        }
    }
}

/// What a name bound by a `let` is known to hold.
#[derive(Debug, Clone, Copy)]
enum Known<'ast> {
    Value(ValueType<'ast>),
    /// A lambda, with its parameters.
    Lambda(&'ast [Param]),
}

impl<'ast> Tables<'ast> {
    fn build(
        program: &'ast Program,
        facts: &Facts,
        modules: HashMap<&'ast str, ModuleInfo<'ast>>,
    ) -> Self {
        let mut tables = Tables {
            functions: HashMap::new(),
            returns: HashMap::new(),
            classes: HashMap::new(),
            deprecated: HashMap::new(),
            catching: HashSet::new(),
            modules,
            imported: HashMap::new(),
            module_names: HashMap::new(),
        };
        for stmt in &program.statements {
            if let StmtKind::Import { path, kind } = &stmt.kind
                && let Some(module) = tables.modules.get(path.as_str())
            {
                match kind {
                    ImportKind::Names(items) => {
                        for item in items {
                            let local = item.alias.as_deref().unwrap_or(&item.name);
                            if facts.is_unique(local)
                                && let Some(Some(params)) = module.exports.get(item.name.as_str())
                            {
                                tables.imported.insert(local, params);
                            }
                        }
                    }
                    ImportKind::Module { alias } => {
                        let local = alias.as_deref().unwrap_or(module_stem(path));
                        if facts.is_unique(local) {
                            tables.module_names.insert(local, path);
                        }
                    }
                    ImportKind::Star { alias } => {
                        if facts.is_unique(alias) {
                            tables.module_names.insert(alias, path);
                        }
                    }
                }
            }
            let stmt = match &stmt.kind {
                StmtKind::Export(inner) => inner.as_ref(),
                _ => stmt,
            };
            if let StmtKind::Fn {
                name,
                return_ty: Some(return_ty),
                annotations,
                is_async: false,
                ..
            } = &stmt.kind
                && facts.is_unique(name)
                && preserves_signature(annotations)
                // An `@inline` function's returns are not checked.
                && !annotations.iter().any(|a| a.name == "inline")
                && let Some(ty) = ValueType::of_annotation(return_ty, facts)
            {
                tables.returns.insert(name, ty);
            }
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
            if let StmtKind::Fn {
                name, params, body, ..
            } = &stmt.kind
            {
                let mut finder = CallsInTry {
                    params: params.iter().map(|p| p.name.as_str()).collect(),
                    in_try: 0,
                    found: false,
                };
                finder.visit_stmt(body);
                if finder.found {
                    tables.catching.insert(name);
                }
            }
            match &stmt.kind {
                StmtKind::Fn {
                    name,
                    params,
                    annotations,
                    ..
                } if facts.is_unique(name) && preserves_signature(annotations) => {
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
                            static_fields: fields
                                .iter()
                                .filter(|f| f.is_static)
                                .map(|f| f.name.as_str())
                                .collect(),
                            methods,
                            properties: properties.iter().map(|p| p.name.as_str()).collect(),
                            nested,
                            private,
                            undecorated: annotations.is_empty(),
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

    /// `class` when a call of it makes an instance of exactly that class:
    /// it is declared here without a decorator, is not abstract, and it and
    /// its superclasses are all declared here (so none is a built-in type).
    fn instance_class(&self, class: &'ast str) -> Option<&'ast str> {
        let info = self.classes.get(class)?;
        if info.is_abstract {
            return None;
        }
        let mut current = info;
        for _ in 0..64 {
            if !current.undecorated {
                return None;
            }
            match current.superclass {
                Some(superclass) => current = self.classes.get(superclass)?,
                None => return Some(class),
            }
        }
        None
    }

    /// The chain of classes `this` may be in a method of `class`, when every
    /// member of every instance is known: the class and its superclasses are
    /// all declared here, unchanged by decorators or `extend`, and nothing else
    /// can subclass or extend them.
    fn closed_chain(&self, class: &str, facts: &Facts) -> Option<Vec<&ClassInfo<'ast>>> {
        if facts.subclassed.contains(class) {
            return None;
        }
        self.member_chain(class, facts)
    }

    /// `class` and its superclasses, when every member of an instance of
    /// exactly `class`, and of `class` itself, is known: they are all declared
    /// here, unchanged by decorators or `extend`, and not exported (an
    /// importer could extend them).
    fn member_chain(&self, class: &str, facts: &Facts) -> Option<Vec<&ClassInfo<'ast>>> {
        if facts.dynamic_members {
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
    /// For each enclosing block, what the names its `let`s bound so far are
    /// known to hold. Only names bound once in the file are entered, so a
    /// name found here can mean nothing else.
    scopes: Vec<HashMap<&'ast str, Known<'ast>>>,
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

    /// Reports `value` when its type is known and cannot match the annotation
    /// `ty`. `describe` makes the message from what was found ("a String
    /// literal", "a Point instance").
    fn check_value(&mut self, ty: &str, value: &Expr, describe: impl FnOnce(&str) -> String) {
        if let Some((found, literal)) = self.value_type(value)
            && literal_matches(ty, found.family(), self.facts) == Some(false)
        {
            let message = describe(&describe_value(found, literal));
            self.report(Severity::Error, message, value.span);
        }
    }

    /// What the innermost `let` that bound `name` is known to hold.
    fn lookup(&self, name: &str) -> Option<Known<'ast>> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    /// The type `expr` has whenever it gives a value, and whether it is a
    /// literal.
    fn value_type(&self, expr: &'ast Expr) -> Option<(ValueType<'ast>, bool)> {
        if let Some((name, family)) = literal_type(expr) {
            return Some((ValueType::Builtin(name, family), true));
        }
        match &expr.kind {
            ExprKind::Paren(inner) => self.value_type(inner),
            ExprKind::Ident(name) => match self.lookup(name)? {
                Known::Value(ty) => Some((ty, false)),
                Known::Lambda(_) => None,
            },
            // `f(1, _)` makes a function.
            ExprKind::Call { callee, args }
                if !args.iter().any(|a| matches!(a, CallArg::Placeholder)) =>
            {
                let ExprKind::Ident(name) = &callee.kind else {
                    return None;
                };
                if let Some(class) = self.tables.instance_class(name) {
                    Some((ValueType::Instance(class), false))
                } else {
                    Some((*self.tables.returns.get(name.as_str())?, false))
                }
            }
            _ => None,
        }
    }

    /// What `let name = value` (with the annotation `ty`, without `mut`)
    /// makes `name` hold.
    fn known_let(&self, ty: Option<&str>, value: &'ast Expr) -> Option<Known<'ast>> {
        if let Some(ty) = ty {
            // The VM checks the value against the annotation.
            return ValueType::of_annotation(ty, self.facts).map(Known::Value);
        }
        match &value.kind {
            ExprKind::Lambda {
                params,
                annotations,
                ..
            } if preserves_signature(annotations) => Some(Known::Lambda(params)),
            _ => self.value_type(value).map(|(ty, _)| Known::Value(ty)),
        }
    }

    /// Names imported from a file that does not export them.
    fn check_imports(&mut self, program: &'ast Program) {
        for stmt in &program.statements {
            let StmtKind::Import {
                path,
                kind: ImportKind::Names(items),
            } = &stmt.kind
            else {
                continue;
            };
            let Some(module) = self.tables.modules.get(path.as_str()) else {
                continue;
            };
            if !module.complete {
                continue;
            }
            for item in items {
                if module.exports.contains_key(item.name.as_str()) {
                    continue;
                }
                let hint = did_you_mean(&item.name, module.exports.keys().copied());
                let message = format!("'{path}' does not export '{}'{hint}", item.name);
                self.report(Severity::Warning, message, stmt.span);
            }
        }
    }

    /// Checks `args` against `params` the way the VM binds them.
    fn check_call(&mut self, callee: &str, params: &[Param], args: &[Arg<'ast>], span: Span) {
        let positional: Vec<&'ast Expr> = args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Positional(expr) => Some(*expr),
                Arg::Named(..) => None,
            })
            .collect();
        let named: Vec<(&str, &'ast Expr)> = args
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

        let mut bound: Vec<(&Param, &'ast Expr)> = Vec::new();
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
                self.check_value(ty, expr, |found| {
                    format!("{callee} expects {ty} for '{}', found {found}", param.name)
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
                } else if let Some(params) = self.tables.imported.get(name.as_str()) {
                    self.check_call(name, params, &effective, span);
                } else if let Some(Known::Lambda(params)) = self.lookup(name) {
                    self.check_call(name, params, &effective, span);
                }
            }
            ExprKind::Property { object, name } if matches!(object.kind, ExprKind::This) => {
                self.check_this_call(name, &effective, callee.span, span);
            }
            ExprKind::Property { object, name } => {
                if let Some((ValueType::Instance(class), _)) = self.value_type(object) {
                    if let Some(chain) = self.tables.member_chain(class, self.facts) {
                        let inside = self.class.as_ref().is_some_and(|(c, _)| *c == class);
                        self.check_method_call(
                            class,
                            &chain,
                            name,
                            &effective,
                            (callee.span, span),
                            inside,
                        );
                    }
                    return;
                }
                let ExprKind::Ident(object) = &object.kind else {
                    return;
                };
                if self.tables.classes.contains_key(object.as_str()) {
                    if let Some(chain) = self.tables.member_chain(object, self.facts) {
                        self.check_static_call(object, &chain, name, &effective, callee.span, span);
                    }
                } else if let Some(path) = self.tables.module_names.get(object.as_str())
                    && let Some(Some(params)) = self.tables.modules[path].exports.get(name.as_str())
                {
                    // The exports win over a map's own methods.
                    self.check_call(&format!("{object}.{name}"), params, &effective, span);
                }
            }
            _ => {}
        }
    }

    fn check_this_call(&mut self, method: &str, args: &[Arg<'ast>], callee_span: Span, span: Span) {
        let Some((class, chain)) = &self.class else {
            return;
        };
        let (class, chain) = (*class, chain.clone());
        self.check_method_call(class, &chain, method, args, (callee_span, span), true);
    }

    /// `receiver.method(args)` on an instance of `class`, whose class and
    /// superclasses are `chain`. `inside` is true in a method of `class`,
    /// where its private members can be used.
    fn check_method_call(
        &mut self,
        class: &str,
        chain: &[&ClassInfo<'ast>],
        method: &str,
        args: &[Arg<'ast>],
        (callee_span, span): (Span, Span),
        inside: bool,
    ) {
        if method == "constructor"
            || self.facts.assigned_properties.contains(method)
            || self.facts.strings.contains(method)
        {
            return;
        }
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
                        .filter(move |name| (inside && depth == 0) || !info.private.contains(name))
                });
                let hint = did_you_mean(method, members);
                let message = format!("{class} has no method '{method}'{hint}");
                self.report(Severity::Warning, message, callee_span);
            }
            Some(decl)
                if decl.kind == FunctionKind::Method
                    && !decl.is_static
                    && preserves_signature(&decl.annotations) =>
            {
                let name = format!("{class}.{method}");
                self.check_call(&name, &decl.params, args, span);
            }
            Some(_) => {}
        }
    }

    /// `Class.method(args)`, `chain` being `class` and its superclasses.
    fn check_static_call(
        &mut self,
        class: &str,
        chain: &[&ClassInfo<'ast>],
        method: &str,
        args: &[Arg<'ast>],
        callee_span: Span,
        span: Span,
    ) {
        if self.facts.assigned_properties.contains(method) || self.facts.strings.contains(method) {
            return;
        }
        // Static methods are looked up through the superclasses, like the VM.
        let found = chain.iter().find_map(|info| {
            info.methods
                .iter()
                .find(|m| m.is_static && m.name == method)
        });
        match found {
            Some(decl) => {
                if decl.kind == FunctionKind::Method && preserves_signature(&decl.annotations) {
                    let name = format!("{class}.{method}");
                    self.check_call(&name, &decl.params, args, span);
                }
            }
            None => {
                // Any other member of that name may be what was meant, or a
                // static field holding a function: leave it to the VM.
                let declared = chain.iter().any(|info| {
                    info.fields.contains(&method)
                        || info.properties.contains(&method)
                        || info.nested.contains(&method)
                        || info.methods.iter().any(|m| m.name == method)
                });
                if declared {
                    return;
                }
                let statics = chain.iter().flat_map(|info| {
                    info.methods
                        .iter()
                        .filter(|m| m.is_static && !info.private.contains(m.name.as_str()))
                        .map(|m| m.name.as_str())
                        .chain(info.static_fields.iter().copied())
                        .chain(info.nested.iter().copied())
                });
                let hint = did_you_mean(method, statics);
                let message = format!("{class} has no static method '{method}'{hint}");
                self.report(Severity::Warning, message, callee_span);
            }
        }
    }

    fn visit_args(&mut self, args: &'ast [CallArg]) {
        for arg in args {
            self.visit_call_arg(arg);
        }
    }
}

/// "an Int literal", "a String value", "a Point instance", "null".
fn describe_value(ty: ValueType, literal: bool) -> String {
    let (name, what) = match ty {
        ValueType::Builtin("Null", _) => return "null".to_string(),
        ValueType::Builtin(name, _) if literal => (name, "literal"),
        ValueType::Builtin(name, _) => (name, "value"),
        ValueType::Instance(class) => (class, "instance"),
    };
    let an = matches!(name.chars().next(), Some('A' | 'E' | 'I' | 'O' | 'U'));
    format!("{} {name} {what}", if an { "an" } else { "a" })
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
                ty,
                value: Some(value),
                is_lazy: false,
                mutable,
                ..
            } => {
                if let Some(ty) = ty {
                    self.check_value(ty, value, |found| {
                        format!("'{name}' is declared {ty} but given {found}")
                    });
                }
                visitor::walk_stmt(self, stmt);
                // A name bound once, without `mut`, holds this value wherever
                // it is in scope.
                if !mutable
                    && self.facts.is_unique(name)
                    && let Some(known) = self.known_let(ty.as_deref(), value)
                    && let Some(scope) = self.scopes.last_mut()
                {
                    scope.insert(name, known);
                }
            }
            // A `let` is in scope until the end of the statement that holds it.
            StmtKind::Block(_)
            | StmtKind::CFor { .. }
            | StmtKind::Module { .. }
            | StmtKind::If { .. }
            | StmtKind::While { .. }
            | StmtKind::For { .. }
            | StmtKind::DoWhile { .. }
            | StmtKind::Labeled { .. } => {
                self.scopes.push(HashMap::new());
                visitor::walk_stmt(self, stmt);
                self.scopes.pop();
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
                    self.check_value(&ty, value, |found| {
                        format!("{name} is declared to return {ty} but returns {found}")
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
                // `assert.throws(fn() { f(1, 2); })` expects the failure, and so
                // does `errorOf(fn() { f(1, 2); })` when `errorOf` calls its
                // argument in a `try`.
                let callee_name = match &callee.kind {
                    ExprKind::Ident(name) | ExprKind::Property { name, .. } => name.as_str(),
                    _ => "",
                };
                let expects_failure = usize::from(
                    callee_name.to_lowercase().contains("throw")
                        || (matches!(callee.kind, ExprKind::Ident(_))
                            && self.tables.catching.contains(callee_name)),
                );
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
                    self.check_value(ty, body, |found| {
                        format!("lambda is declared to return {ty} but returns {found}")
                    });
                }
                self.returns
                    .push(checked.map(|ty| ("lambda".to_string(), ty)));
                visitor::walk_expr(self, expr);
                self.returns.pop();
                self.class = saved;
            }
            ExprKind::Block(_) => {
                self.scopes.push(HashMap::new());
                visitor::walk_expr(self, expr);
                self.scopes.pop();
            }
            _ => visitor::walk_expr(self, expr),
        }
    }
}

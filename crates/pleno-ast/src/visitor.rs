use crate::*;

pub trait Visitor<T: Default = ()> {
    fn visit_program(&mut self, program: &Program) -> T {
        walk_program(self, program)
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> T {
        walk_stmt(self, stmt)
    }

    fn visit_expr(&mut self, expr: &Expr) -> T {
        walk_expr(self, expr)
    }

    fn visit_pattern(&mut self, pattern: &Pattern) -> T {
        walk_pattern(self, pattern)
    }

    fn visit_param(&mut self, param: &Param) -> T {
        walk_param(self, param)
    }

    fn visit_annotation(&mut self, annotation: &Annotation) -> T {
        walk_annotation(self, annotation)
    }

    fn visit_generic_param(&mut self, param: &GenericParam) -> T {
        walk_generic_param(self, param)
    }

    fn visit_field_decl(&mut self, field: &FieldDecl) -> T {
        walk_field_decl(self, field)
    }

    fn visit_function_decl(&mut self, decl: &FunctionDecl) -> T {
        walk_function_decl(self, decl)
    }

    fn visit_property_decl(&mut self, decl: &PropertyDecl) -> T {
        walk_property_decl(self, decl)
    }

    fn visit_enum_decl(&mut self, decl: &EnumDecl) -> T {
        walk_enum_decl(self, decl)
    }

    fn visit_enum_variant(&mut self, variant: &EnumVariant) -> T {
        walk_enum_variant(self, variant)
    }

    fn visit_catch_block(&mut self, catch: &CatchBlock) -> T {
        walk_catch_block(self, catch)
    }

    fn visit_match_arm(&mut self, arm: &MatchArm) -> T {
        walk_match_arm(self, arm)
    }

    fn visit_call_arg(&mut self, arg: &CallArg) -> T {
        walk_call_arg(self, arg)
    }

    fn visit_map_item(&mut self, item: &MapItem) -> T {
        walk_map_item(self, item)
    }
}

pub fn walk_program<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, program: &Program) -> T {
    for stmt in &program.statements {
        visitor.visit_stmt(stmt);
    }
    T::default()
}

pub fn walk_stmt<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, stmt: &Stmt) -> T {
    match &stmt.kind {
        StmtKind::Module { body, .. } => {
            if let Some(stmts) = body {
                for s in stmts {
                    visitor.visit_stmt(s);
                }
            }
        }
        StmtKind::Let { value, .. } => {
            if let Some(v) = value {
                visitor.visit_expr(v);
            }
        }
        StmtKind::MacroDecl { body, .. } => {
            visitor.visit_stmt(body);
        }
        StmtKind::Assign { value, .. } => {
            visitor.visit_expr(value);
        }
        StmtKind::AssignProperty { object, value, .. } => {
            visitor.visit_expr(object);
            visitor.visit_expr(value);
        }
        StmtKind::AssignIndex {
            collection,
            index,
            value,
        } => {
            visitor.visit_expr(collection);
            visitor.visit_expr(index);
            visitor.visit_expr(value);
        }
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expr(condition);
            visitor.visit_stmt(then_branch);
            if let Some(eb) = else_branch {
                visitor.visit_stmt(eb);
            }
        }
        StmtKind::While { condition, body } => {
            visitor.visit_expr(condition);
            visitor.visit_stmt(body);
        }
        StmtKind::For { iterable, body, .. } => {
            visitor.visit_expr(iterable);
            visitor.visit_stmt(body);
        }
        StmtKind::CFor {
            init,
            condition,
            step,
            body,
        } => {
            if let Some(i) = init {
                visitor.visit_stmt(i);
            }
            if let Some(c) = condition {
                visitor.visit_expr(c);
            }
            if let Some(s) = step {
                visitor.visit_stmt(s);
            }
            visitor.visit_stmt(body);
        }
        StmtKind::DoWhile { body, condition } => {
            visitor.visit_stmt(body);
            visitor.visit_expr(condition);
        }
        StmtKind::Import { .. } => {}
        StmtKind::Export(inner) => {
            visitor.visit_stmt(inner);
        }
        StmtKind::ExportList(_) => {}
        StmtKind::Class {
            generics,
            fields,
            methods,
            properties,
            nested_classes,
            ..
        } => {
            for g in generics {
                visitor.visit_generic_param(g);
            }
            for f in fields {
                visitor.visit_field_decl(f);
            }
            for m in methods {
                visitor.visit_function_decl(m);
            }
            for p in properties {
                visitor.visit_property_decl(p);
            }
            for n in nested_classes {
                visitor.visit_stmt(n);
            }
        }
        StmtKind::Interface {
            generics,
            methods,
            properties,
            ..
        } => {
            for g in generics {
                visitor.visit_generic_param(g);
            }
            for m in methods {
                visitor.visit_function_decl(m);
            }
            for p in properties {
                visitor.visit_property_decl(p);
            }
        }
        StmtKind::Enum(decl) => {
            visitor.visit_enum_decl(decl);
        }
        StmtKind::Fn {
            generics,
            params,
            body,
            ..
        } => {
            for g in generics {
                visitor.visit_generic_param(g);
            }
            for p in params {
                visitor.visit_param(p);
            }
            visitor.visit_stmt(body);
        }
        StmtKind::Try {
            try_branch,
            catch_branches,
            finally_branch,
        } => {
            visitor.visit_stmt(try_branch);
            for c in catch_branches {
                visitor.visit_catch_block(c);
            }
            if let Some(f) = finally_branch {
                visitor.visit_stmt(f);
            }
        }
        StmtKind::Throw(expr) => {
            visitor.visit_expr(expr);
        }
        StmtKind::Return(expr) => {
            if let Some(e) = expr {
                visitor.visit_expr(e);
            }
        }
        StmtKind::Break(_) => {}
        StmtKind::Continue(_) => {}
        StmtKind::Labeled { stmt, .. } => {
            visitor.visit_stmt(stmt);
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                visitor.visit_stmt(s);
            }
        }
        StmtKind::TypeAlias { .. } => {}
        StmtKind::Extend {
            generics, methods, ..
        } => {
            for g in generics {
                visitor.visit_generic_param(g);
            }
            for m in methods {
                visitor.visit_function_decl(m);
            }
        }
        StmtKind::Expr(expr) => {
            visitor.visit_expr(expr);
        }
    }
    T::default()
}

pub fn walk_expr<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, expr: &Expr) -> T {
    match &expr.kind {
        ExprKind::Literal(_) => {}
        ExprKind::Ident(_) => {}
        ExprKind::This => {}
        ExprKind::Super => {}
        ExprKind::Unary { expr, .. } => {
            visitor.visit_expr(expr);
        }
        ExprKind::Binary { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        ExprKind::Property { object, .. } => {
            visitor.visit_expr(object);
        }
        ExprKind::SafeProperty { object, .. } => {
            visitor.visit_expr(object);
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                visitor.visit_expr(s);
            }
            if let Some(e) = end {
                visitor.visit_expr(e);
            }
        }
        ExprKind::MapLiteral(items) => {
            for item in items {
                visitor.visit_map_item(item);
            }
        }
        ExprKind::NamedTupleLiteral(entries) => {
            for (_, e) in entries {
                visitor.visit_expr(e);
            }
        }
        ExprKind::ArrayLiteral(items) => {
            for item in items {
                visitor.visit_expr(item);
            }
        }
        ExprKind::TupleLiteral(items) => {
            for item in items {
                visitor.visit_expr(item);
            }
        }
        ExprKind::Lambda { params, body, .. } => {
            for p in params {
                visitor.visit_param(p);
            }
            visitor.visit_expr(body);
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                visitor.visit_stmt(s);
            }
        }
        ExprKind::Underscore => {}
        ExprKind::Index { collection, index } => {
            visitor.visit_expr(collection);
            visitor.visit_expr(index);
        }
        ExprKind::Slice {
            collection,
            start,
            end,
        } => {
            visitor.visit_expr(collection);
            if let Some(s) = start {
                visitor.visit_expr(s);
            }
            if let Some(e) = end {
                visitor.visit_expr(e);
            }
        }
        ExprKind::SetLiteral(items) => {
            for item in items {
                visitor.visit_expr(item);
            }
        }
        ExprKind::Call { callee, args } => {
            visitor.visit_expr(callee);
            for arg in args {
                visitor.visit_call_arg(arg);
            }
        }
        ExprKind::Pipeline { left, right } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        ExprKind::AssertNotNull(expr) => {
            visitor.visit_expr(expr);
        }
        ExprKind::SafeCall { callee, args } => {
            visitor.visit_expr(callee);
            for arg in args {
                visitor.visit_call_arg(arg);
            }
        }
        ExprKind::Match { expr, arms } => {
            visitor.visit_expr(expr);
            for arm in arms {
                visitor.visit_match_arm(arm);
            }
        }
        ExprKind::Await { expr } => {
            visitor.visit_expr(expr);
        }
        ExprKind::Spread(expr) => {
            visitor.visit_expr(expr);
        }
        ExprKind::Paren(expr) => {
            visitor.visit_expr(expr);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expr(condition);
            visitor.visit_expr(then_branch);
            visitor.visit_expr(else_branch);
        }
        ExprKind::Is { left, .. } => {
            visitor.visit_expr(left);
        }
        ExprKind::Specialize { left, .. } => {
            visitor.visit_expr(left);
        }
        ExprKind::MacroCall { args, .. } => {
            for arg in args {
                visitor.visit_call_arg(arg);
            }
        }
        ExprKind::MacroSplice(expr) => {
            visitor.visit_expr(expr);
        }
        ExprKind::InterpolatedString(exprs) => {
            for e in exprs {
                visitor.visit_expr(e);
            }
        }
    }
    T::default()
}

pub fn walk_pattern<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, pattern: &Pattern) -> T {
    match pattern {
        Pattern::Literal(_) => {}
        Pattern::Ident(_) => {}
        Pattern::Tuple(patterns) => {
            for p in patterns {
                visitor.visit_pattern(p);
            }
        }
        Pattern::Rest(_) => {}
        Pattern::Underscore => {}
        Pattern::Variant(_, patterns) => {
            for p in patterns {
                visitor.visit_pattern(p);
            }
        }
        Pattern::Object(items, rest) => {
            for (_, p) in items {
                visitor.visit_pattern(p);
            }
            if let Some(r) = rest {
                visitor.visit_pattern(r);
            }
        }
        Pattern::Type(_, _) => {}
        Pattern::Or(patterns) => {
            for p in patterns {
                visitor.visit_pattern(p);
            }
        }
    }
    T::default()
}

pub fn walk_param<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, param: &Param) -> T {
    if let Some(d) = &param.default {
        visitor.visit_expr(d);
    }
    T::default()
}

pub fn walk_annotation<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    annotation: &Annotation,
) -> T {
    for arg in &annotation.args {
        visitor.visit_expr(arg);
    }
    T::default()
}

pub fn walk_generic_param<V: Visitor<T> + ?Sized, T: Default>(
    _visitor: &mut V,
    _param: &GenericParam,
) -> T {
    T::default()
}

pub fn walk_field_decl<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    field: &FieldDecl,
) -> T {
    if let Some(d) = &field.default {
        visitor.visit_expr(d);
    }
    for a in &field.annotations {
        visitor.visit_annotation(a);
    }
    T::default()
}

pub fn walk_function_decl<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    decl: &FunctionDecl,
) -> T {
    for g in &decl.generics {
        visitor.visit_generic_param(g);
    }
    for p in &decl.params {
        visitor.visit_param(p);
    }
    visitor.visit_stmt(&decl.body);
    for a in &decl.annotations {
        visitor.visit_annotation(a);
    }
    T::default()
}

pub fn walk_property_decl<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    decl: &PropertyDecl,
) -> T {
    if let Some(g) = &decl.getter {
        visitor.visit_function_decl(g);
    }
    if let Some(s) = &decl.setter {
        visitor.visit_function_decl(s);
    }
    for a in &decl.annotations {
        visitor.visit_annotation(a);
    }
    T::default()
}

pub fn walk_enum_decl<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, decl: &EnumDecl) -> T {
    for v in &decl.variants {
        visitor.visit_enum_variant(v);
    }
    for m in &decl.methods {
        visitor.visit_function_decl(m);
    }
    for a in &decl.annotations {
        visitor.visit_annotation(a);
    }
    T::default()
}

pub fn walk_enum_variant<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    variant: &EnumVariant,
) -> T {
    if let Some(params) = &variant.params {
        for p in params {
            visitor.visit_param(p);
        }
    }
    T::default()
}

pub fn walk_catch_block<V: Visitor<T> + ?Sized, T: Default>(
    visitor: &mut V,
    catch: &CatchBlock,
) -> T {
    visitor.visit_stmt(&catch.body);
    T::default()
}

pub fn walk_match_arm<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, arm: &MatchArm) -> T {
    visitor.visit_pattern(&arm.pattern);
    if let Some(g) = &arm.guard {
        visitor.visit_expr(g);
    }
    visitor.visit_expr(&arm.body);
    T::default()
}

pub fn walk_call_arg<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, arg: &CallArg) -> T {
    match arg {
        CallArg::Positional(e) => {
            visitor.visit_expr(e);
        }
        CallArg::Named(_, e) => {
            visitor.visit_expr(e);
        }
        CallArg::Placeholder => {}
        CallArg::Spread(e) => {
            visitor.visit_expr(e);
        }
    }
    T::default()
}

pub fn walk_map_item<V: Visitor<T> + ?Sized, T: Default>(visitor: &mut V, item: &MapItem) -> T {
    match item {
        MapItem::Entry(_, e) => {
            visitor.visit_expr(e);
        }
        MapItem::Spread(e) => {
            visitor.visit_expr(e);
        }
    }
    T::default()
}

use pleno_ast::{
    Expr, ExprKind, FunctionDecl, FunctionKind, Literal, MapItem, Pattern, Program, Span, Stmt,
    StmtKind,
};
use pleno_bytecode::{Chunk, FunctionProto, Instruction, Opcode, ParamSpec};
use pleno_value::Value;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
}

impl CompileError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub struct Compiler {
    chunk: Chunk,
    registers: RegisterAllocator,
    last_expr_register: Option<u8>,
    locals: Vec<HashMap<String, Local>>,
    loop_counter: u32,
    loop_stack: Vec<LoopContext>,
    captures: HashMap<String, usize>,
    is_top_level: bool,
    current_super: Option<String>,
    module_mode: bool,
    module_exports: Vec<ExportItem>,
    module_name: Option<String>,
    source: String,
    current_span: pleno_ast::Span,
    next_labels: Vec<String>,
    type_aliases: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct ExportItem {
    local: String,
    export: String,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl Compiler {
    pub fn new(source: String) -> Self {
        Self {
            chunk: Chunk::new(),
            registers: RegisterAllocator::new(),
            last_expr_register: None,
            locals: vec![HashMap::new()],
            loop_counter: 0,
            loop_stack: Vec::new(),
            captures: HashMap::new(),
            is_top_level: true,
            current_super: None,
            module_mode: false,
            module_exports: Vec::new(),
            module_name: None,
            source,
            current_span: pleno_ast::Span::new(0, 0),
            next_labels: Vec::new(),
            type_aliases: HashMap::new(),
        }
    }

    fn error(&self, message: &str, span: Span) -> CompileError {
        use pleno_diagnostics::{Diagnostic, DiagnosticReporter};
        let diagnostic = Diagnostic::new(message, span);
        let formatted = DiagnosticReporter::report(&diagnostic, &self.source);
        CompileError::new(formatted)
    }

    pub fn compile(&mut self, program: &Program) -> Result<Chunk, CompileError> {
        self.module_mode = false;
        self.module_exports.clear();
        self.module_name = None;
        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }
        let return_reg = match self.last_expr_register {
            Some(reg) => reg,
            None => {
                let reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, reg, 0, 0);
                reg
            }
        };
        self.emit(Opcode::Return, return_reg, 0, 0);
        self.chunk.max_registers = self.registers.max_used;
        self.chunk.module_name = None;
        Ok(self.chunk.clone())
    }

    pub fn compile_module(&mut self, program: &Program) -> Result<Chunk, CompileError> {
        self.module_mode = true;
        self.module_exports.clear();
        self.module_name = None;
        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }
        let exports_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, exports_reg, 0, 0);
        for item in self.module_exports.clone() {
            let export_name = item.export.clone();
            let local_name = item.local.clone();
            let export_idx = self.add_constant(Value::String(export_name.clone().into()));
            let local_idx = self.add_constant(Value::String(local_name.clone().into()));
            let mut temp_reg = None;
            let value_reg = if let Some(local) = self.resolve_local(&local_name) {
                local.reg
            } else {
                let reg = self.registers.allocate()?;
                self.emit(Opcode::GetGlobal, reg, local_idx, 0);
                temp_reg = Some(reg);
                reg
            };
            self.emit(Opcode::SetProperty, value_reg, exports_reg, export_idx);
            if let Some(reg) = temp_reg {
                self.free_temp(reg);
            }
        }
        self.emit(Opcode::Return, exports_reg, 0, 0);
        self.chunk.max_registers = self.registers.max_used;
        self.chunk.module_name = self.module_name.clone();
        Ok(self.chunk.clone())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        self.current_span = stmt.span;

        match &stmt.kind {
            StmtKind::Module { name, body } => {
                if let Some(stmts) = body {
                    for stmt in stmts {
                        self.compile_stmt(stmt)?;
                    }
                } else {
                    if !self.module_mode {
                        return Err(
                            self.error("module declaration only allowed in modules", stmt.span)
                        );
                    }
                    if self.module_name.is_some() {
                        return Err(self.error("module already declared", stmt.span));
                    }
                    self.module_name = Some(name.clone());
                }
                Ok(())
            }
            StmtKind::TypeAlias { name, ty } => {
                self.type_aliases.insert(name.clone(), ty.clone());
                Ok(())
            }
            StmtKind::Expr(expr) => {
                let reg = self.compile_expr(expr)?;
                self.last_expr_register = Some(reg);
                Ok(())
            }
            StmtKind::Let {
                name,
                value,
                mutable,
                is_const,
                is_deep,
                ty,
            } => {
                if *is_const && value.is_none() {
                    return Err(self.error("const requires an initializer", stmt.span));
                }
                let reg = if let Some(expr) = value {
                    let reg = self.compile_expr(expr)?;
                    if let Some(ty_name) = ty {
                        self.emit_type_check(reg, ty_name)?;
                    }
                    reg
                } else {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, reg, 0, 0);
                    // For now, Null is only allowed if no type or if we add optional types later
                    // but many primitives don't allow Null.
                    // Keeping it simple: if typed, Null must match the type (it won't for Int etc)
                    if let Some(ty_name) = ty {
                        self.emit_type_check(reg, ty_name)?;
                    }
                    reg
                };
                if *is_deep {
                    self.emit(Opcode::Freeze, reg, 0, 0);
                }
                self.insert_local(name.clone(), reg, *mutable, ty.clone())?;
                Ok(())
            }
            StmtKind::Assign { name, value } => {
                let value_reg = self.compile_expr(value)?;
                if let Some(local) = self.resolve_local(name) {
                    if !local.mutable {
                        return Err(self.error("cannot assign to immutable binding", stmt.span));
                    }
                    if let Some(ty_name) = &local.ty {
                        self.emit_type_check(value_reg, ty_name)?;
                    }
                    self.emit(Opcode::Move, local.reg, value_reg, 0);
                } else if let Some(index) = self.captures.get(name).cloned() {
                    // Update capture
                    self.emit(Opcode::SetCapture, value_reg, index as u8, 0);
                } else {
                    return Err(
                        self.error(&format!("unknown local or global: {}", name), stmt.span)
                    );
                }
                self.free_temp(value_reg);
                Ok(())
            }
            StmtKind::AssignProperty {
                object,
                name,
                value,
            } => {
                let object_reg = self.compile_expr(object)?;
                let value_reg = self.compile_expr(value)?;
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::SetProperty, value_reg, object_reg, name_idx);
                self.free_temp(value_reg);
                self.free_temp(object_reg);
                Ok(())
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                let collection_reg = self.compile_expr(collection)?;
                let index_reg = self.compile_expr(index)?;
                let value_reg = self.compile_expr(value)?;
                self.emit(Opcode::SetIndex, value_reg, collection_reg, index_reg);
                self.free_temp(value_reg);
                self.free_temp(index_reg);
                self.free_temp(collection_reg);
                Ok(())
            }
            StmtKind::Block(statements) => {
                self.locals.push(HashMap::new());
                for stmt in statements {
                    self.compile_stmt(stmt)?;
                }
                self.pop_scope();
                Ok(())
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_reg = self.compile_expr(condition)?;
                let jump_to_else = self.emit_jump(Opcode::JumpIfNot, cond_reg);
                self.free_temp(cond_reg);
                self.compile_stmt(then_branch)?;
                if let Some(else_branch) = else_branch {
                    let jump_to_end = self.emit_jump(Opcode::Jump, 0);
                    self.patch_jump(jump_to_else)?;
                    self.compile_stmt(else_branch)?;
                    self.patch_jump(jump_to_end)?;
                } else {
                    self.patch_jump(jump_to_else)?;
                }
                Ok(())
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                self.locals.push(HashMap::new());

                if let Some(init_stmt) = init {
                    self.compile_stmt(init_stmt)?;
                }

                let loop_start = self.chunk.code.len();
                let jump_to_end = if let Some(cond_expr) = condition {
                    let cond_reg = self.compile_expr(cond_expr)?;
                    let jump = self.emit_jump(Opcode::JumpIfNot, cond_reg);
                    self.free_temp(cond_reg);
                    Some(jump)
                } else {
                    None
                };

                self.push_loop(Some(loop_start));
                self.compile_stmt(body)?;

                let continue_target = self.chunk.code.len();
                if let Some(ctx) = self.loop_stack.last_mut() {
                    ctx.continue_target = Some(continue_target);
                }

                if let Some(step_stmt) = step {
                    self.compile_stmt(step_stmt)?;
                }

                self.emit_loop(loop_start)?;

                if let Some(jump) = jump_to_end {
                    self.patch_jump(jump)?;
                }

                let loop_ctx = self
                    .pop_loop()
                    .ok_or_else(|| self.error("loop stack underflow", stmt.span))?;
                let loop_end = self.chunk.code.len();
                self.patch_loop_jumps(loop_ctx, loop_end)?;

                self.pop_scope();
                Ok(())
            }
            StmtKind::While { condition, body } => {
                let loop_start = self.chunk.code.len();
                let cond_reg = self.compile_expr(condition)?;
                let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
                self.free_temp(cond_reg);
                self.push_loop(Some(loop_start));
                self.compile_stmt(body)?;
                let loop_ctx = self
                    .pop_loop()
                    .ok_or_else(|| self.error("loop stack underflow", stmt.span))?;
                self.emit_loop(loop_start)?;
                self.patch_jump(jump_to_end)?;
                let loop_end = self.chunk.code.len();
                self.patch_loop_jumps(loop_ctx, loop_end)?;
                Ok(())
            }
            StmtKind::For {
                label: _,
                names,
                iterable,
                body,
            } => self.compile_for_stmt(names, iterable, body),
            StmtKind::DoWhile { body, condition } => {
                let loop_start = self.chunk.code.len();
                self.push_loop(None);
                self.compile_stmt(body)?;
                let continue_target = self.chunk.code.len();
                if let Some(ctx) = self.loop_stack.last_mut() {
                    ctx.continue_target = Some(continue_target);
                }
                let cond_reg = self.compile_expr(condition)?;
                self.emit_loop_if(loop_start, cond_reg)?;
                self.free_temp(cond_reg);
                let loop_ctx = self
                    .pop_loop()
                    .ok_or_else(|| self.error("loop stack underflow", stmt.span))?;
                self.patch_loop_jumps(loop_ctx, self.chunk.code.len())?;
                Ok(())
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => self.compile_try_stmt(try_branch, catch_branches, finally_branch.as_deref()),
            StmtKind::Throw(expr) => {
                let reg = self.compile_expr(expr)?;
                self.emit(Opcode::Throw, reg, 0, 0);
                self.free_temp(reg);
                Ok(())
            }
            StmtKind::Import { path, kind } => {
                let module_reg = self.registers.allocate()?;
                let path_idx = self.add_constant(Value::String(path.clone().into()));
                self.emit(Opcode::Import, module_reg, path_idx, 0);
                match kind {
                    pleno_ast::ImportKind::Module { alias } => {
                        if let Some(name) = alias {
                            self.insert_local(name.clone(), module_reg, false, None)?;
                        } else {
                            self.free_temp(module_reg);
                        }
                    }
                    pleno_ast::ImportKind::Star { alias } => {
                        self.insert_local(alias.clone(), module_reg, false, None)?;
                    }
                    pleno_ast::ImportKind::Names(items) => {
                        for item in items {
                            let name_idx =
                                self.add_constant(Value::String(item.name.clone().into()));
                            let dest = self.registers.allocate()?;
                            self.emit(Opcode::GetProperty, dest, module_reg, name_idx);
                            let local_name = item.alias.clone().unwrap_or(item.name.clone());
                            self.insert_local(local_name, dest, false, None)?;
                        }
                        self.free_temp(module_reg);
                    }
                }
                Ok(())
            }
            StmtKind::Export(stmt) => {
                if !self.module_mode {
                    return Err(CompileError::new("export is only allowed in modules"));
                }
                let export_name = match &stmt.kind {
                    StmtKind::Let { name, .. } => name.clone(),
                    StmtKind::Fn { name, .. } => name.clone(),
                    StmtKind::Class { name, .. } => name.clone(),
                    _ => return Err(self.error("invalid export statement", stmt.span)),
                };
                self.compile_stmt(stmt)?;
                self.module_exports.push(ExportItem {
                    local: export_name.clone(),
                    export: export_name,
                });
                Ok(())
            }
            StmtKind::ExportList(items) => {
                if !self.module_mode {
                    return Err(CompileError::new("export is only allowed in modules"));
                }
                for item in items {
                    self.module_exports.push(ExportItem {
                        local: item.name.clone(),
                        export: item.alias.clone().unwrap_or(item.name.clone()),
                    });
                }
                Ok(())
            }
            StmtKind::Class { name, .. } => {
                let class_reg = self.compile_class_declaration(stmt)?;
                if self.is_top_level {
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::SetGlobal, class_reg, name_idx, 0);
                    self.free_temp(class_reg);
                } else {
                    if self
                        .locals
                        .last()
                        .map(|scope| scope.contains_key(name))
                        .unwrap_or(false)
                    {
                        return Err(self.error("class name already defined in scope", stmt.span));
                    }
                    self.insert_local(name.clone(), class_reg, false, None)?;
                }
                Ok(())
            }
            StmtKind::Fn {
                name,
                generics,
                params,
                body,
                annotations,
                is_async,
            } => {
                let decl = FunctionDecl {
                    name: name.clone(),
                    generics: generics.clone(),
                    params: params.clone(),
                    return_ty: None,
                    body: body.clone(),
                    is_static: false,
                    is_abstract: false,
                    is_async: *is_async,
                    visibility: None,
                    kind: FunctionKind::Method,
                    annotations: annotations.clone(),
                };
                let func_reg = self.compile_function(&decl, false, false, None)?;
                if self.is_top_level {
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::SetGlobal, func_reg, name_idx, 0);
                    self.free_temp(func_reg);
                } else {
                    if self
                        .locals
                        .last()
                        .map(|scope| scope.contains_key(name))
                        .unwrap_or(false)
                    {
                        return Err(self.error("function name already defined in scope", stmt.span));
                    }
                    self.insert_local(name.clone(), func_reg, false, None)?;
                }
                Ok(())
            }
            StmtKind::Return(expr) => {
                let reg = match expr {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, reg, 0, 0);
                        reg
                    }
                };
                self.emit(Opcode::Return, reg, 0, 0);
                Ok(())
            }
            StmtKind::Break(label) => self.compile_break_stmt(label.as_deref(), stmt.span),
            StmtKind::Continue(label) => self.compile_continue_stmt(label.as_deref(), stmt.span),
            StmtKind::Labeled { label, stmt } => {
                self.next_labels.push(label.clone());
                self.compile_stmt(stmt)
            }
            StmtKind::Interface {
                name,
                methods,
                annotations: _,
                generics: _,
                extends: _,
            } => {
                let methods_reg = self.registers.allocate()?;
                self.emit(Opcode::NewMap, methods_reg, 0, 0);

                for method in methods {
                    let meta_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, meta_reg, 0, 0);

                    // Format: [name, type, arity, is_static, is_async]
                    let name_val = Value::String(std::rc::Rc::new(method.name.clone()));
                    let name_reg = self.registers.allocate()?;
                    let name_idx = self.add_constant(name_val);
                    self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                    self.emit(Opcode::ArrayAppend, meta_reg, name_reg, 0);
                    self.free_temp(name_reg);

                    let type_str = match method.kind {
                        FunctionKind::Method => "method",
                        FunctionKind::Getter => "getter",
                        FunctionKind::Setter => "setter",
                    };
                    let type_reg = self.registers.allocate()?;
                    let type_idx =
                        self.add_constant(Value::String(std::rc::Rc::new(type_str.to_string())));
                    self.emit(Opcode::LoadConst, type_reg, type_idx, 0);
                    self.emit(Opcode::ArrayAppend, meta_reg, type_reg, 0);
                    self.free_temp(type_reg);

                    let arity_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadInt, arity_reg, method.params.len() as u8, 0);
                    self.emit(Opcode::ArrayAppend, meta_reg, arity_reg, 0);
                    self.free_temp(arity_reg);

                    let static_reg = self.registers.allocate()?;
                    if method.is_static {
                        self.emit(Opcode::LoadTrue, static_reg, 0, 0);
                    } else {
                        self.emit(Opcode::LoadFalse, static_reg, 0, 0);
                    }
                    self.emit(Opcode::ArrayAppend, meta_reg, static_reg, 0);
                    self.free_temp(static_reg);

                    let async_reg = self.registers.allocate()?;
                    if method.is_async {
                        self.emit(Opcode::LoadTrue, async_reg, 0, 0);
                    } else {
                        self.emit(Opcode::LoadFalse, async_reg, 0, 0);
                    }
                    self.emit(Opcode::ArrayAppend, meta_reg, async_reg, 0);
                    self.free_temp(async_reg);

                    let key_idx =
                        self.add_constant(Value::String(std::rc::Rc::new(method.name.clone())));
                    let key_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadConst, key_reg, key_idx, 0);
                    self.emit(Opcode::SetIndex, meta_reg, methods_reg, key_reg);
                    self.free_temp(key_reg);
                    self.free_temp(meta_reg);
                }

                // TODO: Compile interface properties if needed

                let interface_reg = self.registers.allocate()?;
                let name_idx = self.add_constant(Value::String(std::rc::Rc::new(name.clone())));
                self.emit(
                    Opcode::DefineInterface,
                    interface_reg,
                    name_idx,
                    methods_reg,
                );

                let name_idx_global =
                    self.add_constant(Value::String(std::rc::Rc::new(name.clone())));
                self.emit(Opcode::SetGlobal, interface_reg, name_idx_global, 0);

                self.free_temp(interface_reg);
                self.free_temp(methods_reg);
                Ok(())
            }
            StmtKind::Enum(decl) => {
                let enum_reg = self.compile_enum_declaration(decl)?;
                if self.is_top_level {
                    let name_idx = self.add_constant(Value::String(decl.name.clone().into()));
                    self.emit(Opcode::SetGlobal, enum_reg, name_idx, 0);
                    self.free_temp(enum_reg);
                } else {
                    self.insert_local(decl.name.clone(), enum_reg, false, None)?;
                }
                Ok(())
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        self.current_span = expr.span;
        match &expr.kind {
            ExprKind::Literal(lit) => {
                let reg = self.registers.allocate()?;
                match lit {
                    Literal::Null => self.emit(Opcode::LoadNull, reg, 0, 0),
                    Literal::Bool(true) => self.emit(Opcode::LoadTrue, reg, 0, 0),
                    Literal::Bool(false) => self.emit(Opcode::LoadFalse, reg, 0, 0),
                    Literal::Int(value) if (-128..=127).contains(value) => {
                        self.emit(Opcode::LoadInt, reg, *value as u8, 0)
                    }
                    Literal::Int(value) => {
                        let const_idx = self.add_constant(Value::Int(*value));
                        self.emit(Opcode::LoadConst, reg, const_idx, 0)
                    }
                    Literal::Float(value) => {
                        let const_idx = self.add_constant(Value::Float(*value));
                        self.emit(Opcode::LoadConst, reg, const_idx, 0)
                    }
                    Literal::String(value) => {
                        let const_idx = self.add_constant(Value::String(value.clone().into()));
                        self.emit(Opcode::LoadConst, reg, const_idx, 0)
                    }
                    Literal::Char(value) => {
                        // Represent Char as a single-character String
                        let char_str = value.to_string();
                        let const_idx = self.add_constant(Value::String(char_str.into()));
                        self.emit(Opcode::LoadConst, reg, const_idx, 0)
                    }
                }
                Ok(reg)
            }
            ExprKind::Binary { left, op, right } => {
                match op {
                    pleno_ast::BinaryOp::And => self.compile_logical_and(left, right),
                    pleno_ast::BinaryOp::Or => self.compile_logical_or(left, right),
                    _ => {
                        let left_reg = self.compile_expr(left)?;
                        let right_reg = self.compile_expr(right)?;
                        let dest = self.registers.allocate()?;
                        let opcode = match op {
                            pleno_ast::BinaryOp::Add => Opcode::Add,
                            pleno_ast::BinaryOp::Subtract => Opcode::Sub,
                            pleno_ast::BinaryOp::Multiply => Opcode::Mul,
                            pleno_ast::BinaryOp::Divide => Opcode::Div,
                            pleno_ast::BinaryOp::Equal => Opcode::Eq,
                            pleno_ast::BinaryOp::NotEqual => Opcode::Ne,
                            pleno_ast::BinaryOp::Less => Opcode::Lt,
                            pleno_ast::BinaryOp::LessEqual => Opcode::Le,
                            pleno_ast::BinaryOp::Greater => Opcode::Gt,
                            pleno_ast::BinaryOp::GreaterEqual => Opcode::Ge,
                            pleno_ast::BinaryOp::Modulo => Opcode::Mod,
                            pleno_ast::BinaryOp::NullCoalesce => {
                                // For ?? operator: if left is null, use right, else use left
                                // This requires a conditional jump, so we handle it specially
                                return self.compile_null_coalesce(left, right);
                            }
                            _ => return Err(self.error("unsupported binary op", expr.span)),
                        };
                        self.emit(opcode, dest, left_reg, right_reg);
                        self.free_temp(left_reg);
                        self.free_temp(right_reg);
                        Ok(dest)
                    }
                }
            }
            ExprKind::Unary { op, expr } => {
                let value_reg = self.compile_expr(expr)?;
                let dest = self.registers.allocate()?;
                let opcode = match op {
                    pleno_ast::UnaryOp::Negate => Opcode::Neg,
                    pleno_ast::UnaryOp::Not => Opcode::Not,
                };
                self.emit(opcode, dest, value_reg, 0);
                self.free_temp(value_reg);
                Ok(dest)
            }
            ExprKind::Ident(name) => {
                if let Some(local) = self.resolve_local(name) {
                    Ok(local.reg)
                } else if let Some(index) = self.captures.get(name) {
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::LoadCapture, dest, *index as u8, 0);
                    Ok(dest)
                } else {
                    let dest = self.registers.allocate()?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::GetGlobal, dest, name_idx, 0);
                    Ok(dest)
                }
            }
            ExprKind::This => self
                .resolve_local("this")
                .map(|local| local.reg)
                .ok_or_else(|| self.error("this used outside of method", expr.span)),
            ExprKind::Super => Err(self.error("super is not supported yet", expr.span)),
            ExprKind::Property { object, name } => {
                if matches!(&object.kind, ExprKind::Super) {
                    let this_reg = self
                        .resolve_local("this")
                        .map(|local| local.reg)
                        .ok_or_else(|| self.error("super used outside of method", expr.span))?;
                    if self.current_super.is_none() {
                        return Err(self.error("super used without superclass", expr.span));
                    }
                    let dest = self.registers.allocate()?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::GetSuper, dest, this_reg, name_idx);
                    Ok(dest)
                } else {
                    let object_reg = self.compile_expr(object)?;
                    let dest = self.registers.allocate()?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::GetProperty, dest, object_reg, name_idx);
                    self.free_temp(object_reg);
                    Ok(dest)
                }
            }
            ExprKind::SafeProperty { object, name } => {
                // Safe property access: obj?.prop returns null if obj is null
                let object_reg = self.compile_expr(object)?;
                let dest = self.registers.allocate()?;
                let null_check_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, null_check_reg, 0, 0);
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, is_null_reg, object_reg, null_check_reg);
                self.free_temp(null_check_reg);

                // If object is null, jump to end and return null
                let jump_to_end = self.emit_jump(Opcode::JumpIf, is_null_reg);
                self.free_temp(is_null_reg);

                // Otherwise, get the property
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::GetProperty, dest, object_reg, name_idx);
                self.free_temp(object_reg);

                // Jump past the null case
                let jump_past_null = self.emit_jump(Opcode::Jump, 0);

                // Null case: set dest to null
                self.patch_jump(jump_to_end)?;
                self.emit(Opcode::LoadNull, dest, 0, 0);

                // Patch the jump past null case
                self.patch_jump(jump_past_null)?;

                Ok(dest)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_reg = self.compile_expr(condition)?;
                let else_jump = self.emit_jump(Opcode::JumpIfNot, cond_reg);
                self.free_temp(cond_reg);

                let result_reg = self.registers.allocate()?;

                let then_reg = self.compile_expr(then_branch)?;
                self.emit(Opcode::Move, result_reg, then_reg, 0);
                self.free_temp(then_reg);

                let end_jump = self.emit_jump(Opcode::Jump, 0);

                self.patch_jump(else_jump)?;

                let else_reg = self.compile_expr(else_branch)?;
                self.emit(Opcode::Move, result_reg, else_reg, 0);
                self.free_temp(else_reg);

                self.patch_jump(end_jump)?;

                Ok(result_reg)
            }
            ExprKind::Is { left, ty } => {
                let val_reg = self.compile_expr(left)?;
                let (base_types, is_optional) = self.parse_type_string(ty);
                let dest = self.registers.allocate()?;

                // If optional and null, it's true
                let mut success_jumps = Vec::new();
                if is_optional {
                    let is_null_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, is_null_reg, 0, 0);
                    self.emit(Opcode::Eq, is_null_reg, val_reg, is_null_reg);
                    let jump_success = self.emit_jump(Opcode::JumpIf, is_null_reg);
                    success_jumps.push(jump_success);
                    self.free_temp(is_null_reg);
                }

                // groups logic
                for group in base_types {
                    let mut next_group_jumps = Vec::new();
                    for ty_name in group {
                        let type_reg = self.registers.allocate()?;
                        let name_idx = self.add_constant(Value::String(ty_name.clone().into()));
                        self.emit(Opcode::GetGlobal, type_reg, name_idx, 0);

                        let is_primitive_null_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, is_primitive_null_reg, 0, 0);
                        self.emit(
                            Opcode::Eq,
                            is_primitive_null_reg,
                            type_reg,
                            is_primitive_null_reg,
                        );
                        let skip_primitive_load =
                            self.emit_jump(Opcode::JumpIfNot, is_primitive_null_reg);
                        self.emit(Opcode::LoadConst, type_reg, name_idx, 0);
                        self.patch_jump(skip_primitive_load)?;
                        self.free_temp(is_primitive_null_reg);

                        let is_match_reg = self.registers.allocate()?;
                        self.emit(Opcode::Is, is_match_reg, val_reg, type_reg);
                        let jump_fail = self.emit_jump(Opcode::JumpIfNot, is_match_reg);
                        next_group_jumps.push(jump_fail);
                        self.free_temp(is_match_reg);
                        self.free_temp(type_reg);
                    }
                    // If we get here, the entire AND group matched
                    let jump_success = self.emit_jump(Opcode::Jump, 0);
                    success_jumps.push(jump_success);

                    // Patch fails for this group to start of next group
                    for jump in next_group_jumps {
                        self.patch_jump(jump)?;
                    }
                }

                // If we get here, none matched
                self.emit(Opcode::LoadFalse, dest, 0, 0);
                let jump_end = self.emit_jump(Opcode::Jump, 0);

                // Success case
                for jump in success_jumps {
                    self.patch_jump(jump)?;
                }
                self.emit(Opcode::LoadTrue, dest, 0, 0);

                self.patch_jump(jump_end)?;
                self.free_temp(val_reg);
                Ok(dest)
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                annotations,
            } => {
                let is_pure = annotations.iter().any(|a| a.name == "pure");
                Ok(self.compile_lambda(params, body, return_ty.as_deref(), is_pure)?)
            }
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let start_reg = match start {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, reg, 0, 0);
                        reg
                    }
                };
                let end_reg = match end {
                    Some(expr) => self.compile_expr(expr)?,
                    None => {
                        let reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, reg, 0, 0);
                        reg
                    }
                };
                let dest = self.registers.allocate()?;
                let opcode = if *inclusive {
                    Opcode::BuildRangeInclusive
                } else {
                    Opcode::BuildRange
                };
                self.emit(opcode, dest, start_reg, end_reg);
                self.free_temp(start_reg);
                self.free_temp(end_reg);
                Ok(dest)
            }
            ExprKind::MapLiteral(entries) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewMap, dest, 0, 0);
                for item in entries {
                    match item {
                        MapItem::Entry(key, value_expr) => {
                            let value_reg = self.compile_expr(value_expr)?;
                            let key_idx = self.add_constant(Value::String(key.clone().into()));
                            self.emit(Opcode::SetProperty, value_reg, dest, key_idx);
                            self.free_temp(value_reg);
                        }
                        MapItem::Spread(expr) => {
                            let val_reg = self.compile_expr(expr)?;
                            self.emit(Opcode::MapSpread, dest, val_reg, 0);
                            self.free_temp(val_reg);
                        }
                    }
                }
                Ok(dest)
            }
            ExprKind::NamedTupleLiteral(entries) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewMap, dest, 0, 0);
                for (name, value_expr) in entries {
                    let value_reg = self.compile_expr(value_expr)?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::SetProperty, value_reg, dest, name_idx);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            ExprKind::ArrayLiteral(items) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewArray, dest, 0, 0);
                for item in items {
                    match &item.kind {
                        ExprKind::Spread(inner) => {
                            let val_reg = self.compile_expr(inner)?;
                            self.emit(Opcode::ArraySpread, dest, val_reg, 0);
                            self.free_temp(val_reg);
                        }
                        _ => {
                            let value_reg = self.compile_expr(item)?;
                            self.emit(Opcode::ArrayAppend, dest, value_reg, 0);
                            self.free_temp(value_reg);
                        }
                    }
                }
                Ok(dest)
            }
            ExprKind::TupleLiteral(items) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewArray, dest, 0, 0);
                for item in items {
                    let value_reg = self.compile_expr(item)?;
                    self.emit(Opcode::ArrayAppend, dest, value_reg, 0);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            ExprKind::Block(stmts) => {
                self.locals.push(HashMap::new());
                for stmt in stmts {
                    self.compile_stmt(stmt)?;
                }
                let reg = match self.last_expr_register {
                    Some(reg) => reg,
                    None => {
                        let reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, reg, 0, 0);
                        reg
                    }
                };
                self.pop_scope();
                Ok(reg)
            }
            ExprKind::SetLiteral(items) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewSet, dest, 0, 0);
                for item in items {
                    match &item.kind {
                        ExprKind::Spread(inner) => {
                            let val_reg = self.compile_expr(inner)?;
                            self.emit(Opcode::SetSpread, dest, val_reg, 0);
                            self.free_temp(val_reg);
                        }
                        _ => {
                            let value_reg = self.compile_expr(item)?;
                            self.emit(Opcode::SetAdd, dest, value_reg, 0);
                            self.free_temp(value_reg);
                        }
                    }
                }
                Ok(dest)
            }
            ExprKind::Pipeline { left, right } => {
                let left_reg = self.compile_expr(left)?;
                let right_reg = self.compile_expr(right)?;
                let args_reg = self.registers.allocate()?;
                self.emit(Opcode::NewArray, args_reg, 0, 0);
                self.emit(Opcode::ArrayAppend, args_reg, left_reg, 0);
                let dest = self.registers.allocate()?;
                self.emit(Opcode::Call, dest, right_reg, args_reg);
                self.free_temp(left_reg);
                self.free_temp(right_reg);
                self.free_temp(args_reg);
                Ok(dest)
            }
            ExprKind::Underscore => Err(self.error(
                "underscore is only allowed as a placeholder in calls",
                expr.span,
            )),
            ExprKind::Index { collection, index } => {
                let collection_reg = self.compile_expr(collection)?;
                let index_reg = self.compile_expr(index)?;
                let dest = self.registers.allocate()?;
                self.emit(Opcode::GetIndex, dest, collection_reg, index_reg);
                self.free_temp(collection_reg);
                self.free_temp(index_reg);
                Ok(dest)
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                // Compile as: collection.slice(start, end)
                // This will use the builtin method
                let collection_reg = self.compile_expr(collection)?;
                let args_reg = self.registers.allocate()?;
                self.emit(Opcode::NewArray, args_reg, 0, 0);

                // Add start argument (or null if missing)
                if let Some(start_expr) = start {
                    let start_reg = self.compile_expr(start_expr)?;
                    self.emit(Opcode::ArrayAppend, args_reg, start_reg, 0);
                    self.free_temp(start_reg);
                } else {
                    let null_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, null_reg, 0, 0);
                    self.emit(Opcode::ArrayAppend, args_reg, null_reg, 0);
                    self.free_temp(null_reg);
                }

                // Add end argument (or null if missing)
                if let Some(end_expr) = end {
                    let end_reg = self.compile_expr(end_expr)?;
                    self.emit(Opcode::ArrayAppend, args_reg, end_reg, 0);
                    self.free_temp(end_reg);
                } else {
                    let null_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, null_reg, 0, 0);
                    self.emit(Opcode::ArrayAppend, args_reg, null_reg, 0);
                    self.free_temp(null_reg);
                }

                let dest = self.registers.allocate()?;
                self.emit(Opcode::Slice, dest, collection_reg, args_reg);
                self.free_temp(collection_reg);
                self.free_temp(args_reg);
                Ok(dest)
            }
            ExprKind::Paren(inner) => self.compile_expr(inner),
            ExprKind::Spread(_) => Err(CompileError::new(
                "spread operator only allowed in collection literals",
            )),
            ExprKind::Call { callee, args } => {
                // Handle super() constructor call
                if matches!(&callee.kind, ExprKind::Super) {
                    if self.current_super.is_none() {
                        return Err(self.error("super() called without superclass", expr.span));
                    }
                    let this_reg = self
                        .resolve_local("this")
                        .map(|local| local.reg)
                        .ok_or_else(|| {
                            self.error("super() called outside of constructor", expr.span)
                        })?;

                    // Compile arguments
                    let args_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, args_reg, 0, 0);
                    for arg in args {
                        match arg {
                            pleno_ast::CallArg::Positional(arg_expr) => {
                                let arg_reg = self.compile_expr(arg_expr)?;
                                self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Spread(arg_expr) => {
                                let spread_reg = self.compile_expr(arg_expr)?;
                                self.emit(Opcode::ArrayExtend, args_reg, spread_reg, 0);
                                self.free_temp(spread_reg);
                            }
                            _ => {
                                return Err(CompileError::new(
                                    "super() does not support named/placeholder args",
                                ));
                            }
                        }
                    }

                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::CallSuper, dest, this_reg, args_reg);
                    self.free_temp(args_reg);
                    return Ok(dest);
                }

                let callee_reg = self.compile_expr(callee)?;
                let has_placeholder = args
                    .iter()
                    .any(|arg| matches!(arg, pleno_ast::CallArg::Placeholder));
                let has_named = args
                    .iter()
                    .any(|arg| matches!(arg, pleno_ast::CallArg::Named(_, _)));

                if has_placeholder {
                    let args_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, args_reg, 0, 0);
                    for arg in args {
                        match arg {
                            pleno_ast::CallArg::Positional(expr) => {
                                let arg_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Placeholder => {
                                let arg_reg = self.registers.allocate()?;
                                self.emit(Opcode::LoadMissing, arg_reg, 0, 0);
                                self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Spread(expr) => {
                                let spread_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayExtend, args_reg, spread_reg, 0);
                                self.free_temp(spread_reg);
                            }
                            pleno_ast::CallArg::Named(_, _) => {
                                return Err(CompileError::new(
                                    "named arguments not supported in partials yet",
                                ));
                            }
                        }
                    }
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::Partial, dest, callee_reg, args_reg);
                    self.free_temp(callee_reg);
                    self.free_temp(args_reg);
                    return Ok(dest);
                }

                if has_named {
                    let positional_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, positional_reg, 0, 0);
                    let named_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewMap, named_reg, 0, 0);
                    let mut seen_named = std::collections::HashSet::new();
                    for arg in args {
                        match arg {
                            pleno_ast::CallArg::Positional(expr) => {
                                let arg_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayAppend, positional_reg, arg_reg, 0);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Named(name, expr) => {
                                if !seen_named.insert(name.clone()) {
                                    return Err(CompileError::new("duplicate named argument"));
                                }
                                let arg_reg = self.compile_expr(expr)?;
                                let name_idx =
                                    self.add_constant(Value::String(name.clone().into()));
                                self.emit(Opcode::SetProperty, arg_reg, named_reg, name_idx);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Spread(expr) => {
                                let spread_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayExtend, positional_reg, spread_reg, 0);
                                self.free_temp(spread_reg);
                            }
                            pleno_ast::CallArg::Placeholder => unreachable!(),
                        }
                    }
                    let wrapper_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, wrapper_reg, 0, 0);
                    self.emit(Opcode::ArrayAppend, wrapper_reg, positional_reg, 0);
                    self.emit(Opcode::ArrayAppend, wrapper_reg, named_reg, 0);
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::CallNamed, dest, callee_reg, wrapper_reg);
                    self.free_temp(positional_reg);
                    self.free_temp(named_reg);
                    self.free_temp(wrapper_reg);
                    self.free_temp(callee_reg);
                    Ok(dest)
                } else {
                    let args_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, args_reg, 0, 0);
                    for arg in args {
                        match arg {
                            pleno_ast::CallArg::Positional(expr) => {
                                let arg_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
                                self.free_temp(arg_reg);
                            }
                            pleno_ast::CallArg::Spread(expr) => {
                                let spread_reg = self.compile_expr(expr)?;
                                self.emit(Opcode::ArrayExtend, args_reg, spread_reg, 0);
                                self.free_temp(spread_reg);
                            }
                            pleno_ast::CallArg::Named(_, _) => {
                                return Err(CompileError::new("named arguments require name"));
                            }
                            pleno_ast::CallArg::Placeholder => unreachable!(),
                        }
                    }
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::Call, dest, callee_reg, args_reg);
                    self.free_temp(callee_reg);
                    self.free_temp(args_reg);
                    Ok(dest)
                }
            }
            ExprKind::Match { expr, arms } => {
                let value_reg = self.compile_expr(expr)?;
                let dest = self.registers.allocate()?;
                let mut end_jumps = Vec::new();

                for arm in arms {
                    self.locals.push(HashMap::new());
                    let mut fail_jumps = Vec::new();

                    self.compile_pattern(&arm.pattern, value_reg, &mut fail_jumps)?;

                    // Check guard if present
                    if let Some(guard) = &arm.guard {
                        let guard_reg = self.compile_expr(guard)?;
                        fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, guard_reg));
                        self.free_temp(guard_reg);
                    }

                    let body_reg = self.compile_expr(&arm.body)?;
                    self.emit(Opcode::Move, dest, body_reg, 0);
                    self.free_temp(body_reg);

                    let jump = self.emit_jump(Opcode::Jump, 0);
                    end_jumps.push(jump);

                    for f_jump in fail_jumps {
                        self.patch_jump(f_jump)?;
                    }

                    self.locals.pop();

                    // If it was a wildcard or identity match without a guard, we don't need to check further arms
                    if arm.guard.is_none()
                        && matches!(arm.pattern, Pattern::Underscore | Pattern::Ident(_))
                    {
                        break;
                    }
                }

                for jump in end_jumps {
                    self.patch_jump(jump)?;
                }

                self.free_temp(value_reg);
                Ok(dest)
            }
            ExprKind::Await { expr } => {
                let expr_reg = self.compile_expr(expr)?;
                let dest = self.registers.allocate()?;
                self.emit(Opcode::Await, dest, expr_reg, 0);
                self.free_temp(expr_reg);
                Ok(dest)
            }
        }
    }

    fn add_constant(&mut self, value: Value) -> u8 {
        self.chunk.constants.push(value);
        (self.chunk.constants.len() - 1) as u8
    }

    fn emit(&mut self, opcode: Opcode, a: u8, b: u8, c: u8) {
        self.chunk.code.push(Instruction::new(opcode, a, b, c));
        self.chunk.spans.push(self.current_span);
    }

    fn emit_jump(&mut self, opcode: Opcode, reg: u8) -> usize {
        let index = self.chunk.code.len();
        self.emit(opcode, reg, 0, 0);
        index
    }

    fn emit_loop(&mut self, loop_start: usize) -> Result<(), CompileError> {
        let offset = (loop_start as isize) - (self.chunk.code.len() as isize + 1);
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("loop too large"))?;
        let (b, c) = encode_offset(offset);
        self.emit(Opcode::Jump, 0, b, c);
        Ok(())
    }

    fn emit_loop_if(&mut self, loop_start: usize, reg: u8) -> Result<(), CompileError> {
        let offset = (loop_start as isize) - (self.chunk.code.len() as isize + 1);
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("loop too large"))?;
        let (b, c) = encode_offset(offset);
        self.emit(Opcode::JumpIf, reg, b, c);
        Ok(())
    }

    fn patch_jump(&mut self, jump_index: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = self.chunk.code.len() as isize;
        let offset = jump_to - jump_from;
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("jump too large"))?;
        let (b, c) = encode_offset(offset);
        let instruction = self.chunk.code[jump_index];
        self.chunk.code[jump_index] = Instruction::new(instruction.opcode(), instruction.a(), b, c);
        Ok(())
    }

    fn patch_jump_to(&mut self, jump_index: usize, jump_to: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = jump_to as isize;
        let offset = jump_to - jump_from;
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("jump too large"))?;
        let (b, c) = encode_offset(offset);
        let instruction = self.chunk.code[jump_index];
        self.chunk.code[jump_index] = Instruction::new(instruction.opcode(), instruction.a(), b, c);
        Ok(())
    }

    fn compile_logical_and(&mut self, left: &Expr, right: &Expr) -> Result<u8, CompileError> {
        let dest = self.registers.allocate()?;
        let left_reg = self.compile_expr(left)?;
        let jump_left_false = self.emit_jump(Opcode::JumpIfNot, left_reg);
        self.free_temp(left_reg);

        let right_reg = self.compile_expr(right)?;
        let jump_right_false = self.emit_jump(Opcode::JumpIfNot, right_reg);
        self.emit(Opcode::LoadTrue, dest, 0, 0);
        let jump_after_right = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(jump_right_false)?;
        self.emit(Opcode::LoadFalse, dest, 0, 0);
        self.patch_jump(jump_after_right)?;
        self.free_temp(right_reg);

        let jump_to_end = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(jump_left_false)?;
        self.emit(Opcode::LoadFalse, dest, 0, 0);
        self.patch_jump(jump_to_end)?;
        Ok(dest)
    }

    fn compile_logical_or(&mut self, left: &Expr, right: &Expr) -> Result<u8, CompileError> {
        let dest = self.registers.allocate()?;
        let left_reg = self.compile_expr(left)?;
        let jump_left_true = self.emit_jump(Opcode::JumpIf, left_reg);
        self.free_temp(left_reg);

        let right_reg = self.compile_expr(right)?;
        let jump_right_false = self.emit_jump(Opcode::JumpIfNot, right_reg);
        self.emit(Opcode::LoadTrue, dest, 0, 0);
        let jump_after_right = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(jump_right_false)?;
        self.emit(Opcode::LoadFalse, dest, 0, 0);
        self.patch_jump(jump_after_right)?;
        self.free_temp(right_reg);

        let jump_to_end = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(jump_left_true)?;
        self.emit(Opcode::LoadTrue, dest, 0, 0);
        self.patch_jump(jump_to_end)?;
        Ok(dest)
    }

    fn compile_null_coalesce(&mut self, left: &Expr, right: &Expr) -> Result<u8, CompileError> {
        // For ?? operator: if left is null, use right, else use left
        let dest = self.registers.allocate()?;
        let left_reg = self.compile_expr(left)?;

        // Check if left is null
        let null_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadNull, null_reg, 0, 0);
        let is_null_reg = self.registers.allocate()?;
        self.emit(Opcode::Eq, is_null_reg, left_reg, null_reg);
        self.free_temp(null_reg);

        // If left is null, jump to use right
        let jump_to_right = self.emit_jump(Opcode::JumpIf, is_null_reg);
        self.free_temp(is_null_reg);

        // Left is not null, use left
        self.emit(Opcode::Move, dest, left_reg, 0);
        self.free_temp(left_reg);

        // Jump past the right case
        let jump_past_right = self.emit_jump(Opcode::Jump, 0);

        // Right case: left was null, use right
        self.patch_jump(jump_to_right)?;
        let right_reg = self.compile_expr(right)?;
        self.emit(Opcode::Move, dest, right_reg, 0);
        self.free_temp(right_reg);

        // Patch the jump past right case
        self.patch_jump(jump_past_right)?;

        Ok(dest)
    }

    fn compile_try_stmt(
        &mut self,
        try_branch: &Stmt,
        catch_branches: &[pleno_ast::CatchBlock],
        finally_branch: Option<&Stmt>,
    ) -> Result<(), CompileError> {
        let error_reg = self.registers.allocate()?;
        // Reserve the register so it's not reused as a temp
        let error_name = format!("$try_error_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(error_name.clone(), error_reg, false, None)?;

        let push_handler_idx = self.chunk.code.len();
        self.emit(Opcode::PushHandler, error_reg, 0, 0);

        self.compile_stmt(try_branch)?;
        self.emit(Opcode::PopHandler, 0, 0, 0);

        let jump_to_finally = self.emit_jump(Opcode::Jump, 0);

        // Patch PushHandler to point here
        self.patch_jump(push_handler_idx)?;

        // Error occurred, error_reg contains the value
        let mut end_jumps = Vec::new();

        for catch in catch_branches {
            let mut next_catch_jump = None;
            if let Some(ty_name) = &catch.ty {
                let type_reg = self.registers.allocate()?;
                // Try to resolve as global or treat as primitive name
                let name_idx = self.add_constant(Value::String(ty_name.clone().into()));
                self.emit(Opcode::GetGlobal, type_reg, name_idx, 0);

                // If GetGlobal returned Null, it might be a primitive name
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, is_null_reg, 0, 0);
                self.emit(Opcode::Eq, is_null_reg, type_reg, is_null_reg);
                let skip_primitive_load = self.emit_jump(Opcode::JumpIfNot, is_null_reg);

                // Load primitive name as string
                self.emit(Opcode::LoadConst, type_reg, name_idx, 0);

                self.patch_jump(skip_primitive_load)?;
                self.free_temp(is_null_reg);

                let is_match_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_match_reg, error_reg, type_reg);
                next_catch_jump = Some(self.emit_jump(Opcode::JumpIfNot, is_match_reg));

                self.free_temp(type_reg);
                self.free_temp(is_match_reg);
            }

            self.locals.push(HashMap::new());
            self.insert_local(catch.name.clone(), error_reg, false, catch.ty.clone())?;
            self.compile_stmt(&catch.body)?;
            self.pop_scope();

            let jump = self.emit_jump(Opcode::Jump, 0);
            end_jumps.push(jump);

            if let Some(jump) = next_catch_jump {
                self.patch_jump(jump)?;
            }
        }

        // If no catch blocks matched, or if we fell through all of them
        if !catch_branches.is_empty() {
            // Re-throw if none of the catch blocks matched
            self.emit(Opcode::Throw, error_reg, 0, 0);
        } else if finally_branch.is_none() {
            // This case should be caught by parser (at least one catch or finally)
            // but just in case, re-throw.
            self.emit(Opcode::Throw, error_reg, 0, 0);
        }

        for jump in end_jumps {
            self.patch_jump(jump)?;
        }

        self.patch_jump(jump_to_finally)?;

        if let Some(finally) = finally_branch {
            self.compile_stmt(finally)?;
        }

        Ok(())
    }

    fn compile_for_stmt(
        &mut self,
        names: &[String],
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        if let ExprKind::Range {
            start,
            end,
            inclusive,
        } = &iterable.kind
        {
            if names.len() > 1 {
                return Err(self.error(
                    "range-based for loops only support one variable",
                    iterable.span,
                ));
            }
            let name = &names[0];
            match (start, end) {
                (Some(start), Some(end)) => {
                    return self.compile_for_range(name, start, end, *inclusive, body);
                }
                (None, Some(end)) => {
                    let zero = Expr {
                        kind: ExprKind::Literal(Literal::Int(0)),
                        span: iterable.span,
                    };
                    return self.compile_for_range(name, &zero, end, *inclusive, body);
                }
                (Some(start), None) => {
                    return self.compile_for_open_range(name, start, body);
                }
                _ => {
                    return Err(self.error(
                        "for loop requires a start or end value for range",
                        iterable.span,
                    ));
                }
            }
        }
        if matches!(iterable.kind, ExprKind::Literal(_)) {
            return Err(self.error("for loop expects array, set, or range", iterable.span));
        }
        if names.len() > 2 {
            return Err(self.error(
                "for loop supports at most two variables (key, value)",
                iterable.span,
            ));
        }
        self.compile_for_iterable(names, iterable, body)
    }

    fn compile_break_stmt(&mut self, label: Option<&str>, span: Span) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(self.error("break used outside of loop", span));
        }
        let jump = self.emit_jump(Opcode::Jump, 0);
        if let Some(target_label) = label {
            for ctx in self.loop_stack.iter_mut().rev() {
                if ctx.labels.iter().any(|l| l == target_label) {
                    ctx.break_jumps.push(jump);
                    return Ok(());
                }
            }
            Err(self.error(
                &format!("no loop found with label '{}'", target_label),
                span,
            ))
        } else {
            if let Some(loop_ctx) = self.loop_stack.last_mut() {
                loop_ctx.break_jumps.push(jump);
            }
            Ok(())
        }
    }

    fn compile_continue_stmt(
        &mut self,
        label: Option<&str>,
        span: Span,
    ) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(self.error("continue used outside of loop", span));
        }
        let jump = self.emit_jump(Opcode::Jump, 0);
        if let Some(target_label) = label {
            for ctx in self.loop_stack.iter_mut().rev() {
                if ctx.labels.iter().any(|l| l == target_label) {
                    ctx.continue_jumps.push(jump);
                    return Ok(());
                }
            }
            Err(self.error(
                &format!("no loop found with label '{}'", target_label),
                span,
            ))
        } else {
            if let Some(loop_ctx) = self.loop_stack.last_mut() {
                loop_ctx.continue_jumps.push(jump);
            }
            Ok(())
        }
    }

    fn compile_for_range(
        &mut self,
        name: &str,
        start: &Expr,
        end: &Expr,
        inclusive: bool,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let start_reg = self.compile_expr(start)?;
        self.insert_local(name.to_string(), start_reg, true, None)?;

        let end_reg = self.compile_expr(end)?;
        let end_name = format!("$for_end_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(end_name, end_reg, false, None)?;

        let loop_start = self.chunk.code.len();
        let cond_reg = self.registers.allocate()?;
        let cmp_opcode = if inclusive { Opcode::Le } else { Opcode::Lt };
        self.emit(cmp_opcode, cond_reg, start_reg, end_reg);
        let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
        self.free_temp(cond_reg);

        self.push_loop(None);
        self.compile_stmt(body)?;
        let continue_target = self.chunk.code.len();
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.continue_target = Some(continue_target);
        }

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, start_reg, one_reg);
        self.emit(Opcode::Move, start_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        let loop_end = self.chunk.code.len();
        let loop_ctx = self
            .pop_loop()
            .ok_or_else(|| CompileError::new("loop stack underflow"))?;
        self.patch_loop_jumps(loop_ctx, loop_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_open_range(
        &mut self,
        name: &str,
        start: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let start_reg = self.compile_expr(start)?;
        self.insert_local(name.to_string(), start_reg, true, None)?;

        let loop_start = self.chunk.code.len();
        self.push_loop(None);
        self.compile_stmt(body)?;
        let continue_target = self.chunk.code.len();
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.continue_target = Some(continue_target);
        }

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, start_reg, one_reg);
        self.emit(Opcode::Move, start_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        let loop_end = self.chunk.code.len();
        let loop_ctx = self
            .pop_loop()
            .ok_or_else(|| CompileError::new("loop stack underflow"))?;
        self.patch_loop_jumps(loop_ctx, loop_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_iterable(
        &mut self,
        names: &[String],
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let source_reg = self.compile_expr(iterable)?;
        let collection_reg = self.registers.allocate()?;
        let opcode = if names.len() > 1 {
            Opcode::IterEntries
        } else {
            Opcode::IterValues
        };
        self.emit(opcode, collection_reg, source_reg, 0);
        self.free_temp(source_reg);
        let collection_name = format!("$for_collection_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(collection_name, collection_reg, false, None)?;

        let index_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, index_reg, 0, 0);
        let index_name = format!("$for_index_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(index_name, index_reg, true, None)?;

        let loop_start = self.chunk.code.len();
        let len_reg = self.registers.allocate()?;
        self.emit(Opcode::ArrayLen, len_reg, collection_reg, 0);
        let cond_reg = self.registers.allocate()?;
        self.emit(Opcode::Lt, cond_reg, index_reg, len_reg);
        let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
        self.free_temp(cond_reg);
        self.free_temp(len_reg);

        let item_reg = self.registers.allocate()?;
        self.emit(Opcode::GetIndex, item_reg, collection_reg, index_reg);

        if names.len() == 1 {
            self.insert_local(names[0].clone(), item_reg, true, None)?;
        } else {
            let key_reg = self.registers.allocate()?;
            let zero_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadInt, zero_reg, 0, 0);
            self.emit(Opcode::GetIndex, key_reg, item_reg, zero_reg);
            self.insert_local(names[0].clone(), key_reg, true, None)?;

            let val_reg = self.registers.allocate()?;
            let one_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadInt, one_reg, 1, 0);
            self.emit(Opcode::GetIndex, val_reg, item_reg, one_reg);
            self.insert_local(names[1].clone(), val_reg, true, None)?;

            self.free_temp(zero_reg);
            self.free_temp(one_reg);
            self.free_temp(item_reg);
        }

        self.push_loop(None);
        self.compile_stmt(body)?;
        let continue_target = self.chunk.code.len();
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.continue_target = Some(continue_target);
        }

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, index_reg, one_reg);
        self.emit(Opcode::Move, index_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        let loop_end = self.chunk.code.len();
        let loop_ctx = self
            .pop_loop()
            .ok_or_else(|| CompileError::new("loop stack underflow"))?;
        self.patch_loop_jumps(loop_ctx, loop_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_pattern(
        &mut self,
        pattern: &Pattern,
        value_reg: u8,
        fail_jumps: &mut Vec<usize>,
    ) -> Result<(), CompileError> {
        match pattern {
            Pattern::Literal(lit) => {
                let lit_reg = self.compile_expr(&Expr {
                    kind: ExprKind::Literal(lit.clone()),
                    span: Span::new(0, 0),
                })?;
                let eq_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, eq_reg, value_reg, lit_reg);
                self.free_temp(lit_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, eq_reg));
                self.free_temp(eq_reg);
            }
            Pattern::Ident(name) => {
                let reg = self.registers.allocate()?;
                self.emit(Opcode::Move, reg, value_reg, 0);
                self.insert_local(name.clone(), reg, false, None)?;
            }
            Pattern::Underscore => {}
            Pattern::Tuple(patterns) => {
                // Check if Array (Pleno uses Array for tuples)
                let array_str_idx = self.add_constant(Value::String(Rc::new("Array".to_string())));
                let array_str_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, array_str_reg, array_str_idx, 0);
                let is_array_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_array_reg, value_reg, array_str_reg);
                self.free_temp(array_str_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_array_reg));
                self.free_temp(is_array_reg);

                // Check length
                let len_reg = self.registers.allocate()?;
                self.emit(Opcode::ArrayLen, len_reg, value_reg, 0);

                let has_rest = patterns.iter().any(|p| matches!(p, Pattern::Rest(_)));
                if has_rest {
                    let min_len = patterns.len().saturating_sub(1);
                    let min_len_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadInt, min_len_reg, min_len as u8, 0);
                    let ge_reg = self.registers.allocate()?;
                    self.emit(Opcode::Ge, ge_reg, len_reg, min_len_reg);
                    self.free_temp(min_len_reg);
                    fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, ge_reg));
                    self.free_temp(ge_reg);
                } else {
                    let expected_len = patterns.len();
                    let expected_len_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadInt, expected_len_reg, expected_len as u8, 0);
                    let eq_reg = self.registers.allocate()?;
                    self.emit(Opcode::Eq, eq_reg, len_reg, expected_len_reg);
                    self.free_temp(expected_len_reg);
                    fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, eq_reg));
                    self.free_temp(eq_reg);
                }
                self.free_temp(len_reg);

                // Match elements
                for (i, p) in patterns.iter().enumerate() {
                    if let Pattern::Rest(name) = p {
                        let args_reg = self.registers.allocate()?;
                        self.emit(Opcode::NewArray, args_reg, 0, 0);

                        let start_val_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, start_val_reg, i as u8, 0);
                        self.emit(Opcode::ArrayAppend, args_reg, start_val_reg, 0);
                        self.free_temp(start_val_reg);

                        let end_val_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, end_val_reg, 0, 0);
                        self.emit(Opcode::ArrayAppend, args_reg, end_val_reg, 0);
                        self.free_temp(end_val_reg);

                        let rest_reg = self.registers.allocate()?;
                        self.emit(Opcode::Slice, rest_reg, value_reg, args_reg);
                        self.free_temp(args_reg);
                        if let Some(name_str) = name {
                            self.insert_local(name_str.clone(), rest_reg, false, None)?;
                        } else {
                            self.free_temp(rest_reg);
                        }
                        break;
                    } else {
                        let idx_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, idx_reg, i as u8, 0);
                        let sub_val_reg = self.registers.allocate()?;
                        self.emit(Opcode::GetIndex, sub_val_reg, value_reg, idx_reg);
                        self.free_temp(idx_reg);
                        self.compile_pattern(p, sub_val_reg, fail_jumps)?;
                        self.free_temp(sub_val_reg);
                    }
                }
            }
            Pattern::Variant(name, patterns) => {
                // Check if it's a map first
                let map_str_idx = self.add_constant(Value::String(Rc::new("Map".to_string())));
                let map_str_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, map_str_reg, map_str_idx, 0);
                let is_map_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_map_reg, value_reg, map_str_reg);
                self.free_temp(map_str_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_map_reg));
                self.free_temp(is_map_reg);

                // Check if it's a map with property "variant" == name
                let variant_name_idx =
                    self.add_constant(Value::String(Rc::new("variant".to_string())));
                let variant_reg = self.registers.allocate()?;
                self.emit(
                    Opcode::GetProperty,
                    variant_reg,
                    value_reg,
                    variant_name_idx,
                );

                let expected_name_idx = self.add_constant(Value::String(Rc::new(name.clone())));
                let expected_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, expected_reg, expected_name_idx, 0);

                let eq_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, eq_reg, variant_reg, expected_reg);
                self.free_temp(variant_reg);
                self.free_temp(expected_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, eq_reg));
                self.free_temp(eq_reg);

                if patterns.len() == 1 {
                    let val_name_idx =
                        self.add_constant(Value::String(Rc::new("value".to_string())));
                    let val_reg = self.registers.allocate()?;
                    self.emit(Opcode::GetProperty, val_reg, value_reg, val_name_idx);
                    self.compile_pattern(&patterns[0], val_reg, fail_jumps)?;
                    self.free_temp(val_reg);
                } else if !patterns.is_empty() {
                    for (i, p) in patterns.iter().enumerate() {
                        let name = format!("value{}", i);
                        let name_idx = self.add_constant(Value::String(Rc::new(name)));
                        let val_reg = self.registers.allocate()?;
                        self.emit(Opcode::GetProperty, val_reg, value_reg, name_idx);
                        self.compile_pattern(p, val_reg, fail_jumps)?;
                        self.free_temp(val_reg);
                    }
                }
            }
            Pattern::Object(props, rest) => {
                // Check if it's a map first
                let map_str_idx = self.add_constant(Value::String(Rc::new("Map".to_string())));
                let map_str_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, map_str_reg, map_str_idx, 0);
                let is_map_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_map_reg, value_reg, map_str_reg);
                self.free_temp(map_str_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_map_reg));
                self.free_temp(is_map_reg);

                for (name, p) in props {
                    let name_idx = self.add_constant(Value::String(Rc::new(name.clone())));
                    let prop_reg = self.registers.allocate()?;
                    self.emit(Opcode::GetProperty, prop_reg, value_reg, name_idx);
                    self.compile_pattern(p, prop_reg, fail_jumps)?;
                    self.free_temp(prop_reg);
                }

                if let Some(rest_p) = rest {
                    let rest_map_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewMap, rest_map_reg, 0, 0);
                    self.emit(Opcode::MapSpread, rest_map_reg, value_reg, 0);

                    // Remove already matched keys
                    for (name, _) in props {
                        let name_idx = self.add_constant(Value::String(Rc::new(name.clone())));
                        let name_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                        self.emit(Opcode::MapRemove, rest_map_reg, name_reg, 0);
                        self.free_temp(name_reg);
                    }

                    self.compile_pattern(rest_p, rest_map_reg, fail_jumps)?;
                    self.free_temp(rest_map_reg);
                }
            }
            Pattern::Type(name, binding) => {
                let type_name_idx = self.add_constant(Value::String(Rc::new(name.clone())));
                let type_name_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, type_name_reg, type_name_idx, 0);

                let is_type_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_type_reg, value_reg, type_name_reg);
                self.free_temp(type_name_reg);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_type_reg));
                self.free_temp(is_type_reg);

                if let Some(b) = binding {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::Move, reg, value_reg, 0);
                    self.insert_local(b.clone(), reg, false, Some(name.clone()))?;
                }
            }
            Pattern::Or(patterns) => {
                let mut success_jumps = Vec::new();
                for p in patterns {
                    let mut p_fail_jumps = Vec::new();
                    self.compile_pattern(p, value_reg, &mut p_fail_jumps)?;

                    // If this pattern matched, it's a success for the whole Or pattern
                    success_jumps.push(self.emit_jump(Opcode::Jump, 0));

                    for f in p_fail_jumps {
                        self.patch_jump(f)?;
                    }
                }

                if !patterns.is_empty() {
                    // Fail if none matched
                    fail_jumps.push(self.emit_jump(Opcode::Jump, 0));
                }

                for s in success_jumps {
                    self.patch_jump(s)?;
                }
            }
            Pattern::Rest(name) => {
                if let Some(name_str) = name {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::Move, reg, value_reg, 0);
                    self.insert_local(name_str.clone(), reg, false, None)?;
                }
            }
        }
        Ok(())
    }

    fn compile_function(
        &mut self,
        decl: &FunctionDecl,
        is_method: bool,
        store_as_function: bool,
        current_super: Option<String>,
    ) -> Result<u8, CompileError> {
        let captures = self.analyze_captures(decl, is_method);
        if store_as_function && !captures.is_empty() {
            return Err(CompileError::new(
                "class methods cannot capture outer variables yet",
            ));
        }
        let mut compiler = Compiler::new(String::new());
        compiler.is_top_level = false;
        compiler.locals = vec![HashMap::new()];
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect();
        compiler.current_super = current_super;
        let mut params = Vec::new();
        if is_method {
            params.push("this".to_string());
        }
        params.extend(decl.params.iter().map(|param| param.name.clone()));
        if decl.is_async {
            params.push("__promise_id".to_string());
        }
        for param in &params {
            let reg = compiler.registers.allocate()?;
            let ty = if param == "this" {
                None
            } else {
                decl.params
                    .iter()
                    .find(|p| p.name == *param)
                    .and_then(|p| p.ty.clone())
            };
            compiler.insert_local(param.clone(), reg, true, ty)?;
        }
        for param in decl.params.iter() {
            if let Some(default) = &param.default {
                let param_reg = compiler
                    .resolve_local(&param.name)
                    .ok_or_else(|| CompileError::new("unknown parameter"))?
                    .reg;
                let missing_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::LoadMissing, missing_reg, 0, 0);
                let cond_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::Eq, cond_reg, param_reg, missing_reg);
                let skip_jump = compiler.emit_jump(Opcode::JumpIfNot, cond_reg);
                compiler.free_temp(cond_reg);
                compiler.free_temp(missing_reg);

                let value_reg = compiler.compile_expr(default)?;
                compiler.emit(Opcode::Move, param_reg, value_reg, 0);
                compiler.free_temp(value_reg);
                compiler.patch_jump(skip_jump)?;
            }
        }
        compiler.compile_stmt(&decl.body)?;
        let return_reg = match compiler.last_expr_register {
            Some(reg) => reg,
            None => {
                let reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::LoadNull, reg, 0, 0);
                reg
            }
        };
        compiler.emit(Opcode::Return, return_reg, 0, 0);
        compiler.chunk.max_registers = compiler.registers.max_used.max(params.len());
        compiler.chunk.module_name = self.module_name.clone();
        let func_index = self.chunk.functions.len();
        let arity = params.len() as u8;
        let param_specs = params
            .iter()
            .map(|name| {
                let (has_default, is_variadic, is_keyword_only) = if name == "this" {
                    (false, false, false)
                } else {
                    decl.params
                        .iter()
                        .find(|param| param.name == *name)
                        .map(|param| {
                            (
                                param.default.is_some(),
                                param.is_variadic,
                                param.is_keyword_only,
                            )
                        })
                        .unwrap_or((false, false, false))
                };
                ParamSpec {
                    name: name.clone(),
                    has_default,
                    is_variadic,
                    is_keyword_only,
                }
            })
            .collect();

        let is_pure = decl.annotations.iter().any(|a| a.name == "pure");
        self.chunk.functions.push(FunctionProto {
            name: decl.name.clone(),
            chunk: compiler.chunk,
            arity,
            params: param_specs,
            is_async: decl.is_async,
            is_pure,
        });
        let dest = self.registers.allocate()?;
        if store_as_function {
            self.emit(Opcode::FunctionRef, dest, func_index as u8, 0);
        } else {
            let capture_count = captures.len() as u8;
            self.emit(Opcode::Closure, dest, func_index as u8, capture_count);
            for (idx, name) in captures.iter().enumerate() {
                let reg = self
                    .resolve_local(name)
                    .ok_or_else(|| CompileError::new("unknown capture"))?
                    .reg;
                self.emit(Opcode::Capture, dest, reg, idx as u8);
            }
        }
        Ok(dest)
    }

    fn compile_lambda(
        &mut self,
        params: &[pleno_ast::Param],
        body: &pleno_ast::Expr,
        return_ty: Option<&str>,
        is_pure: bool,
    ) -> Result<u8, CompileError> {
        let body_stmt = match &body.kind {
            ExprKind::Block(stmts) => Stmt {
                kind: StmtKind::Block(stmts.clone()),
                span: body.span,
            },
            _ => Stmt {
                kind: StmtKind::Return(Some((*body).clone())),
                span: body.span,
            },
        };
        let decl = FunctionDecl {
            name: "anonymous".to_string(),
            generics: Vec::new(),
            params: params.to_vec(),
            return_ty: return_ty.map(|s| s.to_string()),
            body: Box::new(body_stmt),
            is_static: false,
            is_abstract: false,
            is_async: false,
            visibility: None,
            kind: FunctionKind::Method,
            annotations: Vec::new(),
        };

        let captures = self.analyze_captures(&decl, false);
        let mut compiler = Compiler::new(self.source.clone());
        compiler.is_top_level = false;
        compiler.locals = vec![HashMap::new()];
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect();

        for param in params {
            let reg = compiler.registers.allocate()?;
            compiler.insert_local(param.name.clone(), reg, true, param.ty.clone())?;
        }

        for param in params.iter() {
            if let Some(default) = &param.default {
                let param_reg = compiler
                    .resolve_local(&param.name)
                    .ok_or_else(|| CompileError::new("unknown parameter"))?
                    .reg;
                let missing_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::LoadMissing, missing_reg, 0, 0);
                let cond_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::Eq, cond_reg, param_reg, missing_reg);
                let skip_jump = compiler.emit_jump(Opcode::JumpIfNot, cond_reg);
                compiler.free_temp(cond_reg);
                compiler.free_temp(missing_reg);

                let value_reg = compiler.compile_expr(default)?;
                compiler.emit(Opcode::Move, param_reg, value_reg, 0);
                compiler.free_temp(value_reg);
                compiler.patch_jump(skip_jump)?;
            }
        }

        compiler.compile_stmt(&decl.body)?;
        let return_reg = match compiler.last_expr_register {
            Some(reg) => reg,
            None => {
                let reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::LoadNull, reg, 0, 0);
                reg
            }
        };
        compiler.emit(Opcode::Return, return_reg, 0, 0);
        compiler.chunk.max_registers = compiler.registers.max_used.max(params.len());
        compiler.chunk.module_name = self.module_name.clone();

        let func_index = self.chunk.functions.len();
        let arity = params.len() as u8;
        let param_specs = params
            .iter()
            .map(|p| ParamSpec {
                name: p.name.clone(),
                has_default: p.default.is_some(),
                is_variadic: p.is_variadic,
                is_keyword_only: p.is_keyword_only,
            })
            .collect();

        self.chunk.functions.push(FunctionProto {
            name: "<lambda>".to_string(),
            chunk: compiler.chunk,
            arity,
            params: param_specs,
            is_async: false,
            is_pure,
        });

        let dest = self.registers.allocate()?;
        let capture_count = captures.len() as u8;
        self.emit(Opcode::Closure, dest, func_index as u8, capture_count);
        for (idx, name) in captures.iter().enumerate() {
            let reg = self
                .resolve_local(name)
                .ok_or_else(|| CompileError::new("unknown capture"))?
                .reg;
            self.emit(Opcode::Capture, dest, reg, idx as u8);
        }
        Ok(dest)
    }

    fn analyze_captures(&self, decl: &FunctionDecl, is_method: bool) -> Vec<String> {
        let mut locals = HashMap::new();
        for param in &decl.params {
            locals.insert(param.name.clone(), ());
        }
        if is_method {
            locals.insert("this".to_string(), ());
        }
        self.collect_locals(&decl.body, &mut locals);
        let mut used = HashMap::new();
        for param in &decl.params {
            if let Some(default) = &param.default {
                self.collect_idents_expr(default, &mut used);
            }
        }
        self.collect_idents(&decl.body, &mut used);
        let mut captures = Vec::new();
        for name in used.keys() {
            if !locals.contains_key(name) && self.resolve_local(name).is_some() {
                captures.push(name.clone());
            }
        }
        captures
    }

    fn compile_class_declaration(&mut self, stmt: &Stmt) -> Result<u8, CompileError> {
        let (
            name,
            fields,
            methods,
            superclass,
            nested_classes,
            properties,
            is_abstract,
            implements,
        ) = match &stmt.kind {
            StmtKind::Class {
                name,
                generics: _,
                fields,
                methods,
                superclass,
                nested_classes,
                annotations: _,
                implements,
                is_abstract,
                properties,
            } => (
                name,
                fields,
                methods,
                superclass,
                nested_classes,
                properties,
                *is_abstract,
                implements.clone(),
            ),
            _ => return Err(CompileError::new("expected class statement")),
        };

        let methods_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, methods_reg, 0, 0);
        let static_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, static_reg, 0, 0);
        let getters_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, getters_reg, 0, 0);
        let setters_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, setters_reg, 0, 0);
        let static_getters_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, static_getters_reg, 0, 0);
        let static_setters_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, static_setters_reg, 0, 0);

        for method in methods {
            let is_method = !method.is_static;
            let func_reg =
                self.compile_function(method, is_method, true, superclass.as_ref().cloned())?;

            let (target_reg, name_str) = match method.kind {
                FunctionKind::Method => {
                    if method.is_static {
                        (static_reg, method.name.clone())
                    } else {
                        (methods_reg, method.name.clone())
                    }
                }
                FunctionKind::Getter => {
                    if method.is_static {
                        (static_getters_reg, method.name.clone())
                    } else {
                        (getters_reg, method.name.clone())
                    }
                }
                FunctionKind::Setter => {
                    if method.is_static {
                        (static_setters_reg, method.name.clone())
                    } else {
                        (setters_reg, method.name.clone())
                    }
                }
            };

            let meta_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, meta_reg, 0, 0);
            self.emit(Opcode::ArrayAppend, meta_reg, func_reg, 0);
            let vis_name = visibility_name(method.visibility.as_ref());
            let vis_idx = self.add_constant(Value::String(std::rc::Rc::new(vis_name.to_string())));
            let vis_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, vis_reg, vis_idx, 0);
            self.emit(Opcode::ArrayAppend, meta_reg, vis_reg, 0);
            self.free_temp(vis_reg);

            let name_idx = self.add_constant(Value::String(std::rc::Rc::new(name_str)));
            let name_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, name_reg, name_idx, 0);

            self.emit(Opcode::SetIndex, meta_reg, target_reg, name_reg);

            self.free_temp(name_reg);
            self.free_temp(func_reg);
            self.free_temp(meta_reg);
        }
        let class_reg = self.registers.allocate()?;
        let name_idx = self.add_constant(Value::String(name.clone().into()));
        self.emit(Opcode::DefineClass, class_reg, name_idx, methods_reg);
        self.emit(Opcode::SetStatic, class_reg, static_reg, 0);
        if is_abstract {
            self.emit(Opcode::SetAbstract, class_reg, 0, 0);
        }

        // Compile Implements
        if !implements.is_empty() {
            let interfaces_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, interfaces_reg, 0, 0);
            for interface_name in implements {
                let interface_reg = self.registers.allocate()?;
                if let Some(local) = self.resolve_local(&interface_name) {
                    self.emit(Opcode::Move, interface_reg, local.reg, 0);
                } else {
                    let name_idx =
                        self.add_constant(Value::String(std::rc::Rc::new(interface_name.clone())));
                    self.emit(Opcode::GetGlobal, interface_reg, name_idx, 0);
                }

                self.emit(Opcode::ArrayAppend, interfaces_reg, interface_reg, 0);
                self.free_temp(interface_reg);
            }
            self.emit(Opcode::SetImplements, class_reg, interfaces_reg, 0);
            self.free_temp(interfaces_reg);
        }
        // Compile Properties
        for prop in properties {
            if let Some(getter) = &prop.getter {
                let func_reg = self.compile_function(
                    getter,
                    !prop.is_static,
                    true,
                    superclass.as_ref().cloned(),
                )?;
                let meta_reg = self.registers.allocate()?;
                self.emit(Opcode::NewArray, meta_reg, 0, 0);
                self.emit(Opcode::ArrayAppend, meta_reg, func_reg, 0);
                let vis_name = visibility_name(prop.visibility.as_ref());
                let vis_idx =
                    self.add_constant(Value::String(std::rc::Rc::new(vis_name.to_string())));
                let vis_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, vis_reg, vis_idx, 0);
                self.emit(Opcode::ArrayAppend, meta_reg, vis_reg, 0);
                self.free_temp(vis_reg);

                let name_idx = self.add_constant(Value::String(prop.name.clone().into()));
                let name_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                if prop.is_static {
                    self.emit(Opcode::SetIndex, meta_reg, static_getters_reg, name_reg);
                } else {
                    self.emit(Opcode::SetIndex, meta_reg, getters_reg, name_reg);
                }
                self.free_temp(name_reg);
                self.free_temp(func_reg);
                self.free_temp(meta_reg);
            }

            if let Some(setter) = &prop.setter {
                let func_reg = self.compile_function(
                    setter,
                    !prop.is_static,
                    true,
                    superclass.as_ref().cloned(),
                )?;
                let meta_reg = self.registers.allocate()?;
                self.emit(Opcode::NewArray, meta_reg, 0, 0);
                self.emit(Opcode::ArrayAppend, meta_reg, func_reg, 0);
                let vis_name = visibility_name(prop.visibility.as_ref());
                let vis_idx =
                    self.add_constant(Value::String(std::rc::Rc::new(vis_name.to_string())));
                let vis_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, vis_reg, vis_idx, 0);
                self.emit(Opcode::ArrayAppend, meta_reg, vis_reg, 0);
                self.free_temp(vis_reg);

                let name_idx = self.add_constant(Value::String(prop.name.clone().into()));
                let name_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                if prop.is_static {
                    self.emit(Opcode::SetIndex, meta_reg, static_setters_reg, name_reg);
                } else {
                    self.emit(Opcode::SetIndex, meta_reg, setters_reg, name_reg);
                }
                self.free_temp(name_reg);
                self.free_temp(func_reg);
                self.free_temp(meta_reg);
            }
        }

        self.emit(Opcode::SetGetters, class_reg, getters_reg, 0);
        self.emit(Opcode::SetSetters, class_reg, setters_reg, 0);
        self.emit(Opcode::SetStaticGetters, class_reg, static_getters_reg, 0);
        self.emit(Opcode::SetStaticSetters, class_reg, static_setters_reg, 0);

        self.free_temp(getters_reg);
        self.free_temp(setters_reg);
        self.free_temp(static_getters_reg);
        self.free_temp(static_setters_reg);
        if let Some(superclass) = superclass {
            let super_reg = self.registers.allocate()?;
            let super_idx = self.add_constant(Value::String(superclass.clone().into()));
            self.emit(Opcode::GetGlobal, super_reg, super_idx, 0);
            self.emit(Opcode::SetSuper, class_reg, super_reg, 0);
            self.free_temp(super_reg);
        }
        for field in fields {
            let field_idx = self.add_constant(Value::String(field.name.clone().into()));
            let vis_code = visibility_code(field.visibility.as_ref());
            if field.is_static {
                let value_reg = if let Some(default) = &field.default {
                    self.compile_expr(default)?
                } else {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, reg, 0, 0);
                    reg
                };
                self.emit(Opcode::SetProperty, value_reg, class_reg, field_idx);
                self.free_temp(value_reg);
                self.emit(
                    Opcode::SetStaticFieldVisibility,
                    class_reg,
                    field_idx,
                    vis_code,
                );
            } else {
                self.emit(Opcode::SetFieldVisibility, class_reg, field_idx, vis_code);
            }
        }
        for nested in nested_classes {
            if let StmtKind::Class {
                name: nested_name, ..
            } = &nested.kind
            {
                // Recursively compile nested class
                let nested_reg = self.compile_class_declaration(nested)?;
                let nested_name_idx = self.add_constant(Value::String(nested_name.clone().into()));

                // Set as static property on the outer class
                self.emit(Opcode::SetProperty, nested_reg, class_reg, nested_name_idx);
                self.free_temp(nested_reg);
            }
        }
        self.free_temp(methods_reg);
        self.free_temp(static_reg);
        Ok(class_reg)
    }

    fn compile_enum_declaration(&mut self, decl: &pleno_ast::EnumDecl) -> Result<u8, CompileError> {
        let class_reg = self.registers.allocate()?;
        let name_idx = self.add_constant(Value::String(decl.name.clone().into()));
        self.emit(Opcode::DefineClass, class_reg, name_idx, 0);

        // For now, compile each variant as a static field
        // Future: full first-class Enum variants with data
        for (i, variant) in decl.variants.iter().enumerate() {
            let variant_idx = self.add_constant(Value::String(variant.name.clone().into()));

            // For now, variants are just their discriminator (Int) if no params
            // If they have params, they should be "constructors" of some sort,
            // but we'll stick to simple discriminators for the MVP.
            let value_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadInt, value_reg, i as u8, 0);

            self.emit(Opcode::SetProperty, value_reg, class_reg, variant_idx);
            self.free_temp(value_reg);
        }

        Ok(class_reg)
    }

    fn collect_locals(&self, stmt: &Stmt, locals: &mut HashMap<String, ()>) {
        match &stmt.kind {
            StmtKind::Let { name, .. } => {
                locals.insert(name.clone(), ());
            }
            StmtKind::Block(items) => {
                for item in items {
                    self.collect_locals(item, locals);
                }
            }
            StmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_locals(then_branch, locals);
                if let Some(else_branch) = else_branch {
                    self.collect_locals(else_branch, locals);
                }
            }
            StmtKind::While { body, .. } => self.collect_locals(body, locals),
            StmtKind::For { names, body, .. } => {
                for name in names {
                    locals.insert(name.clone(), ());
                }
                self.collect_locals(body, locals)
            }
            StmtKind::CFor { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_locals(init, locals);
                }
                self.collect_locals(body, locals)
            }
            StmtKind::Fn { name, .. } => {
                locals.insert(name.clone(), ());
            }
            _ => {}
        }
    }

    fn collect_idents(&self, stmt: &Stmt, used: &mut HashMap<String, ()>) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.collect_idents_expr(expr, used),
            StmtKind::Let {
                value: Some(value), ..
            } => {
                self.collect_idents_expr(value, used);
            }
            StmtKind::Assign { name, value } => {
                used.insert(name.clone(), ());
                self.collect_idents_expr(value, used);
            }
            StmtKind::AssignProperty { object, value, .. } => {
                self.collect_idents_expr(object, used);
                self.collect_idents_expr(value, used);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_idents(init, used);
                }
                if let Some(condition) = condition {
                    self.collect_idents_expr(condition, used);
                }
                if let Some(step) = step {
                    self.collect_idents(step, used);
                }
                self.collect_idents(body, used);
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.collect_idents_expr(collection, used);
                self.collect_idents_expr(index, used);
                self.collect_idents_expr(value, used);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_idents_expr(condition, used);
                self.collect_idents(then_branch, used);
                if let Some(else_branch) = else_branch {
                    self.collect_idents(else_branch, used);
                }
            }
            StmtKind::While { condition, body } => {
                self.collect_idents_expr(condition, used);
                self.collect_idents(body, used);
            }
            StmtKind::For { iterable, body, .. } => {
                self.collect_idents_expr(iterable, used);
                self.collect_idents(body, used);
            }
            StmtKind::Fn { .. } => {}
            StmtKind::Return(Some(expr)) => {
                self.collect_idents_expr(expr, used);
            }
            StmtKind::Block(items) => {
                for item in items {
                    self.collect_idents(item, used);
                }
            }
            StmtKind::Class { .. } => {}
            _ => {}
        }
    }

    fn collect_idents_expr(&self, expr: &Expr, used: &mut HashMap<String, ()>) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                used.insert(name.clone(), ());
            }
            ExprKind::Unary { expr, .. } => self.collect_idents_expr(expr, used),
            ExprKind::Binary { left, right, .. } => {
                self.collect_idents_expr(left, used);
                self.collect_idents_expr(right, used);
            }
            ExprKind::Property { object, .. } => self.collect_idents_expr(object, used),
            ExprKind::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.collect_idents_expr(start, used);
                }
                if let Some(end) = end {
                    self.collect_idents_expr(end, used);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        pleno_ast::MapItem::Entry(_, value) => {
                            self.collect_idents_expr(value, used)
                        }
                        pleno_ast::MapItem::Spread(expr) => self.collect_idents_expr(expr, used),
                    }
                }
            }
            ExprKind::ArrayLiteral(items)
            | ExprKind::SetLiteral(items)
            | ExprKind::TupleLiteral(items) => {
                for item in items {
                    self.collect_idents_expr(item, used);
                }
            }
            ExprKind::Index { collection, index } => {
                self.collect_idents_expr(collection, used);
                self.collect_idents_expr(index, used);
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                self.collect_idents_expr(collection, used);
                if let Some(start_expr) = start {
                    self.collect_idents_expr(start_expr, used);
                }
                if let Some(end_expr) = end {
                    self.collect_idents_expr(end_expr, used);
                }
            }
            ExprKind::Call { callee, args } => {
                self.collect_idents_expr(callee, used);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(expr) => {
                            self.collect_idents_expr(expr, used);
                        }
                        pleno_ast::CallArg::Named(_, expr) => {
                            self.collect_idents_expr(expr, used);
                        }
                        pleno_ast::CallArg::Spread(expr) => {
                            self.collect_idents_expr(expr, used);
                        }
                        pleno_ast::CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Match { expr, arms } => {
                self.collect_idents_expr(expr, used);
                for arm in arms {
                    self.collect_idents_expr(&arm.body, used);
                }
            }
            ExprKind::Pipeline { left, right } => {
                self.collect_idents_expr(left, used);
                self.collect_idents_expr(right, used);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct Local {
    reg: u8,
    mutable: bool,
    ty: Option<String>,
}

#[derive(Clone)]
struct LoopContext {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    continue_target: Option<usize>,
    labels: Vec<String>,
}

impl Compiler {
    fn push_loop(&mut self, continue_target: Option<usize>) {
        let labels = std::mem::take(&mut self.next_labels);
        self.loop_stack.push(LoopContext {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            continue_target,
            labels,
        });
    }

    fn pop_loop(&mut self) -> Option<LoopContext> {
        self.loop_stack.pop()
    }

    fn patch_loop_jumps(
        &mut self,
        loop_ctx: LoopContext,
        loop_end: usize,
    ) -> Result<(), CompileError> {
        let continue_target = loop_ctx.continue_target.unwrap_or(loop_end);
        for jump in loop_ctx.continue_jumps {
            self.patch_jump_to(jump, continue_target)?;
        }
        for jump in loop_ctx.break_jumps {
            self.patch_jump_to(jump, loop_end)?;
        }
        Ok(())
    }

    fn emit_type_check(&mut self, reg: u8, ty_name: &str) -> Result<(), CompileError> {
        // Parse the type string to handle optional and union types
        let (base_types, is_optional) = self.parse_type_string(ty_name);

        // Check if value is null (for optional types)
        if is_optional {
            let null_check_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadNull, null_check_reg, 0, 0);
            self.emit(Opcode::Eq, null_check_reg, reg, null_check_reg);
            let skip_null_check = self.emit_jump(Opcode::JumpIfNot, null_check_reg);
            // If null, type check passes for optional types
            let ok_jump = self.emit_jump(Opcode::Jump, 0);
            self.patch_jump(skip_null_check)?;
            self.free_temp(null_check_reg);

            // Continue to check base type
            self.emit_type_check_for_types(reg, &base_types, ty_name)?;

            self.patch_jump(ok_jump)?;
            Ok(())
        } else {
            self.emit_type_check_for_types(reg, &base_types, ty_name)
        }
    }

    fn parse_type_string(&self, ty_name: &str) -> (Vec<Vec<String>>, bool) {
        let mut name = ty_name.trim();
        let mut is_optional = false;
        if name.ends_with('?') {
            is_optional = true;
            name = name.strip_suffix('?').unwrap().trim();
        }

        // Expand alias if it exists
        let expanded = self
            .type_aliases
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or(name);

        // Split by Union (|)
        let parts: Vec<&str> = expanded.split('|').collect();
        let mut union_types = Vec::new();

        for part in parts {
            // Split by Intersection (&)
            let intersections: Vec<String> = part
                .split('&')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !intersections.is_empty() {
                union_types.push(intersections);
            }
        }

        (union_types, is_optional)
    }

    fn emit_type_check_for_types(
        &mut self,
        reg: u8,
        types: &[Vec<String>],
        original_ty_name: &str,
    ) -> Result<(), CompileError> {
        if types.is_empty() {
            return Ok(());
        }

        // types is OR of ANDs: (A & B) | (C & D)
        let mut success_jumps = Vec::new();
        let mut temp_regs = Vec::new();

        for group in types {
            let mut next_group_jumps = Vec::new();

            for ty_name in group {
                let type_reg = self.registers.allocate()?;
                temp_regs.push(type_reg);

                let name_idx = self.add_constant(Value::String(ty_name.clone().into()));
                self.emit(Opcode::GetGlobal, type_reg, name_idx, 0);

                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, is_null_reg, 0, 0);
                self.emit(Opcode::Eq, is_null_reg, type_reg, is_null_reg);
                let skip_primitive_load = self.emit_jump(Opcode::JumpIfNot, is_null_reg);
                self.emit(Opcode::LoadConst, type_reg, name_idx, 0);
                self.patch_jump(skip_primitive_load)?;
                self.free_temp(is_null_reg);

                let is_match_reg = self.registers.allocate()?;
                temp_regs.push(is_match_reg);
                self.emit(Opcode::Is, is_match_reg, reg, type_reg);

                // If this specific type in the AND block fails, jump to the next OR group
                let fail_jump = self.emit_jump(Opcode::JumpIfNot, is_match_reg);
                next_group_jumps.push(fail_jump);
            }

            // If we reached here, all types in the group matched
            let success_jump = self.emit_jump(Opcode::Jump, 0);
            success_jumps.push(success_jump);

            // Path all fail jumps for this group to start of NEXT group
            for jump in next_group_jumps {
                self.patch_jump(jump)?;
            }
        }

        // If we get here, none of the groups matched
        // Throw type error - use original type name to preserve optional/union syntax
        let error_msg = format!("Type error: expected {}, found ", original_ty_name);
        let msg_idx = self.add_constant(Value::String(error_msg.into()));
        let msg_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, msg_reg, msg_idx, 0);

        let actual_type_reg = self.registers.allocate()?;
        self.emit(Opcode::TypeOf, actual_type_reg, reg, 0);

        let final_msg_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, final_msg_reg, msg_reg, actual_type_reg);

        self.emit(Opcode::Throw, final_msg_reg, 0, 0);

        // Patch all success jumps to skip error
        let final_ok_jump = self.emit_jump(Opcode::Jump, 0);
        for jump in success_jumps {
            self.patch_jump(jump)?;
        }
        self.patch_jump(final_ok_jump)?;

        // Free temp registers
        for temp_reg in temp_regs {
            self.free_temp(temp_reg);
        }
        self.free_temp(msg_reg);
        self.free_temp(actual_type_reg);
        self.free_temp(final_msg_reg);

        Ok(())
    }

    fn insert_local(
        &mut self,
        name: String,
        reg: u8,
        mutable: bool,
        ty: Option<String>,
    ) -> Result<(), CompileError> {
        let scope = self
            .locals
            .last_mut()
            .ok_or_else(|| CompileError::new("scope missing"))?;
        scope.insert(name, Local { reg, mutable, ty });
        Ok(())
    }

    fn resolve_local(&self, name: &str) -> Option<Local> {
        for scope in self.locals.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(local.clone());
            }
        }
        None
    }

    fn is_local_reg(&self, reg: u8) -> bool {
        self.locals
            .iter()
            .any(|scope| scope.values().any(|local| local.reg == reg))
    }

    fn free_temp(&mut self, reg: u8) {
        if !self.is_local_reg(reg) && Some(reg) != self.last_expr_register {
            self.registers.free(reg);
        }
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.locals.pop() {
            for local in scope.values() {
                if Some(local.reg) != self.last_expr_register {
                    self.registers.free(local.reg);
                }
            }
        }
    }
}

struct RegisterAllocator {
    free: Vec<u8>,
    next: u8,
    max_used: usize,
}

impl RegisterAllocator {
    fn new() -> Self {
        Self {
            free: Vec::new(),
            next: 0,
            max_used: 0,
        }
    }

    fn allocate(&mut self) -> Result<u8, CompileError> {
        if let Some(reg) = self.free.pop() {
            return Ok(reg);
        }
        if self.next == u8::MAX {
            return Err(CompileError::new("too many registers in function"));
        }
        let reg = self.next;
        self.next = self.next.saturating_add(1);
        self.max_used = self.max_used.max(self.next as usize);
        Ok(reg)
    }

    fn free(&mut self, reg: u8) {
        if reg + 1 == self.next {
            self.next = self.next.saturating_sub(1);
        } else {
            self.free.push(reg);
        }
    }
}

fn encode_offset(offset: i16) -> (u8, u8) {
    let hi = ((offset >> 8) & 0xFF) as u8;
    let lo = (offset & 0xFF) as u8;
    (hi, lo)
}

fn visibility_name(visibility: Option<&pleno_ast::Visibility>) -> &'static str {
    match visibility {
        Some(pleno_ast::Visibility::Private) => "private",
        Some(pleno_ast::Visibility::Protected) => "protected",
        Some(pleno_ast::Visibility::Internal) => "internal",
        _ => "public",
    }
}

fn visibility_code(visibility: Option<&pleno_ast::Visibility>) -> u8 {
    match visibility {
        Some(pleno_ast::Visibility::Private) => 1,
        Some(pleno_ast::Visibility::Protected) => 2,
        Some(pleno_ast::Visibility::Internal) => 3,
        _ => 0,
    }
}

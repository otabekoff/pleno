use pleno_ast::{Expr, FunctionDecl, FunctionKind, Literal, Program, Stmt};
use pleno_bytecode::{Chunk, FunctionProto, Instruction, Opcode, ParamSpec};
use pleno_value::Value;
use std::collections::HashMap;

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
}

#[derive(Debug, Clone)]
struct ExportItem {
    local: String,
    export: String,
}

impl Compiler {
    pub fn new() -> Self {
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
        }
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
        match stmt {
            Stmt::Module { name } => {
                if !self.module_mode {
                    return Err(CompileError::new("module declaration only allowed in modules"));
                }
                if self.module_name.is_some() {
                    return Err(CompileError::new("module already declared"));
                }
                self.module_name = Some(name.clone());
                Ok(())
            }
            Stmt::Expr(expr) => {
                let reg = self.compile_expr(expr)?;
                self.last_expr_register = Some(reg);
                Ok(())
            }
            Stmt::Let {
                name,
                value,
                mutable,
                is_const,
                ..
            } => {
                if *is_const && value.is_none() {
                    return Err(CompileError::new("const requires an initializer"));
                }
                if let Some(expr) = value {
                    let reg = self.compile_expr(expr)?;
                    self.insert_local(name.clone(), reg, *mutable);
                } else {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, reg, 0, 0);
                    self.insert_local(name.clone(), reg, *mutable);
                }
                Ok(())
            }
            Stmt::Assign { name, value } => {
                let value_reg = self.compile_expr(value)?;
                let local = self
                    .resolve_local(name)
                    .ok_or_else(|| CompileError::new("unknown local in assignment"))?;
                if !local.mutable {
                    return Err(CompileError::new("cannot assign to immutable binding"));
                }
                self.emit(Opcode::Move, local.reg, value_reg, 0);
                self.free_temp(value_reg);
                Ok(())
            }
            Stmt::AssignProperty { object, name, value } => {
                let object_reg = self.compile_expr(object)?;
                let value_reg = self.compile_expr(value)?;
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::SetProperty, value_reg, object_reg, name_idx);
                self.free_temp(value_reg);
                self.free_temp(object_reg);
                Ok(())
            }
            Stmt::AssignIndex {
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
            Stmt::Block(statements) => {
                self.locals.push(HashMap::new());
                for stmt in statements {
                    self.compile_stmt(stmt)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::If {
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
            Stmt::While { condition, body } => {
                let loop_start = self.chunk.code.len();
                let cond_reg = self.compile_expr(condition)?;
                let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
                self.free_temp(cond_reg);
                self.push_loop(Some(loop_start));
                self.compile_stmt(body)?;
                let loop_ctx = self.pop_loop().expect("loop stack underflow");
                self.emit_loop(loop_start)?;
                self.patch_jump(jump_to_end)?;
                let loop_end = self.chunk.code.len();
                self.patch_loop_jumps(loop_ctx, loop_end)?;
                Ok(())
            }
            Stmt::For { name, iterable, body } => self.compile_for_stmt(name, iterable, body),
            Stmt::Import { path, kind } => {
                let module_reg = self.registers.allocate()?;
                let path_idx = self.add_constant(Value::String(path.clone().into()));
                self.emit(Opcode::Import, module_reg, path_idx, 0);
                match kind {
                    pleno_ast::ImportKind::Module { alias } => {
                        if let Some(name) = alias {
                            self.insert_local(name.clone(), module_reg, false);
                        } else {
                            self.free_temp(module_reg);
                        }
                    }
                    pleno_ast::ImportKind::Star { alias } => {
                        self.insert_local(alias.clone(), module_reg, false);
                    }
                    pleno_ast::ImportKind::Names(items) => {
                        for item in items {
                            let name_idx =
                                self.add_constant(Value::String(item.name.clone().into()));
                            let dest = self.registers.allocate()?;
                            self.emit(Opcode::GetProperty, dest, module_reg, name_idx);
                            let local_name = item.alias.clone().unwrap_or(item.name.clone());
                            self.insert_local(local_name, dest, false);
                        }
                        self.free_temp(module_reg);
                    }
                }
                Ok(())
            }
            Stmt::Export(stmt) => {
                if !self.module_mode {
                    return Err(CompileError::new("export is only allowed in modules"));
                }
                let export_name = match stmt.as_ref() {
                    Stmt::Let { name, .. } => name.clone(),
                    Stmt::Fn { name, .. } => name.clone(),
                    Stmt::Class { name, .. } => name.clone(),
                    _ => return Err(CompileError::new("invalid export statement")),
                };
                self.compile_stmt(stmt)?;
                self.module_exports.push(ExportItem {
                    local: export_name.clone(),
                    export: export_name,
                });
                Ok(())
            }
            Stmt::ExportList(items) => {
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
            Stmt::Class {
                name,
                fields,
                methods,
                superclass,
                annotations: _,
            } => {
                let methods_reg = self.registers.allocate()?;
                self.emit(Opcode::NewMap, methods_reg, 0, 0);
                let static_reg = self.registers.allocate()?;
                self.emit(Opcode::NewMap, static_reg, 0, 0);
                for method in methods {
                    let is_method = !method.is_static;
                    let func_reg = self.compile_function(
                        method,
                        is_method,
                        true,
                        superclass.as_ref().cloned(),
                    )?;
                    let method_name = match method.kind {
                        FunctionKind::Method => method.name.clone(),
                        FunctionKind::Getter => format!("get:{}", method.name),
                        FunctionKind::Setter => format!("set:{}", method.name),
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
                    let name_idx =
                        self.add_constant(Value::String(method_name.clone().into()));
                    let name_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                    if method.is_static {
                        self.emit(Opcode::SetIndex, meta_reg, static_reg, name_reg);
                    } else {
                        self.emit(Opcode::SetIndex, meta_reg, methods_reg, name_reg);
                    }
                    self.free_temp(name_reg);
                    self.free_temp(func_reg);
                    self.free_temp(meta_reg);
                }
                let class_reg = self.registers.allocate()?;
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::DefineClass, class_reg, name_idx, methods_reg);
                self.emit(Opcode::SetStatic, class_reg, static_reg, 0);
                if let Some(superclass) = superclass {
                    let super_reg = self.registers.allocate()?;
                    let super_idx =
                        self.add_constant(Value::String(superclass.clone().into()));
                    self.emit(Opcode::GetGlobal, super_reg, super_idx, 0);
                    self.emit(Opcode::SetSuper, class_reg, super_reg, 0);
                    self.free_temp(super_reg);
                }
                for field in fields {
                    let field_idx =
                        self.add_constant(Value::String(field.name.clone().into()));
                    let vis_code = visibility_code(field.visibility.as_ref());
                    if field.is_static {
                        let value_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadNull, value_reg, 0, 0);
                        self.emit(Opcode::SetProperty, value_reg, class_reg, field_idx);
                        self.free_temp(value_reg);
                        self.emit(Opcode::SetStaticFieldVisibility, class_reg, field_idx, vis_code);
                    } else {
                        self.emit(Opcode::SetFieldVisibility, class_reg, field_idx, vis_code);
                    }
                }
                self.emit(Opcode::SetGlobal, class_reg, name_idx, 0);
                self.free_temp(class_reg);
                self.free_temp(methods_reg);
                self.free_temp(static_reg);
                Ok(())
            }
            Stmt::Fn {
                name,
                params,
                body,
                annotations,
            } => {
                let decl = FunctionDecl {
                    name: name.clone(),
                    params: params.clone(),
                    return_ty: None,
                    body: body.clone(),
                    is_static: false,
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
                        return Err(CompileError::new(
                            "function name already defined in scope",
                        ));
                    }
                    self.insert_local(name.clone(), func_reg, false);
                }
                Ok(())
            }
            Stmt::Return(expr) => {
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
            Stmt::Break => self.compile_break_stmt(),
            Stmt::Continue => self.compile_continue_stmt(),
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        match expr {
            Expr::Literal(lit) => {
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
                }
                Ok(reg)
            }
            Expr::Binary { left, op, right } => {
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
                            _ => return Err(CompileError::new("unsupported binary op")),
                        };
                        self.emit(opcode, dest, left_reg, right_reg);
                        self.free_temp(left_reg);
                        self.free_temp(right_reg);
                        Ok(dest)
                    }
                }
            }
            Expr::Unary { op, expr } => {
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
            Expr::Ident(name) => {
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
            Expr::This => self
                .resolve_local("this")
                .map(|local| local.reg)
                .ok_or_else(|| CompileError::new("this used outside of method")),
            Expr::Super => Err(CompileError::new("super is not supported yet")),
            Expr::Property { object, name } => {
                if matches!(**object, Expr::Super) {
                    let this_reg = self
                        .resolve_local("this")
                        .map(|local| local.reg)
                        .ok_or_else(|| CompileError::new("super used outside of method"))?;
                    if self.current_super.is_none() {
                        return Err(CompileError::new("super used without superclass"));
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
            Expr::Range {
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
            Expr::MapLiteral(entries) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewMap, dest, 0, 0);
                for (key, value_expr) in entries {
                    let value_reg = self.compile_expr(value_expr)?;
                    let key_idx = self.add_constant(Value::String(key.clone().into()));
                    self.emit(Opcode::SetProperty, value_reg, dest, key_idx);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            Expr::ArrayLiteral(items) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewArray, dest, 0, 0);
                for item in items {
                    let value_reg = self.compile_expr(item)?;
                    self.emit(Opcode::ArrayAppend, dest, value_reg, 0);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            Expr::SetLiteral(items) => {
                let dest = self.registers.allocate()?;
                self.emit(Opcode::NewSet, dest, 0, 0);
                for item in items {
                    let value_reg = self.compile_expr(item)?;
                    self.emit(Opcode::SetAdd, dest, value_reg, 0);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            Expr::Pipeline { left, right } => {
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
            Expr::Underscore => {
                Err(CompileError::new("underscore is only allowed as a placeholder in calls"))
            }
            Expr::Index { collection, index } => {

                let collection_reg = self.compile_expr(collection)?;
                let index_reg = self.compile_expr(index)?;
                let dest = self.registers.allocate()?;
                self.emit(Opcode::GetIndex, dest, collection_reg, index_reg);
                self.free_temp(collection_reg);
                self.free_temp(index_reg);
                Ok(dest)
            }
            Expr::Call { callee, args } => {
                let callee_reg = self.compile_expr(callee)?;
                let has_placeholder =
                    args.iter()
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
                                let name_idx = self.add_constant(Value::String(name.clone().into()));
                                self.emit(Opcode::SetProperty, arg_reg, named_reg, name_idx);
                                self.free_temp(arg_reg);
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
                        let arg_reg = match arg {
                            pleno_ast::CallArg::Positional(expr) => self.compile_expr(expr)?,
                            pleno_ast::CallArg::Named(_, _) => {
                                return Err(CompileError::new("named arguments require name"))
                            }
                            pleno_ast::CallArg::Placeholder => unreachable!(),
                        };
                        self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
                        self.free_temp(arg_reg);
                    }
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::Call, dest, callee_reg, args_reg);
                    self.free_temp(callee_reg);
                    self.free_temp(args_reg);
                    Ok(dest)
                }
            }
        }
    }

    fn add_constant(&mut self, value: Value) -> u8 {
        self.chunk.constants.push(value);
        (self.chunk.constants.len() - 1) as u8
    }

    fn emit(&mut self, opcode: Opcode, a: u8, b: u8, c: u8) {
        self.chunk.code.push(Instruction::new(opcode, a, b, c));
        self.chunk.lines.push(0);
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

    fn patch_jump(&mut self, jump_index: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = self.chunk.code.len() as isize;
        let offset = jump_to - jump_from;
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("jump too large"))?;
        let (b, c) = encode_offset(offset);
        let instruction = self.chunk.code[jump_index];
        self.chunk.code[jump_index] =
            Instruction::new(instruction.opcode(), instruction.a(), b, c);
        Ok(())
    }

    fn patch_jump_to(&mut self, jump_index: usize, jump_to: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = jump_to as isize;
        let offset = jump_to - jump_from;
        let offset = i16::try_from(offset).map_err(|_| CompileError::new("jump too large"))?;
        let (b, c) = encode_offset(offset);
        let instruction = self.chunk.code[jump_index];
        self.chunk.code[jump_index] =
            Instruction::new(instruction.opcode(), instruction.a(), b, c);
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

    fn compile_for_stmt(
        &mut self,
        name: &str,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        if let Expr::Range { start, end, inclusive } = iterable {
            match (start, end) {
                (Some(start), Some(end)) => {
                    return self.compile_for_range(name, start, end, *inclusive, body);
                }
                (None, Some(end)) => {
                    let zero = Expr::Literal(Literal::Int(0));
                    return self.compile_for_range(name, &zero, end, *inclusive, body);
                }
                (Some(start), None) => {
                    return self.compile_for_open_range(name, start, body);
                }
                _ => {
                    return Err(CompileError::new(
                        "for loop requires a start or end value for range",
                    ));
                }
            }
        }
        if matches!(iterable, Expr::Literal(_)) {
            return Err(CompileError::new("for loop expects array, set, or range"));
        }
        self.compile_for_iterable(name, iterable, body)
    }

    fn compile_break_stmt(&mut self) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new("break used outside of loop"));
        }
        let jump = self.emit_jump(Opcode::Jump, 0);
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.break_jumps.push(jump);
        }
        Ok(())
    }

    fn compile_continue_stmt(&mut self) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(CompileError::new("continue used outside of loop"));
        }
        let jump = self.emit_jump(Opcode::Jump, 0);
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.continue_jumps.push(jump);
        }
        Ok(())
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
        self.insert_local(name.to_string(), start_reg, true);

        let end_reg = self.compile_expr(end)?;
        let end_name = format!("$for_end_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(end_name, end_reg, false);

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
        let loop_ctx = self.pop_loop().expect("loop stack underflow");
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
        self.insert_local(name.to_string(), start_reg, true);

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
        let loop_ctx = self.pop_loop().expect("loop stack underflow");
        self.patch_loop_jumps(loop_ctx, loop_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_iterable(
        &mut self,
        name: &str,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let source_reg = self.compile_expr(iterable)?;
        let collection_reg = self.registers.allocate()?;
        self.emit(Opcode::IterValues, collection_reg, source_reg, 0);
        self.free_temp(source_reg);
        let collection_name = format!("$for_collection_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(collection_name, collection_reg, false);

        let index_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, index_reg, 0, 0);
        let index_name = format!("$for_index_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(index_name, index_reg, true);

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
        if self
            .locals
            .last()
            .map(|scope| scope.contains_key(name))
            .unwrap_or(false)
        {
            return Err(CompileError::new("for loop variable already defined in scope"));
        }
        self.insert_local(name.to_string(), item_reg, true);

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
        let loop_ctx = self.pop_loop().expect("loop stack underflow");
        self.patch_loop_jumps(loop_ctx, loop_end)?;
        self.pop_scope();
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
        let mut compiler = Compiler::new();
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
        for param in &params {
            let reg = compiler.registers.allocate()?;
            compiler.insert_local(param.clone(), reg, true);
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
        self.chunk.functions.push(FunctionProto {
            chunk: compiler.chunk,
            arity,
            params: param_specs,
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

    fn collect_locals(&self, stmt: &Stmt, locals: &mut HashMap<String, ()>) {
        match stmt {
            Stmt::Let { name, .. } => {
                locals.insert(name.clone(), ());
            }
            Stmt::Block(items) => {
                for item in items {
                    self.collect_locals(item, locals);
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_locals(then_branch, locals);
                if let Some(else_branch) = else_branch {
                    self.collect_locals(else_branch, locals);
                }
            }
            Stmt::While { body, .. } => self.collect_locals(body, locals),
            Stmt::For { body, .. } => self.collect_locals(body, locals),
            Stmt::Fn { name, .. } => {
                locals.insert(name.clone(), ());
            }
            _ => {}
        }
    }

    fn collect_idents(&self, stmt: &Stmt, used: &mut HashMap<String, ()>) {
        match stmt {
            Stmt::Expr(expr) => self.collect_idents_expr(expr, used),
            Stmt::Let { value, .. } => {
                if let Some(value) = value {
                    self.collect_idents_expr(value, used);
                }
            }
            Stmt::Assign { name, value } => {
                used.insert(name.clone(), ());
                self.collect_idents_expr(value, used);
            }
            Stmt::AssignProperty { object, value, .. } => {
                self.collect_idents_expr(object, used);
                self.collect_idents_expr(value, used);
            }
            Stmt::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.collect_idents_expr(collection, used);
                self.collect_idents_expr(index, used);
                self.collect_idents_expr(value, used);
            }
            Stmt::If {
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
            Stmt::While { condition, body } => {
                self.collect_idents_expr(condition, used);
                self.collect_idents(body, used);
            }
            Stmt::For { iterable, body, .. } => {
                self.collect_idents_expr(iterable, used);
                self.collect_idents(body, used);
            }
            Stmt::Fn { .. } => {}
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.collect_idents_expr(expr, used);
                }
            }
            Stmt::Block(items) => {
                for item in items {
                    self.collect_idents(item, used);
                }
            }
            Stmt::Class { .. } => {}
            _ => {}
        }
    }

    fn collect_idents_expr(&self, expr: &Expr, used: &mut HashMap<String, ()>) {
        match expr {
            Expr::Ident(name) => {
                used.insert(name.clone(), ());
            }
            Expr::Unary { expr, .. } => self.collect_idents_expr(expr, used),
            Expr::Binary { left, right, .. } => {
                self.collect_idents_expr(left, used);
                self.collect_idents_expr(right, used);
            }
            Expr::Property { object, .. } => self.collect_idents_expr(object, used),
            Expr::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.collect_idents_expr(start, used);
                }
                if let Some(end) = end {
                    self.collect_idents_expr(end, used);
                }
            }
            Expr::MapLiteral(items) => {
                for (_, value) in items {
                    self.collect_idents_expr(value, used);
                }
            }
            Expr::ArrayLiteral(items) | Expr::SetLiteral(items) => {
                for item in items {
                    self.collect_idents_expr(item, used);
                }
            }
            Expr::Index { collection, index } => {
                self.collect_idents_expr(collection, used);
                self.collect_idents_expr(index, used);
            }
            Expr::Call { callee, args } => {
                self.collect_idents_expr(callee, used);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(expr) => {
                            self.collect_idents_expr(expr, used);
                        }
                        pleno_ast::CallArg::Named(_, expr) => {
                            self.collect_idents_expr(expr, used);
                        }
                        pleno_ast::CallArg::Placeholder => {}
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Local {
    reg: u8,
    mutable: bool,
}

#[derive(Clone)]
struct LoopContext {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    continue_target: Option<usize>,
}

impl Compiler {
    fn push_loop(&mut self, continue_target: Option<usize>) {
        self.loop_stack.push(LoopContext {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            continue_target,
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

    fn insert_local(&mut self, name: String, reg: u8, mutable: bool) {
        let scope = self.locals.last_mut().expect("scope exists");
        scope.insert(name, Local { reg, mutable });
    }

    fn resolve_local(&self, name: &str) -> Option<Local> {
        for scope in self.locals.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(*local);
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

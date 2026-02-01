use indexmap::IndexMap;
use pleno_ast::{
    BinaryOp, Expr, ExprKind, FieldDecl, FunctionDecl, FunctionKind, Literal, MapItem, Pattern,
    Program, Span, Stmt, StmtKind, Visibility,
};
use pleno_bytecode::{Chunk, FunctionProto, Instruction, Opcode, ParamSpec};
use pleno_value::gc::GcRc;
use pleno_value::{ArrayState, MapState, RuntimeAnnotation, Value};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
    Test,
}

#[derive(Debug, Clone)]
struct MacroDef {
    params: Vec<String>,
    body: Stmt,
}

#[derive(Debug, Clone)]
struct Local {
    reg: u8,
    mutable: bool,
    ty: Option<String>,
    is_boxed: bool,
}

#[derive(Debug, Clone)]
struct CaptureInfo {
    name: String,
    is_boxed: bool,
}

#[derive(Clone)]
struct LoopContext {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    continue_target: Option<usize>,
    labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnershipState {
    Available,
    Moved,
}

#[derive(Debug, Clone)]
pub struct ParamOwnership {
    pub name: String,
    pub ownership: pleno_ast::Ownership,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub diagnostic: pleno_diagnostics::Diagnostic,
}

impl pleno_diagnostics::ToDiagnostic for CompileError {
    fn to_diagnostic(&self) -> pleno_diagnostics::Diagnostic {
        self.diagnostic.clone()
    }
}

pub struct Compiler {
    chunk: Chunk,
    registers: RegisterAllocator,
    last_expr_register: Option<u8>,
    locals: Vec<HashMap<String, Local>>,
    lazy_locals: HashMap<u8, bool>,
    ownership_states: HashMap<u8, OwnershipState>,
    fn_signatures: Vec<HashMap<String, Vec<ParamOwnership>>>,
    inline_functions: Vec<HashMap<String, FunctionDecl>>,
    inline_return_stack: Vec<Vec<usize>>,
    referenced_entities: HashSet<String>,
    loop_counter: u32,
    loop_stack: Vec<LoopContext>,
    captures: HashMap<String, (usize, bool)>,
    is_top_level: bool,
    current_super: Option<String>,
    module_mode: bool,
    module_exports: Vec<ExportItem>,
    module_name: Option<String>,
    source: String,
    current_span: pleno_ast::Span,
    next_labels: Vec<String>,
    type_aliases: HashMap<String, String>,
    macros: HashMap<String, MacroDef>,
    boxed_vars: Vec<HashSet<String>>,
    finally_stack: Vec<Option<Stmt>>,
    pub is_async: bool,
    pub profile: BuildProfile,
    _source_map: pleno_diagnostics::SourceMap,
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
            lazy_locals: HashMap::new(),
            ownership_states: HashMap::new(),
            fn_signatures: vec![HashMap::new()],
            inline_functions: vec![HashMap::new()],
            inline_return_stack: Vec::new(),
            referenced_entities: HashSet::new(),
            loop_counter: 0,
            loop_stack: Vec::new(),
            captures: HashMap::new(),
            is_top_level: true,
            current_super: None,
            module_mode: false,
            module_exports: Vec::new(),
            module_name: None,
            _source_map: pleno_diagnostics::SourceMap::new("file".to_string(), source.clone()),
            source,
            current_span: pleno_ast::Span::new(0, 0),
            next_labels: Vec::new(),
            type_aliases: HashMap::new(),
            macros: HashMap::new(),
            boxed_vars: vec![HashSet::new()],
            finally_stack: Vec::new(),
            is_async: false,
            profile: BuildProfile::Debug,
        }
    }

    pub fn with_profile(mut self, profile: BuildProfile) -> Self {
        self.profile = profile;
        self
    }

    fn error(&self, message: &str, span: Span) -> CompileError {
        use pleno_diagnostics::Diagnostic;
        let diagnostic = Diagnostic::error(message).with_label(span, "");
        CompileError { diagnostic }
    }

    pub fn compile(&mut self, program: &Program) -> Result<Chunk, CompileError> {
        self.module_mode = false;
        self.module_exports.clear();
        self.module_name = None;

        self.referenced_entities = self.collect_references(&program.statements);

        let boxed = self.find_boxed_vars(&program.statements);
        self.boxed_vars.push(boxed);

        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }

        self.boxed_vars.pop();

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

        self.referenced_entities = self.collect_references(&program.statements);
        let boxed = self.find_boxed_vars(&program.statements);
        self.boxed_vars.push(boxed);

        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }

        self.boxed_vars.pop();

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
            StmtKind::Module { name, body, .. } => {
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
            StmtKind::TypeAlias { name, ty, .. } => {
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
                is_lazy,
                ..
            } => {
                if *is_const && value.is_none() {
                    return Err(self.error("const requires an initializer", stmt.span));
                }
                let reg = if let Some(expr) = value {
                    if *is_lazy {
                        let thunk_reg = self.compile_lazy_thunk(expr)?;
                        if let Some(ty_name) = ty {
                            self.emit_type_check(thunk_reg, ty_name)?;
                        }
                        thunk_reg
                    } else {
                        let reg = self.compile_expr(expr)?;
                        if let Some(ty_name) = ty {
                            self.emit_type_check(reg, ty_name)?;
                        }
                        reg
                    }
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
                if *is_const {
                    let recursive = if *is_deep { 1 } else { 0 };
                    self.emit(Opcode::Freeze, reg, recursive, 0);
                } else if *is_deep {
                    // let deep should also work
                    self.emit(Opcode::Freeze, reg, 1, 0);
                }
                if self
                    .boxed_vars
                    .last()
                    .map(|b| b.contains(name))
                    .unwrap_or(false)
                {
                    let cell_reg = self.registers.allocate()?;
                    self.emit(Opcode::MakeCell, cell_reg, reg, 0);
                    self.emit(Opcode::Move, reg, cell_reg, 0);
                    self.free_temp(cell_reg);
                }
                self.insert_local(name.clone(), reg, *mutable, ty.clone())?;
                if *is_lazy {
                    self.lazy_locals.insert(reg, true);
                }
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
                    if local.is_boxed {
                        self.emit(Opcode::SetCell, local.reg, value_reg, 0);
                    } else {
                        self.emit(Opcode::Move, local.reg, value_reg, 0);
                    }
                    self.ownership_states
                        .insert(local.reg, OwnershipState::Available);
                } else if let Some((index, is_boxed)) = self.captures.get(name).cloned() {
                    // Update capture
                    if is_boxed {
                        let cell_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadCapture, cell_reg, index as u8, 0);
                        self.emit(Opcode::SetCell, cell_reg, value_reg, 0);
                        self.free_temp(cell_reg);
                    } else {
                        self.emit(Opcode::SetCapture, value_reg, index as u8, 0);
                    }
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
                let boxed = self.find_boxed_vars(statements);
                self.boxed_vars.push(boxed);
                self.push_scope();
                for stmt in statements {
                    self.compile_stmt(stmt)?;
                }
                self.pop_scope();
                self.boxed_vars.pop();
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
                self.push_scope();

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
                    return Err(self.error("export is only allowed in modules", stmt.span));
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
                    return Err(self.error("export is only allowed in modules", self.current_span));
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
                // Dead Code Elimination: Only compile if referenced
                if !self.referenced_entities.contains(name) && self.is_top_level {
                    return Ok(());
                }

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
                ..
            } => {
                let ownership = params
                    .iter()
                    .map(|param| ParamOwnership {
                        name: param.name.clone(),
                        ownership: param.ownership.clone(),
                    })
                    .collect::<Vec<_>>();
                // Dead Code Elimination: Only compile if referenced
                if !self.referenced_entities.contains(name) && self.is_top_level {
                    return Ok(());
                }

                if let Some(scope) = self.fn_signatures.last_mut() {
                    scope.insert(name.clone(), ownership);
                }

                let decl = FunctionDecl {
                    span: Span::new(0, 0),
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
                    doc: None,
                };

                if annotations.iter().any(|a| a.name == "inline")
                    && self.referenced_entities.contains(name)
                    && let Some(scope) = self.inline_functions.last_mut()
                {
                    scope.insert(name.clone(), decl.clone());
                }

                let func_reg = self.compile_function(&decl, false, true, None, None)?;
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
                if !self.inline_return_stack.is_empty() {
                    // Inline return handling... (unchanged logic for inline, need to be careful? Inline functions don't cross try boundaries of parent? They do if defined inside try?)
                    // For now, assume inline functions are self-contained.
                    self.last_expr_register = Some(reg);
                    let jump = self.emit_jump(Opcode::Jump, 0);
                    self.inline_return_stack.last_mut().unwrap().push(jump);
                } else {
                    // Execute valid finally blocks in reverse order
                    let pending_finally = self.finally_stack.to_vec();
                    for finally_stmt in pending_finally.iter().rev().flatten() {
                        self.compile_stmt(finally_stmt)?;
                    }
                    self.emit(Opcode::Return, reg, 0, 0);
                }
                Ok(())
            }
            StmtKind::Break(label) => self.compile_break_stmt(label.as_deref(), stmt.span),
            StmtKind::Continue(label) => self.compile_continue_stmt(label.as_deref(), stmt.span),
            StmtKind::Labeled { label, stmt } => {
                self.next_labels.push(label.clone());
                self.compile_stmt(stmt)
            }
            StmtKind::Extend {
                target,
                generics,
                methods,
                ..
            } => {
                self.compile_extend_stmt(target, generics, methods)?;
                Ok(())
            }
            StmtKind::Interface {
                name,
                methods,
                generics,
                properties,
                ..
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
                        FunctionKind::Operator(_) => "operator",
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

                for prop in properties {
                    if let Some(_getter) = &prop.getter {
                        let meta_reg = self.registers.allocate()?;
                        self.emit(Opcode::NewArray, meta_reg, 0, 0);

                        let name_val = Value::String(std::rc::Rc::new(prop.name.clone()));
                        let name_reg = self.registers.allocate()?;
                        let name_idx = self.add_constant(name_val);
                        self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, name_reg, 0);
                        self.free_temp(name_reg);

                        let type_reg = self.registers.allocate()?;
                        let type_idx = self
                            .add_constant(Value::String(std::rc::Rc::new("getter".to_string())));
                        self.emit(Opcode::LoadConst, type_reg, type_idx, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, type_reg, 0);
                        self.free_temp(type_reg);

                        let arity_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, arity_reg, 0, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, arity_reg, 0);
                        self.free_temp(arity_reg);

                        let static_reg = self.registers.allocate()?;
                        if prop.is_static {
                            self.emit(Opcode::LoadTrue, static_reg, 0, 0);
                        } else {
                            self.emit(Opcode::LoadFalse, static_reg, 0, 0);
                        }
                        self.emit(Opcode::ArrayAppend, meta_reg, static_reg, 0);
                        self.free_temp(static_reg);

                        let async_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadFalse, async_reg, 0, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, async_reg, 0);
                        self.free_temp(async_reg);

                        let key_idx =
                            self.add_constant(Value::String(std::rc::Rc::new(prop.name.clone())));
                        let key_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, key_reg, key_idx, 0);
                        self.emit(Opcode::SetIndex, meta_reg, methods_reg, key_reg);
                        self.free_temp(key_reg);
                        self.free_temp(meta_reg);
                    }

                    if let Some(_setter) = &prop.setter {
                        let meta_reg = self.registers.allocate()?;
                        self.emit(Opcode::NewArray, meta_reg, 0, 0);

                        let name_val = Value::String(std::rc::Rc::new(prop.name.clone()));
                        let name_reg = self.registers.allocate()?;
                        let name_idx = self.add_constant(name_val);
                        self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, name_reg, 0);
                        self.free_temp(name_reg);

                        let type_reg = self.registers.allocate()?;
                        let type_idx = self
                            .add_constant(Value::String(std::rc::Rc::new("setter".to_string())));
                        self.emit(Opcode::LoadConst, type_reg, type_idx, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, type_reg, 0);
                        self.free_temp(type_reg);

                        let arity_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, arity_reg, 1, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, arity_reg, 0);
                        self.free_temp(arity_reg);

                        let static_reg = self.registers.allocate()?;
                        if prop.is_static {
                            self.emit(Opcode::LoadTrue, static_reg, 0, 0);
                        } else {
                            self.emit(Opcode::LoadFalse, static_reg, 0, 0);
                        }
                        self.emit(Opcode::ArrayAppend, meta_reg, static_reg, 0);
                        self.free_temp(static_reg);

                        let async_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadFalse, async_reg, 0, 0);
                        self.emit(Opcode::ArrayAppend, meta_reg, async_reg, 0);
                        self.free_temp(async_reg);

                        let key_idx =
                            self.add_constant(Value::String(std::rc::Rc::new(prop.name.clone())));
                        let key_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, key_reg, key_idx, 0);
                        self.emit(Opcode::SetIndex, meta_reg, methods_reg, key_reg);
                        self.free_temp(key_reg);
                        self.free_temp(meta_reg);
                    }
                }

                let interface_reg = self.registers.allocate()?;
                let name_idx = self.add_constant(Value::String(std::rc::Rc::new(name.clone())));
                self.emit(
                    Opcode::DefineInterface,
                    interface_reg,
                    name_idx,
                    methods_reg,
                );

                if !generics.is_empty() {
                    let params_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, params_reg, 0, 0);

                    let constraints_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewMap, constraints_reg, 0, 0);

                    for g in generics {
                        let name_idx =
                            self.add_constant(Value::String(std::rc::Rc::new(g.name.clone())));
                        let name_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                        self.emit(Opcode::ArrayAppend, params_reg, name_reg, 0);

                        if !g.constraints.is_empty() {
                            let cons_list_reg = self.registers.allocate()?;
                            self.emit(Opcode::NewArray, cons_list_reg, 0, 0);
                            for constraint in &g.constraints {
                                let c_idx = self.add_constant(Value::String(std::rc::Rc::new(
                                    constraint.clone(),
                                )));
                                let c_reg = self.registers.allocate()?;
                                self.emit(Opcode::LoadConst, c_reg, c_idx, 0);
                                self.emit(Opcode::ArrayAppend, cons_list_reg, c_reg, 0);
                                self.free_temp(c_reg);
                            }
                            // Reuse name_reg (which has the param name) as key
                            self.emit(Opcode::SetIndex, cons_list_reg, constraints_reg, name_reg);
                            self.free_temp(cons_list_reg);
                        }

                        self.free_temp(name_reg);
                    }
                    self.emit(
                        Opcode::SetTypeParams,
                        interface_reg,
                        params_reg,
                        constraints_reg,
                    );
                    self.free_temp(params_reg);
                    self.free_temp(constraints_reg);
                }

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
            StmtKind::MacroDecl {
                name, params, body, ..
            } => {
                self.macros.insert(
                    name.clone(),
                    MacroDef {
                        params: params.clone(),
                        body: (**body).clone(),
                    },
                );
                Ok(())
            }
        }
    }

    fn try_fold_expr(&self, expr: &Expr) -> Option<Value> {
        match &expr.kind {
            ExprKind::Literal(lit) => Some(match lit {
                Literal::Null => Value::Null,
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Int(i) => Value::Int(*i),
                Literal::Float(f) => Value::Float(*f),
                Literal::String(s) => Value::String(s.clone().into()),
                Literal::Char(c) => Value::Char(*c),
            }),
            ExprKind::ArrayLiteral(items) => {
                let mut values = Vec::new();
                for item in items {
                    values.push(self.try_fold_expr(item)?);
                }
                Some(Value::Array(GcRc::new(ArrayState {
                    elements: values,
                    is_immutable: false,
                })))
            }
            ExprKind::MapLiteral(entries) => {
                let mut map = IndexMap::new();
                for item in entries {
                    match item {
                        MapItem::Entry(key, value_expr) => {
                            let value = self.try_fold_expr(value_expr)?;
                            map.insert(key.clone(), value);
                        }
                        MapItem::Spread(_) => return None,
                    }
                }
                Some(Value::Map(GcRc::new(MapState {
                    entries: map,
                    is_immutable: false,
                })))
            }
            ExprKind::Binary { left, op, right } => {
                let l_val = self.try_fold_expr(left)?;
                let r_val = self.try_fold_expr(right)?;

                match (op, l_val, r_val) {
                    (BinaryOp::Add, Value::Int(a), Value::Int(b)) => {
                        Some(Value::Int(a.wrapping_add(b)))
                    }
                    (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Some(Value::Float(a + b)),
                    (BinaryOp::Subtract, Value::Int(a), Value::Int(b)) => {
                        Some(Value::Int(a.wrapping_sub(b)))
                    }
                    (BinaryOp::Subtract, Value::Float(a), Value::Float(b)) => {
                        Some(Value::Float(a - b))
                    }
                    (BinaryOp::Multiply, Value::Int(a), Value::Int(b)) => {
                        Some(Value::Int(a.wrapping_mul(b)))
                    }
                    (BinaryOp::Multiply, Value::Float(a), Value::Float(b)) => {
                        Some(Value::Float(a * b))
                    }
                    (BinaryOp::Divide, Value::Int(a), Value::Int(b)) => {
                        if b == 0 {
                            None
                        } else {
                            Some(Value::Int(a.wrapping_div(b)))
                        }
                    }
                    (BinaryOp::Divide, Value::Float(a), Value::Float(b)) => {
                        Some(Value::Float(a / b))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        self.current_span = expr.span;

        // Try constant folding (skip for simple literals as they are handled efficiently below)
        if !matches!(expr.kind, ExprKind::Literal(_))
            && let Some(folded) = self.try_fold_expr(expr)
        {
            let reg = self.registers.allocate()?;
            match folded {
                Value::Null => self.emit(Opcode::LoadNull, reg, 0, 0),
                Value::Bool(true) => self.emit(Opcode::LoadTrue, reg, 0, 0),
                Value::Bool(false) => self.emit(Opcode::LoadFalse, reg, 0, 0),
                Value::Int(val) if (-128..=127).contains(&val) => {
                    self.emit(Opcode::LoadInt, reg, val as u8, 0);
                }
                _ => {
                    let const_idx = self.add_constant(folded);
                    self.emit(Opcode::LoadConst, reg, const_idx, 0);
                }
            }
            return Ok(reg);
        }

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
                        let const_idx = self.add_constant(Value::Char(*value));
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
            ExprKind::Unary { op, expr } => match op {
                pleno_ast::UnaryOp::PreIncrement => self.compile_increment(expr, true, true),
                pleno_ast::UnaryOp::PostIncrement => self.compile_increment(expr, false, true),
                pleno_ast::UnaryOp::PreDecrement => self.compile_increment(expr, true, false),
                pleno_ast::UnaryOp::PostDecrement => self.compile_increment(expr, false, false),
                _ => {
                    let value_reg = self.compile_expr(expr)?;
                    let dest = self.registers.allocate()?;
                    let opcode = match op {
                        pleno_ast::UnaryOp::Negate => Opcode::Neg,
                        pleno_ast::UnaryOp::Not => Opcode::Not,
                        _ => unreachable!(),
                    };
                    self.emit(opcode, dest, value_reg, 0);
                    self.free_temp(value_reg);
                    Ok(dest)
                }
            },
            ExprKind::Ident(name) => {
                if let Some(local) = self.resolve_local(name) {
                    if self
                        .ownership_states
                        .get(&local.reg)
                        .is_some_and(|state| matches!(state, OwnershipState::Moved))
                    {
                        return Err(self.error("use of moved value", expr.span));
                    }
                    let reg = if local.is_boxed {
                        let dest = self.registers.allocate()?;
                        self.emit(Opcode::LoadCell, dest, local.reg, 0);
                        dest
                    } else {
                        local.reg
                    };
                    if self.lazy_locals.contains_key(&local.reg) {
                        let dest = self.registers.allocate()?;
                        self.emit(Opcode::ForceThunk, dest, reg, 0);
                        if local.is_boxed {
                            self.free_temp(reg);
                        }
                        Ok(dest)
                    } else {
                        Ok(reg)
                    }
                } else if let Some((index, is_boxed)) = self.captures.get(name).cloned() {
                    let dest = self.registers.allocate()?;
                    self.emit(Opcode::LoadCapture, dest, index as u8, 0);
                    let val_reg = if is_boxed {
                        let val = self.registers.allocate()?;
                        self.emit(Opcode::LoadCell, val, dest, 0);
                        self.free_temp(dest);
                        val
                    } else {
                        dest
                    };
                    let forced = self.registers.allocate()?;
                    self.emit(Opcode::ForceThunk, forced, val_reg, 0);
                    self.free_temp(val_reg);
                    Ok(forced)
                } else {
                    let dest = self.registers.allocate()?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::GetGlobal, dest, name_idx, 0);
                    let forced = self.registers.allocate()?;
                    self.emit(Opcode::ForceThunk, forced, dest, 0);
                    self.free_temp(dest);
                    Ok(forced)
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
                        let type_reg = self.emit_reified_type(&ty_name)?;

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
            ExprKind::Specialize { left, type_args } => {
                let left_reg = self.compile_expr(left)?;
                let args_reg = self.registers.allocate()?;
                self.emit(Opcode::NewArray, args_reg, 0, 0);

                for ty_name in type_args {
                    let type_reg = self.emit_reified_type(ty_name)?;
                    self.emit(Opcode::ArrayAppend, args_reg, type_reg, 0);
                    self.free_temp(type_reg);
                }

                let dest = self.registers.allocate()?;
                self.emit(Opcode::Specialize, dest, left_reg, args_reg);
                self.free_temp(left_reg);
                self.free_temp(args_reg);
                Ok(dest)
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                annotations,
            } => {
                let is_pure = annotations.iter().any(|a| a.name == "pure");
                Ok(self.compile_lambda(params, body, return_ty.as_deref(), is_pure, annotations)?)
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
                self.emit(Opcode::NewTuple, dest, 0, 0);
                for (name, value_expr) in entries {
                    let value_reg = self.compile_expr(value_expr)?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::TupleNamedAppend, dest, value_reg, name_idx);
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
                self.emit(Opcode::NewTuple, dest, 0, 0);
                for item in items {
                    let value_reg = self.compile_expr(item)?;
                    self.emit(Opcode::TupleAppend, dest, value_reg, 0);
                    self.free_temp(value_reg);
                }
                Ok(dest)
            }
            ExprKind::Block(stmts) => {
                let boxed = self.find_boxed_vars(stmts);
                self.boxed_vars.push(boxed);
                self.push_scope();
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
                self.boxed_vars.pop();
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
            ExprKind::Spread(_) => Err(self.error(
                "spread operator only allowed in collection literals",
                expr.span,
            )),
            ExprKind::Call { callee, args } => {
                if let ExprKind::Ident(name) = &callee.kind
                    && let Some(decl) = self.resolve_inline_function(name).cloned()
                {
                    let dest = self.registers.allocate()?;
                    self.compile_inlined_call(&decl, args, dest, expr.span)?;
                    return Ok(dest);
                }
                if let Some(params) = match &callee.kind {
                    ExprKind::Ident(name) => self.resolve_fn_signature(name),
                    _ => None,
                } {
                    let params = params.to_vec();
                    self.enforce_call_ownership(&params, args, expr.span)?;
                }
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
                                return Err(self.error(
                                    "super() does not support named/placeholder args",
                                    expr.span,
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
                let dest = self.registers.allocate()?;
                self.compile_call_internal(callee_reg, args, dest, expr.span)?;
                self.free_temp(callee_reg);
                Ok(dest)
            }
            ExprKind::SafeCall { callee, args } => {
                let callee_reg = self.compile_expr(callee)?;
                let dest = self.registers.allocate()?;

                let null_check_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, null_check_reg, 0, 0);
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, is_null_reg, callee_reg, null_check_reg);
                self.free_temp(null_check_reg);

                let jump_to_end = self.emit_jump(Opcode::JumpIf, is_null_reg);
                self.free_temp(is_null_reg);

                self.compile_call_internal(callee_reg, args, dest, expr.span)?;

                let jump_past_null = self.emit_jump(Opcode::Jump, 0);
                self.patch_jump(jump_to_end)?;
                self.emit(Opcode::LoadNull, dest, 0, 0);
                self.patch_jump(jump_past_null)?;

                self.free_temp(callee_reg);
                Ok(dest)
            }
            ExprKind::AssertNotNull(inner) => {
                let val_reg = self.compile_expr(inner)?;
                let dest = self.registers.allocate()?;
                self.emit(Opcode::AssertNotNull, dest, val_reg, 0);
                self.free_temp(val_reg);
                Ok(dest)
            }
            ExprKind::Match { expr, arms } => {
                let value_reg = self.compile_expr(expr)?;
                let dest = self.registers.allocate()?;
                let mut end_jumps = Vec::new();

                for arm in arms {
                    self.push_scope();
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

                    self.pop_scope();

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
            ExprKind::MacroCall { name, args } => {
                let macro_def = self
                    .macros
                    .get(name)
                    .ok_or_else(|| self.error(&format!("macro not found: {}", name), expr.span))?
                    .clone();
                if args.len() != macro_def.params.len() {
                    return Err(self.error("macro argument count mismatch", expr.span));
                }
                let mut substitutions: HashMap<String, pleno_ast::Expr> = HashMap::new();
                for (param, arg) in macro_def.params.iter().zip(args.iter()) {
                    if let pleno_ast::CallArg::Positional(arg_expr) = arg {
                        substitutions.insert(param.clone(), arg_expr.clone());
                    } else {
                        return Err(
                            self.error("macro only supports positional arguments", expr.span)
                        );
                    }
                }
                let mut expanded_body = macro_def.body.clone();
                self.expand_stmt(&mut expanded_body, &substitutions);
                self.compile_stmt(&expanded_body)?;
                Ok(self.last_expr_register.unwrap_or_else(|| {
                    let reg = self.registers.allocate().unwrap();
                    self.emit(Opcode::LoadNull, reg, 0, 0);
                    reg
                }))
            }
            ExprKind::MacroSplice(_) => {
                Err(self.error("macro splice is not implemented", expr.span))
            }
            ExprKind::InterpolatedString(segments) => {
                let reg = self.registers.allocate()?;
                let const_idx = self.add_constant(Value::String(Rc::new("".into())));
                self.emit(Opcode::LoadConst, reg, const_idx, 0);

                let mut current_reg = reg;
                for segment in segments {
                    let seg_reg = self.compile_expr(segment)?;
                    let next_reg = self.registers.allocate()?;
                    self.emit(Opcode::Add, next_reg, current_reg, seg_reg);
                    self.free_temp(current_reg);
                    self.free_temp(seg_reg);
                    current_reg = next_reg;
                }
                Ok(current_reg)
            }
        }
    }

    fn add_constant(&mut self, value: Value) -> u8 {
        for (idx, constant) in self.chunk.constants.iter().enumerate() {
            if constant == &value {
                return idx as u8;
            }
        }
        self.chunk.constants.push(value);
        let idx = self.chunk.constants.len() - 1;
        if idx > 255 {
            panic!("too many constants in function (max 255)");
        }
        idx as u8
    }

    fn emit(&mut self, opcode: Opcode, a: u8, b: u8, c: u8) {
        if opcode == Opcode::Move && a == b {
            return;
        }
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
        let offset =
            i16::try_from(offset).map_err(|_| self.error("loop too large", self.current_span))?;
        let (b, c) = encode_offset(offset);
        self.emit(Opcode::Jump, 0, b, c);
        Ok(())
    }

    fn emit_loop_if(&mut self, loop_start: usize, reg: u8) -> Result<(), CompileError> {
        let offset = (loop_start as isize) - (self.chunk.code.len() as isize + 1);
        let offset =
            i16::try_from(offset).map_err(|_| self.error("loop too large", self.current_span))?;
        let (b, c) = encode_offset(offset);
        self.emit(Opcode::JumpIf, reg, b, c);
        Ok(())
    }

    fn patch_jump(&mut self, jump_index: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = self.chunk.code.len() as isize;
        let offset = jump_to - jump_from;
        let offset =
            i16::try_from(offset).map_err(|_| self.error("jump too large", self.current_span))?;
        let (b, c) = encode_offset(offset);
        let instruction = self.chunk.code[jump_index];
        self.chunk.code[jump_index] = Instruction::new(instruction.opcode(), instruction.a(), b, c);
        Ok(())
    }

    fn patch_jump_to(&mut self, jump_index: usize, jump_to: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        let jump_to = jump_to as isize;
        let offset = jump_to - jump_from;
        let offset =
            i16::try_from(offset).map_err(|_| self.error("jump too large", self.current_span))?;
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

    fn compile_increment(
        &mut self,
        expr: &Expr,
        is_pre: bool,
        is_inc: bool,
    ) -> Result<u8, CompileError> {
        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);

        match &expr.kind {
            ExprKind::Ident(name) => {
                let return_reg = self.registers.allocate()?;

                if let Some(local) = self.resolve_local(name) {
                    if !local.mutable {
                        return Err(self.error("cannot increment immutable variable", expr.span));
                    }

                    let current_val = local.reg;
                    if !is_pre {
                        self.emit(Opcode::Move, return_reg, current_val, 0);
                    }

                    if is_inc {
                        self.emit(Opcode::Add, current_val, current_val, one_reg);
                    } else {
                        self.emit(Opcode::Sub, current_val, current_val, one_reg);
                    }

                    if is_pre {
                        self.emit(Opcode::Move, return_reg, current_val, 0);
                    }
                } else if let Some((index, is_boxed)) = self.captures.get(name).cloned() {
                    let temp = self.registers.allocate()?;
                    self.emit(Opcode::LoadCapture, temp, index as u8, 0);
                    let val_in_temp = if is_boxed {
                        let inner = self.registers.allocate()?;
                        self.emit(Opcode::LoadCell, inner, temp, 0);
                        inner
                    } else {
                        temp
                    };
                    let val = self.registers.allocate()?;
                    self.emit(Opcode::ForceThunk, val, val_in_temp, 0);
                    if is_boxed {
                        self.free_temp(val_in_temp);
                    } else {
                        // temp is val_in_temp
                        self.free_temp(temp);
                    }

                    if !is_pre {
                        self.emit(Opcode::Move, return_reg, val, 0);
                    }

                    if is_inc {
                        self.emit(Opcode::Add, val, val, one_reg);
                    } else {
                        self.emit(Opcode::Sub, val, val, one_reg);
                    }

                    if is_pre {
                        self.emit(Opcode::Move, return_reg, val, 0);
                    }

                    if is_boxed {
                        self.emit(Opcode::SetCell, temp, val, 0);
                        self.free_temp(temp);
                    } else {
                        self.emit(Opcode::SetCapture, val, index as u8, 0);
                    }
                    self.free_temp(val);
                } else {
                    return Err(self.error(&format!("undefined variable: {}", name), expr.span));
                }

                self.free_temp(one_reg);
                Ok(return_reg)
            }
            ExprKind::Property { object, name } => {
                let obj_reg = self.compile_expr(object)?;
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                let current_val = self.registers.allocate()?;
                self.emit(Opcode::GetProperty, current_val, obj_reg, name_idx);

                let return_reg = self.registers.allocate()?;
                if !is_pre {
                    self.emit(Opcode::Move, return_reg, current_val, 0);
                }

                if is_inc {
                    self.emit(Opcode::Add, current_val, current_val, one_reg);
                } else {
                    self.emit(Opcode::Sub, current_val, current_val, one_reg);
                }

                if is_pre {
                    self.emit(Opcode::Move, return_reg, current_val, 0);
                }

                self.emit(Opcode::SetProperty, current_val, obj_reg, name_idx);
                self.free_temp(current_val);
                self.free_temp(obj_reg);
                self.free_temp(one_reg);
                Ok(return_reg)
            }
            ExprKind::Index { collection, index } => {
                let coll_reg = self.compile_expr(collection)?;
                let idx_reg = self.compile_expr(index)?;
                let current_val = self.registers.allocate()?;
                self.emit(Opcode::GetIndex, current_val, coll_reg, idx_reg);

                let return_reg = self.registers.allocate()?;
                if !is_pre {
                    self.emit(Opcode::Move, return_reg, current_val, 0);
                }

                if is_inc {
                    self.emit(Opcode::Add, current_val, current_val, one_reg);
                } else {
                    self.emit(Opcode::Sub, current_val, current_val, one_reg);
                }

                if is_pre {
                    self.emit(Opcode::Move, return_reg, current_val, 0);
                }

                self.emit(Opcode::SetIndex, current_val, coll_reg, idx_reg);
                self.free_temp(current_val);
                self.free_temp(idx_reg);
                self.free_temp(coll_reg);
                self.free_temp(one_reg);
                Ok(return_reg)
            }
            _ => Err(self.error("invalid increment target", expr.span)),
        }
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

        self.finally_stack.push(finally_branch.cloned());
        self.compile_stmt(try_branch)?;
        self.finally_stack.pop();

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

            self.push_scope();
            self.finally_stack.push(finally_branch.cloned());
            self.insert_local(catch.name.clone(), error_reg, false, catch.ty.clone())?;
            self.compile_stmt(&catch.body)?;
            self.finally_stack.pop();
            self.pop_scope();

            let jump = self.emit_jump(Opcode::Jump, 0);
            end_jumps.push(jump);

            if let Some(jump) = next_catch_jump {
                self.patch_jump(jump)?;
            }
        }

        // If no catch blocks matched, or if we fell through all of them
        // If no catch blocks matched, or if we fell through all of them
        if !catch_branches.is_empty() {
            // Re-throw if none of the catch blocks matched
            if let Some(finally) = finally_branch {
                self.compile_stmt(finally)?;
            }
            self.emit(Opcode::Throw, error_reg, 0, 0);
        } else if finally_branch.is_none() {
            // This case should be caught by parser (at least one catch or finally)
            // but just in case, re-throw.
            self.emit(Opcode::Throw, error_reg, 0, 0);
        } else {
            if let Some(finally) = finally_branch {
                self.compile_stmt(finally)?;
            }
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
            if names.len() != 1 {
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

    fn compile_call_internal(
        &mut self,
        callee_reg: u8,
        args: &[pleno_ast::CallArg],
        dest: u8,
        span: Span,
    ) -> Result<(), CompileError> {
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
                        return Err(
                            self.error("named arguments not supported in partials yet", span)
                        );
                    }
                }
            }
            self.emit(Opcode::Partial, dest, callee_reg, args_reg);
            self.free_temp(args_reg);
            return Ok(());
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
                            return Err(self.error("duplicate named argument", span));
                        }
                        let arg_reg = self.compile_expr(expr)?;
                        let name_idx = self.add_constant(Value::String(name.clone().into()));
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
            self.emit(Opcode::CallNamed, dest, callee_reg, wrapper_reg);
            self.free_temp(positional_reg);
            self.free_temp(named_reg);
            self.free_temp(wrapper_reg);
            Ok(())
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
                        return Err(self.error("named arguments require name", span));
                    }
                    pleno_ast::CallArg::Placeholder => unreachable!(),
                }
            }
            self.emit(Opcode::Call, dest, callee_reg, args_reg);
            self.free_temp(args_reg);
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
        self.push_scope();
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
            .ok_or_else(|| self.error("loop stack underflow", self.current_span))?;
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
        self.push_scope();
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
            .ok_or_else(|| self.error("loop stack underflow", self.current_span))?;
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
        self.push_scope();
        let source_reg = self.compile_expr(iterable)?;
        let iter_reg = self.registers.allocate()?;

        let is_entries = names.len() > 1;
        self.emit(
            Opcode::GetIterator,
            iter_reg,
            source_reg,
            if is_entries { 1 } else { 0 },
        );
        self.free_temp(source_reg);

        let iter_name = format!("$for_iterator_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(iter_name, iter_reg, false, None)?;

        let missing_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadMissing, missing_reg, 0, 0);

        let loop_start = self.chunk.code.len();
        let item_reg = self.registers.allocate()?;
        self.emit(Opcode::IterNext, item_reg, iter_reg, 0);
        if self.is_async {
            self.emit(Opcode::Await, item_reg, item_reg, 0);
        }

        let is_done_reg = self.registers.allocate()?;
        self.emit(Opcode::Eq, is_done_reg, item_reg, missing_reg);
        let jump_to_end = self.emit_jump(Opcode::JumpIf, is_done_reg);
        self.free_temp(is_done_reg);

        if names.len() == 1 {
            self.insert_local(names[0].clone(), item_reg, true, None)?;
        } else {
            for (i, name) in names.iter().enumerate() {
                let val_reg = self.registers.allocate()?;
                let idx_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadInt, idx_reg, i as u8, 0);
                self.emit(Opcode::GetIndex, val_reg, item_reg, idx_reg);
                self.free_temp(idx_reg);
                self.insert_local(name.clone(), val_reg, true, None)?;
            }
            self.free_temp(item_reg);
        }

        self.push_loop(None);
        self.compile_stmt(body)?;

        let continue_target = self.chunk.code.len();
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.continue_target = Some(continue_target);
        }

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;

        let loop_end = self.chunk.code.len();
        let loop_ctx = self
            .pop_loop()
            .ok_or_else(|| self.error("loop stack underflow", self.current_span))?;
        self.patch_loop_jumps(loop_ctx, loop_end)?;

        self.free_temp(missing_reg);
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
                // Check if Tuple or Array
                let tuple_str_idx = self.add_constant(Value::String(Rc::new("Tuple".to_string())));
                let tuple_str_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, tuple_str_reg, tuple_str_idx, 0);

                let array_str_idx = self.add_constant(Value::String(Rc::new("Array".to_string())));
                let array_str_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, array_str_reg, array_str_idx, 0);

                let is_tuple_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_tuple_reg, value_reg, tuple_str_reg);

                let is_array_reg = self.registers.allocate()?;
                self.emit(Opcode::Is, is_array_reg, value_reg, array_str_reg);

                let is_either_reg = self.registers.allocate()?;
                self.emit(Opcode::Or, is_either_reg, is_tuple_reg, is_array_reg);

                self.free_temp(tuple_str_reg);
                self.free_temp(array_str_reg);
                self.free_temp(is_tuple_reg);
                self.free_temp(is_array_reg);

                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_either_reg));
                self.free_temp(is_either_reg);

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
                let name_idx = self.add_constant(Value::String(Rc::new(name.clone())));
                let is_match_reg = self.registers.allocate()?;
                self.emit(Opcode::MatchVariant, is_match_reg, value_reg, name_idx);
                fail_jumps.push(self.emit_jump(Opcode::JumpIfNot, is_match_reg));
                self.free_temp(is_match_reg);

                for (i, p) in patterns.iter().enumerate() {
                    let field_reg = self.registers.allocate()?;
                    self.emit(Opcode::GetVariantField, field_reg, value_reg, i as u8);
                    self.compile_pattern(p, field_reg, fail_jumps)?;
                    self.free_temp(field_reg);
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

    fn compile_annotation(
        &mut self,
        annotation: &pleno_ast::Annotation,
    ) -> Result<RuntimeAnnotation, CompileError> {
        let mut args = Vec::new();
        for arg in &annotation.args {
            if let ExprKind::Literal(lit) = &arg.kind {
                let val = match lit {
                    Literal::Int(i) => Value::Int(*i),
                    Literal::Float(f) => Value::Float(*f),
                    Literal::String(s) => Value::String(s.clone().into()),
                    Literal::Bool(b) => Value::Bool(*b),
                    Literal::Null => Value::Null,
                    Literal::Char(c) => Value::Char(*c),
                };
                args.push(val);
            } else {
                return Err(self.error("annotation arguments must be literals", arg.span));
            }
        }
        Ok(RuntimeAnnotation {
            name: annotation.name.clone(),
            args,
        })
    }

    fn emit_annotation_creation(
        &mut self,
        annotation: &RuntimeAnnotation,
    ) -> Result<u8, CompileError> {
        // Create [name, args] object
        // For simplicity, we can create a simple object or Map.
        // VM expects Object with "name" and "args" fields.
        // Let's use Map for now?
        // No, SetAnnotations code I wrote checks: `if let Value::Object(obj) = val`.
        // And `Value::Object` wraps `Object` struct.
        // `Object` struct has `class` field.
        // If we don't have an Annotation class, we can't create Value::Object easily without a class.
        // Wait, I should change SetAnnotations in VM to accept Map OR Object.
        // OR I can use `Value::Map`.

        // Let's change SetAnnotations in VM to simple check for map-like structure or just change it to match what we can emit here.
        // Since we don't have "Annotation" class defined in stdlib yet (maybe we should?),
        // using Maps is easier for internal representation.

        // But for now, let's emit code to create a Map: { "name": ..., "args": ... }
        // And I will update VM SetAnnotations to expect Map.

        let obj_reg = self.registers.allocate()?;
        // Emit Map
        self.emit(Opcode::NewMap, obj_reg, 0, 0);

        // Set "name"
        let name_key_idx = self.add_constant(Value::String(Rc::new("name".to_string())));
        let name_val_idx = self.add_constant(Value::String(Rc::new(annotation.name.clone())));
        let val_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, val_reg, name_val_idx, 0);
        self.emit(Opcode::SetProperty, val_reg, obj_reg, name_key_idx);
        self.free_temp(val_reg);

        // Set "args"
        let args_key_idx = self.add_constant(Value::String(Rc::new("args".to_string())));
        let args_reg = self.registers.allocate()?;
        self.emit(Opcode::NewArray, args_reg, 0, 0);

        for arg in &annotation.args {
            let arg_reg = self.registers.allocate()?;
            let arg_idx = self.add_constant(arg.clone());
            self.emit(Opcode::LoadConst, arg_reg, arg_idx, 0);
            self.emit(Opcode::ArrayAppend, args_reg, arg_reg, 0);
            self.free_temp(arg_reg);
        }
        self.emit(Opcode::SetProperty, args_reg, obj_reg, args_key_idx);
        self.free_temp(args_reg);

        Ok(obj_reg)
    }

    fn compile_function(
        &mut self,
        decl: &FunctionDecl,
        is_method: bool,
        store_as_function: bool,
        current_super: Option<String>,
        instance_fields: Option<&[FieldDecl]>,
    ) -> Result<u8, CompileError> {
        let captures = self.analyze_captures(decl, is_method);
        let mut compiler = Compiler::new(String::new());
        compiler.is_top_level = false;
        compiler.is_async = decl.is_async;
        compiler.push_scope();
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, info)| (info.name.clone(), (idx, info.is_boxed)))
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
                    .ok_or_else(|| compiler.error("unknown parameter", decl.span))?
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

            // Type Enforcement
            if let Some(ty_name) = &param.ty
                && ty_name != "Any"
            {
                let param_reg = compiler
                    .resolve_local(&param.name)
                    .ok_or_else(|| compiler.error("unknown parameter", decl.span))?
                    .reg;
                let type_reg = compiler.registers.allocate()?;
                let ty_idx = compiler.add_constant(Value::String(Rc::new(ty_name.clone())));
                compiler.emit(Opcode::GetGlobal, type_reg, ty_idx, 0);

                let result_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::Is, result_reg, param_reg, type_reg);

                let skip_jump = compiler.emit_jump(Opcode::JumpIf, result_reg);

                let msg = format!(
                    "Type error: parameter '{}' expected {}, got ",
                    param.name, ty_name
                );
                let msg_reg = compiler.registers.allocate()?;
                let msg_const = compiler.add_constant(Value::String(Rc::new(msg)));
                compiler.emit(Opcode::LoadConst, msg_reg, msg_const, 0);

                // Add current value's type to the message for better debugging
                let type_name_reg = compiler.registers.allocate()?;
                compiler.emit(Opcode::TypeOf, type_name_reg, param_reg, 0);
                compiler.emit(Opcode::Add, msg_reg, msg_reg, type_name_reg);

                compiler.emit(Opcode::Throw, msg_reg, 0, 0);

                compiler.patch_jump(skip_jump)?;

                compiler.free_temp(type_reg);
                compiler.free_temp(result_reg);
                compiler.free_temp(msg_reg);
                compiler.free_temp(type_name_reg);
            }
        }

        if let Some(fields) = instance_fields {
            let this_reg = compiler
                .resolve_local("this")
                .ok_or_else(|| compiler.error("'this' not found in constructor", decl.span))?
                .reg;
            for field in fields {
                if !field.is_static
                    && let Some(default) = &field.default
                {
                    let value_reg = compiler.compile_expr(default)?;
                    let name_idx = compiler.add_constant(Value::String(field.name.clone().into()));
                    compiler.emit(Opcode::SetProperty, value_reg, this_reg, name_idx);
                    compiler.free_temp(value_reg);
                }
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
                    (false, false, false) // 'this' has no default, not variadic, not keyword-only
                } else if let Some(param) = decl.params.iter().find(|param| param.name == *name) {
                    (
                        param.default.is_some(),
                        param.is_variadic,
                        param.is_keyword_only,
                    )
                } else {
                    // This case should ideally not be reached if 'params' correctly reflects 'decl.params'
                    // and 'this' is handled. Fallback to default.
                    (false, false, false)
                };
                ParamSpec {
                    name: name.clone(),
                    has_default,
                    is_variadic,
                    is_keyword_only,
                    ownership: if name == "this" {
                        pleno_ast::Ownership::None
                    } else {
                        decl.params
                            .iter()
                            .find(|param| param.name == *name)
                            .map(|param| param.ownership.clone())
                            .unwrap_or(pleno_ast::Ownership::None)
                    },
                }
            })
            .collect();

        let is_pure = decl.annotations.iter().any(|a| a.name == "pure");
        let mut runtime_annotations = Vec::new();
        for anno in &decl.annotations {
            if anno.name != "pure" && anno.name != "native" {
                match self.compile_annotation(anno) {
                    Ok(a) => runtime_annotations.push(a),
                    Err(e) => return Err(e),
                }
            }
        }

        let mut type_constraints = IndexMap::new();
        for g in &decl.generics {
            if !g.constraints.is_empty() {
                type_constraints.insert(g.name.clone(), g.constraints.clone());
            }
        }

        self.chunk.functions.push(FunctionProto {
            name: decl.name.clone(),
            chunk: compiler.chunk.clone(),
            arity,
            params: param_specs,
            is_async: decl.is_async,
            is_pure,
            is_inline: decl.annotations.iter().any(|a| a.name == "inline"),
            is_hot: decl.annotations.iter().any(|a| a.name == "hot"),
            no_optimize: decl.annotations.iter().any(|a| a.name == "no_optimize"),
            type_params: decl.generics.iter().map(|g| g.name.clone()).collect(),
            type_constraints,
            annotations: runtime_annotations,
        });
        let dest = self.registers.allocate()?;
        if store_as_function && captures.is_empty() {
            self.emit(Opcode::FunctionRef, dest, func_index as u8, 0);
        } else {
            let capture_count = captures.len() as u8;
            self.emit(Opcode::Closure, dest, func_index as u8, capture_count);
            for (idx, info) in captures.iter().enumerate() {
                let reg = if let Some(local) = self.resolve_local(&info.name) {
                    local.reg
                } else if let Some((capture_idx, _)) = self.captures.get(&info.name) {
                    let temp = self.registers.allocate()?;
                    self.emit(Opcode::LoadCapture, temp, *capture_idx as u8, 0);
                    temp
                } else {
                    return Err(self.error("unknown capture", self.current_span));
                };
                self.emit(Opcode::Capture, dest, reg, idx as u8);
                if !self.is_local_reg(reg) {
                    self.free_temp(reg);
                }
            }
        }

        // Apply decorators in reverse order
        for annotation in decl.annotations.iter().rev() {
            if annotation.name == "pure"
                || annotation.name == "native"
                || annotation.name == "inline"
                || annotation.name == "hot"
                || annotation.name == "no_optimize"
            {
                continue;
            }

            // Compile decorator expression
            // If no args: load(name)
            // If args: call(name, args) -> returns decorator function
            let decorator_reg = if annotation.args.is_empty() {
                let expr = pleno_ast::Expr {
                    kind: pleno_ast::ExprKind::Ident(annotation.name.clone()),
                    span: pleno_ast::Span::new(0, 0),
                };
                self.compile_expr(&expr)?
            } else {
                // Call the decorator factory
                let callee = pleno_ast::Expr {
                    kind: pleno_ast::ExprKind::Ident(annotation.name.clone()),
                    span: pleno_ast::Span::new(0, 0),
                };
                let args: Vec<pleno_ast::CallArg> = annotation
                    .args
                    .iter()
                    .map(|arg| pleno_ast::CallArg::Positional(arg.clone()))
                    .collect();

                let expr = pleno_ast::Expr {
                    kind: pleno_ast::ExprKind::Call {
                        callee: Box::new(callee),
                        args,
                    },
                    span: pleno_ast::Span::new(0, 0),
                };
                self.compile_expr(&expr)?
            };

            // Now call the decorator with the function/class as argument
            // decorated = decorator(target)
            let args_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, args_reg, 0, 0);
            self.emit(Opcode::ArrayAppend, args_reg, dest, 0);

            let new_dest = self.registers.allocate()?;
            self.emit(Opcode::Call, new_dest, decorator_reg, args_reg);

            self.free_temp(args_reg);
            self.free_temp(decorator_reg);

            // Move result back to dest
            self.emit(Opcode::Move, dest, new_dest, 0);
            self.free_temp(new_dest);
        }

        compiler.pop_scope();
        Ok(dest)
    }

    fn compile_lambda(
        &mut self,
        params: &[pleno_ast::Param],
        body: &pleno_ast::Expr,
        return_ty: Option<&str>,
        is_pure: bool,
        annotations: &[pleno_ast::Annotation],
    ) -> Result<u8, CompileError> {
        let body_stmt = match &body.kind {
            ExprKind::Block(stmts) => Stmt {
                kind: StmtKind::Block(stmts.clone()),
                span: body.span,
                leading_blank_lines: 0,
                trailing_comment: None,
            },
            _ => Stmt {
                kind: StmtKind::Return(Some((*body).clone())),
                span: body.span,
                leading_blank_lines: 0,
                trailing_comment: None,
            },
        };
        let decl = FunctionDecl {
            span: Span::new(0, 0),
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
            doc: None,
        };

        let captures = self.analyze_captures(&decl, false);
        let mut compiler = Compiler::new(self.source.clone());
        compiler.is_top_level = false;
        compiler.push_scope();
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, info)| (info.name.clone(), (idx, info.is_boxed)))
            .collect();

        for param in params {
            let reg = compiler.registers.allocate()?;
            compiler.insert_local(param.name.clone(), reg, true, param.ty.clone())?;
        }

        for param in params.iter() {
            if let Some(default) = &param.default {
                let param_reg = compiler
                    .resolve_local(&param.name)
                    .ok_or_else(|| compiler.error("unknown parameter", param.span))?
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
                ownership: p.ownership.clone(),
            })
            .collect();

        let mut runtime_annotations = Vec::new();
        for anno in annotations {
            if anno.name != "pure"
                && anno.name != "native"
                && let Ok(a) = self.compile_annotation(anno)
            {
                runtime_annotations.push(a);
            }
        }
        self.chunk.functions.push(FunctionProto {
            name: "<lambda>".to_string(),
            chunk: compiler.chunk.clone(),
            arity,
            params: param_specs,
            is_async: false,
            is_pure,
            is_inline: false,
            is_hot: false,
            no_optimize: false,
            type_params: Vec::new(),
            type_constraints: IndexMap::new(),
            annotations: runtime_annotations,
        });

        let dest = self.registers.allocate()?;
        let capture_count = captures.len() as u8;
        self.emit(Opcode::Closure, dest, func_index as u8, capture_count);
        for (idx, info) in captures.iter().enumerate() {
            let reg = if let Some(local) = self.resolve_local(&info.name) {
                local.reg
            } else if let Some((capture_idx, _)) = self.captures.get(&info.name) {
                let temp = self.registers.allocate()?;
                self.emit(Opcode::LoadCapture, temp, *capture_idx as u8, 0);
                temp
            } else {
                return Err(self.error("unknown capture", self.current_span));
            };
            self.emit(Opcode::Capture, dest, reg, idx as u8);
            if !self.is_local_reg(reg) {
                self.free_temp(reg);
            }
        }
        compiler.pop_scope();
        Ok(dest)
    }

    fn analyze_captures(&self, decl: &FunctionDecl, is_method: bool) -> Vec<CaptureInfo> {
        let mut locals_map = HashMap::new();
        let mut scope_stack = vec![HashSet::new()];

        for param in &decl.params {
            locals_map.insert(param.name.clone(), ());
            scope_stack[0].insert(param.name.clone());
        }
        if is_method {
            locals_map.insert("this".to_string(), ());
            scope_stack[0].insert("this".to_string());
        }

        self.collect_locals(&decl.body, &mut locals_map);
        for name in locals_map.keys() {
            scope_stack[0].insert(name.clone());
        }

        let mut used = HashMap::new();
        for param in &decl.params {
            if let Some(default) = &param.default {
                self.collect_idents_expr(default, &mut used, &mut scope_stack);
            }
        }
        self.collect_idents(&decl.body, &mut used, &mut scope_stack);

        let mut captures = Vec::new();
        for name in used.keys() {
            if !locals_map.contains_key(name) {
                if let Some(local) = self.resolve_local(name) {
                    captures.push(CaptureInfo {
                        name: name.clone(),
                        is_boxed: local.is_boxed,
                    });
                } else if let Some((_, is_boxed)) = self.captures.get(name) {
                    captures.push(CaptureInfo {
                        name: name.clone(),
                        is_boxed: *is_boxed,
                    });
                }
            }
        }
        captures
    }

    fn compile_extend_stmt(
        &mut self,
        target: &str,
        _generics: &[pleno_ast::GenericParam],
        methods: &[pleno_ast::FunctionDecl],
    ) -> Result<(), CompileError> {
        let class_reg = self.registers.allocate()?;

        // Resolve target class
        if let Some(local) = self.resolve_local(target) {
            self.emit(Opcode::Move, class_reg, local.reg, 0);
        } else {
            let name_idx = self.add_constant(Value::String(std::rc::Rc::new(target.to_string())));
            self.emit(Opcode::GetGlobal, class_reg, name_idx, 0);
        }

        for method in methods {
            // Extension methods are instance methods, so is_method=true
            // store_as_function=true because we want the function object to add to the class
            let func_reg = self.compile_function(method, true, true, None, None)?;

            let meta_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, meta_reg, 0, 0);
            self.emit(Opcode::ArrayAppend, meta_reg, func_reg, 0);

            let vis_name = visibility_name(method.visibility.as_ref());
            let vis_idx = self.add_constant(Value::String(std::rc::Rc::new(vis_name.to_string())));
            let vis_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, vis_reg, vis_idx, 0);
            self.emit(Opcode::ArrayAppend, meta_reg, vis_reg, 0);
            self.free_temp(vis_reg);

            let name_idx = self.add_constant(Value::String(std::rc::Rc::new(method.name.clone())));
            self.emit(Opcode::AddMethod, meta_reg, class_reg, name_idx);

            self.free_temp(func_reg);
            self.free_temp(meta_reg);
        }

        self.free_temp(class_reg);
        Ok(())
    }

    fn compile_class_declaration(&mut self, stmt: &Stmt) -> Result<u8, CompileError> {
        let (
            name,
            generics,
            fields,
            methods,
            superclass,
            nested_classes,
            properties,
            is_abstract,
            implements,
            annotations,
        ) = match &stmt.kind {
            StmtKind::Class {
                name,
                generics,
                fields,
                methods,
                superclass,
                nested_classes,
                annotations,
                implements,
                is_abstract,
                properties,
                ..
            } => (
                name,
                generics,
                fields,
                methods,
                superclass,
                nested_classes,
                properties,
                *is_abstract,
                implements.clone(),
                annotations,
            ),
            _ => return Err(self.error("expected class statement", self.current_span)),
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
        let operators_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, operators_reg, 0, 0);

        let mut methods_to_compile = methods.clone();
        if !methods_to_compile.iter().any(|m| m.name == "constructor") {
            methods_to_compile.push(FunctionDecl {
                span: Span::new(0, 0),
                name: "constructor".to_string(),
                params: Vec::new(),
                body: Box::new(Stmt::new(StmtKind::Block(Vec::new()), Span::new(0, 0))),
                return_ty: None,
                generics: Vec::new(),
                annotations: Vec::new(),
                is_async: false,
                is_abstract: false,
                visibility: Some(Visibility::Public),
                is_static: false,
                kind: FunctionKind::Method,
                doc: None,
            });
        }

        for method in &methods_to_compile {
            let is_method = !method.is_static;
            let func_reg = self.compile_function(
                method,
                is_method,
                true,
                superclass.as_ref().cloned(),
                if is_method && method.name == "constructor" {
                    Some(fields)
                } else {
                    None
                },
            )?;

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
                FunctionKind::Operator(ref op) => (operators_reg, op.clone()),
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

        if !annotations.is_empty() {
            let annos_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, annos_reg, 0, 0);
            for anno in annotations {
                if anno.name != "native" {
                    // Skip "native" if it is processed elsewhere or just keep it?
                    if let Ok(runtime_anno) = self.compile_annotation(anno)
                        && let Ok(obj_reg) = self.emit_annotation_creation(&runtime_anno)
                    {
                        self.emit(Opcode::ArrayAppend, annos_reg, obj_reg, 0);
                        self.free_temp(obj_reg);
                    }
                }
            }
            self.emit(Opcode::SetAnnotations, class_reg, annos_reg, 0);
            self.free_temp(annos_reg);
        }

        if !generics.is_empty() {
            let params_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, params_reg, 0, 0);

            let constraints_reg = self.registers.allocate()?;
            self.emit(Opcode::NewMap, constraints_reg, 0, 0);

            for g in generics {
                let name_idx = self.add_constant(Value::String(std::rc::Rc::new(g.name.clone())));
                let name_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                self.emit(Opcode::ArrayAppend, params_reg, name_reg, 0);

                if !g.constraints.is_empty() {
                    let cons_list_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, cons_list_reg, 0, 0);
                    for constraint in &g.constraints {
                        let c_idx =
                            self.add_constant(Value::String(std::rc::Rc::new(constraint.clone())));
                        let c_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, c_reg, c_idx, 0);
                        self.emit(Opcode::ArrayAppend, cons_list_reg, c_reg, 0);
                        self.free_temp(c_reg);
                    }
                    // Reuse name_reg (which has the param name) as key
                    self.emit(Opcode::SetIndex, cons_list_reg, constraints_reg, name_reg);
                    self.free_temp(cons_list_reg);
                }

                self.free_temp(name_reg);
            }
            self.emit(
                Opcode::SetTypeParams,
                class_reg,
                params_reg,
                constraints_reg,
            );
            self.free_temp(params_reg);
            self.free_temp(constraints_reg);
        }

        self.emit(Opcode::SetStatic, class_reg, static_reg, 0);
        if is_abstract {
            self.emit(Opcode::SetAbstract, class_reg, 0, 0);
        }
        let is_pooled = annotations.iter().any(|a| a.name == "pooled");
        if is_pooled {
            self.emit(Opcode::SetPooled, class_reg, 1, 0);
        }

        // Compile Properties
        for prop in properties {
            if let Some(getter) = &prop.getter {
                let func_reg = self.compile_function(
                    getter,
                    !prop.is_static,
                    true,
                    superclass.as_ref().cloned(),
                    None,
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
                    None,
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
        self.emit(Opcode::SetOperators, class_reg, operators_reg, 0);

        self.free_temp(getters_reg);
        self.free_temp(setters_reg);
        self.free_temp(static_getters_reg);
        self.free_temp(static_setters_reg);
        self.free_temp(operators_reg);
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
                    let r = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, r, 0, 0);
                    r
                };
                self.emit(Opcode::SetProperty, value_reg, class_reg, field_idx);
                self.emit(
                    Opcode::SetStaticFieldVisibility,
                    class_reg,
                    field_idx,
                    vis_code,
                );
                self.free_temp(value_reg);
            } else {
                self.emit(Opcode::SetFieldVisibility, class_reg, field_idx, vis_code);
            }

            if let Some(ty_name) = &field.ty {
                let ty_idx = self.add_constant(Value::String(Rc::new(ty_name.clone())));
                if field.is_static {
                    self.emit(Opcode::SetStaticFieldType, class_reg, field_idx, ty_idx);
                } else {
                    self.emit(Opcode::SetFieldType, class_reg, field_idx, ty_idx);
                }
            }
        }

        // NOTE: Annotations are now stored as metadata via SetAnnotations opcode.
        // The decorator call pattern (calling annotation as function) is disabled.
        // If decorator functionality is needed in the future, add a separate mechanism.
        // for annotation in annotations.iter().rev() { ... }

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
        // Compile Implements
        if !implements.is_empty() {
            let interfaces_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, interfaces_reg, 0, 0);
            for interface_name in implements {
                let interface_reg = self.registers.allocate()?;

                // Check if this is a generic type (contains '<')
                if let Some(lt_pos) = interface_name.find('<') {
                    // Parse generic: "Container<Int>" -> base="Container", args=["Int"]
                    let base_name = &interface_name[..lt_pos];
                    let args_str = &interface_name[lt_pos + 1..interface_name.len() - 1]; // Strip < and >

                    // Load base interface
                    let base_reg = self.registers.allocate()?;
                    if let Some(local) = self.resolve_local(base_name) {
                        self.emit(Opcode::Move, base_reg, local.reg, 0);
                    } else {
                        let name_idx = self
                            .add_constant(Value::String(std::rc::Rc::new(base_name.to_string())));
                        self.emit(Opcode::GetGlobal, base_reg, name_idx, 0);
                    }

                    // Parse and load type arguments
                    let args_array_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewArray, args_array_reg, 0, 0);

                    // Split args by comma (simple split, doesn't handle nested generics)
                    for arg in args_str.split(',') {
                        let arg = arg.trim();
                        if arg.is_empty() {
                            continue;
                        }
                        let arg_reg = self.registers.allocate()?;
                        // Try to resolve as local, global, or treat as primitive type name
                        if let Some(local) = self.resolve_local(arg) {
                            self.emit(Opcode::Move, arg_reg, local.reg, 0);
                        } else {
                            let arg_idx =
                                self.add_constant(Value::String(std::rc::Rc::new(arg.to_string())));
                            self.emit(Opcode::GetGlobal, arg_reg, arg_idx, 0);
                        }
                        self.emit(Opcode::ArrayAppend, args_array_reg, arg_reg, 0);
                        self.free_temp(arg_reg);
                    }

                    // Emit Specialize to create BoundInterface
                    self.emit(Opcode::Specialize, interface_reg, base_reg, args_array_reg);
                    self.free_temp(base_reg);
                    self.free_temp(args_array_reg);
                } else {
                    // Non-generic interface
                    if let Some(local) = self.resolve_local(&interface_name) {
                        self.emit(Opcode::Move, interface_reg, local.reg, 0);
                    } else {
                        let name_idx = self
                            .add_constant(Value::String(std::rc::Rc::new(interface_name.clone())));
                        self.emit(Opcode::GetGlobal, interface_reg, name_idx, 0);
                    }
                }

                self.emit(Opcode::ArrayAppend, interfaces_reg, interface_reg, 0);
                self.free_temp(interface_reg);
            }
            self.emit(Opcode::SetImplements, class_reg, interfaces_reg, 0);
            self.free_temp(interfaces_reg);
        }

        self.free_temp(methods_reg);
        self.free_temp(static_reg);
        Ok(class_reg)
    }

    fn compile_enum_declaration(&mut self, decl: &pleno_ast::EnumDecl) -> Result<u8, CompileError> {
        let enum_reg = self.registers.allocate()?;
        let name_idx = self.add_constant(Value::String(decl.name.clone().into()));
        self.emit(Opcode::DefineEnum, enum_reg, name_idx, 0);

        for variant in &decl.variants {
            let variant_name_idx = self.add_constant(Value::String(variant.name.clone().into()));
            let param_count = variant.params.as_ref().map_or(0, |p| p.len());
            self.emit(
                Opcode::AddVariant,
                enum_reg,
                variant_name_idx,
                param_count as u8,
            );
        }

        for method in &decl.methods {
            let method_reg = self.compile_function(method, !method.is_static, true, None, None)?;
            let name_idx = self.add_constant(Value::String(method.name.clone().into()));

            if method.is_static {
                self.emit(Opcode::AddStaticMethod, method_reg, enum_reg, name_idx);
            } else {
                self.emit(Opcode::AddMethod, method_reg, enum_reg, name_idx);
            }
            self.free_temp(method_reg);
        }

        Ok(enum_reg)
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

    fn collect_idents(
        &self,
        stmt: &Stmt,
        used: &mut HashMap<String, bool>,
        scope: &mut Vec<HashSet<String>>,
    ) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.collect_idents_expr(expr, used, scope),
            StmtKind::Let {
                name,
                value: Some(value),
                ..
            } => {
                self.collect_idents_expr(value, used, scope);
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }
            }
            StmtKind::Let { name, .. } => {
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }
            }
            StmtKind::Assign { name, value } => {
                let is_shadowed = scope.iter().any(|s| s.contains(name));
                if !is_shadowed {
                    used.insert(name.clone(), true);
                }
                self.collect_idents_expr(value, used, scope);
            }
            StmtKind::AssignProperty { object, value, .. } => {
                self.collect_idents_expr(object, used, scope);
                self.collect_idents_expr(value, used, scope);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                scope.push(HashSet::new());
                if let Some(init) = init {
                    self.collect_idents(init, used, scope);
                }
                if let Some(condition) = condition {
                    self.collect_idents_expr(condition, used, scope);
                }
                if let Some(step) = step {
                    self.collect_idents(step, used, scope);
                }
                self.collect_idents(body, used, scope);
                scope.pop();
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.collect_idents_expr(collection, used, scope);
                self.collect_idents_expr(index, used, scope);
                self.collect_idents_expr(value, used, scope);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_idents_expr(condition, used, scope);
                self.collect_idents(then_branch, used, scope);
                if let Some(else_branch) = else_branch {
                    self.collect_idents(else_branch, used, scope);
                }
            }
            StmtKind::While { condition, body } => {
                self.collect_idents_expr(condition, used, scope);
                self.collect_idents(body, used, scope);
            }
            StmtKind::For {
                iterable,
                body,
                names,
                ..
            } => {
                self.collect_idents_expr(iterable, used, scope);
                scope.push(HashSet::new());
                for name in names {
                    scope.last_mut().unwrap().insert(name.clone());
                }
                self.collect_idents(body, used, scope);
                scope.pop();
            }
            StmtKind::Fn {
                body, params, name, ..
            } => {
                // The function name itself is defined in the current scope
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }

                scope.push(HashSet::new());
                for param in params {
                    if let Some(default) = &param.default {
                        self.collect_idents_expr(default, used, scope);
                    }
                    scope.last_mut().unwrap().insert(param.name.clone());
                }
                self.collect_idents(body, used, scope);
                scope.pop();
            }
            StmtKind::Return(Some(expr)) => {
                self.collect_idents_expr(expr, used, scope);
            }
            StmtKind::Block(items) => {
                scope.push(HashSet::new());
                for item in items {
                    self.collect_idents(item, used, scope);
                }
                scope.pop();
            }
            StmtKind::Class {
                name,
                methods,
                properties,
                fields,
                nested_classes,
                ..
            } => {
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }

                // We don't recurse into fields or nested classes names for simplicity,
                // but we must recurse into method bodies and property bodies.
                for method in methods {
                    scope.push(HashSet::new());
                    scope.last_mut().unwrap().insert("this".to_string());
                    for p in &method.params {
                        if let Some(d) = &p.default {
                            self.collect_idents_expr(d, used, scope);
                        }
                        scope.last_mut().unwrap().insert(p.name.clone());
                    }
                    self.collect_idents(&method.body, used, scope);
                    scope.pop();
                }
                for prop in properties {
                    if let Some(getter) = &prop.getter {
                        scope.push(HashSet::new());
                        scope.last_mut().unwrap().insert("this".to_string());
                        self.collect_idents(&getter.body, used, scope);
                        scope.pop();
                    }
                    if let Some(setter) = &prop.setter {
                        scope.push(HashSet::new());
                        scope.last_mut().unwrap().insert("this".to_string());
                        // Setters usually have one param (the value)
                        for p in &setter.params {
                            scope.last_mut().unwrap().insert(p.name.clone());
                        }
                        self.collect_idents(&setter.body, used, scope);
                        scope.pop();
                    }
                }
                for field in fields {
                    if let Some(default) = &field.default {
                        self.collect_idents_expr(default, used, scope);
                    }
                }
                for nested in nested_classes {
                    self.collect_idents(nested, used, scope);
                }
            }
            _ => {}
        }
    }

    fn collect_idents_expr(
        &self,
        expr: &Expr,
        used: &mut HashMap<String, bool>,
        scope: &mut Vec<HashSet<String>>,
    ) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                let is_shadowed = scope.iter().any(|s| s.contains(name));
                if !is_shadowed {
                    used.entry(name.clone()).or_insert(false);
                }
            }
            ExprKind::Unary { op, expr } => match op {
                pleno_ast::UnaryOp::PreIncrement
                | pleno_ast::UnaryOp::PostIncrement
                | pleno_ast::UnaryOp::PreDecrement
                | pleno_ast::UnaryOp::PostDecrement => {
                    if let ExprKind::Ident(name) = &expr.kind {
                        let is_shadowed = scope.iter().any(|s| s.contains(name));
                        if !is_shadowed {
                            used.insert(name.clone(), true);
                        }
                    }
                }
                _ => self.collect_idents_expr(expr, used, scope),
            },
            ExprKind::Binary { left, right, .. } => {
                self.collect_idents_expr(left, used, scope);
                self.collect_idents_expr(right, used, scope);
            }
            ExprKind::Property { object, .. } => self.collect_idents_expr(object, used, scope),
            ExprKind::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.collect_idents_expr(start, used, scope);
                }
                if let Some(end) = end {
                    self.collect_idents_expr(end, used, scope);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        pleno_ast::MapItem::Entry(_, value) => {
                            self.collect_idents_expr(value, used, scope)
                        }
                        pleno_ast::MapItem::Spread(expr) => {
                            self.collect_idents_expr(expr, used, scope)
                        }
                    }
                }
            }
            ExprKind::ArrayLiteral(items)
            | ExprKind::SetLiteral(items)
            | ExprKind::TupleLiteral(items) => {
                for item in items {
                    self.collect_idents_expr(item, used, scope);
                }
            }
            ExprKind::Index { collection, index } => {
                self.collect_idents_expr(collection, used, scope);
                self.collect_idents_expr(index, used, scope);
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                self.collect_idents_expr(collection, used, scope);
                if let Some(start_expr) = start {
                    self.collect_idents_expr(start_expr, used, scope);
                }
                if let Some(end_expr) = end {
                    self.collect_idents_expr(end_expr, used, scope);
                }
            }
            ExprKind::Call { callee, args } => {
                self.collect_idents_expr(callee, used, scope);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(expr) => {
                            self.collect_idents_expr(expr, used, scope);
                        }
                        pleno_ast::CallArg::Named(_, expr) => {
                            self.collect_idents_expr(expr, used, scope);
                        }
                        pleno_ast::CallArg::Spread(expr) => {
                            self.collect_idents_expr(expr, used, scope);
                        }
                        pleno_ast::CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Match { expr, arms } => {
                self.collect_idents_expr(expr, used, scope);
                for arm in arms {
                    // Arm pattern can define locals, but for simplicity let's just recurse
                    self.collect_idents_expr(&arm.body, used, scope);
                }
            }
            ExprKind::Pipeline { left, right } => {
                self.collect_idents_expr(left, used, scope);
                self.collect_idents_expr(right, used, scope);
            }
            ExprKind::Lambda { params, body, .. } => {
                scope.push(HashSet::new());
                for param in params {
                    if let Some(default) = &param.default {
                        self.collect_idents_expr(default, used, scope);
                    }
                    scope.last_mut().unwrap().insert(param.name.clone());
                }
                self.collect_idents_expr(body, used, scope);
                scope.pop();
            }
            ExprKind::Block(stmts) => {
                scope.push(HashSet::new());
                for stmt in stmts {
                    self.collect_idents(stmt, used, scope);
                }
                scope.pop();
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_idents_expr(condition, used, scope);
                self.collect_idents_expr(then_branch, used, scope);
                self.collect_idents_expr(else_branch, used, scope);
            }
            ExprKind::MacroCall { args, .. } => {
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e)
                        | pleno_ast::CallArg::Named(_, e)
                        | pleno_ast::CallArg::Spread(e) => {
                            self.collect_idents_expr(e, used, scope);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::Await { expr } => self.collect_idents_expr(expr, used, scope),
            ExprKind::Paren(expr) => self.collect_idents_expr(expr, used, scope),
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.collect_idents_expr(part, used, scope);
                }
            }
            ExprKind::AssertNotNull(expr) => self.collect_idents_expr(expr, used, scope),
            ExprKind::SafeCall { callee, args } => {
                self.collect_idents_expr(callee, used, scope);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e)
                        | pleno_ast::CallArg::Named(_, e)
                        | pleno_ast::CallArg::Spread(e) => {
                            self.collect_idents_expr(e, used, scope);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::SafeProperty { object, .. } => self.collect_idents_expr(object, used, scope),
            _ => {}
        }
    }

    fn enforce_call_ownership(
        &mut self,
        params: &[ParamOwnership],
        args: &[pleno_ast::CallArg],
        span: pleno_ast::Span,
    ) -> Result<(), CompileError> {
        let mut positional_idx = 0;
        for arg in args {
            match arg {
                pleno_ast::CallArg::Positional(expr) => {
                    if let Some(param) = params.get(positional_idx) {
                        self.enforce_arg_ownership(param, expr, span)?;
                    }
                    positional_idx += 1;
                }
                pleno_ast::CallArg::Named(name, expr) => {
                    if let Some(param) = params.iter().find(|p| p.name == *name) {
                        self.enforce_arg_ownership(param, expr, span)?;
                    }
                }
                pleno_ast::CallArg::Placeholder | pleno_ast::CallArg::Spread(_) => {}
            }
        }
        Ok(())
    }

    fn expand_stmt(
        &self,
        stmt: &mut pleno_ast::Stmt,
        substitutions: &HashMap<String, pleno_ast::Expr>,
    ) {
        use pleno_ast::StmtKind;
        match &mut stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.expand_stmt(s, substitutions);
                }
            }
            StmtKind::Expr(expr) => {
                self.expand_expr(expr, substitutions);
            }
            StmtKind::Let { value: Some(v), .. } => {
                self.expand_expr(v, substitutions);
            }
            StmtKind::Assign { value, .. } => {
                self.expand_expr(value, substitutions);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expand_expr(condition, substitutions);
                self.expand_stmt(then_branch, substitutions);
                if let Some(eb) = else_branch {
                    self.expand_stmt(eb, substitutions);
                }
            }
            StmtKind::While { condition, body } => {
                self.expand_expr(condition, substitutions);
                self.expand_stmt(body, substitutions);
            }
            StmtKind::For { iterable, body, .. } => {
                self.expand_expr(iterable, substitutions);
                self.expand_stmt(body, substitutions);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(i) = init {
                    self.expand_stmt(i, substitutions);
                }
                if let Some(c) = condition {
                    self.expand_expr(c, substitutions);
                }
                if let Some(s) = step {
                    self.expand_stmt(s, substitutions);
                }
                self.expand_stmt(body, substitutions);
            }
            StmtKind::Return(Some(e)) => {
                self.expand_expr(e, substitutions);
            }
            StmtKind::Fn {
                params,
                body,
                annotations,
                ..
            } => {
                for p in params {
                    if let Some(d) = &mut p.default {
                        self.expand_expr(d, substitutions);
                    }
                }
                for a in annotations {
                    for arg in &mut a.args {
                        self.expand_expr(arg, substitutions);
                    }
                }
                self.expand_stmt(body, substitutions);
            }
            StmtKind::Class {
                fields,
                methods,
                properties,
                annotations,
                nested_classes,
                ..
            } => {
                for f in fields {
                    if let Some(d) = &mut f.default {
                        self.expand_expr(d, substitutions);
                    }
                }
                for m in methods {
                    self.expand_stmt(&mut m.body, substitutions);
                }
                for p in properties {
                    if let Some(g) = &mut p.getter {
                        self.expand_stmt(&mut g.body, substitutions);
                    }
                    if let Some(s) = &mut p.setter {
                        self.expand_stmt(&mut s.body, substitutions);
                    }
                }
                for a in annotations {
                    for arg in &mut a.args {
                        self.expand_expr(arg, substitutions);
                    }
                }
                for nc in nested_classes {
                    self.expand_stmt(nc, substitutions);
                }
            }
            _ => {}
        }
    }

    fn expand_expr(
        &self,
        expr: &mut pleno_ast::Expr,
        substitutions: &HashMap<String, pleno_ast::Expr>,
    ) {
        use pleno_ast::ExprKind;
        match &mut expr.kind {
            ExprKind::Ident(name) => {
                if let Some(sub) = substitutions.get(name) {
                    *expr = sub.clone();
                }
            }
            ExprKind::Unary { expr, .. } => {
                self.expand_expr(expr, substitutions);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expand_expr(left, substitutions);
                self.expand_expr(right, substitutions);
            }
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.expand_expr(callee, substitutions);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e) => self.expand_expr(e, substitutions),
                        pleno_ast::CallArg::Named(_, e) => self.expand_expr(e, substitutions),
                        pleno_ast::CallArg::Spread(e) => self.expand_expr(e, substitutions),
                        _ => {}
                    }
                }
            }
            ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
                self.expand_expr(object, substitutions)
            }
            ExprKind::Index { collection, index } => {
                self.expand_expr(collection, substitutions);
                self.expand_expr(index, substitutions);
            }
            ExprKind::ArrayLiteral(elements)
            | ExprKind::TupleLiteral(elements)
            | ExprKind::SetLiteral(elements) => {
                for e in elements {
                    self.expand_expr(e, substitutions);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        pleno_ast::MapItem::Entry(_, e) => self.expand_expr(e, substitutions),
                        pleno_ast::MapItem::Spread(e) => self.expand_expr(e, substitutions),
                    }
                }
            }
            ExprKind::NamedTupleLiteral(entries) => {
                for (_, e) in entries {
                    self.expand_expr(e, substitutions);
                }
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.expand_expr(s, substitutions);
                }
                if let Some(e) = end {
                    self.expand_expr(e, substitutions);
                }
            }
            ExprKind::Paren(expr) | ExprKind::Spread(expr) | ExprKind::AssertNotNull(expr) => {
                self.expand_expr(expr, substitutions);
            }
            ExprKind::Lambda {
                params,
                body,
                annotations,
                ..
            } => {
                for p in params {
                    if let Some(d) = &mut p.default {
                        self.expand_expr(d, substitutions);
                    }
                }
                for a in annotations {
                    for arg in &mut a.args {
                        self.expand_expr(arg, substitutions);
                    }
                }
                self.expand_expr(body, substitutions);
            }
            ExprKind::InterpolatedString(segments) => {
                for s in segments {
                    self.expand_expr(s, substitutions);
                }
            }
            ExprKind::Match { expr: e, arms } => {
                self.expand_expr(e, substitutions);
                for arm in arms {
                    if let Some(g) = &mut arm.guard {
                        self.expand_expr(g, substitutions);
                    }
                    self.expand_expr(&mut arm.body, substitutions);
                }
            }
            ExprKind::Block(stmts) => {
                for s in stmts {
                    self.expand_stmt(s, substitutions);
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expand_expr(condition, substitutions);
                self.expand_expr(then_branch, substitutions);
                self.expand_expr(else_branch, substitutions);
            }
            ExprKind::Pipeline { left, right } => {
                self.expand_expr(left, substitutions);
                self.expand_expr(right, substitutions);
            }
            ExprKind::MacroCall { args, .. } => {
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e) => self.expand_expr(e, substitutions),
                        pleno_ast::CallArg::Named(_, e) => self.expand_expr(e, substitutions),
                        pleno_ast::CallArg::Spread(e) => self.expand_expr(e, substitutions),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn enforce_arg_ownership(
        &mut self,
        param: &ParamOwnership,
        expr: &Expr,
        span: pleno_ast::Span,
    ) -> Result<(), CompileError> {
        if let Some(local) = match &expr.kind {
            ExprKind::Ident(name) => self.resolve_local(name),
            _ => None,
        } {
            if self
                .ownership_states
                .get(&local.reg)
                .is_some_and(|state| matches!(state, OwnershipState::Moved))
            {
                return Err(self.error("use of moved value", span));
            }
            match param.ownership {
                pleno_ast::Ownership::Move => {
                    self.ownership_states
                        .insert(local.reg, OwnershipState::Moved);
                }
                pleno_ast::Ownership::BorrowMut => {
                    if !local.mutable {
                        return Err(
                            self.error("cannot take mutable reference to immutable binding", span)
                        );
                    }
                }
                pleno_ast::Ownership::Borrow | pleno_ast::Ownership::None => {}
            }
        }
        Ok(())
    }

    fn resolve_inline_function(&self, name: &str) -> Option<&FunctionDecl> {
        for scope in self.inline_functions.iter().rev() {
            if let Some(decl) = scope.get(name) {
                return Some(decl);
            }
        }
        None
    }

    fn compile_inlined_call(
        &mut self,
        decl: &FunctionDecl,
        args: &[pleno_ast::CallArg],
        dest: u8,
        _span: Span,
    ) -> Result<(), CompileError> {
        self.push_scope();

        // Map arguments to parameters
        for (i, param) in decl.params.iter().enumerate() {
            let arg_reg = if i < args.len() {
                match &args[i] {
                    pleno_ast::CallArg::Positional(expr) => self.compile_expr(expr)?,
                    _ => {
                        return Err(self.error(
                            "Inlining only supported for positional arguments",
                            self.current_span,
                        ));
                    }
                }
            } else if let Some(default) = &param.default {
                self.compile_expr(default)?
            } else {
                let r = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, r, 0, 0);
                r
            };
            self.insert_local(param.name.clone(), arg_reg, true, param.ty.clone())?;
        }

        let old_last_expr = self.last_expr_register;
        let mut return_jumps = Vec::new();
        self.inline_return_stack.push(return_jumps);

        self.compile_stmt(&decl.body)?;

        if let Some(reg) = self.last_expr_register {
            self.emit(Opcode::Move, dest, reg, 0);
        } else {
            self.emit(Opcode::LoadNull, dest, 0, 0);
        }

        return_jumps = self.inline_return_stack.pop().unwrap();
        self.last_expr_register = old_last_expr;
        self.pop_scope();

        let end_pos = self.chunk.code.len();
        for jump in return_jumps {
            self.patch_jump_to(jump, end_pos)?;
        }
        Ok(())
    }

    fn collect_references(&self, stmts: &[Stmt]) -> HashSet<String> {
        let mut referenced = HashSet::new();
        for stmt in stmts {
            self.collect_stmt_refs(stmt, &mut referenced);
        }
        referenced
    }

    fn collect_stmt_refs(&self, stmt: &Stmt, referenced: &mut HashSet<String>) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.collect_expr_refs(expr, referenced),
            StmtKind::Export(stmt) => {
                match &stmt.kind {
                    StmtKind::Let { name, .. }
                    | StmtKind::Fn { name, .. }
                    | StmtKind::Class { name, .. } => {
                        referenced.insert(name.clone());
                    }
                    _ => {}
                }
                self.collect_stmt_refs(stmt, referenced);
            }
            StmtKind::ExportList(items) => {
                for item in items {
                    referenced.insert(item.name.clone());
                }
            }
            StmtKind::Let {
                value: Some(value), ..
            } => {
                self.collect_expr_refs(value, referenced);
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr_refs(condition, referenced);
                self.collect_stmt_refs(then_branch, referenced);
                if let Some(eb) = else_branch {
                    self.collect_stmt_refs(eb, referenced);
                }
            }
            StmtKind::While { condition, body } => {
                self.collect_expr_refs(condition, referenced);
                self.collect_stmt_refs(body, referenced);
            }
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.collect_stmt_refs(s, referenced);
                }
            }
            StmtKind::Fn { body, .. } => {
                self.collect_stmt_refs(body, referenced);
            }
            StmtKind::Class {
                methods,
                superclass,
                ..
            } => {
                if let Some(superclass) = superclass {
                    referenced.insert(superclass.clone());
                }
                for method in methods {
                    self.collect_stmt_refs(&method.body, referenced);
                }
            }
            StmtKind::Return(Some(expr)) => self.collect_expr_refs(expr, referenced),
            StmtKind::For { iterable, body, .. } => {
                self.collect_expr_refs(iterable, referenced);
                self.collect_stmt_refs(body, referenced);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_stmt_refs(init, referenced);
                }
                if let Some(condition) = condition {
                    self.collect_expr_refs(condition, referenced);
                }
                if let Some(step) = step {
                    self.collect_stmt_refs(step, referenced);
                }
                self.collect_stmt_refs(body, referenced);
            }
            StmtKind::DoWhile { body, condition } => {
                self.collect_stmt_refs(body, referenced);
                self.collect_expr_refs(condition, referenced);
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.collect_stmt_refs(try_branch, referenced);
                for catch in catch_branches {
                    self.collect_stmt_refs(&catch.body, referenced);
                }
                if let Some(finally) = finally_branch {
                    self.collect_stmt_refs(finally, referenced);
                }
            }
            StmtKind::Throw(expr) => self.collect_expr_refs(expr, referenced),
            StmtKind::Labeled { stmt, .. } => self.collect_stmt_refs(stmt, referenced),
            _ => {}
        }
    }

    fn collect_expr_refs(&self, expr: &Expr, referenced: &mut HashSet<String>) {
        // println!("Compiler::collect_expr_refs: testing {:?}", expr.kind);
        match &expr.kind {
            ExprKind::Ident(name) => {
                referenced.insert(name.clone());
            }
            ExprKind::Call { callee, args } => {
                self.collect_expr_refs(callee, referenced);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e) => self.collect_expr_refs(e, referenced),
                        pleno_ast::CallArg::Named(_, e) => self.collect_expr_refs(e, referenced),
                        pleno_ast::CallArg::Spread(e) => self.collect_expr_refs(e, referenced),
                        _ => {}
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_expr_refs(left, referenced);
                self.collect_expr_refs(right, referenced);
            }
            ExprKind::Unary { expr, .. } => self.collect_expr_refs(expr, referenced),
            ExprKind::Property { object, .. } => self.collect_expr_refs(object, referenced),
            ExprKind::Index { collection, index } => {
                self.collect_expr_refs(collection, referenced);
                self.collect_expr_refs(index, referenced);
            }
            ExprKind::ArrayLiteral(exprs) => {
                for e in exprs {
                    self.collect_expr_refs(e, referenced);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        pleno_ast::MapItem::Entry(_, value) => {
                            self.collect_expr_refs(value, referenced)
                        }
                        pleno_ast::MapItem::Spread(expr) => {
                            self.collect_expr_refs(expr, referenced)
                        }
                    }
                }
            }
            ExprKind::Lambda { body, .. } => {
                let body_stmt = match &body.kind {
                    ExprKind::Block(stmts) => Stmt {
                        kind: StmtKind::Block(stmts.clone()),
                        span: body.span,
                        leading_blank_lines: 0,
                        trailing_comment: None,
                    },
                    _ => Stmt {
                        kind: StmtKind::Return(Some((**body).clone())),
                        span: body.span,
                        leading_blank_lines: 0,
                        trailing_comment: None,
                    },
                };
                self.collect_stmt_refs(&body_stmt, referenced)
            }
            ExprKind::Await { expr } => self.collect_expr_refs(expr, referenced),
            ExprKind::Paren(expr) => self.collect_expr_refs(expr, referenced),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr_refs(condition, referenced);
                self.collect_expr_refs(then_branch, referenced);
                self.collect_expr_refs(else_branch, referenced);
            }
            ExprKind::Match { expr, arms } => {
                self.collect_expr_refs(expr, referenced);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr_refs(guard, referenced);
                    }
                    self.collect_expr_refs(&arm.body, referenced);
                }
            }
            ExprKind::Is { left, .. } => self.collect_expr_refs(left, referenced),
            ExprKind::SafeCall { callee, args } => {
                self.collect_expr_refs(callee, referenced);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e) => self.collect_expr_refs(e, referenced),
                        pleno_ast::CallArg::Named(_, e) => self.collect_expr_refs(e, referenced),
                        pleno_ast::CallArg::Spread(e) => self.collect_expr_refs(e, referenced),
                        _ => {}
                    }
                }
            }
            ExprKind::SafeProperty { object, .. } => self.collect_expr_refs(object, referenced),
            ExprKind::AssertNotNull(expr) => self.collect_expr_refs(expr, referenced),
            ExprKind::Pipeline { left, right } => {
                self.collect_expr_refs(left, referenced);
                self.collect_expr_refs(right, referenced);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.collect_expr_refs(part, referenced);
                }
            }
            ExprKind::TupleLiteral(exprs) => {
                for e in exprs {
                    self.collect_expr_refs(e, referenced);
                }
            }
            ExprKind::NamedTupleLiteral(items) => {
                for (_, e) in items {
                    self.collect_expr_refs(e, referenced);
                }
            }
            ExprKind::Specialize { left, type_args } => {
                self.collect_expr_refs(left, referenced);
                for arg in type_args {
                    referenced.insert(arg.clone());
                }
            }
            ExprKind::SetLiteral(exprs) => {
                for e in exprs {
                    self.collect_expr_refs(e, referenced);
                }
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                self.collect_expr_refs(collection, referenced);
                if let Some(s) = start {
                    self.collect_expr_refs(s, referenced);
                }
                if let Some(e) = end {
                    self.collect_expr_refs(e, referenced);
                }
            }
            ExprKind::Spread(expr) => self.collect_expr_refs(expr, referenced),
            ExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.collect_expr_refs(s, referenced);
                }
                if let Some(e) = end {
                    self.collect_expr_refs(e, referenced);
                }
            }
            _ => {}
        }
    }

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

    fn split_aware(&self, s: &str, sep: char) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        for c in s.chars() {
            if c == sep && depth == 0 {
                parts.push(current.trim().to_string());
                current = String::new();
            } else {
                if c == '<' {
                    depth += 1;
                }
                if c == '>' {
                    depth -= 1;
                }
                current.push(c);
            }
        }
        parts.push(current.trim().to_string());
        parts.into_iter().filter(|s| !s.is_empty()).collect()
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

        let union_parts = self.split_aware(expanded, '|');
        let mut union_types = Vec::new();

        for part in union_parts {
            let intersections = self.split_aware(&part, '&');
            if !intersections.is_empty() {
                union_types.push(intersections);
            }
        }

        (union_types, is_optional)
    }

    fn emit_reified_type(&mut self, ty_name: &str) -> Result<u8, CompileError> {
        let ty_name = ty_name.trim();
        if let Some(start_idx) = ty_name.find('<') {
            let base_name = ty_name[..start_idx].trim();
            let args_str = &ty_name[start_idx + 1..ty_name.len() - 1]; // Assuming it ends with >
            let args = self.split_aware(args_str, ',');

            let base_reg = self.registers.allocate()?;
            let name_idx =
                self.add_constant(Value::String(std::rc::Rc::new(base_name.to_string())));
            self.emit(Opcode::GetGlobal, base_reg, name_idx, 0);

            let args_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, args_reg, 0, 0);
            for arg_name in args {
                let arg_type_reg = self.emit_reified_type(&arg_name)?;
                self.emit(Opcode::ArrayAppend, args_reg, arg_type_reg, 0);
                self.free_temp(arg_type_reg);
            }

            let dest = self.registers.allocate()?;
            self.emit(Opcode::Specialize, dest, base_reg, args_reg);
            self.free_temp(base_reg);
            self.free_temp(args_reg);
            Ok(dest)
        } else {
            let type_reg = self.registers.allocate()?;
            let name_idx = self.add_constant(Value::String(std::rc::Rc::new(ty_name.to_string())));
            self.emit(Opcode::GetGlobal, type_reg, name_idx, 0);

            // Handle primitives if global is null
            let is_null_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadNull, is_null_reg, 0, 0);
            self.emit(Opcode::Eq, is_null_reg, type_reg, is_null_reg);
            let skip_primitive_load = self.emit_jump(Opcode::JumpIfNot, is_null_reg);
            self.emit(Opcode::LoadConst, type_reg, name_idx, 0);
            self.patch_jump(skip_primitive_load)?;
            self.free_temp(is_null_reg);

            Ok(type_reg)
        }
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
                let type_reg = self.emit_reified_type(ty_name)?;
                temp_regs.push(type_reg);

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

    pub fn compile_lazy_thunk(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        let callable_reg = self.compile_lambda(&[], expr, None, false, &[])?;
        let thunk_reg = self.registers.allocate()?;
        self.emit(Opcode::MakeThunk, thunk_reg, callable_reg, 0);
        self.free_temp(callable_reg);
        Ok(thunk_reg)
    }

    fn insert_local(
        &mut self,
        name: String,
        reg: u8,
        mutable: bool,
        ty: Option<String>,
    ) -> Result<(), CompileError> {
        let is_boxed = self
            .boxed_vars
            .last()
            .map(|b| b.contains(&name))
            .unwrap_or(false);
        if let Some(scope) = self.locals.last_mut() {
            if scope.contains_key(&name) {
                return Err(self.error(
                    &format!("Local '{}' already defined", name),
                    self.current_span,
                ));
            }
            scope.insert(
                name,
                Local {
                    reg,
                    mutable,
                    ty,
                    is_boxed,
                },
            );
        }
        Ok(())
    }

    fn resolve_local(&self, name: &str) -> Option<Local> {
        for (_i, scope) in self.locals.iter().enumerate().rev() {
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

    fn push_scope(&mut self) {
        self.locals.push(HashMap::new());
        self.fn_signatures.push(HashMap::new());
        self.inline_functions.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.locals.pop() {
            for local in scope.values() {
                if Some(local.reg) != self.last_expr_register {
                    self.registers.free(local.reg);
                }
                self.lazy_locals.remove(&local.reg);
                self.ownership_states.remove(&local.reg);
            }
        }
        self.fn_signatures.pop();
        self.inline_functions.pop();
    }

    fn resolve_fn_signature(&self, name: &str) -> Option<&Vec<ParamOwnership>> {
        for scope in self.fn_signatures.iter().rev() {
            if let Some(sig) = scope.get(name) {
                return Some(sig);
            }
        }
        None
    }

    fn find_boxed_vars(&self, stmts: &[Stmt]) -> HashSet<String> {
        let mut boxed = HashSet::new();
        for stmt in stmts {
            self.find_closures_and_captures_in_stmt(stmt, &mut boxed);
        }
        boxed
    }

    fn find_closures_and_captures_in_stmt(&self, stmt: &Stmt, boxed: &mut HashSet<String>) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.find_closures_and_captures_in_expr(expr, boxed),
            StmtKind::Let {
                value: Some(val), ..
            } => self.find_closures_and_captures_in_expr(val, boxed),
            StmtKind::Assign { value, .. } => self.find_closures_and_captures_in_expr(value, boxed),
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.find_closures_and_captures_in_stmt(s, boxed);
                }
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.find_closures_and_captures_in_expr(condition, boxed);
                self.find_closures_and_captures_in_stmt(then_branch, boxed);
                if let Some(eb) = else_branch {
                    self.find_closures_and_captures_in_stmt(eb, boxed);
                }
            }
            StmtKind::While { condition, body } => {
                self.find_closures_and_captures_in_expr(condition, boxed);
                self.find_closures_and_captures_in_stmt(body, boxed);
            }
            StmtKind::For { iterable, body, .. } => {
                self.find_closures_and_captures_in_expr(iterable, boxed);
                self.find_closures_and_captures_in_stmt(body, boxed);
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.find_closures_and_captures_in_stmt(init, boxed);
                }
                if let Some(condition) = condition {
                    self.find_closures_and_captures_in_expr(condition, boxed);
                }
                if let Some(step) = step {
                    self.find_closures_and_captures_in_stmt(step, boxed);
                }
                self.find_closures_and_captures_in_stmt(body, boxed);
            }
            StmtKind::Fn {
                name,
                generics,
                params,
                body,
                annotations,
                is_async,
                ..
            } => {
                let decl = FunctionDecl {
                    span: Span::new(0, 0),
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
                    doc: None,
                };
                let captures = self.analyze_captures_names(&decl, false);
                for name in captures {
                    boxed.insert(name);
                }
                self.find_closures_and_captures_in_stmt(body, boxed);
            }
            StmtKind::Return(Some(expr)) => self.find_closures_and_captures_in_expr(expr, boxed),
            _ => {}
        }
    }

    fn find_closures_and_captures_in_expr(&self, expr: &Expr, boxed: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::Lambda { params, body, .. } => {
                let body_stmt = match &body.kind {
                    ExprKind::Block(stmts) => Stmt {
                        kind: StmtKind::Block(stmts.clone()),
                        span: body.span,
                        leading_blank_lines: 0,
                        trailing_comment: None,
                    },
                    _ => Stmt {
                        kind: StmtKind::Return(Some((**body).clone())),
                        span: body.span,
                        leading_blank_lines: 0,
                        trailing_comment: None,
                    },
                };
                let decl = FunctionDecl {
                    span: Span::new(0, 0),
                    name: "anonymous".to_string(),
                    generics: Vec::new(),
                    params: params.clone(),
                    return_ty: None,
                    body: Box::new(body_stmt),
                    is_static: false,
                    is_abstract: false,
                    is_async: false,
                    visibility: None,
                    kind: FunctionKind::Method,
                    annotations: Vec::new(),
                    doc: None,
                };
                let captures = self.analyze_captures_names(&decl, false);
                for name in captures {
                    boxed.insert(name);
                }
                self.find_closures_and_captures_in_expr(body, boxed);
            }
            ExprKind::Binary { left, right, .. } => {
                self.find_closures_and_captures_in_expr(left, boxed);
                self.find_closures_and_captures_in_expr(right, boxed);
            }
            ExprKind::Unary { expr, .. } => self.find_closures_and_captures_in_expr(expr, boxed),
            ExprKind::Call { callee, args } => {
                self.find_closures_and_captures_in_expr(callee, boxed);
                for arg in args {
                    match arg {
                        pleno_ast::CallArg::Positional(e)
                        | pleno_ast::CallArg::Named(_, e)
                        | pleno_ast::CallArg::Spread(e) => {
                            self.find_closures_and_captures_in_expr(e, boxed);
                        }
                        pleno_ast::CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Block(stmts) => {
                for s in stmts {
                    self.find_closures_and_captures_in_stmt(s, boxed);
                }
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        pleno_ast::MapItem::Entry(_, e) | pleno_ast::MapItem::Spread(e) => {
                            self.find_closures_and_captures_in_expr(e, boxed);
                        }
                    }
                }
            }
            ExprKind::ArrayLiteral(items)
            | ExprKind::SetLiteral(items)
            | ExprKind::TupleLiteral(items) => {
                for item in items {
                    self.find_closures_and_captures_in_expr(item, boxed);
                }
            }
            ExprKind::Index { collection, index } => {
                self.find_closures_and_captures_in_expr(collection, boxed);
                self.find_closures_and_captures_in_expr(index, boxed);
            }
            ExprKind::Property { object, .. } => {
                self.find_closures_and_captures_in_expr(object, boxed)
            }
            ExprKind::Match { expr, arms } => {
                self.find_closures_and_captures_in_expr(expr, boxed);
                for arm in arms {
                    self.find_closures_and_captures_in_expr(&arm.body, boxed);
                }
            }
            _ => {}
        }
    }

    fn analyze_captures_names(&self, decl: &FunctionDecl, is_method: bool) -> Vec<String> {
        let mut locals_map = HashMap::new();
        let mut scope_stack = vec![HashSet::new()];

        for param in &decl.params {
            locals_map.insert(param.name.clone(), ());
            scope_stack[0].insert(param.name.clone());
        }
        if is_method {
            locals_map.insert("this".to_string(), ());
            scope_stack[0].insert("this".to_string());
        }

        self.collect_locals(&decl.body, &mut locals_map);
        for name in locals_map.keys() {
            scope_stack[0].insert(name.clone());
        }

        let mut used = HashMap::new();
        for param in &decl.params {
            if let Some(default) = &param.default {
                self.collect_idents_expr(default, &mut used, &mut scope_stack);
            }
        }
        self.collect_idents(&decl.body, &mut used, &mut scope_stack);

        let mut captures = Vec::new();
        for (name, is_mutated) in used {
            if is_mutated && !locals_map.contains_key(&name) {
                captures.push(name);
            }
        }
        captures
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
            return Err(CompileError {
                diagnostic: pleno_diagnostics::Diagnostic::error("too many registers in function"),
            });
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

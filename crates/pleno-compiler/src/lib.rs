pub mod check;
mod prelude;

use indexmap::IndexMap;
use pleno_ast::{
    BinaryOp, Expr, ExprKind, FieldDecl, FunctionDecl, FunctionKind, Literal, MapItem, Pattern,
    Program, Span, Stmt, StmtKind, Visibility,
};
use pleno_bytecode::{Chunk, FunctionProto, Instruction, Opcode, ParamSpec};
use pleno_value::{RuntimeAnnotation, Value};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
    Test,
}

/// What a `for ... in` loop binds each item to.
#[derive(Clone, Copy)]
enum ForBinding<'a> {
    /// `for (x in xs)` or `for (k, v in m)`.
    Names(&'a [String]),
    /// `for ((k, v) in pairs)`.
    Pattern(&'a pleno_ast::BindingPattern),
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
    start_ip: usize,
    /// A `&mut` parameter: reassigning it would not reach the caller.
    borrow_mut: bool,
}

/// The class, enum or extension whose methods are being compiled.
#[derive(Debug, Clone)]
struct OwnerContext {
    name: String,
    generics: Vec<String>,
}

impl OwnerContext {
    fn new(name: &str, generics: &[pleno_ast::GenericParam]) -> Self {
        Self {
            name: name.to_string(),
            generics: generics.iter().map(|g| g.name.clone()).collect(),
        }
    }
}

/// A function's return annotation as checked at each `return`.
#[derive(Debug, Clone)]
struct ReturnCheck {
    /// The annotation the value must match (for `async`, the resolved value).
    ty: String,
    /// "TypeError: <name> returned ".
    prefix: String,
    /// ", expected <annotation>".
    suffix: String,
}

#[derive(Debug, Clone)]
struct CaptureInfo {
    name: String,
    is_boxed: bool,
    ty: Option<String>,
    /// Whether the captured variable was declared `mut`.
    mutable: bool,
    /// A captured `&mut` parameter: reassigning it would not reach the caller.
    borrow_mut: bool,
}

#[derive(Clone)]
struct LoopContext {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    continue_target: Option<usize>,
    labels: Vec<String>,
    /// How many cleanups were open when the loop started: `break` and
    /// `continue` run the ones above this.
    cleanup_depth: usize,
}

/// Code that must run when control leaves a `try` or `with` early, through
/// `return`, `break` or `continue`, innermost first.
#[derive(Clone)]
struct Cleanup {
    /// The region has an exception handler pushed, popped before the action
    /// runs (so an error from the action is not caught by its own handler).
    pops_handler: bool,
    action: CleanupAction,
}

#[derive(Clone)]
enum CleanupAction {
    None,
    /// A `finally` block.
    Finally(Box<Stmt>),
    /// `close()` on the resource of a `with`, in the hidden local of that name.
    Close(String),
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

/// The end of a chain of `Compiler::scope_names` entries.
const NO_SCOPE_NAME: usize = usize::MAX;

pub struct Compiler {
    /// Names the program reads as globals, with where (for `pleno check`) and
    /// the `scope_names` entry of the innermost local in scope there.
    global_reads: Vec<(String, Span, usize)>,
    /// Every local declared so far, each with the entry of the local declared
    /// before it in an enclosing or the same scope (`NO_SCOPE_NAME` for none):
    /// following the links from an entry lists the locals in scope at a point.
    /// Shared with the compilers of nested functions, which see the
    /// enclosing locals. Read only for "did you mean" hints.
    scope_names: Rc<std::cell::RefCell<Vec<(String, usize)>>>,
    /// The entry of the innermost local in scope now.
    scope_head: usize,
    /// `scope_head` when each open scope began.
    scope_heads: Vec<usize>,
    /// Names the program defines as globals: functions, classes, enums, interfaces.
    global_defs: HashSet<String>,
    chunk: Chunk,
    /// Where each `LoadConst` is, by constant index. A constant that is only
    /// ever loaded can move above index 255 (`LoadConst` takes a 16-bit index)
    /// to make room for one that an 8-bit operand names; see `add_constant`.
    constant_loads: HashMap<usize, Vec<usize>>,
    /// Constants named by an 8-bit operand (names, keys), which cannot move.
    pinned_constants: HashSet<usize>,
    /// Set when a function needs more constants than its operands can name.
    constant_overflow: Option<(String, Span)>,
    registers: RegisterAllocator,
    last_expr_register: Option<u8>,
    locals: Vec<HashMap<String, Local>>,
    lazy_locals: HashMap<u8, bool>,
    ownership_states: HashMap<u8, OwnershipState>,
    fn_signatures: Vec<HashMap<String, Vec<ParamOwnership>>>,
    inline_functions: Vec<HashMap<String, FunctionDecl>>,
    /// For each `@inline` call being compiled: its `return` jumps, how many
    /// cleanups were open when it started, and the register for its result.
    inline_return_stack: Vec<(Vec<usize>, usize, u8)>,
    referenced_entities: HashSet<String>,
    /// Compile every declaration and statement as written: no optimizer pass
    /// and no skipping of unused functions and classes. Used to find the
    /// errors in code that the optimized build would leave out.
    keep_unused_code: bool,
    /// Top-level functions, classes, enums and interfaces by name, with what
    /// each is, for the error when code assigns to one.
    declaration_kinds: HashMap<String, &'static str>,
    loop_counter: u32,
    loop_stack: Vec<LoopContext>,
    captures: HashMap<String, (usize, CaptureInfo)>,
    is_top_level: bool,
    current_super: Option<String>,
    module_mode: bool,
    module_exports: Vec<ExportItem>,
    module_name: Option<String>,
    source: String,
    current_span: pleno_ast::Span,
    next_labels: Vec<String>,
    /// Type aliases by name: their type parameters and the aliased type.
    type_aliases: HashMap<String, (Vec<String>, String)>,
    macros: HashMap<String, MacroDef>,
    boxed_vars: Vec<HashSet<String>>,
    /// The `try` and `with` regions around the code being compiled.
    cleanup_stack: Vec<Cleanup>,
    optional_chain_jumps: Vec<usize>,
    /// Set while a postfix operation (property, index, call) compiles its object,
    /// so a `?.` inside that object continues the same chain.
    in_postfix_chain: bool,
    expr_depth: usize,
    /// One past the furthest instruction a jump lands on, so `retarget_last`
    /// never removes a `Move` that a jump goes to.
    jump_targets_end: usize,
    /// Set while compiling the methods of a class, enum or extension.
    owner_context: Option<OwnerContext>,
    /// Type parameters in scope (of enclosing functions and classes).
    generic_names: Vec<String>,
    /// The return annotation checked at each `return` of this function.
    return_check: Option<ReturnCheck>,
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
            global_reads: Vec::new(),
            scope_names: Rc::default(),
            scope_head: NO_SCOPE_NAME,
            scope_heads: Vec::new(),
            global_defs: HashSet::new(),
            chunk: Chunk::new(),
            constant_loads: HashMap::new(),
            pinned_constants: HashSet::new(),
            constant_overflow: None,
            registers: RegisterAllocator::new(),
            last_expr_register: None,
            locals: vec![HashMap::new()],
            lazy_locals: HashMap::new(),
            ownership_states: HashMap::new(),
            fn_signatures: vec![HashMap::new()],
            inline_functions: vec![HashMap::new()],
            inline_return_stack: Vec::new(),
            referenced_entities: HashSet::new(),
            keep_unused_code: false,
            declaration_kinds: HashMap::new(),
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
            cleanup_stack: Vec::new(),
            optional_chain_jumps: Vec::new(),
            in_postfix_chain: false,
            expr_depth: 0,
            jump_targets_end: 0,
            owner_context: None,
            generic_names: Vec::new(),
            return_check: None,
            is_async: false,
            profile: BuildProfile::Debug,
        }
    }

    pub fn with_profile(mut self, profile: BuildProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Compiles all code as written, including unused functions and classes
    /// and code after a `return`, so every compile error and every global
    /// read is found. The result runs, without the optimizer's changes.
    pub fn keeping_unused_code(mut self) -> Self {
        self.keep_unused_code = true;
        self
    }

    /// Before an optimized compile drops unused code, compiles a copy of the
    /// whole program to report the errors in that code too.
    fn check_unused_code(&self, program: &Program, module: bool) -> Result<(), CompileError> {
        let mut checker = Compiler::new(self.source.clone())
            .with_profile(self.profile)
            .keeping_unused_code();
        let mut copy = program.clone();
        if module {
            checker.compile_module(&mut copy)?;
        } else {
            checker.compile(&mut copy)?;
        }
        Ok(())
    }

    /// Optimizes `program` unless all code is kept.
    fn prepare(&self, program: &mut Program, module: bool) -> Result<(), CompileError> {
        if !self.keep_unused_code {
            self.check_unused_code(program, module)?;
            pleno_optimizer::optimize(program);
        }
        Ok(())
    }

    /// Globals the program reads but never defines, other than those
    /// `is_builtin` accepts, with where each is read. Code removed as unused
    /// is not checked unless the compiler keeps it (`keeping_unused_code`).
    pub fn undefined_globals(&self, is_builtin: impl Fn(&str) -> bool) -> Vec<(String, Span)> {
        self.global_reads
            .iter()
            .filter(|(name, _, _)| !self.global_defs.contains(name) && !is_builtin(name))
            .map(|(name, span, _)| (name.clone(), *span))
            .collect()
    }

    /// [`undefined_globals`](Self::undefined_globals), each with a hint to
    /// append to its error: `"; did you mean 'x'?"` for the closest name in
    /// scope where it is read (a local, a global the program defines, or one
    /// of `builtins`), or an empty string.
    pub fn undefined_globals_with_hints<'a>(
        &self,
        is_builtin: impl Fn(&str) -> bool,
        builtins: impl IntoIterator<Item = &'a str>,
    ) -> Vec<(String, Span, String)> {
        let undefined: Vec<&(String, Span, usize)> = self
            .global_reads
            .iter()
            .filter(|(name, _, _)| !self.global_defs.contains(name) && !is_builtin(name))
            .collect();
        if undefined.is_empty() {
            return Vec::new();
        }
        let builtins: Vec<&str> = builtins
            .into_iter()
            .filter(|name| !name.starts_with("__"))
            .collect();
        let scope_names = self.scope_names.borrow();
        undefined
            .into_iter()
            .map(|(name, span, head)| {
                let mut candidates: Vec<&str> = Vec::new();
                let mut entry = *head;
                while let Some((local, parent)) = scope_names.get(entry) {
                    candidates.push(local);
                    entry = *parent;
                }
                candidates.extend(self.global_defs.iter().map(String::as_str));
                candidates.extend(builtins.iter().copied());
                let hint = pleno_value::suggest::did_you_mean(name, candidates);
                (name.clone(), *span, hint)
            })
            .collect()
    }

    fn error(&self, message: &str, span: Span) -> CompileError {
        use pleno_diagnostics::Diagnostic;
        let diagnostic = Diagnostic::error(message).with_label(span, "");
        CompileError { diagnostic }
    }

    pub fn compile(&mut self, program: &mut Program) -> Result<Chunk, CompileError> {
        self.prepare(program, false)?;
        self.module_mode = false;
        self.module_exports.clear();
        self.module_name = None;

        self.referenced_entities = self.collect_references(&program.statements);
        self.declaration_kinds = declaration_kinds(&program.statements);

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
        self.check_constants()?;
        self.record_open_locals();
        self.chunk.max_registers = self.registers.max_used;
        self.chunk.module_name = None;
        Ok(self.chunk.clone())
    }

    pub fn compile_module(&mut self, program: &mut Program) -> Result<Chunk, CompileError> {
        self.prepare(program, true)?;
        self.module_mode = true;
        self.module_exports.clear();
        self.module_name = None;

        self.referenced_entities = self.collect_references(&program.statements);
        self.declaration_kinds = declaration_kinds(&program.statements);
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
        self.record_open_locals();
        self.chunk.max_registers = self.registers.max_used;
        self.chunk.module_name = self.module_name.clone();
        self.check_constants()?;
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
            StmtKind::TypeAlias {
                name, params, ty, ..
            } => {
                self.type_aliases
                    .insert(name.clone(), (params.clone(), ty.clone()));
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
                            self.emit_type_check_only(thunk_reg, ty_name)?;
                        }
                        thunk_reg
                    } else {
                        let mut reg = self.compile_expr(expr)?;
                        // `let y = x;` gets x's own register back; the new
                        // variable needs a copy, or assigning one changes both.
                        if self.is_local_reg(reg) {
                            let copy = self.registers.allocate()?;
                            self.emit(Opcode::Move, copy, reg, 0);
                            reg = copy;
                        }
                        match ty {
                            Some(ty_name) => self.emit_type_check(reg, ty_name)?,
                            None => reg,
                        }
                    }
                } else {
                    let reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadNull, reg, 0, 0);
                    // For now, Null is only allowed if no type or if we add optional types later
                    // but many primitives don't allow Null.
                    // Keeping it simple: if typed, Null must match the type (it won't for Int etc)
                    if let Some(ty_name) = ty {
                        self.emit_type_check_only(reg, ty_name)?;
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
            StmtKind::LetPattern {
                pattern,
                mutable,
                ty,
                value,
                ..
            } => {
                let reg = self.compile_expr(value)?;
                if let Some(ty_name) = ty {
                    self.emit_type_check_only(reg, ty_name)?;
                }
                self.compile_binding_pattern(pattern, reg, *mutable)?;
                self.free_temp(reg);
                Ok(())
            }
            StmtKind::Assign { name, value } => {
                let value_start = self.chunk.code.len();
                let mut value_reg = self.compile_expr(value)?;
                if let Some(local) = self.resolve_local(name) {
                    if local.borrow_mut {
                        return Err(self.error(&mut_param_assign_message(name), stmt.span));
                    }
                    if !local.mutable {
                        return Err(self.error("cannot assign to immutable binding", stmt.span));
                    }
                    if let Some(ty_name) = &local.ty {
                        value_reg = self.emit_type_check(value_reg, ty_name)?;
                    }
                    if local.is_boxed {
                        self.emit(Opcode::SetCell, local.reg, value_reg, 0);
                    } else if local.ty.is_some()
                        || !self.retarget_last(value_reg, local.reg, value_start)
                    {
                        self.emit(Opcode::Move, local.reg, value_reg, 0);
                    }
                    self.ownership_states
                        .insert(local.reg, OwnershipState::Available);
                } else if let Some((index, info)) = self.captures.get(name).cloned() {
                    // Update capture
                    if info.borrow_mut {
                        return Err(self.error(&mut_param_assign_message(name), stmt.span));
                    }
                    if !info.mutable {
                        return Err(self.error("cannot assign to immutable binding", stmt.span));
                    }
                    let is_boxed = info.is_boxed;

                    if let Some(ty) = &info.ty {
                        value_reg = self.emit_type_check(value_reg, ty)?;
                    }

                    if is_boxed {
                        let cell_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadCapture, cell_reg, index as u8, 0);
                        self.emit(Opcode::SetCell, cell_reg, value_reg, 0);
                        self.free_temp(cell_reg);
                    } else {
                        self.emit(Opcode::SetCapture, value_reg, index as u8, 0);
                    }
                } else if let Some(kind) = self.declaration_kinds.get(name) {
                    return Err(self.error(
                        &format!("cannot assign to {kind} '{name}': declare a variable with `let mut` instead"),
                        stmt.span,
                    ));
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
                pattern,
                iterable,
                body,
            } => match pattern {
                Some(pattern) => self.compile_for_pattern(pattern, iterable, body),
                None => self.compile_for_stmt(names, iterable, body),
            },
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
            StmtKind::With { resources, body } => {
                self.compile_with_stmt(resources, body, stmt.span)
            }
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
                    StmtKind::LetPattern { .. } => {
                        return Err(self.error(
                            "a destructuring 'let' cannot be exported; export each name separately",
                            stmt.span,
                        ));
                    }
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
            StmtKind::Class {
                name,
                annotations,
                implements,
                ..
            } => {
                // Dead Code Elimination: skip unreferenced classes, unless defining the
                // class has effects (decorators, interface conformance checks).
                if !self.keep_unused_code
                    && !self.referenced_entities.contains(name)
                    && self.is_top_level
                    && annotations.is_empty()
                    && implements.is_empty()
                {
                    return self.check_abstract_methods(stmt);
                }

                let class_reg = self.compile_class_declaration(stmt)?;
                if self.is_top_level {
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    self.emit(Opcode::SetGlobal, class_reg, name_idx, 0);
                    self.global_defs.insert(name.clone());
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
                return_ty,
                body,
                annotations,
                is_async,
                name_span,
                ..
            } => {
                let ownership = params
                    .iter()
                    .map(|param| ParamOwnership {
                        name: param.name.clone(),
                        ownership: param.ownership.clone(),
                    })
                    .collect::<Vec<_>>();
                // Dead Code Elimination: skip unreferenced functions, unless they are
                // annotated (a decorator may register them or have other effects).
                if !self.keep_unused_code
                    && !self.referenced_entities.contains(name)
                    && self.is_top_level
                    && annotations.is_empty()
                {
                    return Ok(());
                }

                if let Some(scope) = self.fn_signatures.last_mut() {
                    scope.insert(name.clone(), ownership);
                }

                let decl = FunctionDecl {
                    span: Span::new(0, 0),
                    name: name.clone(),
                    name_span: *name_span,
                    generics: generics.clone(),
                    params: params.clone(),
                    return_ty: return_ty.clone(),
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
                    self.global_defs.insert(name.clone());
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
                if let Some(&(_, depth, dest)) = self.inline_return_stack.last() {
                    // A `return` in an inlined body stores the result, leaves
                    // the `try` and `with` regions opened inside that body and
                    // jumps past its end.
                    self.emit(Opcode::Move, dest, reg, 0);
                    self.emit_cleanups(depth)?;
                    let jump = self.emit_jump(Opcode::Jump, 0);
                    if let Some((jumps, _, _)) = self.inline_return_stack.last_mut() {
                        jumps.push(jump);
                    }
                } else {
                    let reg = self.emit_return_check(reg)?;
                    // Run every open `finally` and `with` close, innermost first.
                    self.emit_cleanups(0)?;
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
                        let meta_reg = self.registers.allocate()?;
                        self.emit(Opcode::NewMap, meta_reg, 0, 0);

                        let name_idx =
                            self.add_constant(Value::String(std::rc::Rc::new(g.name.clone())));
                        let name_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                        let name_key_idx =
                            self.add_constant(Value::String(std::rc::Rc::new("name".to_string())));
                        self.emit(Opcode::SetProperty, name_reg, meta_reg, name_key_idx);

                        let var_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, var_reg, g.variance as u8, 0);
                        let var_key_idx = self
                            .add_constant(Value::String(std::rc::Rc::new("variance".to_string())));
                        self.emit(Opcode::SetProperty, var_reg, meta_reg, var_key_idx);
                        self.free_temp(var_reg);

                        self.emit(Opcode::ArrayAppend, params_reg, meta_reg, 0);

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
                        self.free_temp(meta_reg);
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
                self.global_defs.insert(name.clone());

                self.free_temp(interface_reg);
                self.free_temp(methods_reg);
                Ok(())
            }
            StmtKind::Enum(decl) => {
                let enum_reg = self.compile_enum_declaration(decl)?;
                if self.is_top_level {
                    let name_idx = self.add_constant(Value::String(decl.name.clone().into()));
                    self.emit(Opcode::SetGlobal, enum_reg, name_idx, 0);
                    self.global_defs.insert(decl.name.clone());
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
            // Arrays and maps are mutable, so each evaluation of a literal must
            // create a new one: a folded constant would be shared by every call.
            ExprKind::ArrayLiteral(_) | ExprKind::MapLiteral(_) => None,
            ExprKind::Binary { left, op, right } => {
                let l_val = self.try_fold_expr(left)?;
                let r_val = self.try_fold_expr(right)?;

                match (op, l_val, r_val) {
                    (BinaryOp::Add, Value::Int(a), Value::Int(b)) => {
                        a.checked_add(b).map(Value::Int)
                    }
                    (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Some(Value::Float(a + b)),
                    (BinaryOp::Subtract, Value::Int(a), Value::Int(b)) => {
                        a.checked_sub(b).map(Value::Int)
                    }
                    (BinaryOp::Subtract, Value::Float(a), Value::Float(b)) => {
                        Some(Value::Float(a - b))
                    }
                    (BinaryOp::Multiply, Value::Int(a), Value::Int(b)) => {
                        a.checked_mul(b).map(Value::Int)
                    }
                    (BinaryOp::Multiply, Value::Float(a), Value::Float(b)) => {
                        Some(Value::Float(a * b))
                    }
                    // Folding must agree with the VM: integer `/` produces a
                    // Float, and division by zero is left to fail at runtime.
                    (BinaryOp::Divide, Value::Int(a), Value::Int(b)) if b != 0 => {
                        Some(Value::Float(a as f64 / b as f64))
                    }
                    (BinaryOp::Divide, Value::Float(a), Value::Float(b)) if b != 0.0 => {
                        Some(Value::Float(a / b))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn is_postfix_op(kind: &ExprKind) -> bool {
        matches!(
            kind,
            ExprKind::Property { .. }
                | ExprKind::SafeProperty { .. }
                | ExprKind::Call { .. }
                | ExprKind::SafeCall { .. }
                | ExprKind::Index { .. }
                | ExprKind::SafeIndex { .. }
                | ExprKind::Slice { .. }
                | ExprKind::AssertNotNull(_)
        )
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        self.expr_depth += 1;
        let is_postfix = Self::is_postfix_op(&expr.kind);
        let start_jumps = self.optional_chain_jumps.len();
        // Only the first operand compiled by a postfix operation (its object or
        // callee) continues the chain; arguments and indices start a fresh one.
        let parent_is_postfix = std::mem::take(&mut self.in_postfix_chain);
        self.in_postfix_chain = is_postfix;

        let result = self.compile_expr_internal(expr);
        self.in_postfix_chain = false;
        self.expr_depth -= 1;
        let reg = result?;

        // A `?.` short-circuit skips to the end of its chain: the outermost postfix
        // operation, not the whole enclosing expression (e.g. `a?.b ?? c`).
        if self.optional_chain_jumps.len() > start_jumps
            && (!is_postfix || !parent_is_postfix || self.expr_depth == 0)
        {
            // Patch all optional chain jumps to this point
            let jumps_to_patch: Vec<usize> =
                self.optional_chain_jumps.drain(start_jumps..).collect();
            let jump_past_null = self.emit_jump(Opcode::Jump, 0);

            for jump in jumps_to_patch {
                self.patch_jump(jump)?;
            }

            self.emit(Opcode::LoadNull, reg, 0, 0);
            self.patch_jump(jump_past_null)?;
        }

        Ok(reg)
    }

    fn compile_expr_internal(&mut self, expr: &Expr) -> Result<u8, CompileError> {
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
                    Literal::Int(value) => self.emit_load_constant(reg, Value::Int(*value)),
                    Literal::Float(value) => self.emit_load_constant(reg, Value::Float(*value)),
                    Literal::String(value) => {
                        self.emit_load_constant(reg, Value::String(value.clone().into()))
                    }
                    Literal::Char(value) => self.emit_load_constant(reg, Value::Char(*value)),
                }
                Ok(reg)
            }
            ExprKind::Binary { left, op, right } => {
                match op {
                    pleno_ast::BinaryOp::And => self.compile_logical_and(left, right),
                    pleno_ast::BinaryOp::Or => self.compile_logical_or(left, right),
                    pleno_ast::BinaryOp::Add | pleno_ast::BinaryOp::Subtract
                        if matches!(
                            right.kind,
                            ExprKind::Literal(Literal::Int(value)) if (0..=255).contains(&value)
                        ) =>
                    {
                        // `x + 1` and `x - 1`: the constant goes in the instruction.
                        let ExprKind::Literal(Literal::Int(value)) = right.kind else {
                            unreachable!()
                        };
                        let left_reg = self.compile_expr(left)?;
                        let dest = self.registers.allocate()?;
                        let opcode = if matches!(op, pleno_ast::BinaryOp::Add) {
                            Opcode::AddInt
                        } else {
                            Opcode::SubInt
                        };
                        self.emit(opcode, dest, left_reg, value as u8);
                        self.free_temp(left_reg);
                        Ok(dest)
                    }
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
                } else if let Some((index, info)) = self.captures.get(name).cloned() {
                    let is_boxed = info.is_boxed;
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
                    // `c = 1`: reading an undefined name is an error.
                    self.emit(Opcode::GetGlobal, dest, name_idx, 1);
                    self.global_reads
                        .push((name.clone(), expr.span, self.scope_head));
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
                let object_reg = self.compile_expr(object)?;
                let dest = self.registers.allocate()?;

                let null_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, null_reg, 0, 0);
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, is_null_reg, object_reg, null_reg);
                self.free_temp(null_reg);

                let jump_to_end = self.emit_jump(Opcode::JumpIf, is_null_reg);
                self.optional_chain_jumps.push(jump_to_end);
                self.free_temp(is_null_reg);

                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::GetProperty, dest, object_reg, name_idx);
                self.free_temp(object_reg);

                Ok(dest)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.compile_if_expr(condition, then_branch, else_branch.as_deref()),
            ExprKind::Is { left, ty } => self.compile_is_expr(left, ty),
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
                is_async,
            } => {
                let is_pure = annotations.iter().any(|a| a.name == "pure");
                Ok(self.compile_lambda(
                    params,
                    body,
                    return_ty.as_deref(),
                    is_pure,
                    annotations,
                    *is_async,
                )?)
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
            ExprKind::MapLiteral(entries) => self.compile_map_literal(entries),
            ExprKind::NamedTupleLiteral(entries) => self.compile_named_tuple_literal(entries),
            ExprKind::ArrayLiteral(items) => self.compile_array_literal(items),
            ExprKind::TupleLiteral(items) => self.compile_tuple_literal(items),
            ExprKind::SetLiteral(items) => self.compile_set_literal(items),
            ExprKind::Block(stmts) => self.compile_block_expr(stmts),
            // `x |> f(a)` is `f(x, a)`; `x |> f(a, _)` puts `x` at the placeholder.
            ExprKind::Pipeline { left, right } if matches!(right.kind, ExprKind::Call { .. }) => {
                let ExprKind::Call { callee, args } = &right.kind else {
                    unreachable!()
                };
                let mut args = args.clone();
                let piped = pleno_ast::CallArg::Positional((**left).clone());
                match args
                    .iter()
                    .position(|arg| matches!(arg, pleno_ast::CallArg::Placeholder))
                {
                    Some(index) => args[index] = piped,
                    None => args.insert(0, piped),
                }
                let call = Expr::new(
                    ExprKind::Call {
                        callee: callee.clone(),
                        args,
                    },
                    expr.span,
                );
                self.compile_expr(&call)
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
            ExprKind::SafeIndex { collection, index } => {
                let col_reg = self.compile_expr(collection)?;
                let dest = self.registers.allocate()?;

                let null_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, null_reg, 0, 0);
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, is_null_reg, col_reg, null_reg);
                self.free_temp(null_reg);

                let jump_to_end = self.emit_jump(Opcode::JumpIf, is_null_reg);
                self.optional_chain_jumps.push(jump_to_end);
                self.free_temp(is_null_reg);

                let idx_reg = self.compile_expr(index)?;
                self.emit(Opcode::GetIndex, dest, col_reg, idx_reg);
                self.free_temp(idx_reg);
                self.free_temp(col_reg);

                Ok(dest)
            }
            ExprKind::Slice {
                collection,
                start,
                end,
                inclusive,
            } => self.compile_slice_expr(collection, start.as_deref(), end.as_deref(), *inclusive),
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
                // Arguments passed to `move` parameters become unusable after the call.
                let moved_regs = match match &callee.kind {
                    ExprKind::Ident(name) => self.resolve_fn_signature(name),
                    _ => None,
                } {
                    Some(params) => {
                        let params = params.to_vec();
                        self.enforce_call_ownership(&params, args, expr.span)?
                    }
                    None => Vec::new(),
                };
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

                // `object.name(args)`: look the method up where `GetProperty`
                // would read it, then call it with the receiver, without a bound
                // method in between.
                if let ExprKind::Property { object, name } = &callee.kind
                    && !matches!(object.kind, ExprKind::Super)
                    && args
                        .iter()
                        .all(|arg| matches!(arg, pleno_ast::CallArg::Positional(_)))
                {
                    let object_reg = self.compile_expr(object)?;
                    let name_idx = self.add_constant(Value::String(name.clone().into()));
                    let dest = self.registers.allocate()?;
                    if let Some(block) = self.registers.allocate_block(args.len() + 3) {
                        self.emit(Opcode::GetMethod, block, object_reg, name_idx);
                        self.free_temp(object_reg);
                        for (index, arg) in args.iter().enumerate() {
                            if let pleno_ast::CallArg::Positional(arg_expr) = arg {
                                let start = self.chunk.code.len();
                                let arg_reg = self.compile_expr(arg_expr)?;
                                let slot = block + 3 + index as u8;
                                if !self.retarget_last(arg_reg, slot, start) {
                                    self.emit(Opcode::Move, slot, arg_reg, 0);
                                }
                                self.free_temp(arg_reg);
                            }
                        }
                        self.emit(Opcode::CallMethod, dest, block, args.len() as u8);
                        self.registers.free_block(block, args.len() + 3);
                    } else {
                        let callee_reg = self.registers.allocate()?;
                        self.emit(Opcode::GetProperty, callee_reg, object_reg, name_idx);
                        self.free_temp(object_reg);
                        self.compile_call_internal(callee_reg, args, dest, expr.span)?;
                        self.free_temp(callee_reg);
                    }
                    return Ok(dest);
                }

                let callee_start = self.chunk.code.len();
                let callee_reg = self.compile_expr(callee)?;
                let dest = self.registers.allocate()?;
                self.compile_call_from(callee_reg, Some(callee_start), args, dest, expr.span)?;
                self.free_temp(callee_reg);
                for reg in moved_regs {
                    self.ownership_states.insert(reg, OwnershipState::Moved);
                }
                Ok(dest)
            }
            ExprKind::SafeCall { callee, args } => {
                let callee_reg = self.compile_expr(callee)?;
                let dest = self.registers.allocate()?;

                let null_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadNull, null_reg, 0, 0);
                let is_null_reg = self.registers.allocate()?;
                self.emit(Opcode::Eq, is_null_reg, callee_reg, null_reg);
                self.free_temp(null_reg);

                let jump_to_end = self.emit_jump(Opcode::JumpIf, is_null_reg);
                self.optional_chain_jumps.push(jump_to_end);
                self.free_temp(is_null_reg);

                self.compile_call_internal(callee_reg, args, dest, expr.span)?;
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
            ExprKind::Match { expr, arms } => self.compile_match_expr(expr, arms),
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

    fn constant_index(&mut self, value: Value) -> usize {
        if let Some(idx) = self.chunk.constants.iter().position(|c| c == &value) {
            return idx;
        }
        self.chunk.constants.push(value);
        self.chunk.constants.len() - 1
    }

    /// The index of a constant for an 8-bit operand (a name, a key, ...).
    ///
    /// Only 256 constants fit such operands, but constants that are only ever
    /// loaded with `LoadConst` (16-bit index) can live anywhere, so when the
    /// first 256 slots are full one of those moves up to make room.
    fn add_constant(&mut self, value: Value) -> u8 {
        let mut idx = self.constant_index(value);
        if idx > 255 {
            match (0..=255).find(|slot| !self.pinned_constants.contains(slot)) {
                Some(slot) => {
                    self.swap_constants(slot, idx);
                    idx = slot;
                }
                None => {
                    if self.constant_overflow.is_none() {
                        self.constant_overflow = Some((
                            "this function uses more than 256 different names and constant keys;                              split it into smaller functions"
                                .to_string(),
                            self.current_span,
                        ));
                    }
                    return 0;
                }
            }
        }
        self.pinned_constants.insert(idx);
        idx as u8
    }

    /// Emits `LoadConst reg, idx` with the 16-bit index in operands b and c.
    fn emit_load_constant(&mut self, reg: u8, value: Value) {
        let idx = self.constant_index(value);
        if idx > u16::MAX as usize {
            if self.constant_overflow.is_none() {
                self.constant_overflow = Some((
                    "this function has more than 65536 constants; split it into smaller functions"
                        .to_string(),
                    self.current_span,
                ));
            }
            return;
        }
        self.emit(Opcode::LoadConst, reg, (idx & 0xff) as u8, (idx >> 8) as u8);
    }

    /// Swaps two constants and repoints the `LoadConst` instructions of both.
    fn swap_constants(&mut self, a: usize, b: usize) {
        self.chunk.constants.swap(a, b);
        let loads_a = self.constant_loads.remove(&a).unwrap_or_default();
        let loads_b = self.constant_loads.remove(&b).unwrap_or_default();
        for (positions, target) in [(&loads_a, b), (&loads_b, a)] {
            for &pos in positions {
                let register = self.chunk.code[pos].a();
                self.chunk.code[pos] = Instruction::new(
                    Opcode::LoadConst,
                    register,
                    (target & 0xff) as u8,
                    (target >> 8) as u8,
                );
            }
        }
        if !loads_a.is_empty() {
            self.constant_loads.insert(b, loads_a);
        }
        if !loads_b.is_empty() {
            self.constant_loads.insert(a, loads_b);
        }
    }

    /// Fails if the function needed more constants than its operands can name.
    fn check_constants(&self) -> Result<(), CompileError> {
        match &self.constant_overflow {
            Some((message, span)) => Err(self.error(message, *span)),
            None => Ok(()),
        }
    }

    fn emit(&mut self, opcode: Opcode, a: u8, b: u8, c: u8) {
        if opcode == Opcode::Move && a == b {
            return;
        }
        if opcode == Opcode::LoadConst {
            let idx = b as usize | (c as usize) << 8;
            self.constant_loads
                .entry(idx)
                .or_default()
                .push(self.chunk.code.len());
        }
        self.chunk.code.push(Instruction::new(opcode, a, b, c));
        let line = self._source_map.get_line_col(self.current_span.start).0;
        self.chunk.lines.push(line as u32);
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

    /// Makes the instruction that just computed the temporary `temp` write
    /// `target` instead, so that `target = temp` needs no `Move`. Only for
    /// an instruction emitted at or after `start` (by the expression that
    /// produced `temp`) that only writes its result register, and only when
    /// no jump lands after it (that path would skip the result). The caller
    /// must not read `temp` afterwards.
    /// Makes the `ForceThunk` that just read a callee (a global's value) into
    /// the temporary `temp` write the call's callee register `target` instead,
    /// under the conditions of `retarget_last`. The callee is read before the
    /// arguments either way; only its copy into the call's registers moves.
    fn retarget_callee(&mut self, temp: u8, target: u8, start: usize) -> bool {
        let len = self.chunk.code.len();
        if len == 0
            || len - 1 < start
            || self.jump_targets_end > len - 1
            || self.is_local_reg(temp)
            || Some(temp) == self.last_expr_register
        {
            return false;
        }
        let last = self.chunk.code[len - 1];
        if last.opcode() != Opcode::ForceThunk || last.a() != temp {
            return false;
        }
        self.chunk.code[len - 1] = Instruction::new(last.opcode(), target, last.b(), last.c());
        true
    }

    fn retarget_last(&mut self, temp: u8, target: u8, start: usize) -> bool {
        let len = self.chunk.code.len();
        if len == 0
            || len - 1 < start
            || self.jump_targets_end > len - 1
            || self.is_local_reg(temp)
            || Some(temp) == self.last_expr_register
        {
            return false;
        }
        let last = self.chunk.code[len - 1];
        let pure_result = matches!(
            last.opcode(),
            Opcode::Add
                | Opcode::Sub
                | Opcode::Mul
                | Opcode::Div
                | Opcode::Mod
                | Opcode::AddInt
                | Opcode::SubInt
                | Opcode::Neg
                | Opcode::Not
                | Opcode::Eq
                | Opcode::Ne
                | Opcode::Lt
                | Opcode::Le
                | Opcode::Gt
                | Opcode::Ge
                | Opcode::LoadInt
                | Opcode::LoadConst
                | Opcode::LoadNull
                | Opcode::LoadTrue
                | Opcode::LoadFalse
                | Opcode::GetIndex
                | Opcode::CallRegs
                | Opcode::CallMethod
        );
        if last.a() != temp || !pure_result {
            return false;
        }
        self.chunk.code[len - 1] = Instruction::new(last.opcode(), target, last.b(), last.c());
        true
    }

    fn patch_jump(&mut self, jump_index: usize) -> Result<(), CompileError> {
        let jump_from = jump_index as isize + 1;
        self.jump_targets_end = self.jump_targets_end.max(self.chunk.code.len() + 1);
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
        self.jump_targets_end = self.jump_targets_end.max(jump_to + 1);
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
                    if local.borrow_mut {
                        return Err(self.error(&mut_param_assign_message(name), expr.span));
                    }
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
                } else if let Some((index, info)) = self.captures.get(name).cloned() {
                    if info.borrow_mut {
                        return Err(self.error(&mut_param_assign_message(name), expr.span));
                    }
                    if !info.mutable {
                        return Err(self.error("cannot increment immutable variable", expr.span));
                    }
                    let is_boxed = info.is_boxed;
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

        self.cleanup_stack.push(Cleanup {
            pops_handler: true,
            action: finally_action(finally_branch),
        });
        self.compile_stmt(try_branch)?;
        self.cleanup_stack.pop();

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
            self.cleanup_stack.push(Cleanup {
                pops_handler: false,
                action: finally_action(finally_branch),
            });
            // The catch variable gets its own register: leaving this scope frees
            // it, and `error_reg` must stay live for the following catches.
            let catch_reg = self.registers.allocate()?;
            self.emit(Opcode::Move, catch_reg, error_reg, 0);
            self.insert_local(catch.name.clone(), catch_reg, false, catch.ty.clone())?;
            self.compile_stmt(&catch.body)?;
            self.cleanup_stack.pop();
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
        self.compile_for_iterable(ForBinding::Names(names), iterable, body)
    }

    /// `for ((k, v) in pairs)`: each item is matched against the pattern the
    /// way a destructuring `let` matches its value. Over a map, the items are
    /// its `(key, value)` entries.
    fn compile_for_pattern(
        &mut self,
        pattern: &pleno_ast::BindingPattern,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        if matches!(iterable.kind, ExprKind::Range { .. }) {
            return Err(self.error(
                "a range yields numbers, which a destructuring pattern cannot match",
                pattern.span,
            ));
        }
        if matches!(iterable.kind, ExprKind::Literal(_)) {
            return Err(self.error("for loop expects array, set, or range", iterable.span));
        }
        self.compile_for_iterable(ForBinding::Pattern(pattern), iterable, body)
    }

    fn compile_break_stmt(&mut self, label: Option<&str>, span: Span) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(self.error("break used outside of loop", span));
        }
        let target = self.loop_target(label, span)?;
        self.emit_cleanups(self.loop_stack[target].cleanup_depth)?;
        let jump = self.emit_jump(Opcode::Jump, 0);
        self.loop_stack[target].break_jumps.push(jump);
        Ok(())
    }

    fn compile_continue_stmt(
        &mut self,
        label: Option<&str>,
        span: Span,
    ) -> Result<(), CompileError> {
        if self.loop_stack.is_empty() {
            return Err(self.error("continue used outside of loop", span));
        }
        let target = self.loop_target(label, span)?;
        self.emit_cleanups(self.loop_stack[target].cleanup_depth)?;
        let jump = self.emit_jump(Opcode::Jump, 0);
        self.loop_stack[target].continue_jumps.push(jump);
        Ok(())
    }

    /// The index in `loop_stack` of the loop a `break` or `continue` leaves:
    /// the innermost one, or the one with `label`.
    fn loop_target(&self, label: Option<&str>, span: Span) -> Result<usize, CompileError> {
        match label {
            None => Ok(self.loop_stack.len() - 1),
            Some(target_label) => self
                .loop_stack
                .iter()
                .rposition(|ctx| ctx.labels.iter().any(|l| l == target_label))
                .ok_or_else(|| {
                    self.error(
                        &format!("no loop found with label '{}'", target_label),
                        span,
                    )
                }),
        }
    }

    /// Emits the code that leaving the open `try` and `with` regions above
    /// `depth` runs, innermost first: pop each region's handler, then run its
    /// `finally` block or close its resource. The code of each cleanup is
    /// compiled as if the regions inside it were already closed.
    fn emit_cleanups(&mut self, depth: usize) -> Result<(), CompileError> {
        let saved = self.cleanup_stack.clone();
        let mut result = Ok(());
        while self.cleanup_stack.len() > depth {
            let Some(cleanup) = self.cleanup_stack.pop() else {
                break;
            };
            if cleanup.pops_handler {
                self.emit(Opcode::PopHandler, 0, 0, 0);
            }
            result = match &cleanup.action {
                CleanupAction::None => Ok(()),
                CleanupAction::Finally(stmt) => self.compile_stmt(stmt),
                CleanupAction::Close(name) => self.emit_with_close(name),
            };
            if result.is_err() {
                break;
            }
        }
        self.cleanup_stack = saved;
        result
    }

    /// `with (a = x, b = y) body`: binds `a`, then, with `a` protected, the
    /// rest. See [`Compiler::compile_with_resource`].
    fn compile_with_stmt(
        &mut self,
        resources: &[pleno_ast::WithResource],
        body: &Stmt,
        span: Span,
    ) -> Result<(), CompileError> {
        // Resource names that closures in the body capture are stored in cells,
        // as `let` does.
        let boxed = self.find_boxed_vars(std::slice::from_ref(body));
        self.boxed_vars.push(boxed);
        self.push_scope();
        let result = self.compile_with_resource(resources, body, span);
        self.pop_scope();
        self.boxed_vars.pop();
        result
    }

    /// Binds the first of `resources` and compiles the rest (or the body) so
    /// that the resource is closed however they end:
    ///
    /// ```text
    ///     res = value; check that res has close()
    ///     PushHandler err -> on_error
    ///     <rest>                      // return/break/continue: PopHandler, close
    ///     PopHandler
    ///     res.close()                 // an error here propagates
    ///     Jump end
    /// on_error:
    ///     PushHandler close_err -> suppress
    ///     res.close()
    ///     PopHandler
    ///     Jump rethrow
    /// suppress:
    ///     AddSuppressed err, close_err  // err.suppressed gets the close error
    /// rethrow:
    ///     Throw err                     // the body's error wins
    /// end:
    /// ```
    fn compile_with_resource(
        &mut self,
        resources: &[pleno_ast::WithResource],
        body: &Stmt,
        span: Span,
    ) -> Result<(), CompileError> {
        let Some((resource, rest)) = resources.split_first() else {
            return self.compile_stmt(body);
        };
        self.current_span = resource.value.span;
        // The resource stays in a hidden local, so the variable can be boxed
        // and `close()` still reaches the value.
        let hidden = format!("$with_{}", self.loop_counter);
        self.loop_counter += 1;
        let value_reg = self.compile_expr(&resource.value)?;
        let res_reg = self.registers.allocate()?;
        self.emit(Opcode::Move, res_reg, value_reg, 0);
        self.free_temp(value_reg);
        self.insert_local(hidden.clone(), res_reg, false, None)?;

        // Check for `close` before the body runs. Reading it can throw (an
        // `Int` has no properties); that counts as having none.
        let close_reg = self.registers.allocate()?;
        let read_handler = self.chunk.code.len();
        self.emit(Opcode::PushHandler, close_reg, 0, 0);
        let close_name = self.add_constant(Value::String(Rc::new("close".to_string())));
        self.emit(Opcode::GetProperty, close_reg, res_reg, close_name);
        self.emit(Opcode::PopHandler, 0, 0, 0);
        let to_check = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(read_handler)?;
        self.emit(Opcode::LoadNull, close_reg, 0, 0);
        self.patch_jump(to_check)?;
        self.emit(Opcode::CheckClose, res_reg, close_reg, 0);
        self.free_temp(close_reg);

        let var_reg = self.registers.allocate()?;
        if self
            .boxed_vars
            .last()
            .is_some_and(|boxed| boxed.contains(&resource.name))
        {
            self.emit(Opcode::MakeCell, var_reg, res_reg, 0);
        } else {
            self.emit(Opcode::Move, var_reg, res_reg, 0);
        }
        self.insert_local(resource.name.clone(), var_reg, false, None)?;

        let error_reg = self.registers.allocate()?;
        let error_name = format!("$with_error_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(error_name, error_reg, false, None)?;
        let push_handler = self.chunk.code.len();
        self.emit(Opcode::PushHandler, error_reg, 0, 0);
        self.cleanup_stack.push(Cleanup {
            pops_handler: true,
            action: CleanupAction::Close(hidden.clone()),
        });
        let result = self.compile_with_resource(rest, body, span);
        self.cleanup_stack.pop();
        result?;
        self.emit(Opcode::PopHandler, 0, 0, 0);
        self.current_span = span;
        self.emit_with_close(&hidden)?;
        let to_end = self.emit_jump(Opcode::Jump, 0);

        // The body (or a later resource) threw: close, record an error from
        // `close()` on the first error, and throw the first error again.
        self.patch_jump(push_handler)?;
        let close_error_reg = self.registers.allocate()?;
        let close_handler = self.chunk.code.len();
        self.emit(Opcode::PushHandler, close_error_reg, 0, 0);
        self.emit_with_close(&hidden)?;
        self.emit(Opcode::PopHandler, 0, 0, 0);
        let to_rethrow = self.emit_jump(Opcode::Jump, 0);
        self.patch_jump(close_handler)?;
        self.emit(Opcode::AddSuppressed, error_reg, close_error_reg, 0);
        self.patch_jump(to_rethrow)?;
        self.free_temp(close_error_reg);
        self.emit(Opcode::Throw, error_reg, 0, 0);
        self.patch_jump(to_end)?;
        Ok(())
    }

    /// `name.close()`, for the hidden local of a `with` resource.
    fn emit_with_close(&mut self, name: &str) -> Result<(), CompileError> {
        let span = self.current_span;
        let call = Expr::new(
            ExprKind::Call {
                callee: Box::new(Expr::new(
                    ExprKind::Property {
                        object: Box::new(Expr::new(ExprKind::Ident(name.to_string()), span)),
                        name: "close".to_string(),
                    },
                    span,
                )),
                args: Vec::new(),
            },
            span,
        );
        let reg = self.compile_expr(&call)?;
        self.free_temp(reg);
        Ok(())
    }

    fn compile_call_internal(
        &mut self,
        callee_reg: u8,
        args: &[pleno_ast::CallArg],
        dest: u8,
        span: Span,
    ) -> Result<(), CompileError> {
        self.compile_call_from(callee_reg, None, args, dest, span)
    }

    /// `compile_call_internal`, for a callee whose code starts at
    /// `callee_start` (when known) and ends just before this call.
    fn compile_call_from(
        &mut self,
        callee_reg: u8,
        callee_start: Option<usize>,
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
            self.emit(Opcode::NewMap, wrapper_reg, 0, 0);

            // Set "positional" key
            let pos_key_idx =
                self.add_constant(Value::String(std::rc::Rc::new("positional".to_string())));
            let pos_key_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, pos_key_reg, pos_key_idx, 0);
            self.emit(Opcode::SetIndex, positional_reg, wrapper_reg, pos_key_reg);
            self.free_temp(pos_key_reg);

            // Set "named" key
            let named_key_idx =
                self.add_constant(Value::String(std::rc::Rc::new("named".to_string())));
            let named_key_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, named_key_reg, named_key_idx, 0);
            self.emit(Opcode::SetIndex, named_reg, wrapper_reg, named_key_reg);
            self.free_temp(named_key_reg);

            self.emit(Opcode::CallNamed, dest, callee_reg, wrapper_reg);
            self.free_temp(positional_reg);
            self.free_temp(named_reg);
            self.free_temp(wrapper_reg);
            Ok(())
        } else {
            // Positional arguments only: pass them in consecutive registers
            // after a copy of the callee, without building an argument array.
            let all_positional = args
                .iter()
                .all(|arg| matches!(arg, pleno_ast::CallArg::Positional(_)));
            if all_positional && let Some(block) = self.registers.allocate_block(args.len() + 1) {
                // A global callee is read straight into the call's registers.
                let callee_in_block = callee_start
                    .is_some_and(|start| self.retarget_callee(callee_reg, block, start));
                for (index, arg) in args.iter().enumerate() {
                    if let pleno_ast::CallArg::Positional(expr) = arg {
                        let start = self.chunk.code.len();
                        let arg_reg = self.compile_expr(expr)?;
                        let slot = block + 1 + index as u8;
                        if !self.retarget_last(arg_reg, slot, start) {
                            self.emit(Opcode::Move, slot, arg_reg, 0);
                        }
                        self.free_temp(arg_reg);
                    }
                }
                // Read the callee after the arguments, as `Call` does.
                if !callee_in_block {
                    self.emit(Opcode::Move, block, callee_reg, 0);
                }
                self.emit(Opcode::CallRegs, dest, block, args.len() as u8);
                self.registers.free_block(block, args.len() + 1);
                return Ok(());
            }
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

        self.emit(Opcode::AddInt, start_reg, start_reg, 1);

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

        self.emit(Opcode::AddInt, start_reg, start_reg, 1);

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
        binding: ForBinding<'_>,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.push_scope();
        let source_reg = self.compile_expr(iterable)?;
        let iter_reg = self.registers.allocate()?;

        let mode = match binding {
            ForBinding::Names(names) if names.len() > 1 => pleno_bytecode::ITER_ENTRIES,
            ForBinding::Names(_) => pleno_bytecode::ITER_VALUES,
            ForBinding::Pattern(_) => pleno_bytecode::ITER_MAP_ENTRIES,
        };
        self.emit(Opcode::GetIterator, iter_reg, source_reg, mode);
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

        if let ForBinding::Pattern(pattern) = binding {
            // Loop variables are reassignable, as in the plain form.
            self.compile_binding_pattern(pattern, item_reg, true)?;
            self.free_temp(item_reg);
        } else if let ForBinding::Names([name]) = binding {
            self.insert_local(name.clone(), item_reg, true, None)?;
        } else if let ForBinding::Names(names) = binding {
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

    /// Binds the names of a destructuring `let` pattern from `value_reg`.
    /// The VM checks the shape (`Opcode::Destructure`) before any element is read.
    fn compile_binding_pattern(
        &mut self,
        pattern: &pleno_ast::BindingPattern,
        value_reg: u8,
        mutable: bool,
    ) -> Result<(), CompileError> {
        use pleno_ast::BindingPatternKind;
        match &pattern.kind {
            BindingPatternKind::Ignore => Ok(()),
            BindingPatternKind::Name(name) => {
                let reg = self.registers.allocate()?;
                self.emit(Opcode::Move, reg, value_reg, 0);
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
                let previous_span = self.current_span;
                self.current_span = pattern.span;
                let result = self.insert_local(name.clone(), reg, mutable, None);
                self.current_span = previous_span;
                result
            }
            BindingPatternKind::Tuple(items) => {
                let count = self.pattern_len(items.len(), pattern.span)?;
                self.emit_destructure(
                    value_reg,
                    pleno_bytecode::DESTRUCTURE_TUPLE,
                    count,
                    pattern.span,
                );
                self.bind_pattern_elements(items, value_reg, mutable)
            }
            BindingPatternKind::Array { elements, rest } => {
                let count = self.pattern_len(elements.len(), pattern.span)?;
                let mode = if rest.is_some() {
                    pleno_bytecode::DESTRUCTURE_ARRAY_AT_LEAST
                } else {
                    pleno_bytecode::DESTRUCTURE_ARRAY
                };
                self.emit_destructure(value_reg, mode, count, pattern.span);
                self.bind_pattern_elements(elements, value_reg, mutable)?;
                if let Some(rest) = rest
                    && let BindingPatternKind::Name(_) = rest.kind
                {
                    // rest = value.slice(start: count)
                    let args_reg = self.registers.allocate()?;
                    self.emit(Opcode::NewMap, args_reg, 0, 0);
                    let start_key_idx =
                        self.add_constant(Value::String(Rc::new("start".to_string())));
                    let start_key_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadConst, start_key_reg, start_key_idx, 0);
                    let start_val_reg = self.registers.allocate()?;
                    self.emit(Opcode::LoadInt, start_val_reg, count, 0);
                    self.emit(Opcode::SetIndex, start_val_reg, args_reg, start_key_reg);
                    self.free_temp(start_key_reg);
                    self.free_temp(start_val_reg);
                    let rest_reg = self.registers.allocate()?;
                    self.emit(Opcode::Slice, rest_reg, value_reg, args_reg);
                    self.free_temp(args_reg);
                    self.compile_binding_pattern(rest, rest_reg, mutable)?;
                    self.free_temp(rest_reg);
                }
                Ok(())
            }
        }
    }

    /// Emits the shape check with the pattern's span, so an error points at it.
    fn emit_destructure(&mut self, value_reg: u8, mode: u8, count: u8, span: Span) {
        let previous_span = self.current_span;
        self.current_span = span;
        self.emit(Opcode::Destructure, value_reg, mode, count);
        self.current_span = previous_span;
    }

    fn pattern_len(&self, len: usize, span: Span) -> Result<u8, CompileError> {
        u8::try_from(len)
            .map_err(|_| self.error("a destructuring pattern has at most 255 elements", span))
    }

    fn bind_pattern_elements(
        &mut self,
        items: &[pleno_ast::BindingPattern],
        value_reg: u8,
        mutable: bool,
    ) -> Result<(), CompileError> {
        for (i, item) in items.iter().enumerate() {
            if matches!(item.kind, pleno_ast::BindingPatternKind::Ignore) {
                continue;
            }
            let idx_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadInt, idx_reg, i as u8, 0);
            let element_reg = self.registers.allocate()?;
            self.emit(Opcode::GetIndex, element_reg, value_reg, idx_reg);
            self.free_temp(idx_reg);
            self.compile_binding_pattern(item, element_reg, mutable)?;
            self.free_temp(element_reg);
        }
        Ok(())
    }

    fn compile_pattern(
        &mut self,
        pattern: &Pattern,
        value_reg: u8,
        fail_jumps: &mut Vec<usize>,
    ) -> Result<(), CompileError> {
        match &pattern.kind {
            pleno_ast::PatternKind::Literal(lit) => {
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
            pleno_ast::PatternKind::Ident(name) => {
                let reg = self.registers.allocate()?;
                self.emit(Opcode::Move, reg, value_reg, 0);
                self.insert_local(name.clone(), reg, false, None)?;
            }
            pleno_ast::PatternKind::Underscore => {}
            pleno_ast::PatternKind::Tuple(patterns) => {
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

                let has_rest = patterns
                    .iter()
                    .any(|p| matches!(p.kind, pleno_ast::PatternKind::Rest(_)));
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
                    if let pleno_ast::PatternKind::Rest(name) = &p.kind {
                        let args_reg = self.registers.allocate()?;
                        self.emit(Opcode::NewMap, args_reg, 0, 0);

                        // Set "start" = i
                        let start_key_val = Value::String(std::rc::Rc::new("start".to_string()));
                        let start_key_idx = self.add_constant(start_key_val);
                        let start_key_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadConst, start_key_reg, start_key_idx, 0);

                        let start_val_reg = self.registers.allocate()?;
                        self.emit(Opcode::LoadInt, start_val_reg, i as u8, 0);

                        // args[start_key] = start_val
                        // SetIndex: a=val, b=map, c=key
                        self.emit(Opcode::SetIndex, start_val_reg, args_reg, start_key_reg);

                        self.free_temp(start_key_reg);
                        self.free_temp(start_val_reg);

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
            pleno_ast::PatternKind::Variant(name, patterns) => {
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
            pleno_ast::PatternKind::Object(props, rest) => {
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
            pleno_ast::PatternKind::Type(name, binding) => {
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
            pleno_ast::PatternKind::Or(patterns) => {
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
            pleno_ast::PatternKind::Rest(name) => {
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
        // The function's instructions map back to lines of the same source.
        let mut compiler = Compiler::new(self.source.clone());
        compiler.scope_names = Rc::clone(&self.scope_names);
        compiler.scope_head = self.scope_head;
        compiler.is_top_level = false;
        compiler.is_async = decl.is_async;
        // Calls inside the function check ownership against enclosing
        // signatures, and annotations use the enclosing type aliases.
        compiler.fn_signatures = self.fn_signatures.clone();
        compiler.type_aliases = self.type_aliases.clone();
        compiler.declaration_kinds = self.declaration_kinds.clone();
        compiler.push_scope();
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, info)| (info.name.clone(), (idx, info.clone())))
            .collect();
        compiler.current_super = current_super;
        self.setup_return_check(&mut compiler, decl);
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
            // A `&` (read-only borrow) parameter cannot be reassigned.
            let borrowed = decl
                .params
                .iter()
                .any(|p| p.name == *param && p.ownership == pleno_ast::Ownership::Borrow);
            compiler.insert_local(param.clone(), reg, !borrowed, ty)?;
            // A `&mut` parameter cannot be reassigned either: the new value
            // would not reach the caller's variable.
            if decl
                .params
                .iter()
                .any(|p| p.name == *param && p.ownership == pleno_ast::Ownership::BorrowMut)
                && let Some(local) = compiler.locals.last_mut().and_then(|s| s.get_mut(param))
            {
                local.borrow_mut = true;
            }
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

            compiler.emit_param_type_check(param, &decl.generics, decl.span)?;
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
        let return_reg = compiler.emit_return_check(return_reg)?;
        compiler.emit(Opcode::Return, return_reg, 0, 0);
        compiler.check_constants()?;
        compiler.record_open_locals();
        compiler.chunk.max_registers = compiler.registers.max_used.max(params.len());
        compiler.chunk.module_name = self.module_name.clone();
        self.global_reads.append(&mut compiler.global_reads);
        self.global_defs.extend(compiler.global_defs.drain());
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
                runtime_annotations.push(self.compile_annotation(anno)?);
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
            type_params: decl
                .generics
                .iter()
                .map(|g| pleno_value::GenericParamMetadata {
                    name: g.name.clone(),
                    variance: g.variance,
                })
                .collect(),
            type_constraints,
            annotations: runtime_annotations,
        });
        let dest = self.registers.allocate()?;
        if store_as_function && captures.is_empty() {
            self.emit(Opcode::FunctionRef, dest, func_index as u8, 0);
        } else {
            let capture_count = captures.len() as u8;
            log::trace!(
                "Emitting Closure for function {} with {} captures",
                func_index,
                capture_count
            );
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
                log::trace!("Emitting Capture: dest={}, reg={}, idx={}", dest, reg, idx);
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

            self.emit_decorator(annotation, dest)?;
        }

        compiler.pop_scope();
        Ok(dest)
    }

    /// Applies `@name` or `@name(args)` to the function or class in `target`
    /// when `name` refers to a callable at runtime; other annotations are
    /// metadata only. A decorator that returns null leaves the target unchanged.
    fn emit_decorator(
        &mut self,
        annotation: &pleno_ast::Annotation,
        target: u8,
    ) -> Result<(), CompileError> {
        let no_span = Span::new(0, 0);
        let name = Expr::new(ExprKind::Ident(annotation.name.clone()), no_span);
        // An annotation that names nothing (`@Version("1.0")`) is metadata, so
        // the global lookup must not fail when the name is undefined.
        let callee_reg = if self.resolve_local(&annotation.name).is_some() {
            self.compile_expr(&name)?
        } else {
            let reg = self.registers.allocate()?;
            let name_idx = self.add_constant(Value::String(annotation.name.clone().into()));
            self.emit(Opcode::GetGlobal, reg, name_idx, 0);
            reg
        };

        let fn_type_idx = self.add_constant(Value::String(Rc::new("() => Any".to_string())));
        let fn_type_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, fn_type_reg, fn_type_idx, 0);
        let is_callable_reg = self.registers.allocate()?;
        self.emit(Opcode::Is, is_callable_reg, callee_reg, fn_type_reg);
        let skip_jump = self.emit_jump(Opcode::JumpIfNot, is_callable_reg);
        // A class used as an annotation (`@Entity`, `@Route("/users")`) is
        // metadata, not a decorator, even though classes are callable.
        let class_name_idx = self.add_constant(Value::String(Rc::new("Class".to_string())));
        self.emit(Opcode::TypeOf, is_callable_reg, callee_reg, 0);
        self.emit(Opcode::LoadConst, fn_type_reg, class_name_idx, 0);
        self.emit(Opcode::Eq, is_callable_reg, is_callable_reg, fn_type_reg);
        let skip_class_jump = self.emit_jump(Opcode::JumpIf, is_callable_reg);
        self.free_temp(fn_type_reg);
        self.free_temp(is_callable_reg);

        // `@factory(args)` first calls the factory to get the decorator. It
        // runs only when the name is callable, so it is not a global read.
        let reads_before = self.global_reads.len();
        let decorator_reg = if annotation.args.is_empty() {
            callee_reg
        } else {
            let call = Expr::new(
                ExprKind::Call {
                    callee: Box::new(name),
                    args: annotation
                        .args
                        .iter()
                        .map(|arg| pleno_ast::CallArg::Positional(arg.clone()))
                        .collect(),
                },
                no_span,
            );
            self.compile_expr(&call)?
        };
        self.global_reads.truncate(reads_before);
        // A factory that returns something other than a function is metadata.
        let skip_result_jump = if decorator_reg != callee_reg {
            let fn_type_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, fn_type_reg, fn_type_idx, 0);
            let is_callable_reg = self.registers.allocate()?;
            self.emit(Opcode::Is, is_callable_reg, decorator_reg, fn_type_reg);
            let jump = self.emit_jump(Opcode::JumpIfNot, is_callable_reg);
            self.free_temp(fn_type_reg);
            self.free_temp(is_callable_reg);
            Some(jump)
        } else {
            None
        };

        let args_reg = self.registers.allocate()?;
        self.emit(Opcode::NewArray, args_reg, 0, 0);
        self.emit(Opcode::ArrayAppend, args_reg, target, 0);
        let result_reg = self.registers.allocate()?;
        self.emit(Opcode::Call, result_reg, decorator_reg, args_reg);
        self.free_temp(args_reg);

        let is_null_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadNull, is_null_reg, 0, 0);
        self.emit(Opcode::Eq, is_null_reg, result_reg, is_null_reg);
        let keep_jump = self.emit_jump(Opcode::JumpIf, is_null_reg);
        self.emit(Opcode::Move, target, result_reg, 0);
        self.patch_jump(keep_jump)?;
        self.free_temp(is_null_reg);
        self.free_temp(result_reg);

        if let Some(jump) = skip_result_jump {
            self.patch_jump(jump)?;
        }
        self.patch_jump(skip_class_jump)?;
        self.patch_jump(skip_jump)?;
        if decorator_reg != callee_reg {
            self.free_temp(decorator_reg);
        }
        self.free_temp(callee_reg);
        Ok(())
    }

    /// Checks a parameter's annotation at function entry, throwing
    /// `TypeError: parameter 'x' expected Int, got String` on a mismatch; an
    /// `Int` passed for a `Float` annotation becomes a `Float`. Emits nothing
    /// for an unannotated or `Any` parameter. `generics` are the function's
    /// type parameters, whose constraints the message names.
    fn emit_param_type_check(
        &mut self,
        param: &pleno_ast::Param,
        generics: &[pleno_ast::GenericParam],
        span: Span,
    ) -> Result<(), CompileError> {
        let Some(ty_name) = param.ty.as_deref().filter(|ty| *ty != "Any") else {
            return Ok(());
        };
        let param_reg = self
            .resolve_local(&param.name)
            .ok_or_else(|| self.error("unknown parameter", span))?
            .reg;
        let type_reg = self.registers.allocate()?;

        // Resolve type identifier (it could be a local, capture or global)
        if let Some(local) = self.resolve_local(ty_name) {
            self.emit(Opcode::Move, type_reg, local.reg, 0);
        } else if let Some((idx, _)) = self.captures.get(ty_name) {
            self.emit(Opcode::LoadCapture, type_reg, *idx as u8, 0);
        } else {
            let reified = self.emit_reified_type(ty_name)?;
            self.emit(Opcode::Move, type_reg, reified, 0);
            self.free_temp(reified);
        }

        let result_reg = self.registers.allocate()?;
        self.emit(Opcode::Is, result_reg, param_reg, type_reg);

        let skip_jump = self.emit_jump(Opcode::JumpIf, result_reg);

        // A constrained type parameter names its constraint: `T: Named`.
        let expected = match generics
            .iter()
            .find(|g| g.name == ty_name && !g.constraints.is_empty())
        {
            Some(generic) => format!("{}: {}", ty_name, generic.constraints.join(" & ")),
            None => ty_name.to_string(),
        };
        let msg = format!(
            "TypeError: parameter '{}' expected {}, got ",
            param.name, expected
        );
        let msg_reg = self.registers.allocate()?;
        let msg_const = self.add_constant(Value::String(Rc::new(msg)));
        self.emit(Opcode::LoadConst, msg_reg, msg_const, 0);

        // Add current value's type to the message for better debugging
        let type_name_reg = self.registers.allocate()?;
        self.emit(Opcode::TypeOf, type_name_reg, param_reg, 0);
        self.emit(Opcode::Add, msg_reg, msg_reg, type_name_reg);

        // `c = 1`: thrown as an `Error`, like the VM's own errors. The error
        // points at the parameter.
        let previous_span = self.current_span;
        self.current_span = param.span;
        self.emit(Opcode::Throw, msg_reg, 0, 1);
        self.current_span = previous_span;

        self.patch_jump(skip_jump)?;
        if self.widens_int(ty_name) {
            self.emit(Opcode::ToFloat, param_reg, param_reg, 0);
        }

        self.free_temp(type_reg);
        self.free_temp(result_reg);
        self.free_temp(msg_reg);
        self.free_temp(type_name_reg);
        Ok(())
    }

    fn compile_lambda(
        &mut self,
        params: &[pleno_ast::Param],
        body: &pleno_ast::Expr,
        return_ty: Option<&str>,
        is_pure: bool,
        annotations: &[pleno_ast::Annotation],
        is_async: bool,
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
            name_span: Span::new(0, 0),
            generics: Vec::new(),
            params: params.to_vec(),
            return_ty: return_ty.map(|s| s.to_string()),
            body: Box::new(body_stmt),
            is_static: false,
            is_abstract: false,
            is_async,
            visibility: None,
            kind: FunctionKind::Method,
            annotations: Vec::new(),
            doc: None,
        };
        // An async lambda compiles like an async function declaration, which
        // sets up its promise.
        if is_async {
            return self.compile_function(&decl, false, true, None, None);
        }

        let captures = self.analyze_captures(&decl, false);
        let mut compiler = Compiler::new(self.source.clone());
        compiler.scope_names = Rc::clone(&self.scope_names);
        compiler.scope_head = self.scope_head;
        compiler.is_top_level = false;
        compiler.fn_signatures = self.fn_signatures.clone();
        compiler.type_aliases = self.type_aliases.clone();
        compiler.declaration_kinds = self.declaration_kinds.clone();
        compiler.push_scope();
        compiler.module_name = self.module_name.clone();
        compiler.captures = captures
            .iter()
            .enumerate()
            .map(|(idx, info)| (info.name.clone(), (idx, info.clone())))
            .collect();
        // Before the parameter checks: they need the type parameters in scope.
        self.setup_return_check(&mut compiler, &decl);

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
            // A type parameter of an enclosing function is not known in the
            // lambda's own frame, so such an annotation is left unchecked.
            let mentions_generic = param.ty.as_deref().is_some_and(|ty| {
                mentions_any(
                    &compiler.expand_type_aliases_deep(ty),
                    &compiler.generic_names,
                )
            });
            if !mentions_generic {
                compiler.emit_param_type_check(param, &[], param.span)?;
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
        let return_reg = compiler.emit_return_check(return_reg)?;
        compiler.emit(Opcode::Return, return_reg, 0, 0);
        compiler.check_constants()?;
        compiler.record_open_locals();
        compiler.chunk.max_registers = compiler.registers.max_used.max(params.len());
        compiler.chunk.module_name = self.module_name.clone();
        self.global_reads.append(&mut compiler.global_reads);
        self.global_defs.extend(compiler.global_defs.drain());

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
        log::trace!(
            "Emitting Closure for function {} with {} captures",
            func_index,
            capture_count
        );
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
            log::trace!("Emitting Capture: dest={}, reg={}, idx={}", dest, reg, idx);
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
            if let Some(ty) = &param.ty
                && !ty.is_empty()
                && ty.chars().next().unwrap().is_alphabetic()
                && ty != "Any"
                && ty != "Int"
                && ty != "Float"
                && ty != "Bool"
                && ty != "String"
            {
                used.insert(ty.clone(), true);
            }
        }
        self.collect_idents(&decl.body, &mut used, &mut scope_stack);
        if let Some(ret_ty) = &decl.return_ty
            && !ret_ty.is_empty()
            && ret_ty.chars().next().unwrap().is_alphabetic()
            && ret_ty != "Any"
            && ret_ty != "Int"
            && ret_ty != "Float"
            && ret_ty != "Bool"
            && ret_ty != "String"
        {
            used.insert(ret_ty.clone(), true);
        }

        let mut captures = Vec::new();
        for name in used.keys() {
            if !locals_map.contains_key(name) {
                if let Some(local) = self.resolve_local(name) {
                    captures.push(CaptureInfo {
                        name: name.clone(),
                        is_boxed: local.is_boxed,
                        ty: local.ty.clone(),
                        mutable: local.mutable,
                        borrow_mut: local.borrow_mut,
                    });
                } else if let Some((_, info)) = self.captures.get(name) {
                    captures.push(CaptureInfo {
                        name: name.clone(),
                        is_boxed: info.is_boxed,
                        ty: info.ty.clone(),
                        mutable: info.mutable,
                        borrow_mut: info.borrow_mut,
                    });
                }
            }
        }
        captures
    }

    fn compile_extend_stmt(
        &mut self,
        target: &str,
        generics: &[pleno_ast::GenericParam],
        methods: &[pleno_ast::FunctionDecl],
    ) -> Result<(), CompileError> {
        let saved_owner = self
            .owner_context
            .replace(OwnerContext::new(target, generics));
        let class_reg = self.registers.allocate()?;

        // Resolve target class; built-in types are passed by name.
        if BUILTIN_EXTENSION_TYPES.contains(&target) {
            let name_idx = self.add_constant(Value::String(std::rc::Rc::new(target.to_string())));
            self.emit(Opcode::LoadConst, class_reg, name_idx, 0);
        } else if let Some(local) = self.resolve_local(target) {
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
        self.owner_context = saved_owner;
        Ok(())
    }

    /// Only an abstract class may declare abstract methods.
    fn check_abstract_methods(&self, stmt: &Stmt) -> Result<(), CompileError> {
        if let StmtKind::Class {
            name,
            methods,
            is_abstract: false,
            ..
        } = &stmt.kind
            && let Some(method) = methods.iter().find(|m| m.is_abstract)
        {
            return Err(self.error(
                &format!(
                    "abstract method '{}' in class '{}', which is not abstract",
                    method.name, name
                ),
                method.span,
            ));
        }
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
        self.check_abstract_methods(stmt)?;
        let saved_owner = self
            .owner_context
            .replace(OwnerContext::new(name, generics));

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
            // A subclass without a constructor forwards its arguments to the parent:
            // `constructor(...args) { super(...args); }`.
            let (params, body) = if superclass.is_some() {
                let no_span = Span::new(0, 0);
                let args_ident = Expr::new(ExprKind::Ident("__args".to_string()), no_span);
                let super_call = Expr::new(
                    ExprKind::Call {
                        callee: Box::new(Expr::new(ExprKind::Super, no_span)),
                        args: vec![pleno_ast::CallArg::Spread(args_ident)],
                    },
                    no_span,
                );
                let params = vec![pleno_ast::Param {
                    span: no_span,
                    name: "__args".to_string(),
                    name_span: no_span,
                    ty: None,
                    default: None,
                    is_variadic: true,
                    is_keyword_only: false,
                    ownership: pleno_ast::Ownership::None,
                }];
                (params, vec![Stmt::new(StmtKind::Expr(super_call), no_span)])
            } else {
                (Vec::new(), Vec::new())
            };
            methods_to_compile.push(FunctionDecl {
                span: Span::new(0, 0),
                name: "constructor".to_string(),
                name_span: Span::new(0, 0),
                params,
                body: Box::new(Stmt::new(StmtKind::Block(body), Span::new(0, 0))),
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
                let meta_reg = self.registers.allocate()?;
                self.emit(Opcode::NewMap, meta_reg, 0, 0);

                let name_idx = self.add_constant(Value::String(std::rc::Rc::new(g.name.clone())));
                let name_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadConst, name_reg, name_idx, 0);
                let name_key_idx =
                    self.add_constant(Value::String(std::rc::Rc::new("name".to_string())));
                self.emit(Opcode::SetProperty, name_reg, meta_reg, name_key_idx);

                let var_reg = self.registers.allocate()?;
                self.emit(Opcode::LoadInt, var_reg, g.variance as u8, 0);
                let var_key_idx =
                    self.add_constant(Value::String(std::rc::Rc::new("variance".to_string())));
                self.emit(Opcode::SetProperty, var_reg, meta_reg, var_key_idx);
                self.free_temp(var_reg);

                self.emit(Opcode::ArrayAppend, params_reg, meta_reg, 0);

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
                self.free_temp(meta_reg);
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
        // `c = 1`: record one abstract method, which subclasses must define.
        for method in methods.iter().filter(|m| m.is_abstract && !m.is_static) {
            let method_idx = self.add_constant(Value::String(Rc::new(method.name.clone())));
            self.emit(Opcode::SetAbstract, class_reg, method_idx, 1);
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
            // Bit 4 marks a `const` field, which cannot be reassigned.
            let vis_code = visibility_code(field.visibility.as_ref())
                | if field.is_const { CONST_FIELD_FLAG } else { 0 };
            if field.is_static {
                let value_reg = if let Some(default) = &field.default {
                    let reg = self.compile_expr(default)?;
                    // `static rate: Float = 1;` stores a Float, as an assignment would.
                    match &field.ty {
                        Some(ty) if self.widens_int(ty) => {
                            let dest = self.registers.allocate()?;
                            self.emit(Opcode::ToFloat, dest, reg, 0);
                            self.free_temp(reg);
                            dest
                        }
                        _ => reg,
                    }
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
                // The VM checks fields from this text, so aliases are expanded.
                let ty_name = self.expand_type_aliases_deep(ty_name);
                let ty_idx = self.add_constant(Value::String(Rc::new(ty_name)));
                if field.is_static {
                    self.emit(Opcode::SetStaticFieldType, class_reg, field_idx, ty_idx);
                } else {
                    self.emit(Opcode::SetFieldType, class_reg, field_idx, ty_idx);
                }
            }
        }

        // Annotations are stored as metadata via SetAnnotations; those naming a
        // function are also applied as class decorators.
        for annotation in annotations.iter().rev() {
            self.emit_decorator(annotation, class_reg)?;
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
        // Compile Implements
        if !implements.is_empty() {
            let interfaces_reg = self.registers.allocate()?;
            self.emit(Opcode::NewArray, interfaces_reg, 0, 0);
            for interface_name in implements {
                let interface_reg = self.registers.allocate()?;

                // Check if this is a generic type (contains '<')
                if let Some(lt_pos) = interface_name.find('<') {
                    // Parse generic: "Container<Int>" -> base="Container", args=["Int"]
                    if !interface_name.ends_with('>') || lt_pos + 1 >= interface_name.len() - 1 {
                        return Err(self.error(
                            &format!("invalid interface type '{interface_name}'"),
                            self.current_span,
                        ));
                    }
                    let base_name = &interface_name[..lt_pos];
                    let args_str = &interface_name[lt_pos + 1..interface_name.len() - 1];

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
        self.owner_context = saved_owner;
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
            if let Some(value) = &variant.value {
                let constant = match &value.kind {
                    ExprKind::Literal(lit) => literal_value(lit),
                    ExprKind::Unary {
                        op: pleno_ast::UnaryOp::Negate,
                        expr,
                    } => match &expr.kind {
                        ExprKind::Literal(Literal::Int(i)) => Some(Value::Int(-i)),
                        ExprKind::Literal(Literal::Float(f)) => Some(Value::Float(-f)),
                        _ => None,
                    },
                    _ => None,
                }
                .ok_or_else(|| self.error("enum variant values must be literals", value.span))?;
                let value_idx = self.add_constant(constant);
                self.emit(
                    Opcode::SetVariantValue,
                    enum_reg,
                    variant_name_idx,
                    value_idx,
                );
            }
        }

        let saved_owner = self
            .owner_context
            .replace(OwnerContext::new(&decl.name, &[]));
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
        self.owner_context = saved_owner;

        Ok(enum_reg)
    }

    fn collect_locals(&self, stmt: &Stmt, locals: &mut HashMap<String, ()>) {
        match &stmt.kind {
            StmtKind::Let { name, .. } => {
                locals.insert(name.clone(), ());
            }
            StmtKind::LetPattern { pattern, .. } => {
                for (name, _) in pattern.bindings() {
                    locals.insert(name.to_string(), ());
                }
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
            StmtKind::With { resources, body } => {
                for resource in resources {
                    locals.insert(resource.name.clone(), ());
                }
                self.collect_locals(body, locals)
            }
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
            StmtKind::LetPattern { pattern, value, .. } => {
                self.collect_idents_expr(value, used, scope);
                if let Some(s) = scope.last_mut() {
                    for (name, _) in pattern.bindings() {
                        s.insert(name.to_string());
                    }
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
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.collect_idents(try_branch, used, scope);
                for catch in catch_branches {
                    scope.push(HashSet::from([catch.name.clone()]));
                    self.collect_idents(&catch.body, used, scope);
                    scope.pop();
                }
                if let Some(finally_branch) = finally_branch {
                    self.collect_idents(finally_branch, used, scope);
                }
            }
            StmtKind::With { resources, body } => {
                scope.push(HashSet::new());
                for resource in resources {
                    self.collect_idents_expr(&resource.value, used, scope);
                    scope.last_mut().unwrap().insert(resource.name.clone());
                }
                self.collect_idents(body, used, scope);
                scope.pop();
            }
            StmtKind::Throw(expr) => self.collect_idents_expr(expr, used, scope),
            StmtKind::DoWhile { body, condition } => {
                self.collect_idents(body, used, scope);
                self.collect_idents_expr(condition, used, scope);
            }
            StmtKind::Labeled { stmt, .. } | StmtKind::Export(stmt) => {
                self.collect_idents(stmt, used, scope);
            }
            StmtKind::Enum(decl) => {
                if let Some(s) = scope.last_mut() {
                    s.insert(decl.name.clone());
                }
                self.collect_method_idents(&decl.methods, used, scope);
            }
            StmtKind::Extend { methods, .. } => {
                self.collect_method_idents(methods, used, scope);
            }
            _ => {}
        }
    }

    fn collect_method_idents(
        &self,
        methods: &[pleno_ast::FunctionDecl],
        used: &mut HashMap<String, bool>,
        scope: &mut Vec<HashSet<String>>,
    ) {
        for method in methods {
            scope.push(HashSet::from(["this".to_string()]));
            for p in &method.params {
                if let Some(d) = &p.default {
                    self.collect_idents_expr(d, used, scope);
                }
                scope.last_mut().unwrap().insert(p.name.clone());
            }
            self.collect_idents(&method.body, used, scope);
            scope.pop();
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
                ..
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
                    scope.push(HashSet::new());
                    self.collect_pattern_idents(&arm.pattern, scope);
                    if let Some(guard) = &arm.guard {
                        self.collect_idents_expr(guard, used, scope);
                    }
                    self.collect_idents_expr(&arm.body, used, scope);
                    scope.pop();
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
                if let Some(eb) = else_branch {
                    self.collect_idents_expr(eb, used, scope);
                }
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
            ExprKind::SafeIndex { collection, index } => {
                self.collect_idents_expr(collection, used, scope);
                self.collect_idents_expr(index, used, scope);
            }
            ExprKind::Is { left, .. } | ExprKind::Specialize { left, .. } => {
                self.collect_idents_expr(left, used, scope);
            }
            ExprKind::Spread(inner) => self.collect_idents_expr(inner, used, scope),
            ExprKind::NamedTupleLiteral(items) => {
                for (_, item) in items {
                    self.collect_idents_expr(item, used, scope);
                }
            }
            _ => {}
        }
    }

    fn collect_pattern_idents(&self, pattern: &Pattern, scope: &mut Vec<HashSet<String>>) {
        match &pattern.kind {
            pleno_ast::PatternKind::Ident(name) => {
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }
            }
            pleno_ast::PatternKind::Type(_, Some(name)) => {
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }
            }
            pleno_ast::PatternKind::Tuple(patterns) => {
                for p in patterns {
                    self.collect_pattern_idents(p, scope);
                }
            }
            pleno_ast::PatternKind::Variant(_, patterns) => {
                for p in patterns {
                    self.collect_pattern_idents(p, scope);
                }
            }
            pleno_ast::PatternKind::Object(props, rest) => {
                for (_, p) in props {
                    self.collect_pattern_idents(p, scope);
                }
                if let Some(p) = rest {
                    self.collect_pattern_idents(p, scope);
                }
            }
            pleno_ast::PatternKind::Or(patterns) => {
                for p in patterns {
                    self.collect_pattern_idents(p, scope);
                }
            }
            pleno_ast::PatternKind::Rest(Some(name)) => {
                if let Some(s) = scope.last_mut() {
                    s.insert(name.clone());
                }
            }
            _ => {}
        }
    }

    /// Checks the arguments against the callee's ownership hints and returns
    /// the registers of variables passed to `move` parameters, which the
    /// caller marks as moved once the call is compiled.
    fn enforce_call_ownership(
        &mut self,
        params: &[ParamOwnership],
        args: &[pleno_ast::CallArg],
        span: pleno_ast::Span,
    ) -> Result<Vec<u8>, CompileError> {
        let mut moved = Vec::new();
        let mut positional_idx = 0;
        for arg in args {
            let (param, expr) = match arg {
                pleno_ast::CallArg::Positional(expr) => {
                    positional_idx += 1;
                    (params.get(positional_idx - 1), expr)
                }
                pleno_ast::CallArg::Named(name, expr) => {
                    (params.iter().find(|p| p.name == *name), expr)
                }
                pleno_ast::CallArg::Placeholder | pleno_ast::CallArg::Spread(_) => continue,
            };
            if let Some(param) = param {
                moved.extend(self.enforce_arg_ownership(param, expr, span)?);
            }
        }
        Ok(moved)
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
            StmtKind::Let { value: Some(v), .. } | StmtKind::LetPattern { value: v, .. } => {
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
            StmtKind::With { resources, body } => {
                for resource in resources {
                    self.expand_expr(&mut resource.value, substitutions);
                }
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
            // `${param}` splices the argument; `${expr}` splices the expanded expression.
            ExprKind::MacroSplice(inner) => {
                let mut spliced = (**inner).clone();
                self.expand_expr(&mut spliced, substitutions);
                *expr = spliced;
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
                if let Some(eb) = else_branch {
                    self.expand_expr(eb, substitutions);
                }
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
    ) -> Result<Option<u8>, CompileError> {
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
                pleno_ast::Ownership::Move => return Ok(Some(local.reg)),
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
        Ok(None)
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
        self.inline_return_stack
            .push((Vec::new(), self.cleanup_stack.len(), dest));

        self.compile_stmt(&decl.body)?;

        if let Some(reg) = self.last_expr_register {
            self.emit(Opcode::Move, dest, reg, 0);
        } else {
            self.emit(Opcode::LoadNull, dest, 0, 0);
        }

        let return_jumps = self
            .inline_return_stack
            .pop()
            .map(|(jumps, _, _)| jumps)
            .unwrap_or_default();
        self.last_expr_register = old_last_expr;
        self.pop_scope();

        let end_pos = self.chunk.code.len();
        for jump in return_jumps {
            self.patch_jump_to(jump, end_pos)?;
        }
        Ok(())
    }

    fn collect_references(&self, stmts: &[Stmt]) -> HashSet<String> {
        let mut collector = prelude::ReferenceCollector::default();
        for stmt in stmts {
            pleno_ast::visitor::Visitor::visit_stmt(&mut collector, stmt);
        }
        let mut names = collector.names;
        // An unused declaration close to a name the program reads but never
        // declares (`totl()` next to `fn total`) is kept, so the runtime error
        // can say "did you mean 'total'?" instead of the function being dropped.
        let declared = declaration_kinds(stmts);
        let unused: Vec<&str> = declared
            .keys()
            .filter(|name| !names.contains(*name))
            .map(String::as_str)
            .collect();
        if !unused.is_empty() {
            let kept: Vec<String> = names
                .iter()
                .filter(|name| !declared.contains_key(*name))
                .filter_map(|name| pleno_value::suggest::best_match(name, unused.iter().copied()))
                .map(str::to_string)
                .collect();
            names.extend(kept);
        }
        names
    }

    fn push_loop(&mut self, continue_target: Option<usize>) {
        let labels = std::mem::take(&mut self.next_labels);
        self.loop_stack.push(LoopContext {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            continue_target,
            labels,
            cleanup_depth: self.cleanup_stack.len(),
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

    /// Checks the value in `reg` against `ty_name` and returns the register
    /// holding the value to store: a new one holding a `Float` when the
    /// annotation widens an `Int` (`reg` itself is left unchanged, since it may
    /// be another variable's register).
    fn emit_type_check(&mut self, reg: u8, ty_name: &str) -> Result<u8, CompileError> {
        self.emit_type_check_only(reg, ty_name)?;
        if self.widens_int(ty_name) {
            let dest = self.registers.allocate()?;
            self.emit(Opcode::ToFloat, dest, reg, 0);
            self.free_temp(reg);
            return Ok(dest);
        }
        Ok(reg)
    }

    /// Whether a value checked against `ty_name` is stored as a `Float` when
    /// it is an `Int` (see `pleno_bytecode::annotation_widens_int`).
    fn widens_int(&self, ty_name: &str) -> bool {
        let ty = ty_name.trim();
        let core = ty.strip_suffix('?').unwrap_or(ty).trim();
        let expanded = self.expand_type_aliases_deep(core);
        // `T | Float` with `T = Int` must keep an `Int`.
        !mentions_any(&expanded, &self.generic_names)
            && pleno_bytecode::annotation_widens_int(&expanded)
    }

    /// Sets up `child`, the compiler of `decl`'s body: the type parameters in
    /// scope and the check of its return annotation.
    fn setup_return_check(&self, child: &mut Compiler, decl: &FunctionDecl) {
        let mut generic_names = self.generic_names.clone();
        if let Some(owner) = &self.owner_context {
            generic_names.extend(owner.generics.iter().cloned());
        }
        generic_names.extend(decl.generics.iter().map(|g| g.name.clone()));
        child.return_check = self.return_check_for(decl, &generic_names);
        child.generic_names = generic_names;
    }

    /// The check of `decl`'s return annotation, or `None` when there is
    /// nothing this can check reliably: no annotation, `Any`, `Void`, a type
    /// that mentions a type parameter, setters, constructors and abstract
    /// methods. For an `async` function the annotation describes the resolved
    /// value, so `Promise<Int>` checks for an `Int`.
    fn return_check_for(
        &self,
        decl: &FunctionDecl,
        generic_names: &[String],
    ) -> Option<ReturnCheck> {
        let declared = decl.return_ty.as_deref()?.trim();
        if decl.is_abstract || decl.name == "constructor" || decl.kind == FunctionKind::Setter {
            return None;
        }
        let mut ty = declared;
        if decl.is_async {
            if let Some(inner) = ty
                .strip_prefix("Promise<")
                .and_then(|t| t.strip_suffix('>'))
            {
                ty = inner.trim();
            } else if ty == "Promise" {
                return None;
            }
        }
        let core = ty.strip_suffix('?').unwrap_or(ty).trim();
        let expanded = self.expand_type_aliases_deep(core);
        if ty.is_empty()
            || matches!(expanded.trim(), "Any" | "Void")
            || mentions_any(&expanded, generic_names)
            || mentions_any(&expanded, &["Self".to_string(), "This".to_string()])
        {
            return None;
        }
        // A user generic such as `Box<Int>` is checked by its class alone, which
        // needs no allocation; anything else with type arguments that the VM does
        // not read from text is skipped.
        let mut checked = ty.to_string();
        if !expanded.starts_with('(')
            && let Some((base, _)) = expanded.split_once('<')
            && !matches!(base.trim(), "Array" | "Map" | "Set" | "Result" | "Option")
        {
            let simple = expanded.ends_with('>')
                && base.trim().chars().all(|c| c.is_alphanumeric() || c == '_');
            if !simple || ty.ends_with('?') {
                return None;
            }
            checked = base.trim().to_string();
        }
        let name = if decl.name == "anonymous" {
            "lambda".to_string()
        } else {
            let base = match decl.kind {
                FunctionKind::Getter => decl.name.strip_prefix("get_").unwrap_or(&decl.name),
                _ => decl.name.as_str(),
            };
            match &self.owner_context {
                Some(owner) => format!("{}.{base}", owner.name),
                None => base.to_string(),
            }
        };
        Some(ReturnCheck {
            ty: checked,
            prefix: format!("TypeError: {name} returned "),
            suffix: format!(", expected {ty}"),
        })
    }

    /// Checks a value about to be returned against the function's return
    /// annotation and returns the register holding the value to return. On
    /// success this costs a type lookup, an `Is` and a jump, and allocates
    /// nothing; without an annotation it emits nothing.
    fn emit_return_check(&mut self, reg: u8) -> Result<u8, CompileError> {
        let Some(check) = self.return_check.clone() else {
            return Ok(reg);
        };
        let type_reg = self.registers.allocate()?;
        if let Some(local) = self.resolve_local(&check.ty) {
            self.emit(Opcode::Move, type_reg, local.reg, 0);
        } else if let Some((idx, _)) = self.captures.get(&check.ty) {
            self.emit(Opcode::LoadCapture, type_reg, *idx as u8, 0);
        } else {
            let reified = self.emit_reified_type(&check.ty)?;
            self.emit(Opcode::Move, type_reg, reified, 0);
            self.free_temp(reified);
        }
        let ok_reg = self.registers.allocate()?;
        self.emit(Opcode::Is, ok_reg, reg, type_reg);
        let skip_jump = self.emit_jump(Opcode::JumpIf, ok_reg);

        let msg_reg = self.registers.allocate()?;
        let prefix = self.add_constant(Value::String(Rc::new(check.prefix.clone())));
        self.emit(Opcode::LoadConst, msg_reg, prefix, 0);
        let part_reg = self.registers.allocate()?;
        self.emit(Opcode::TypeOf, part_reg, reg, 0);
        self.emit(Opcode::Add, msg_reg, msg_reg, part_reg);
        let suffix = self.add_constant(Value::String(Rc::new(check.suffix.clone())));
        self.emit(Opcode::LoadConst, part_reg, suffix, 0);
        self.emit(Opcode::Add, msg_reg, msg_reg, part_reg);
        // `c = 1`: thrown as an `Error`, like the VM's own errors.
        self.emit(Opcode::Throw, msg_reg, 0, 1);
        self.patch_jump(skip_jump)?;

        self.free_temp(part_reg);
        self.free_temp(msg_reg);
        self.free_temp(ok_reg);
        self.free_temp(type_reg);

        if self.widens_int(&check.ty) {
            let dest = self.registers.allocate()?;
            self.emit(Opcode::ToFloat, dest, reg, 0);
            return Ok(dest);
        }
        Ok(reg)
    }

    fn emit_type_check_only(&mut self, reg: u8, ty_name: &str) -> Result<(), CompileError> {
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
        let mut prev = ' ';
        for c in s.chars() {
            if c == sep && depth == 0 {
                parts.push(current.trim().to_string());
                current = String::new();
            } else {
                match c {
                    '<' | '(' | '[' => depth += 1,
                    // `=>` in a function type is not a closing bracket.
                    '>' if prev != '=' => depth -= 1,
                    ')' | ']' => depth -= 1,
                    _ => {}
                }
                current.push(c);
            }
            prev = c;
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

        let expanded = self.expand_type_alias(name);
        let expanded = expanded.as_str();

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

    /// Expands a type alias once: `Pair<Int>` with `type Pair<T> = (T, T);`
    /// becomes `(Int, Int)`. Other types are returned unchanged.
    fn expand_type_alias(&self, name: &str) -> String {
        let (base, args) = match name.find('<') {
            Some(open) if name.ends_with('>') => (
                &name[..open],
                self.split_aware(&name[open + 1..name.len() - 1], ','),
            ),
            _ => (name, Vec::new()),
        };
        let Some((params, ty)) = self.type_aliases.get(base.trim()) else {
            return name.to_string();
        };
        if params.is_empty() || params.len() != args.len() {
            return if params.is_empty() && args.is_empty() {
                ty.clone()
            } else {
                name.to_string()
            };
        }
        let mut expanded = String::new();
        let mut word = String::new();
        for ch in ty.chars().chain(std::iter::once(' ')) {
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
                continue;
            }
            match params.iter().position(|param| *param == word) {
                Some(index) => expanded.push_str(args[index].trim()),
                None => expanded.push_str(&word),
            }
            word.clear();
            expanded.push(ch);
        }
        expanded.pop();
        expanded
    }

    /// Expands type aliases anywhere in an annotation: the whole annotation as
    /// [`Self::expand_type_alias`] does, and aliases without type parameters
    /// used inside a larger type (`Money?`, `Money | String`, `Money[]`).
    fn expand_type_aliases_deep(&self, ty: &str) -> String {
        let mut current = ty.trim().to_string();
        // Aliases may name other aliases; the bound stops alias cycles.
        for _ in 0..8 {
            let mut next = self.expand_type_alias(&current);
            if next == current && !self.type_aliases.is_empty() {
                let mut out = String::with_capacity(next.len());
                let mut word = String::new();
                let mut in_string = false;
                for ch in next.chars().map(Some).chain(std::iter::once(None)) {
                    if let Some(ch) = ch
                        && (ch.is_alphanumeric() || ch == '_')
                    {
                        word.push(ch);
                        continue;
                    }
                    match self.type_aliases.get(&word) {
                        // A union or optional cannot be pasted into a larger
                        // type without changing its meaning (`U[]`).
                        Some((params, aliased))
                            if params.is_empty()
                                && !in_string
                                && !aliased.contains(['|', '&', '?']) =>
                        {
                            out.push_str(aliased.trim())
                        }
                        _ => out.push_str(&word),
                    }
                    word.clear();
                    if let Some(ch) = ch {
                        if ch == '"' {
                            in_string = !in_string;
                        }
                        out.push(ch);
                    }
                }
                next = out;
            }
            if next == current {
                break;
            }
            current = next;
        }
        current
    }

    fn emit_reified_type(&mut self, ty_name: &str) -> Result<u8, CompileError> {
        let expanded = self.expand_type_aliases_deep(ty_name);
        let ty_name = expanded.as_str();
        // Built-in generic collections, `Result` and `Option` are checked from
        // the annotation text by the VM.
        let builtin_generic = ty_name.split_once('<').is_some_and(|(base, _)| {
            matches!(base.trim(), "Array" | "Map" | "Set" | "Result" | "Option")
        });
        // Tuple and function types are checked from the annotation text too.
        let structural = ty_name.starts_with('(');
        if let Some(start_idx) = ty_name
            .find('<')
            .filter(|_| !builtin_generic && !structural)
        {
            // `Name<Args>`: anything else (`a<b`, a union part like `x<y`) is
            // not a type this can reify.
            if !ty_name.ends_with('>') || start_idx + 1 >= ty_name.len() - 1 {
                return Err(self.error(
                    &format!("invalid type annotation '{ty_name}'"),
                    self.current_span,
                ));
            }
            let base_name = ty_name[..start_idx].trim();
            let args_str = &ty_name[start_idx + 1..ty_name.len() - 1];
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
        let error_msg = format!("TypeError: expected {}, found ", original_ty_name);
        let msg_idx = self.add_constant(Value::String(error_msg.into()));
        let msg_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, msg_reg, msg_idx, 0);

        let actual_type_reg = self.registers.allocate()?;
        self.emit(Opcode::TypeOf, actual_type_reg, reg, 0);

        let final_msg_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, final_msg_reg, msg_reg, actual_type_reg);

        self.emit(Opcode::Throw, final_msg_reg, 0, 1);

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
        let callable_reg = self.compile_lambda(&[], expr, None, false, &[], false)?;
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
                name.clone(),
                Local {
                    reg,
                    mutable,
                    ty,
                    is_boxed,
                    start_ip: self.chunk.code.len(),
                    borrow_mut: false,
                },
            );
            let mut scope_names = self.scope_names.borrow_mut();
            scope_names.push((name, self.scope_head));
            self.scope_head = scope_names.len() - 1;
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
        self.scope_heads.push(self.scope_head);
        self.locals.push(HashMap::new());
        self.fn_signatures.push(HashMap::new());
        self.inline_functions.push(HashMap::new());
    }

    /// Records the locals of the scopes still open when a chunk is finished
    /// (a function's outermost scope, a program's top level), as `pop_scope`
    /// does for the others: the VM reads them for debugging and hints.
    fn record_open_locals(&mut self) {
        let end_ip = self.chunk.code.len();
        for scope in &self.locals {
            for (name, local) in scope {
                self.chunk.locals.push(pleno_bytecode::LocalVarInfo {
                    name: name.clone(),
                    register: local.reg,
                    start_ip: local.start_ip,
                    end_ip,
                });
            }
        }
    }

    fn pop_scope(&mut self) {
        if let Some(head) = self.scope_heads.pop() {
            self.scope_head = head;
        }
        if let Some(scope) = self.locals.pop() {
            let end_ip = self.chunk.code.len();
            for (name, local) in scope {
                self.chunk.locals.push(pleno_bytecode::LocalVarInfo {
                    name,
                    register: local.reg,
                    start_ip: local.start_ip,
                    end_ip,
                });
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
            }
            | StmtKind::LetPattern { value: val, .. } => {
                self.find_closures_and_captures_in_expr(val, boxed)
            }
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
            StmtKind::With { resources, body } => {
                for resource in resources {
                    self.find_closures_and_captures_in_expr(&resource.value, boxed);
                }
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
                name_span,
                ..
            } => {
                let decl = FunctionDecl {
                    span: Span::new(0, 0),
                    name: name.clone(),
                    name_span: *name_span,
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
                    name_span: Span::new(0, 0),
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

    fn compile_is_expr(&mut self, left: &pleno_ast::Expr, ty: &str) -> Result<u8, CompileError> {
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

    fn compile_slice_expr(
        &mut self,
        collection: &pleno_ast::Expr,
        start: Option<&pleno_ast::Expr>,
        end: Option<&pleno_ast::Expr>,
        inclusive: bool,
    ) -> Result<u8, CompileError> {
        // Compile as: collection.slice(start, end)
        // This will use the builtin method
        let collection_reg = self.compile_expr(collection)?;
        let args_reg = self.registers.allocate()?;
        self.emit(Opcode::NewMap, args_reg, 0, 0);

        // Add start argument
        let start_key_idx = self.add_constant(Value::String(std::rc::Rc::new("start".to_string())));
        let start_key_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, start_key_reg, start_key_idx, 0);

        let start_val_reg = if let Some(start_expr) = start {
            self.compile_expr(start_expr)?
        } else {
            let r = self.registers.allocate()?;
            self.emit(Opcode::LoadNull, r, 0, 0);
            r
        };
        self.emit(Opcode::SetIndex, start_val_reg, args_reg, start_key_reg);
        self.free_temp(start_key_reg);
        self.free_temp(start_val_reg);

        // Add end argument
        let end_key_idx = self.add_constant(Value::String(std::rc::Rc::new("end".to_string())));
        let end_key_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadConst, end_key_reg, end_key_idx, 0);

        let end_val_reg = if let Some(end_expr) = end {
            self.compile_expr(end_expr)?
        } else {
            let r = self.registers.allocate()?;
            self.emit(Opcode::LoadNull, r, 0, 0);
            r
        };
        self.emit(Opcode::SetIndex, end_val_reg, args_reg, end_key_reg);
        self.free_temp(end_key_reg);
        self.free_temp(end_val_reg);

        // `[..=end]`: the VM moves the end past the element it names.
        if inclusive {
            let key_idx =
                self.add_constant(Value::String(std::rc::Rc::new("inclusive".to_string())));
            let key_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadConst, key_reg, key_idx, 0);
            let flag_reg = self.registers.allocate()?;
            self.emit(Opcode::LoadTrue, flag_reg, 0, 0);
            self.emit(Opcode::SetIndex, flag_reg, args_reg, key_reg);
            self.free_temp(key_reg);
            self.free_temp(flag_reg);
        }

        let dest = self.registers.allocate()?;
        self.emit(Opcode::Slice, dest, collection_reg, args_reg);
        self.free_temp(collection_reg);
        self.free_temp(args_reg);
        Ok(dest)
    }

    fn compile_map_literal(&mut self, entries: &[MapItem]) -> Result<u8, CompileError> {
        let dest = self.registers.allocate()?;
        self.emit(Opcode::NewMap, dest, 0, 0);
        for item in entries {
            match item {
                MapItem::Entry(key, value_expr) => {
                    let value_reg = self.compile_expr(value_expr)?;
                    let key_idx = self.add_constant(Value::String(key.clone().into()));
                    self.emit(Opcode::MapLiteralSet, value_reg, dest, key_idx);
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

    fn compile_named_tuple_literal(
        &mut self,
        entries: &[(String, pleno_ast::Expr)],
    ) -> Result<u8, CompileError> {
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

    fn compile_array_literal(&mut self, items: &[pleno_ast::Expr]) -> Result<u8, CompileError> {
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

    fn compile_tuple_literal(&mut self, items: &[pleno_ast::Expr]) -> Result<u8, CompileError> {
        let dest = self.registers.allocate()?;
        self.emit(Opcode::NewTuple, dest, 0, 0);
        for item in items {
            let value_reg = self.compile_expr(item)?;
            self.emit(Opcode::TupleAppend, dest, value_reg, 0);
            self.free_temp(value_reg);
        }
        Ok(dest)
    }

    fn compile_set_literal(&mut self, items: &[pleno_ast::Expr]) -> Result<u8, CompileError> {
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

    fn compile_if_expr(
        &mut self,
        condition: &pleno_ast::Expr,
        then_branch: &pleno_ast::Expr,
        else_branch: Option<&pleno_ast::Expr>,
    ) -> Result<u8, CompileError> {
        let cond_reg = self.compile_expr(condition)?;
        let else_jump = self.emit_jump(Opcode::JumpIfNot, cond_reg);
        self.free_temp(cond_reg);

        let result_reg = self.registers.allocate()?;

        let then_reg = self.compile_expr(then_branch)?;
        self.emit(Opcode::Move, result_reg, then_reg, 0);
        self.free_temp(then_reg);

        let end_jump = self.emit_jump(Opcode::Jump, 0);

        self.patch_jump(else_jump)?;

        if let Some(eb) = else_branch {
            let else_reg = self.compile_expr(eb)?;
            self.emit(Opcode::Move, result_reg, else_reg, 0);
            self.free_temp(else_reg);
        } else {
            self.emit(Opcode::LoadNull, result_reg, 0, 0);
        }

        self.patch_jump(end_jump)?;

        Ok(result_reg)
    }

    fn compile_match_expr(
        &mut self,
        expr: &pleno_ast::Expr,
        arms: &[pleno_ast::MatchArm],
    ) -> Result<u8, CompileError> {
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
                && matches!(
                    &arm.pattern.kind,
                    pleno_ast::PatternKind::Underscore | pleno_ast::PatternKind::Ident(_)
                )
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
    fn compile_block_expr(&mut self, stmts: &[pleno_ast::Stmt]) -> Result<u8, CompileError> {
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
}

fn literal_value(lit: &Literal) -> Option<Value> {
    Some(match lit {
        Literal::Int(i) => Value::Int(*i),
        Literal::Float(f) => Value::Float(*f),
        Literal::String(s) => Value::String(Rc::new(s.clone())),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Char(c) => Value::Char(*c),
        Literal::Null => Value::Null,
    })
}

/// Built-in types that `extend` can add methods to.
const BUILTIN_EXTENSION_TYPES: &[&str] = &[
    "String", "Int", "Float", "Bool", "Char", "Array", "Map", "Set", "Tuple",
];

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

    /// `count` consecutive registers above every register in use, or `None`
    /// if they do not fit. Free them with `free_block`.
    fn allocate_block(&mut self, count: usize) -> Option<u8> {
        let start = self.next;
        let end = start as usize + count;
        if end >= u8::MAX as usize {
            return None;
        }
        self.next = end as u8;
        self.max_used = self.max_used.max(end);
        Some(start)
    }

    fn free_block(&mut self, start: u8, count: usize) {
        for offset in (0..count).rev() {
            self.free(start + offset as u8);
        }
    }

    fn free(&mut self, reg: u8) {
        if reg + 1 == self.next {
            self.next = self.next.saturating_sub(1);
        } else {
            self.free.push(reg);
        }
    }
}

/// `&mut` marks a parameter whose value the function may change in place; a
/// parameter holds its own copy of the reference, so reassigning it (for any
/// type) would never reach the caller's variable and is rejected.
fn mut_param_assign_message(name: &str) -> String {
    format!(
        "cannot assign to '&mut' parameter '{name}': the caller's variable would not change; \
         mutate the value in place or return the new value"
    )
}

/// Whether the annotation `ty` mentions one of `names` as a whole word.
fn mentions_any(ty: &str, names: &[String]) -> bool {
    !names.is_empty()
        && ty
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| names.iter().any(|name| name == word))
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

/// Added to a field's visibility code when the field is `const`.
pub const CONST_FIELD_FLAG: u8 = 0x10;

fn visibility_code(visibility: Option<&pleno_ast::Visibility>) -> u8 {
    match visibility {
        Some(pleno_ast::Visibility::Private) => 1,
        Some(pleno_ast::Visibility::Protected) => 2,
        Some(pleno_ast::Visibility::Internal) => 3,
        _ => 0,
    }
}

/// What each top-level declaration in `stmts` is: `function`, `class`,
/// `enum` or `interface`.
fn declaration_kinds(stmts: &[Stmt]) -> HashMap<String, &'static str> {
    let mut kinds = HashMap::new();
    for stmt in stmts {
        let mut stmt = stmt;
        while let StmtKind::Export(inner) = &stmt.kind {
            stmt = inner;
        }
        let (name, kind) = match &stmt.kind {
            StmtKind::Fn { name, .. } => (name, "function"),
            StmtKind::Class { name, .. } => (name, "class"),
            StmtKind::Enum(decl) => (&decl.name, "enum"),
            StmtKind::Interface { name, .. } => (name, "interface"),
            _ => continue,
        };
        kinds.insert(name.clone(), kind);
    }
    kinds
}

/// The cleanup a `try` region runs when left early: its `finally` block, if any.
fn finally_action(finally_branch: Option<&Stmt>) -> CleanupAction {
    match finally_branch {
        Some(stmt) => CleanupAction::Finally(Box::new(stmt.clone())),
        None => CleanupAction::None,
    }
}

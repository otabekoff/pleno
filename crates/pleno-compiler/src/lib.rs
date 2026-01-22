use pleno_ast::{Expr, Literal, Program, Stmt};
use pleno_bytecode::{Chunk, Instruction, Opcode};
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
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            registers: RegisterAllocator::new(),
            last_expr_register: None,
            locals: vec![HashMap::new()],
            loop_counter: 0,
        }
    }

    pub fn compile(&mut self, program: &Program) -> Result<Chunk, CompileError> {
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
        Ok(self.chunk.clone())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
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
                self.compile_stmt(body)?;
                self.emit_loop(loop_start)?;
                self.patch_jump(jump_to_end)?;
                Ok(())
            }
            Stmt::For { name, iterable, body } => self.compile_for_stmt(name, iterable, body),
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
            Expr::Ident(name) => self
                .resolve_local(name)
                .map(|local| local.reg)
                .ok_or_else(|| CompileError::new("unknown local")),
            Expr::Property { object, name } => {
                let object_reg = self.compile_expr(object)?;
                let dest = self.registers.allocate()?;
                let name_idx = self.add_constant(Value::String(name.clone().into()));
                self.emit(Opcode::GetProperty, dest, object_reg, name_idx);
                self.free_temp(object_reg);
                Ok(dest)
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
            Expr::Index { collection, index } => {
                let collection_reg = self.compile_expr(collection)?;
                let index_reg = self.compile_expr(index)?;
                let dest = self.registers.allocate()?;
                self.emit(Opcode::GetIndex, dest, collection_reg, index_reg);
                self.free_temp(collection_reg);
                self.free_temp(index_reg);
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
        if let Expr::Range {
            start: Some(start),
            end: Some(end),
            inclusive,
        } = iterable
        {
            return self.compile_for_range(name, start, end, *inclusive, body);
        }

        if let Expr::Range { .. } = iterable {
            return Err(CompileError::new(
                "for loop requires start and end for range",
            ));
        }
        if matches!(iterable, Expr::MapLiteral(_)) {
            return self.compile_for_map(name, iterable, body);
        }
        if matches!(iterable, Expr::SetLiteral(_)) {
            return self.compile_for_set(name, iterable, body);
        }
        if matches!(iterable, Expr::Literal(_)) {
            return Err(CompileError::new("for loop expects array, map, or range"));
        }
        self.compile_for_array(name, iterable, body)
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

        self.compile_stmt(body)?;

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, start_reg, one_reg);
        self.emit(Opcode::Move, start_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_array(
        &mut self,
        name: &str,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let collection_reg = self.compile_expr(iterable)?;
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

        self.compile_stmt(body)?;

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, index_reg, one_reg);
        self.emit(Opcode::Move, index_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_map(
        &mut self,
        name: &str,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let map_reg = self.compile_expr(iterable)?;
        let keys_reg = self.registers.allocate()?;
        self.emit(Opcode::MapKeys, keys_reg, map_reg, 0);
        self.free_temp(map_reg);

        let collection_name = format!("$for_map_keys_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(collection_name, keys_reg, false);

        let index_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, index_reg, 0, 0);
        let index_name = format!("$for_index_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(index_name, index_reg, true);

        let loop_start = self.chunk.code.len();
        let len_reg = self.registers.allocate()?;
        self.emit(Opcode::ArrayLen, len_reg, keys_reg, 0);
        let cond_reg = self.registers.allocate()?;
        self.emit(Opcode::Lt, cond_reg, index_reg, len_reg);
        let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
        self.free_temp(cond_reg);
        self.free_temp(len_reg);

        let item_reg = self.registers.allocate()?;
        self.emit(Opcode::GetIndex, item_reg, keys_reg, index_reg);
        if self
            .locals
            .last()
            .map(|scope| scope.contains_key(name))
            .unwrap_or(false)
        {
            return Err(CompileError::new("for loop variable already defined in scope"));
        }
        self.insert_local(name.to_string(), item_reg, true);

        self.compile_stmt(body)?;

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, index_reg, one_reg);
        self.emit(Opcode::Move, index_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        self.pop_scope();
        Ok(())
    }

    fn compile_for_set(
        &mut self,
        name: &str,
        iterable: &Expr,
        body: &Stmt,
    ) -> Result<(), CompileError> {
        self.locals.push(HashMap::new());
        let set_reg = self.compile_expr(iterable)?;
        let values_reg = self.registers.allocate()?;
        self.emit(Opcode::SetValues, values_reg, set_reg, 0);
        self.free_temp(set_reg);

        let collection_name = format!("$for_set_values_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(collection_name, values_reg, false);

        let index_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, index_reg, 0, 0);
        let index_name = format!("$for_index_{}", self.loop_counter);
        self.loop_counter += 1;
        self.insert_local(index_name, index_reg, true);

        let loop_start = self.chunk.code.len();
        let len_reg = self.registers.allocate()?;
        self.emit(Opcode::ArrayLen, len_reg, values_reg, 0);
        let cond_reg = self.registers.allocate()?;
        self.emit(Opcode::Lt, cond_reg, index_reg, len_reg);
        let jump_to_end = self.emit_jump(Opcode::JumpIfNot, cond_reg);
        self.free_temp(cond_reg);
        self.free_temp(len_reg);

        let item_reg = self.registers.allocate()?;
        self.emit(Opcode::GetIndex, item_reg, values_reg, index_reg);
        if self
            .locals
            .last()
            .map(|scope| scope.contains_key(name))
            .unwrap_or(false)
        {
            return Err(CompileError::new("for loop variable already defined in scope"));
        }
        self.insert_local(name.to_string(), item_reg, true);

        self.compile_stmt(body)?;

        let one_reg = self.registers.allocate()?;
        self.emit(Opcode::LoadInt, one_reg, 1, 0);
        let sum_reg = self.registers.allocate()?;
        self.emit(Opcode::Add, sum_reg, index_reg, one_reg);
        self.emit(Opcode::Move, index_reg, sum_reg, 0);
        self.free_temp(one_reg);
        self.free_temp(sum_reg);

        self.emit_loop(loop_start)?;
        self.patch_jump(jump_to_end)?;
        self.pop_scope();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Local {
    reg: u8,
    mutable: bool,
}

impl Compiler {
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

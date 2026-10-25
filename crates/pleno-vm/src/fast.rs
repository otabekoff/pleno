//! A small interpreter loop for the instructions that dominate tight code:
//! moves, small constants, integer and float arithmetic, comparisons, jumps,
//! cached global reads, built-in methods of arrays and strings, and plain
//! calls and returns. `run_step` hands over to
//! it before each instruction when no debugger and no sandbox limit is
//! active; it runs instructions until it meets one it does not handle (or a
//! case of one it handles only partly, such as an addition that overflows),
//! and leaves that instruction for `run_step`, which then runs it in full.
//!
//! Being a separate, small function, the loop keeps its state (the
//! instruction pointer, the frame's first register, the code) in machine
//! registers instead of reloading it for every instruction. Every case it
//! handles does what the general instruction does for those operands, and it
//! counts instructions the same way. None of them allocates a collected
//! object except the native methods `CallMethod` calls, after which the loop
//! stops when the collector is due, so `run_step` collects as it would have.

use crate::VM;
use crate::utils::{apply_offset, decode_offset};
use pleno_bytecode::{Instruction, Opcode};
use pleno_value::RuntimeError;
use pleno_value::nan_value::NanValue;

/// Whether `run_fast` runs (some cases of) this instruction.
#[inline]
pub(crate) fn handles(opcode: Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Move
            | Opcode::LoadInt
            | Opcode::LoadNull
            | Opcode::LoadTrue
            | Opcode::LoadFalse
            | Opcode::AddInt
            | Opcode::SubInt
            | Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Mod
            | Opcode::Lt
            | Opcode::Le
            | Opcode::Gt
            | Opcode::Ge
            | Opcode::Eq
            | Opcode::Ne
            | Opcode::Jump
            | Opcode::JumpIf
            | Opcode::JumpIfNot
            | Opcode::ForceThunk
            | Opcode::GetGlobal
            | Opcode::CallRegs
            | Opcode::GetMethod
            | Opcode::CallMethod
            | Opcode::Return
    )
}

impl VM {
    /// Runs instructions of the current frame (and of frames it calls and
    /// returns to, above `depth + 1`) until one needs `run_step`, and stores
    /// the instruction pointer of that one in its frame.
    #[inline(never)]
    pub(crate) fn run_fast(&mut self, depth: usize) -> Result<(), RuntimeError> {
        let Some(frame) = self.frames.last() else {
            return Ok(());
        };
        let mut function_index = frame.function_index;
        let mut ip = frame.ip;
        let mut base = frame.base;
        // The code of the current function. A function's code is never changed
        // or freed once it is in `self.functions` (see `run_step`).
        let mut code: *const Instruction = self.functions[function_index].chunk.code.as_ptr();
        let mut code_len = self.functions[function_index].chunk.code.len();
        // `self.instruction_count`, kept here while the loop runs.
        let mut count = self.instruction_count;

        // Leaves the loop with the current instruction for `run_step`.
        macro_rules! stop {
            () => {{
                if let Some(frame) = self.frames.last_mut() {
                    frame.ip = ip;
                }
                self.instruction_count = count;
                return Ok(());
            }};
        }
        // Switches to the code of the (new) top frame after a call or return.
        macro_rules! load_frame {
            () => {{
                let frame = self.frames.last().expect("a frame");
                function_index = frame.function_index;
                ip = frame.ip;
                base = frame.base;
                code = self.functions[function_index].chunk.code.as_ptr();
                code_len = self.functions[function_index].chunk.code.len();
            }};
        }
        // After a comparison that stored `$result` in register `$dest`: a
        // conditional jump on that register right after it runs here too (as
        // the separate instruction would, counted as one).
        macro_rules! fused_jump {
            ($dest:expr, $result:expr) => {
                if ip + 1 < code_len {
                    // SAFETY: `ip + 1 < code_len` (see the loop).
                    let jump = unsafe { *code.add(ip + 1) };
                    if matches!(jump.opcode(), Opcode::JumpIf | Opcode::JumpIfNot)
                        && jump.a() as usize == $dest
                    {
                        let next = if $result == (jump.opcode() == Opcode::JumpIf) {
                            match apply_offset(ip + 2, decode_offset(jump.b(), jump.c())) {
                                Ok(next) => next,
                                // `run_step` reports the bad jump.
                                Err(_) => {
                                    count += 1;
                                    ip += 1;
                                    stop!();
                                }
                            }
                        } else {
                            ip + 2
                        };
                        count += 2;
                        ip = next;
                        continue;
                    }
                }
            };
        }
        macro_rules! reg {
            ($index:expr) => {
                self.registers[base + $index]
            };
        }

        loop {
            if ip >= code_len {
                stop!();
            }
            // SAFETY: `ip < code_len`, and `code` points to the `code_len`
            // instructions of function `function_index`, which stay in place.
            let instruction = unsafe { *code.add(ip) };
            let a = instruction.a() as usize;
            let b = instruction.b() as usize;
            let c = instruction.c() as usize;
            match instruction.opcode() {
                Opcode::Move => reg!(a) = reg!(b),
                Opcode::LoadInt => {
                    let value = (b as i8) as i64;
                    reg!(a) = NanValue::from_int(value);
                    // `x < 2`: a comparison of an integer with this constant
                    // right after it (and a jump on its result) runs here too.
                    if ip + 1 < code_len {
                        // SAFETY: `ip + 1 < code_len` (see above).
                        let compare = unsafe { *code.add(ip + 1) };
                        if matches!(
                            compare.opcode(),
                            Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge
                        ) && compare.c() as usize == a
                            && let Some(left) = reg!(compare.b() as usize).as_inline_int()
                        {
                            let result = match compare.opcode() {
                                Opcode::Lt => left < value,
                                Opcode::Le => left <= value,
                                Opcode::Gt => left > value,
                                _ => left >= value,
                            };
                            let dest = compare.a() as usize;
                            reg!(dest) = NanValue::from_bool(result);
                            count += 1;
                            ip += 1;
                            fused_jump!(dest, result);
                        }
                    }
                }
                Opcode::LoadNull => reg!(a) = NanValue::null(),
                Opcode::LoadTrue => reg!(a) = NanValue::from_bool(true),
                Opcode::LoadFalse => reg!(a) = NanValue::from_bool(false),
                Opcode::AddInt | Opcode::SubInt => {
                    let Some(left) = reg!(b).as_inline_int() else {
                        stop!();
                    };
                    let result = if instruction.opcode() == Opcode::AddInt {
                        left + c as i64
                    } else {
                        left - c as i64
                    };
                    let Some(result) = NanValue::inline_int(result) else {
                        stop!();
                    };
                    reg!(a) = result;
                }
                Opcode::Add | Opcode::Sub | Opcode::Mul => {
                    let (left, right) = (reg!(b), reg!(c));
                    let result =
                        if let (Some(l), Some(r)) = (left.as_inline_int(), right.as_inline_int()) {
                            // Inline integers have 46 bits, so these cannot overflow
                            // an `i64` except a product, which `checked_mul` catches.
                            let value = match instruction.opcode() {
                                Opcode::Add => Some(l + r),
                                Opcode::Sub => Some(l - r),
                                _ => l.checked_mul(r),
                            };
                            match value.and_then(NanValue::inline_int) {
                                Some(result) => result,
                                None => stop!(),
                            }
                        } else if left.is_f64() && right.is_f64() {
                            let (l, r) = (left.as_f64(), right.as_f64());
                            NanValue::from_f64(match instruction.opcode() {
                                Opcode::Add => l + r,
                                Opcode::Sub => l - r,
                                _ => l * r,
                            })
                        } else {
                            stop!();
                        };
                    reg!(a) = result;
                }
                Opcode::Mod => {
                    let (left, right) = (reg!(b), reg!(c));
                    let result = if let (Some(l), Some(r)) =
                        (left.as_inline_int(), right.as_inline_int())
                        && r != 0
                    {
                        // `|l % r| < |r|`, so the remainder is inline too.
                        NanValue::from_int(l % r)
                    } else if left.is_f64() && right.is_f64() && right.as_f64() != 0.0 {
                        NanValue::from_f64(left.as_f64() % right.as_f64())
                    } else {
                        stop!();
                    };
                    reg!(a) = result;
                }
                Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                    let (Some(l), Some(r)) = (reg!(b).as_inline_int(), reg!(c).as_inline_int())
                    else {
                        stop!();
                    };
                    let result = match instruction.opcode() {
                        Opcode::Lt => l < r,
                        Opcode::Le => l <= r,
                        Opcode::Gt => l > r,
                        _ => l >= r,
                    };
                    reg!(a) = NanValue::from_bool(result);
                    fused_jump!(a, result);
                }
                Opcode::Eq | Opcode::Ne => {
                    let (Some(l), Some(r)) = (reg!(b).as_inline_int(), reg!(c).as_inline_int())
                    else {
                        stop!();
                    };
                    let result = (l == r) == (instruction.opcode() == Opcode::Eq);
                    reg!(a) = NanValue::from_bool(result);
                    fused_jump!(a, result);
                }
                Opcode::Jump | Opcode::JumpIf | Opcode::JumpIfNot => {
                    let jump = match instruction.opcode() {
                        Opcode::Jump => true,
                        Opcode::JumpIf => crate::value_ops::is_truthy_nan(reg!(a)),
                        _ => !crate::value_ops::is_truthy_nan(reg!(a)),
                    };
                    let next = if jump {
                        match apply_offset(ip + 1, decode_offset(b as u8, c as u8)) {
                            Ok(next) => next,
                            Err(_) => stop!(),
                        }
                    } else {
                        ip + 1
                    };
                    count += 1;
                    ip = next;
                    continue;
                }
                Opcode::ForceThunk => {
                    let value = reg!(b);
                    if value.is_thunk() {
                        stop!();
                    }
                    reg!(a) = value;
                }
                Opcode::GetGlobal => {
                    let remembered = self
                        .global_values
                        .get(function_index)
                        .and_then(|values| values.get(b))
                        .filter(|(epoch, _)| *epoch == self.globals_epoch);
                    let value = match remembered {
                        Some(&(_, value)) => value,
                        None => {
                            // Only the cached read of a name outside modules (see
                            // `run_step`): its slot still holds a global of that name.
                            let function = &self.functions[function_index];
                            if function.module_path.is_some() {
                                stop!();
                            }
                            let Some(pleno_value::Value::String(name)) =
                                function.chunk.constants.get(b)
                            else {
                                stop!();
                            };
                            let cached = self
                                .global_slots
                                .get(function_index)
                                .and_then(|slots| slots.get(b))
                                .copied()
                                .unwrap_or(0) as usize;
                            let Some((key, value)) = self.globals.get_index(cached.wrapping_sub(1))
                            else {
                                stop!();
                            };
                            if key.as_str() != name.as_str() {
                                stop!();
                            }
                            let value = NanValue::from_value(value);
                            self.remember_global(function_index, b, value);
                            value
                        }
                    };
                    reg!(a) = value;
                    // The `ForceThunk` that follows a global read, when there is
                    // nothing to force.
                    if ip + 1 < code_len && !value.is_thunk() {
                        // SAFETY: `ip + 1 < code_len` (see above).
                        let next = unsafe { *code.add(ip + 1) };
                        if next.opcode() == Opcode::ForceThunk && next.b() as usize == a {
                            reg!(next.a() as usize) = value;
                            count += 2;
                            ip += 2;
                            continue;
                        }
                    }
                }
                Opcode::GetMethod => {
                    // A built-in method of an array or a string, with no `extend`
                    // block anywhere (`run_step` handles everything else).
                    let receiver = reg!(b);
                    let type_name = receiver.type_name();
                    if !matches!(type_name, "Array" | "String")
                        || !self.builtin_extensions.is_empty()
                        || base + a + 2 >= self.registers.len()
                    {
                        stop!();
                    }
                    let Some(pleno_value::Value::String(name)) =
                        self.functions[function_index].chunk.constants.get(c)
                    else {
                        stop!();
                    };
                    let Some(native) =
                        self.builtin_methods
                            .get(self.builtin_resolver, type_name, name)
                    else {
                        stop!();
                    };
                    reg!(a) = NanValue::from_value(&pleno_value::Value::NativeFunction(native));
                    reg!(a + 1) = receiver;
                    reg!(a + 2) = NanValue::from_bool(true);
                }
                Opcode::CallMethod => {
                    let method_at = base + b;
                    let args_start = method_at + 3;
                    if args_start + c > self.registers.len() {
                        stop!();
                    }
                    let callee = self.registers[method_at];
                    let receiver = self.registers[method_at + 1];
                    let kind = self.registers[method_at + 2];
                    if kind.as_bool() == Some(true) {
                        let pleno_value::Value::NativeFunction(native) = callee.to_value() else {
                            stop!();
                        };
                        // As in `run_step`: the instruction counts and the frame
                        // points past it while the native runs (a callback runs
                        // more instructions; an error reports this one).
                        count += 1;
                        self.instruction_count = count;
                        if let Some(frame) = self.frames.last_mut() {
                            frame.ip = ip + 1;
                        }
                        let mut args = std::mem::take(&mut self.native_args);
                        args.push(receiver.to_value());
                        args.extend(
                            self.registers[args_start..args_start + c]
                                .iter()
                                .map(|arg| arg.to_value()),
                        );
                        let result = native(self, &args);
                        args.clear();
                        self.native_args = args;
                        match result {
                            Ok(result) => self.registers[base + a] = NanValue::from_value(&result),
                            Err(err) => return Err(self.throw_runtime_error(&err.message)),
                        }
                        count = self.instruction_count;
                        ip += 1;
                        // The native may have allocated: let `run_step` collect
                        // when it is time to.
                        if self.gc.current_allocation >= self.gc.gc.allocation_threshold
                            || (self.gc.sweeps_boxed_values
                                && pleno_value::nan_value::boxed_values_added()
                                    > crate::BOXED_VALUES_PER_COLLECTION
                                        .max(self.gc.current_allocation))
                        {
                            stop!();
                        }
                        continue;
                    }
                    // The caller resumes after this instruction.
                    if let Some(frame) = self.frames.last_mut() {
                        frame.ip = ip + 1;
                    }
                    let pushed = if kind.as_bool() == Some(false) {
                        self.try_push_register_frame(callee, None, args_start, c, base + a, None)?
                    } else {
                        match crate::owner_from_nan(kind) {
                            Some(owner) => self.try_push_register_frame(
                                callee,
                                Some(receiver),
                                args_start,
                                c,
                                base + a,
                                Some(owner),
                            )?,
                            None => false,
                        }
                    };
                    if !pushed {
                        stop!();
                    }
                    count += 1;
                    load_frame!();
                    continue;
                }
                Opcode::CallRegs => {
                    let args_start = base + b + 1;
                    if args_start + c > self.registers.len() {
                        stop!();
                    }
                    // The caller resumes after this instruction.
                    if let Some(frame) = self.frames.last_mut() {
                        frame.ip = ip + 1;
                    }
                    self.instruction_count = count;
                    if !self.try_push_register_frame(
                        reg!(b),
                        None,
                        args_start,
                        c,
                        base + a,
                        None,
                    )? {
                        stop!();
                    }
                    count += 1;
                    load_frame!();
                    continue;
                }
                Opcode::Return => {
                    let function = &self.functions[function_index];
                    let frame = self.frames.last().expect("a frame");
                    if function.is_async
                        || frame.return_override.is_some()
                        || self.frames.len() <= depth + 1
                    {
                        stop!();
                    }
                    let (frame_base, return_reg) = (frame.base, frame.return_reg);
                    let result = reg!(a);
                    drop(self.frames.pop());
                    self.release_registers(frame_base);
                    self.registers[return_reg] = result;
                    count += 1;
                    load_frame!();
                    continue;
                }
                _ => stop!(),
            }
            count += 1;
            ip += 1;
        }
    }

    /// Remembers the value of a global read by instruction operand `name_index`
    /// of function `function_index` until the next change to any global
    /// (`globals_epoch`). Only values that hold their target alive through the
    /// global itself: not strings, big integers or ranges, whose table slots a
    /// collection may free while only the global's `Value` refers to them.
    fn remember_global(&mut self, function_index: usize, name_index: usize, value: NanValue) {
        if value.is_string()
            || value.is_range()
            || (value.as_int().is_some() && value.as_inline_int().is_none())
        {
            return;
        }
        if self.global_values.len() <= function_index {
            self.global_values.resize(function_index + 1, Vec::new());
        }
        let values = &mut self.global_values[function_index];
        if values.len() <= name_index {
            values.resize(name_index + 1, (0, NanValue::null()));
        }
        values[name_index] = (self.globals_epoch, value);
    }
}

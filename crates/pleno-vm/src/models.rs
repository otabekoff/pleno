use pleno_value::{
    ClosureState, MethodOwner, MultiWaiter, Value,
    gc::{GcObject, GcRc, MarkAndSweepGC},
};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct AsyncTask {
    pub promise_id: u64,
    pub value: Value,
    pub callee: Option<Value>,
    pub args: Option<Vec<Value>>,
    pub is_async: bool,
    pub is_resolved: bool,
}

impl GcObject for AsyncTask {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.value.mark(gc);
        if let Some(callee) = &self.callee {
            callee.mark(gc);
        }
        if let Some(args) = &self.args {
            for arg in args {
                arg.mark(gc);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SuspendedFrame {
    pub frame: CallFrame,
    pub await_reg: usize,
}

impl GcObject for SuspendedFrame {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        self.frame.mark(gc);
    }
}

#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    pub catch_ip: usize,
    pub base: usize,
    pub error_reg: usize,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_index: usize,
    pub ip: usize,
    pub base: usize,
    pub return_reg: usize,
    pub return_override: Option<Value>,
    pub closure_state: Option<GcRc<ClosureState>>,
    pub current_owner: Option<MethodOwner>,
    pub current_module: Option<Rc<String>>,
    pub current_module_path: Option<Rc<String>>,
    pub exception_handlers: Vec<ExceptionHandler>,
}

impl GcObject for CallFrame {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        if let Some(val) = &self.return_override {
            val.mark(gc);
        }
        if let Some(state) = &self.closure_state {
            gc.mark_object(&state.to_gc_object());
        }
        if let Some(owner) = &self.current_owner {
            match owner {
                MethodOwner::Class(class) => gc.mark_object(&class.to_gc_object()),
                MethodOwner::Enum(enum_type) => gc.mark_object(&enum_type.to_gc_object()),
            }
        }
    }
}

pub struct VmRoots<'a> {
    pub registers: &'a [Value],
    pub frames: &'a [CallFrame],
    pub globals: &'a HashMap<String, Value>,
    pub modules: &'a HashMap<String, Value>,
    pub pending_tasks: &'a [AsyncTask],
    pub suspended_frames: &'a HashMap<u64, SuspendedFrame>,
    pub promises: &'a HashMap<u64, Value>,
    pub multi_waiters: &'a [MultiWaiter],
}

impl<'a> GcObject for VmRoots<'a> {
    fn mark(&self, gc: &mut MarkAndSweepGC) {
        for reg in self.registers {
            reg.mark(gc);
        }
        for frame in self.frames {
            frame.mark(gc);
        }
        for val in self.globals.values() {
            val.mark(gc);
        }
        for val in self.modules.values() {
            val.mark(gc);
        }
        for task in self.pending_tasks {
            task.mark(gc);
        }
        for promise in self.promises.values() {
            promise.mark(gc);
        }
        for frame in self.suspended_frames.values() {
            frame.mark(gc);
        }
        for waiter in self.multi_waiters {
            waiter.mark(gc);
        }
    }
}

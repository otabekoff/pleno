use pleno_value::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum DebugState {
    #[default]
    Running,
    Paused(PauseReason),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PauseReason {
    Step,
    Breakpoint,
    Exception,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Breakpoint {
    pub line: usize,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

#[derive(Debug, Default)]
pub struct VariableTracker {
    next_id: u64,
    id_to_value: HashMap<u64, Value>,
}

impl VariableTracker {
    pub fn new() -> Self {
        Self {
            next_id: 1, // 0 is reserved for "no reference" in DAP
            id_to_value: HashMap::new(),
        }
    }

    pub fn track(&mut self, value: Value) -> u64 {
        let id = self.next_id;
        self.id_to_value.insert(id, value);
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: u64) -> Option<&Value> {
        self.id_to_value.get(&id)
    }

    pub fn clear(&mut self) {
        self.id_to_value.clear();
        self.next_id = 1;
    }
}

#[derive(Debug, Default)]
pub struct DebugContext {
    pub state: DebugState,
    pub breakpoints: HashMap<String, Vec<Breakpoint>>, // file -> breakpoints
    pub data_breakpoints: HashMap<String, Value>,      // variable name -> last seen value
    pub hit_counts: HashMap<(String, usize), usize>,   // (file, line) -> hits
    pub step_over_depth: Option<usize>,                // call stack depth
    pub variable_tracker: VariableTracker,
    /// Set when resuming, so execution moves past the instruction it paused on.
    pub skip_next_check: bool,
    /// Set while stepping, so the VM returns to the debugger after each instruction.
    pub stepping: bool,
}

impl DebugContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_breakpoints(&mut self, path: String, breakpoints: Vec<Breakpoint>) {
        self.hit_counts.retain(|(p, _), _| p != &path);
        self.breakpoints.insert(path, breakpoints);
    }

    pub fn remove_breakpoints(&mut self, path: &str) {
        self.breakpoints.remove(path);
    }

    pub fn pause(&mut self) {
        self.state = DebugState::Paused(PauseReason::Step);
    }

    pub fn resume(&mut self) {
        self.state = DebugState::Running;
        self.skip_next_check = true;
    }

    pub fn get_breakpoint(
        &self,
        func_idx: usize,
        ip: usize,
        functions: &[crate::Function],
    ) -> Option<&Breakpoint> {
        let function = &functions[func_idx];
        let path = function.module_path.as_deref()?;

        if ip >= function.chunk.lines.len() {
            return None;
        }
        let line = function.chunk.lines[ip] as usize;
        // A breakpoint triggers once per line: on the line's first instruction.
        if ip > 0 && function.chunk.lines[ip - 1] as usize == line {
            return None;
        }

        if let Some(breakpoints) = self.breakpoints.get(path) {
            return breakpoints.iter().find(|bp| bp.line == line);
        }
        None
    }

    pub fn should_pause(
        &mut self,
        func_idx: usize,
        ip: usize,
        functions: &[crate::Function],
    ) -> bool {
        if std::mem::take(&mut self.skip_next_check) {
            return false;
        }
        if let DebugState::Paused(_) = self.state {
            return true;
        }
        // Without breakpoints there is nothing to look up (the common case).
        if self.breakpoints.is_empty() {
            return false;
        }

        if let Some(_bp) = self.get_breakpoint(func_idx, ip, functions) {
            // we'll handle conditions in the VM
            return true;
        }
        false
    }
}

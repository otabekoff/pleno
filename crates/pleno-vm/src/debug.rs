use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Default)]
pub struct DebugContext {
    pub state: DebugState,
    pub breakpoints: HashMap<String, HashSet<usize>>, // file -> lines
    pub step_over_depth: Option<usize>,               // call stack depth
}

impl DebugContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_breakpoint(&mut self, path: String, line: usize) {
        self.breakpoints.entry(path).or_default().insert(line);
    }

    pub fn remove_breakpoint(&mut self, path: &str, line: usize) {
        if let Some(lines) = self.breakpoints.get_mut(path) {
            lines.remove(&line);
        }
    }

    pub fn pause(&mut self) {
        self.state = DebugState::Paused(PauseReason::Step);
    }

    pub fn resume(&mut self) {
        self.state = DebugState::Running;
    }

    pub fn should_pause(
        &mut self,
        func_idx: usize,
        ip: usize,
        functions: &[crate::Function],
    ) -> bool {
        if let DebugState::Paused(_) = self.state {
            return true;
        }

        let function = &functions[func_idx];
        // If module_path is None, we might be in REPL or anonymous script.
        // For now, ignore or use a default name if breakpoints are set for it.
        let path = if let Some(p) = &function.module_path {
            p.as_str()
        } else {
            return false;
        };

        if ip >= function.chunk.lines.len() {
            return false;
        }
        let line = function.chunk.lines[ip] as usize;

        if let Some(lines) = self.breakpoints.get(path)
            && lines.contains(&line)
        {
            // Check if we already paused here recently?
            // For simplified logic: yes, pause.
            self.state = DebugState::Paused(PauseReason::Breakpoint);
            return true;
        }
        false
    }
}

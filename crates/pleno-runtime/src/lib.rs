#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
}

pub struct Runtime;

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self
    }
}

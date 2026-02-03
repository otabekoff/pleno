pub mod logging;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Variance {
    Invariant = 0,
    Covariant = 1,     // out
    Contravariant = 2, // in
}

impl Variance {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Covariant,
            2 => Self::Contravariant,
            _ => Self::Invariant,
        }
    }
}

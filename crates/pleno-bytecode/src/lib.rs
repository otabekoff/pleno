use pleno_value::Value;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Move,
    LoadConst,
    LoadNull,
    LoadTrue,
    LoadFalse,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    And,
    Or,
    Jump,
    JumpIf,
    JumpIfNot,
    GetGlobal,
    SetGlobal,
    Call,
    Return,
    Closure,
    GetProperty,
    SetProperty,
    GetIndex,
    SetIndex,
    NewArray,
    NewMap,
    ArrayAppend,
    ArrayLen,
    NewSet,
    SetAdd,
    SetHas,
    SetLen,
    SetValues,
    MapKeys,
    BuildRange,
    BuildRangeInclusive,
    Import,
    Throw,
    TypeOf,
    Is,
    LoadInt,
    AddInt,
    SubInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction(u32);

impl Instruction {
    pub fn new(opcode: Opcode, a: u8, b: u8, c: u8) -> Self {
        let bits = opcode as u32 | ((a as u32) << 8) | ((b as u32) << 16) | ((c as u32) << 24);
        Self(bits)
    }

    pub fn opcode(self) -> Opcode {
        unsafe { std::mem::transmute((self.0 & 0xFF) as u8) }
    }

    pub fn a(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    pub fn b(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    pub fn c(self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub lines: Vec<u32>,
    pub max_registers: usize,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            max_registers: 0,
        }
    }
}

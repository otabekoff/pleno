use pleno_value::Value;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Move,
    LoadConst,
    LoadMissing,
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
    CallNamed,
    Return,
    Closure,
    FunctionRef,
    LoadCapture,
    Capture,
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
    IterValues,
    DefineClass,
    SetSuper,
    SetStatic,
    GetSuper,
    SetFieldVisibility,
    SetStaticFieldVisibility,
    NewObject,
    GetField,
    SetField,
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
    pub functions: Vec<FunctionProto>,
    pub lines: Vec<u32>,
    pub max_registers: usize,
    pub module_name: Option<String>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: Vec::new(),
            lines: Vec::new(),
            max_registers: 0,
            module_name: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionProto {
    pub chunk: Chunk,
    pub arity: u8,
    pub params: Vec<ParamSpec>,
}

#[derive(Debug, Clone)]
pub struct ParamSpec {
    pub name: String,
    pub has_default: bool,
    pub is_variadic: bool,
    pub is_keyword_only: bool,
}

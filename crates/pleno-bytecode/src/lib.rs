use pleno_ast::Ownership;
use pleno_common::Span;
use pleno_value::Value;
use serde::{Deserialize, Serialize};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, ToPrimitive)]
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
    IterEntries,
    DefineClass,
    SetPooled,
    DefineInterface,
    SetSuper,
    SetStatic,
    GetSuper,
    SetAbstract,
    SetImplements,
    Freeze,
    SetGetters,
    SetSetters,
    SetStaticGetters,
    SetStaticSetters,
    SetFieldVisibility,
    SetStaticFieldVisibility,
    NewObject,
    GetField,
    SetField,
    BuildRange,
    BuildRangeInclusive,
    Import,
    PushHandler,
    PopHandler,
    Throw,
    TypeOf,
    Is,
    LoadInt,
    AddInt,
    SubInt,
    Partial,
    AsyncFunction,
    Await,
    ArraySpread,
    MapSpread,
    SetSpread,
    Slice,
    MulInt,
    DivInt,
    ModInt,
    ArrayExtend,
    CallSuper,
    SetCapture,
    MapRemove,
    NewTuple,
    TupleAppend,
    TupleNamedAppend,
    SetOperators,
    GetIterator,
    IterNext,
    Specialize,
    SetTypeParams,
    AddMethod,
    DefineEnum,
    AddVariant,
    ConstructVariant,
    MatchVariant,
    GetVariantField,
    AssertNotNull,
    MakeThunk,
    ForceThunk,
    MakeCell,
    LoadCell,
    SetCell,
    SetAnnotations,
    AddStaticMethod,
    SetFieldType,
    SetStaticFieldType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction(u32);

impl Instruction {
    pub fn new(opcode: Opcode, a: u8, b: u8, c: u8) -> Self {
        let bits = opcode as u32 | ((a as u32) << 8) | ((b as u32) << 16) | ((c as u32) << 24);
        Self(bits)
    }

    pub fn opcode(self) -> Opcode {
        Opcode::from_u8((self.0 & 0xFF) as u8).unwrap_or(Opcode::LoadNull)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub functions: Vec<FunctionProto>,
    pub spans: Vec<Span>,
    pub max_registers: usize,
    pub module_name: Option<String>,
    pub lines: Vec<u32>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: Vec::new(),
            spans: Vec::new(),
            max_registers: 0,
            module_name: None,
            lines: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionProto {
    pub name: String,
    pub chunk: Chunk,
    pub arity: u8,
    pub params: Vec<ParamSpec>,
    pub is_async: bool,
    pub is_pure: bool,
    pub is_inline: bool,
    pub is_hot: bool,
    pub no_optimize: bool,
    pub type_params: Vec<String>,
    #[serde(default)]
    pub type_constraints: indexmap::IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub annotations: Vec<pleno_value::RuntimeAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub has_default: bool,
    pub is_variadic: bool,
    pub is_keyword_only: bool,
    pub ownership: Ownership,
}

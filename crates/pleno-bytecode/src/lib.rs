use pleno_ast::Ownership;
use pleno_common::Span;
use pleno_value::Value;
use serde::{Deserialize, Serialize};

pub mod format;
pub use format::{
    BYTECODE_FORMAT_VERSION, BYTECODE_HEADER_LEN, BYTECODE_MAGIC, BytecodeFormatError,
    decode_chunk, encode_chunk,
};

use num_derive::{FromPrimitive, ToPrimitive};
#[cfg(test)]
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
    /// a = enum, b = variant name constant, c = value constant
    SetVariantValue,
    /// Checks the shape of a value before a destructuring `let` reads it:
    /// a = value, b = mode (`DESTRUCTURE_*`), c = element count.
    Destructure,
    /// a = b, with an `Int` converted to a `Float`: emitted after a value
    /// passes a check against an annotation that widens `Int` (see
    /// [`annotation_widens_int`]).
    ToFloat,
    /// Calls the value in register b with the c arguments in the registers
    /// after it (b + 1 ..= b + c) and stores the result in a. Unlike `Call`,
    /// it needs no argument array.
    CallRegs,
    /// Looks up method c (a name constant) of the value in register b for a
    /// `CallMethod` at a: R[a] = the method or other callee, R[a+1] = the
    /// receiver, R[a+2] = how to call R[a]: `false` for a plain call, `true`
    /// for a native method that takes the receiver first, or the method's
    /// owner (a class, enum, interface or built-in type name) for a bytecode
    /// method. Reading a property that is not such a method stores what
    /// `GetProperty` gives (calling a getter) with `false`.
    GetMethod,
    /// Calls the method that `GetMethod` looked up in registers b..=b+2 with
    /// the c arguments in the registers after them (b + 3 ..= b + 2 + c) and
    /// stores the result in a, without allocating a bound method.
    CallMethod,
    /// Sets key c (a name constant) of the map a map literal is building, in
    /// register b, to R[a]. Unlike `SetProperty` it is allowed in `@pure`
    /// functions: the map was created by the same expression.
    MapLiteralSet,
}

/// Whether a value checked against the annotation `ty` is stored as a `Float`
/// when it is an `Int`: `Float`, `Float?` and unions with `Float` but no `Int`,
/// `Number` or `Any` (`Float | String`). In `Int | Float` an `Int` stays an
/// `Int`. Collections (`Float[]`, `Map<String, Float>`) accept `Int` elements
/// without converting them. `ty` must already have its type aliases expanded.
pub fn annotation_widens_int(ty: &str) -> bool {
    let ty = ty.trim();
    let ty = ty.strip_suffix('?').unwrap_or(ty);
    let mut has_float = false;
    let mut depth = 0i32;
    let mut prev = ' ';
    let mut start = 0;
    let mut check = |part: &str| -> bool {
        match part.trim().trim_end_matches('?').trim() {
            "Float" => has_float = true,
            "Int" | "Number" | "Any" => return false,
            _ => {}
        }
        true
    };
    for (index, ch) in ty.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            // `=>` in a function type is not a closing bracket.
            '>' if prev != '=' => depth -= 1,
            ')' | ']' => depth -= 1,
            '|' if depth == 0 => {
                if !check(&ty[start..index]) {
                    return false;
                }
                start = index + 1;
            }
            _ => {}
        }
        prev = ch;
    }
    check(&ty[start..]) && has_float
}

/// The opcode with the highest discriminant: keep it the last variant.
const LAST_OPCODE: Opcode = Opcode::MapLiteralSet;

/// `Destructure` mode: a tuple with exactly `c` elements.
pub const DESTRUCTURE_TUPLE: u8 = 0;
/// `Destructure` mode: an array with exactly `c` elements.
pub const DESTRUCTURE_ARRAY: u8 = 1;
/// `Destructure` mode: an array with at least `c` elements (the pattern has a rest).
pub const DESTRUCTURE_ARRAY_AT_LEAST: u8 = 2;

/// `GetIterator` mode: the collection's values (`for (x in xs)`).
pub const ITER_VALUES: u8 = 0;
/// `GetIterator` mode: `(key, value)` entries, an array's being `(index, value)`
/// (`for (k, v in m)`).
pub const ITER_ENTRIES: u8 = 1;
/// `GetIterator` mode: a map's `(key, value)` entries, any other collection's
/// values (`for ((k, v) in m)`).
pub const ITER_MAP_ENTRIES: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction(u32);

impl Instruction {
    pub fn new(opcode: Opcode, a: u8, b: u8, c: u8) -> Self {
        let bits = opcode as u32 | ((a as u32) << 8) | ((b as u32) << 16) | ((c as u32) << 24);
        Self(bits)
    }

    #[inline]
    pub fn opcode(self) -> Opcode {
        let byte = (self.0 & 0xFF) as u8;
        if byte <= LAST_OPCODE as u8 {
            // SAFETY: `Opcode` is `repr(u8)` with the discriminants
            // `0..=LAST_OPCODE` and no gaps (checked by a test).
            unsafe { std::mem::transmute::<u8, Opcode>(byte) }
        } else {
            Opcode::LoadNull
        }
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
    pub locals: Vec<LocalVarInfo>,
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
            locals: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVarInfo {
    pub name: String,
    pub register: u8,
    pub start_ip: usize,
    pub end_ip: usize,
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
    pub type_params: Vec<pleno_value::GenericParamMetadata>,
    #[serde(default)]
    pub type_constraints: indexmap::IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub annotations: Vec<pleno_value::RuntimeAnnotation>,
    /// Type parameters of an enclosing function or class that a closure's
    /// annotations mention, each with the index of the capture holding its
    /// binding where the closure was created (see `captured_type_params` on
    /// the VM's `Function`).
    #[serde(default)]
    pub captured_type_params: Vec<(String, u8)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub has_default: bool,
    pub is_variadic: bool,
    pub is_keyword_only: bool,
    pub ownership: Ownership,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_decode_like_from_primitive() {
        for byte in 0..=u8::MAX {
            let decoded = Instruction(byte as u32).opcode();
            let expected = Opcode::from_u8(byte).unwrap_or(Opcode::LoadNull);
            assert_eq!(decoded, expected, "opcode byte {byte}");
        }
        assert!(Opcode::from_u8(LAST_OPCODE as u8 + 1).is_none());
    }

    #[test]
    fn float_annotations_widen_int() {
        for ty in [
            "Float",
            " Float ",
            "Float?",
            "Float | String",
            "String | Float?",
            "Float | Null",
        ] {
            assert!(annotation_widens_int(ty), "{ty}");
        }
    }

    #[test]
    fn other_annotations_keep_int() {
        for ty in [
            "Int",
            "Int | Float",
            "Float | Number",
            "Any | Float",
            "Float[]",
            "Map<String, Float>",
            "Array<Float>",
            "(Float, Int)",
            "(Int) => Float",
            "String",
            "Float & Shape",
        ] {
            assert!(!annotation_widens_int(ty), "{ty}");
        }
    }
}

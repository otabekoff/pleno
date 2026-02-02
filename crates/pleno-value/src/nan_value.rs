// This file is part of the Pleno programming language.
use crate::{
    ArrayState, Class, ClosureState, EnumType, Interface, MapState, Object, Range, SetState,
    ThunkState, TupleState, Value, gc::GcObject, gc::GcRc,
};
use std::mem::ManuallyDrop;
use std::rc::Rc;

/// Optimized 64-bit representation of Pleno values using NaN boxing.
///
/// Layout:
/// - Any bit pattern where exponent (bits 62-52) is NOT all 1s is a standard IEEE 754 f64.
/// - If exponent is 0x7FF, it's a tagged value.
/// - We use the sign bit (63) and bits 50-48 as tags.
///
/// Tags (bits 50-48):
/// - 0x0: Float (Actually not used as tag, identified by exponent)
/// - 0x1: Integer (Lower 48 bits)
/// - 0x2: Boolean (Bit 0)
/// - 0x3: Null
/// - 0x4: Missing
/// - 0x5: Heap Pointer (Bits 47-0)
/// - 0x6: Function/Closure Index
/// - 0x7: Char
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct NanValue(u64);

const QNAN: u64 = 0x7ff8_0000_0000_0000;
const SIGN_BIT: u64 = 0x8000_0000_0000_0000;

const TAG_INT: u64 = 0x1;
const TAG_BOOL: u64 = 0x2;
const TAG_NULL: u64 = 0x3;
const TAG_MISSING: u64 = 0x4;
const TAG_PTR: u64 = 0x5;
const TAG_FUNC: u64 = 0x6;
const TAG_CHAR: u64 = 0x7;

const TAG_SHIFT: u32 = 48;
const SUB_TAG_SHIFT: u32 = 44;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const PTR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Sub-tags for TAG_PTR (bits 47-44)
pub const SUB_TAG_STRING: u64 = 0x0;
pub const SUB_TAG_MAP: u64 = 0x1;
pub const SUB_TAG_ARRAY: u64 = 0x2;
pub const SUB_TAG_SET: u64 = 0x3;
pub const SUB_TAG_OBJECT: u64 = 0x4;
pub const SUB_TAG_CLASS: u64 = 0x5;
pub const SUB_TAG_CLOSURE: u64 = 0x6;
pub const SUB_TAG_THUNK: u64 = 0x7;
pub const SUB_TAG_RANGE: u64 = 0x8;
pub const SUB_TAG_TUPLE: u64 = 0x9;
pub const SUB_TAG_ENUM: u64 = 0xA;
pub const SUB_TAG_VARIANT: u64 = 0xB;
pub const SUB_TAG_INTERFACE: u64 = 0xC;
pub const SUB_TAG_FILE: u64 = 0xD;
pub const SUB_TAG_MUTEX: u64 = 0xE;
pub const SUB_TAG_CHANNEL: u64 = 0xF;

// Since we ran out of bits in SUB_TAG (4 bits = 16 values), we'll need to handle more carefully if we add more.
// For now, 16 is enough for the core types.

impl NanValue {
    #[inline]
    pub fn from_f64(f: f64) -> Self {
        Self(f.to_bits())
    }

    #[inline]
    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }

    #[inline]
    pub fn is_f64(self) -> bool {
        (self.0 & 0x7ff0_0000_0000_0000) != 0x7ff0_0000_0000_0000
    }

    #[inline]
    fn encode_typed(tag: u64, sub_tag: u64, payload: u64) -> Self {
        Self(
            SIGN_BIT
                | QNAN
                | (tag << TAG_SHIFT)
                | (sub_tag << SUB_TAG_SHIFT)
                | (payload & PTR_MASK),
        )
    }

    #[inline]
    fn encode(tag: u64, payload: u64) -> Self {
        Self(SIGN_BIT | QNAN | (tag << TAG_SHIFT) | (payload & PAYLOAD_MASK))
    }

    #[inline]
    fn tag(self) -> u64 {
        if self.is_f64() {
            0
        } else {
            (self.0 >> TAG_SHIFT) & 0x7
        }
    }

    #[inline]
    pub fn sub_tag(self) -> u64 {
        if self.tag() == TAG_PTR {
            (self.0 >> SUB_TAG_SHIFT) & 0xF
        } else {
            0
        }
    }

    #[inline]
    pub fn from_int(i: i64) -> Self {
        Self::encode(TAG_INT, i as u64)
    }

    #[inline]
    pub fn as_int(self) -> Option<i64> {
        if self.tag() == TAG_INT {
            let val = self.0 & PAYLOAD_MASK;
            if (val & (1 << 47)) != 0 {
                Some((val | !PAYLOAD_MASK) as i64)
            } else {
                Some(val as i64)
            }
        } else {
            None
        }
    }

    #[inline]
    pub fn from_bool(b: bool) -> Self {
        Self::encode(TAG_BOOL, if b { 1 } else { 0 })
    }

    #[inline]
    pub fn as_bool(self) -> Option<bool> {
        if self.tag() == TAG_BOOL {
            Some((self.0 & 0x1) != 0)
        } else {
            None
        }
    }

    #[inline]
    pub fn null() -> Self {
        Self::encode(TAG_NULL, 0)
    }

    #[inline]
    pub fn is_null(self) -> bool {
        self.tag() == TAG_NULL
    }

    #[inline]
    pub fn missing() -> Self {
        Self::encode(TAG_MISSING, 0)
    }

    #[inline]
    pub fn is_missing(self) -> bool {
        self.tag() == TAG_MISSING
    }

    #[inline]
    pub fn from_char(c: char) -> Self {
        Self::encode(TAG_CHAR, c as u32 as u64)
    }

    #[inline]
    pub fn as_char(self) -> Option<char> {
        if self.tag() == TAG_CHAR {
            std::char::from_u32((self.0 & PAYLOAD_MASK) as u32)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::encode_typed(TAG_PTR, 0xF, ptr as usize as u64)
    }

    #[inline]
    pub fn from_typed_ptr<T>(ptr: *const T, sub_tag: u64) -> Self {
        Self::encode_typed(TAG_PTR, sub_tag, ptr as usize as u64)
    }

    #[inline]
    pub fn as_ptr<T>(self) -> Option<*const T> {
        if self.tag() == TAG_PTR {
            Some((self.0 & PTR_MASK) as usize as *const T)
        } else {
            None
        }
    }

    pub fn to_value(self) -> Value {
        if self.is_f64() {
            Value::Float(self.as_f64())
        } else {
            match self.tag() {
                TAG_INT => Value::Int(self.as_int().unwrap()),
                TAG_BOOL => Value::Bool(self.as_bool().unwrap()),
                TAG_NULL => Value::Null,
                TAG_MISSING => Value::Missing,
                TAG_CHAR => Value::Char(self.as_char().unwrap()),
                TAG_FUNC => Value::Function(self.as_ptr::<()>().unwrap() as usize),
                TAG_PTR => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    match self.sub_tag() {
                        SUB_TAG_STRING => {
                            let rc =
                                unsafe { ManuallyDrop::new(Rc::from_raw(ptr as *const String)) };
                            Value::String(Rc::clone(&rc))
                        }
                        SUB_TAG_MAP => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<MapState>,
                                ))
                            };
                            Value::Map(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_ARRAY => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<ArrayState>,
                                ))
                            };
                            Value::Array(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_SET => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<SetState>,
                                ))
                            };
                            Value::Set(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_OBJECT => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<Object>,
                                ))
                            };
                            Value::Object(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_CLASS => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<Class>,
                                ))
                            };
                            Value::Class(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_CLOSURE => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<ClosureState>,
                                ))
                            };
                            Value::Closure(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_THUNK => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<ThunkState>,
                                ))
                            };
                            Value::Thunk(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_RANGE => Value::Range(unsafe { (*(ptr as *const Range)).clone() }),
                        SUB_TAG_TUPLE => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<TupleState>,
                                ))
                            };
                            Value::Tuple(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_ENUM => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<EnumType>,
                                ))
                            };
                            Value::Enum(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_INTERFACE => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<Interface>,
                                ))
                            };
                            Value::Interface(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_FILE => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<crate::FileState>,
                                ))
                            };
                            Value::File(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_MUTEX => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<crate::MutexState>,
                                ))
                            };
                            Value::Mutex(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        SUB_TAG_CHANNEL => {
                            let rc = unsafe {
                                ManuallyDrop::new(Rc::from_raw(
                                    ptr as *const std::cell::RefCell<crate::ChannelState>,
                                ))
                            };
                            Value::Channel(GcRc {
                                inner: Rc::clone(&rc),
                            })
                        }
                        _ => panic!(
                            "Reconstructing complex type {:?} from NanValue is not yet implemented",
                            self.sub_tag()
                        ),
                    }
                }
                _ => Value::Null,
            }
        }
    }

    pub fn from_value(v: &Value) -> Self {
        match v {
            Value::Null => Self::null(),
            Value::Missing => Self::missing(),
            Value::Bool(b) => Self::from_bool(*b),
            Value::Int(i) => Self::from_int(*i),
            Value::Float(f) => Self::from_f64(*f),
            Value::Char(c) => Self::from_char(*c),
            Value::Function(idx) => Self::encode(TAG_FUNC, *idx as u64),
            Value::String(s) => Self::from_typed_ptr(Rc::as_ptr(s), SUB_TAG_STRING),
            Value::Array(a) => Self::from_typed_ptr(a.as_ptr(), SUB_TAG_ARRAY),
            Value::Map(m) => Self::from_typed_ptr(m.as_ptr(), SUB_TAG_MAP),
            Value::Object(o) => Self::from_typed_ptr(o.as_ptr(), SUB_TAG_OBJECT),
            Value::Class(c) => Self::from_typed_ptr(c.as_ptr(), SUB_TAG_CLASS),
            Value::Closure(c) => Self::from_typed_ptr(c.as_ptr(), SUB_TAG_CLOSURE),
            Value::Thunk(t) => Self::from_typed_ptr(t.as_ptr(), SUB_TAG_THUNK),
            Value::Range(r) => Self::from_typed_ptr(r as *const Range, SUB_TAG_RANGE),
            Value::Set(s) => Self::from_typed_ptr(s.as_ptr(), SUB_TAG_SET),
            Value::Tuple(t) => Self::from_typed_ptr(t.as_ptr(), SUB_TAG_TUPLE),
            Value::Enum(e) => Self::from_typed_ptr(e.as_ptr(), SUB_TAG_ENUM),
            Value::Interface(i) => Self::from_typed_ptr(i.as_ptr(), SUB_TAG_INTERFACE),
            Value::File(f) => Self::from_typed_ptr(f.as_ptr(), SUB_TAG_FILE),
            Value::Mutex(m) => Self::from_typed_ptr(m.as_ptr(), SUB_TAG_MUTEX),
            Value::Channel(c) => Self::from_typed_ptr(c.as_ptr(), SUB_TAG_CHANNEL),
            _ => {
                // For complex types like BoundMethod, Partial, etc. that aren't heap-GCRc objects directly
                // we currently box them as Null or we could implement more tags.
                // However, they should ideally be allocated via GC if they need to be in registers.
                Self::null()
            }
        }
    }

    pub fn is_truthy(self) -> bool {
        if self.is_f64() {
            true
        } else {
            match self.tag() {
                TAG_NULL | TAG_MISSING => false,
                TAG_BOOL => self.as_bool().unwrap(),
                _ => true,
            }
        }
    }

    pub fn type_name(self) -> &'static str {
        if self.is_f64() {
            "Float"
        } else {
            match self.tag() {
                TAG_INT => "Int",
                TAG_BOOL => "Bool",
                TAG_NULL => "Null",
                TAG_MISSING => "Missing",
                TAG_FUNC => "Function",
                TAG_CHAR => "Char",
                TAG_PTR => match self.sub_tag() {
                    SUB_TAG_STRING => "String",
                    SUB_TAG_MAP => "Map",
                    SUB_TAG_ARRAY => "Array",
                    SUB_TAG_SET => "Set",
                    SUB_TAG_OBJECT => "Object",
                    SUB_TAG_CLASS => "Class",
                    SUB_TAG_CLOSURE => "Closure",
                    SUB_TAG_THUNK => "Thunk",
                    SUB_TAG_RANGE => "Range",
                    SUB_TAG_TUPLE => "Tuple",
                    SUB_TAG_ENUM => "Enum",
                    SUB_TAG_VARIANT => "Variant",
                    SUB_TAG_INTERFACE => "Interface",
                    SUB_TAG_FILE => "File",
                    SUB_TAG_MUTEX => "Mutex",
                    SUB_TAG_CHANNEL => "Channel",
                    _ => "Object",
                },
                _ => "Unknown",
            }
        }
    }

    pub fn mark(&self, gc: &mut crate::gc::MarkAndSweepGC) {
        if self.tag() == TAG_PTR {
            let ptr = self.as_ptr::<()>().unwrap();
            match self.sub_tag() {
                SUB_TAG_MAP => {
                    let ptr = ptr as *const std::cell::RefCell<MapState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_ARRAY => {
                    let ptr = ptr as *const std::cell::RefCell<ArrayState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_SET => {
                    let ptr = ptr as *const std::cell::RefCell<SetState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_OBJECT => {
                    let ptr = ptr as *const std::cell::RefCell<Object>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_CLASS => {
                    let ptr = ptr as *const std::cell::RefCell<Class>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_CLOSURE => {
                    let ptr = ptr as *const std::cell::RefCell<ClosureState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_THUNK => {
                    let ptr = ptr as *const std::cell::RefCell<ThunkState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_TUPLE => {
                    let ptr = ptr as *const std::cell::RefCell<TupleState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_FILE => {
                    let ptr = ptr as *const std::cell::RefCell<crate::FileState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_MUTEX => {
                    let ptr = ptr as *const std::cell::RefCell<crate::MutexState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                SUB_TAG_CHANNEL => {
                    let ptr = ptr as *const std::cell::RefCell<crate::ChannelState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                _ => {}
            }
        }
    }
}

impl std::fmt::Debug for NanValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_f64() {
            write!(f, "Float({})", self.as_f64())
        } else {
            match self.tag() {
                TAG_INT => write!(f, "Int({:?})", self.as_int().unwrap()),
                TAG_BOOL => write!(f, "Bool({:?})", self.as_bool().unwrap()),
                TAG_NULL => write!(f, "Null"),
                TAG_MISSING => write!(f, "Missing"),
                TAG_PTR => write!(
                    f,
                    "Ptr({:p}, sub={})",
                    self.as_ptr::<()>().unwrap(),
                    self.sub_tag()
                ),
                TAG_FUNC => write!(f, "Func(...)"),
                TAG_CHAR => write!(f, "Char({:?})", self.as_char().unwrap()),
                _ => write!(f, "Unknown({:#x})", self.0),
            }
        }
    }
}

impl std::fmt::Display for NanValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_f64() {
            write!(f, "{}", self.as_f64())
        } else {
            match self.tag() {
                TAG_INT => write!(f, "{}", self.as_int().unwrap()),
                TAG_BOOL => write!(f, "{}", self.as_bool().unwrap()),
                TAG_NULL => write!(f, "null"),
                TAG_MISSING => write!(f, "missing"),
                TAG_PTR => write!(f, "<object@{:p}>", self.as_ptr::<()>().unwrap()),
                TAG_FUNC => write!(f, "<function>"),
                TAG_CHAR => write!(f, "{}", self.as_char().unwrap()),
                _ => write!(f, "unknown"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nan_value_float() {
        let f = std::f64::consts::PI;
        let v = NanValue::from_f64(f);
        assert!(v.is_f64());
        assert_eq!(v.as_f64(), f);
    }

    #[test]
    fn test_nan_value_int() {
        let i = 12345678;
        let v = NanValue::from_int(i);
        assert!(!v.is_f64());
        assert_eq!(v.as_int(), Some(i));

        let neg = -12345678;
        let vn = NanValue::from_int(neg);
        assert!(!vn.is_f64());
        assert_eq!(vn.as_int(), Some(neg));
    }

    #[test]
    fn test_nan_value_bool() {
        let v = NanValue::from_bool(true);
        assert!(!v.is_f64());
        assert_eq!(v.as_bool(), Some(true));

        let v2 = NanValue::from_bool(false);
        assert_eq!(v2.as_bool(), Some(false));
    }

    #[test]
    fn test_nan_value_null() {
        let v = NanValue::null();
        assert!(!v.is_f64());
        assert!(v.is_null());
    }

    #[test]
    fn test_nan_value_char() {
        let c = 'A';
        let v = NanValue::from_char(c);
        assert!(!v.is_f64());
        assert_eq!(v.as_char(), Some(c));
    }

    #[test]
    fn test_nan_value_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Missing,
            Value::Bool(true),
            Value::Bool(false),
            Value::Int(123),
            Value::Int(-456),
            Value::Float(1.23),
            Value::Char('Z'),
        ];

        for val in values {
            let nan = NanValue::from_value(&val);
            let roundtrip = nan.to_value();
            match (val, roundtrip) {
                (Value::Float(f1), Value::Float(f2)) => {
                    assert!((f1 - f2).abs() < 1e-10);
                }
                (v1, v2) => assert_eq!(v1, v2),
            }
        }
    }
}

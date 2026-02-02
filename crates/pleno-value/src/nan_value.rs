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
/// - Tag bits (5 bits total): Bit 63 (sign) and bits 50-47.
/// - Payload/Pointer (47 bits): Bits 46-0.
///
/// This layout works on 64-bit Linux because user-space pointers are always within [0, 2^47-1].
/// Thus bit 47 is always 0, and we use bits 51-47 for tags safely. Bit 51 is kept 1 for QNAN.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct NanValue(u64);

const QNAN: u64 = 0x7ff8_0000_0000_0000;

const TAG_INT: u64 = 1;
const TAG_BOOL: u64 = 2;
const TAG_NULL: u64 = 3;
const TAG_MISSING: u64 = 4;
const TAG_CHAR: u64 = 5;
const TAG_FUNC: u64 = 6;
const TAG_STRING: u64 = 7;
const TAG_MAP: u64 = 8;
const TAG_ARRAY: u64 = 9;
const TAG_SET: u64 = 10;
const TAG_OBJECT: u64 = 11;
const TAG_CLASS: u64 = 12;
const TAG_CLOSURE: u64 = 13;
const TAG_THUNK: u64 = 14;
const TAG_TUPLE: u64 = 15;
const TAG_ENUM: u64 = 16;
const TAG_INTERFACE: u64 = 17;
const TAG_FILE: u64 = 18;
const TAG_MUTEX: u64 = 19;
const TAG_CHANNEL: u64 = 20;
const TAG_RANGE: u64 = 22;
const TAG_BOXED: u64 = 23;

const PAYLOAD_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;

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
    fn tag(self) -> u64 {
        if self.is_f64() {
            0
        } else {
            let low = (self.0 >> 47) & 0xF;
            let high = (self.0 >> 63) & 0x1;
            low | (high << 4)
        }
    }

    #[inline]
    fn encode(tag: u64, payload: u64) -> Self {
        let low = tag & 0xF;
        let high = (tag >> 4) & 0x1;
        Self((high << 63) | QNAN | (low << 47) | (payload & PAYLOAD_MASK))
    }

    #[inline]
    pub fn from_int(i: i64) -> Self {
        Self::encode(TAG_INT, i as u64)
    }

    #[inline]
    pub fn as_int(self) -> Option<i64> {
        if self.tag() == TAG_INT {
            let val = self.0 & PAYLOAD_MASK;
            if (val & (1 << 46)) != 0 {
                // Sign extend 47th bit (bit 46) if necessary
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
    pub fn as_ptr<T>(self) -> Option<*const T> {
        if self.tag() >= TAG_STRING {
            // All tags >= TAG_STRING are pointers
            Some((self.0 & PAYLOAD_MASK) as usize as *const T)
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
                TAG_FUNC => Value::Function((self.0 & PAYLOAD_MASK) as usize),
                TAG_STRING => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe { ManuallyDrop::new(Rc::from_raw(ptr as *const String)) };
                    Value::String(Rc::clone(&rc))
                }
                TAG_MAP => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<MapState>))
                    };
                    Value::Map(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_ARRAY => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<ArrayState>,
                        ))
                    };
                    Value::Array(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_SET => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<SetState>))
                    };
                    Value::Set(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_OBJECT => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<Object>))
                    };
                    Value::Object(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_CLASS => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<Class>))
                    };
                    Value::Class(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_CLOSURE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<ClosureState>,
                        ))
                    };
                    Value::Closure(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_THUNK => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<ThunkState>,
                        ))
                    };
                    Value::Thunk(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_TUPLE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<TupleState>,
                        ))
                    };
                    Value::Tuple(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_ENUM => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<EnumType>))
                    };
                    Value::Enum(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_INTERFACE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<Interface>))
                    };
                    Value::Interface(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_FILE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::FileState>,
                        ))
                    };
                    Value::File(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_MUTEX => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::MutexState>,
                        ))
                    };
                    Value::Mutex(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_CHANNEL => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::ChannelState>,
                        ))
                    };
                    Value::Channel(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_RANGE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    Value::Range(unsafe { (*(ptr as *const Range)).clone() })
                }
                TAG_BOXED => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    unsafe { (*(ptr as *const Value)).clone() }
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
            Value::String(s) => {
                let ptr = Rc::as_ptr(s);
                unsafe { Rc::increment_strong_count(ptr) };
                Self::encode(TAG_STRING, ptr as usize as u64)
            }
            Value::Array(a) => Self::encode(TAG_ARRAY, a.as_refcell_ptr() as usize as u64),
            Value::Map(m) => Self::encode(TAG_MAP, m.as_refcell_ptr() as usize as u64),
            Value::Set(s) => Self::encode(TAG_SET, s.as_refcell_ptr() as usize as u64),
            Value::Object(o) => Self::encode(TAG_OBJECT, o.as_refcell_ptr() as usize as u64),
            Value::Class(c) => Self::encode(TAG_CLASS, c.as_refcell_ptr() as usize as u64),
            Value::Closure(c) => Self::encode(TAG_CLOSURE, c.as_refcell_ptr() as usize as u64),
            Value::Thunk(t) => Self::encode(TAG_THUNK, t.as_refcell_ptr() as usize as u64),
            Value::Tuple(t) => Self::encode(TAG_TUPLE, t.as_refcell_ptr() as usize as u64),
            Value::Enum(e) => Self::encode(TAG_ENUM, e.as_refcell_ptr() as usize as u64),
            Value::Interface(i) => Self::encode(TAG_INTERFACE, i.as_refcell_ptr() as usize as u64),
            Value::File(f) => Self::encode(TAG_FILE, f.as_refcell_ptr() as usize as u64),
            Value::Mutex(m) => Self::encode(TAG_MUTEX, m.as_refcell_ptr() as usize as u64),
            Value::Channel(c) => Self::encode(TAG_CHANNEL, c.as_refcell_ptr() as usize as u64),
            Value::Range(r) => Self::encode(TAG_RANGE, r as *const Range as usize as u64),
            v => {
                let boxed = Box::new(v.clone());
                let ptr = Box::into_raw(boxed);
                Self::encode(TAG_BOXED, ptr as usize as u64)
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
                TAG_STRING => "String",
                TAG_MAP => "Map",
                TAG_ARRAY => "Array",
                TAG_SET => "Set",
                TAG_OBJECT => "Object",
                TAG_CLASS => "Class",
                TAG_CLOSURE => "Closure",
                TAG_THUNK => "Thunk",
                TAG_RANGE => "Range",
                TAG_TUPLE => "Tuple",
                TAG_ENUM => "Enum",
                TAG_INTERFACE => "Interface",
                TAG_FILE => "File",
                TAG_MUTEX => "Mutex",
                TAG_CHANNEL => "Channel",
                _ => "Unknown",
            }
        }
    }

    pub fn mark(&self, gc: &mut crate::gc::MarkAndSweepGC) {
        let tag = self.tag();
        if (TAG_MAP..=TAG_CHANNEL).contains(&tag) {
            let ptr = self.as_ptr::<()>().unwrap();
            match tag {
                TAG_MAP => {
                    let ptr = ptr as *const std::cell::RefCell<MapState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_ARRAY => {
                    let ptr = ptr as *const std::cell::RefCell<ArrayState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_SET => {
                    let ptr = ptr as *const std::cell::RefCell<SetState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_OBJECT => {
                    let ptr = ptr as *const std::cell::RefCell<Object>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_CLASS => {
                    let ptr = ptr as *const std::cell::RefCell<Class>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_CLOSURE => {
                    let ptr = ptr as *const std::cell::RefCell<ClosureState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_THUNK => {
                    let ptr = ptr as *const std::cell::RefCell<ThunkState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_TUPLE => {
                    let ptr = ptr as *const std::cell::RefCell<TupleState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_FILE => {
                    let ptr = ptr as *const std::cell::RefCell<crate::FileState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_MUTEX => {
                    let ptr = ptr as *const std::cell::RefCell<crate::MutexState>;
                    unsafe { (*ptr).borrow().mark(gc) };
                }
                TAG_CHANNEL => {
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
                TAG_FUNC => write!(f, "Func({})", self.0 & PAYLOAD_MASK),
                TAG_CHAR => write!(f, "Char({:?})", self.as_char().unwrap()),
                t if t >= TAG_STRING => {
                    write!(f, "Ptr({:p}, tag={})", self.as_ptr::<()>().unwrap(), t)
                }
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
                t if t >= TAG_STRING => write!(f, "<object@{:p}>", self.as_ptr::<()>().unwrap()),
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

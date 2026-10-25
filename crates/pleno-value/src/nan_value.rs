// This file is part of the Pleno programming language.
use crate::{
    ArrayState, BoundFunctionState, BoundMethodState, BoundNativeMethodState, CellState, Class,
    ClosureState, EnumType, Interface, MapState, Object, PartialState, Range, SetState,
    StaticMethodState, ThunkState, TupleState, Value, gc::GcObject, gc::GcRc,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::rc::Rc;

/// Values that cannot live inside a NaN-boxed payload are kept in per-thread
/// tables and referenced by slot index:
///
/// - integers outside the inline 46-bit range,
/// - strings (the table owns one `Rc`, so encoding a string never leaks it),
/// - ranges (stored by value; they have no heap allocation of their own).
///
/// NaN-boxed values are `Copy` and only stored in VM registers, which every
/// collection marks. A sweep after the collection frees the unmarked slots.
struct Slots<T> {
    values: Vec<Option<T>>,
    marked: Vec<bool>,
    free: Vec<usize>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            marked: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Slots<T> {
    fn insert(&mut self, value: T) -> usize {
        match self.free.pop() {
            Some(slot) => {
                self.values[slot] = Some(value);
                slot
            }
            None => {
                self.values.push(Some(value));
                self.marked.push(false);
                self.values.len() - 1
            }
        }
    }

    fn live(&self) -> usize {
        self.values.iter().filter(|value| value.is_some()).count()
    }

    fn get(&self, slot: usize) -> &T {
        self.values[slot]
            .as_ref()
            .expect("NaN-boxed value refers to a freed slot")
    }

    fn mark(&mut self, slot: usize) {
        if let Some(mark) = self.marked.get_mut(slot) {
            *mark = true;
        }
    }

    /// Frees unmarked slots, passing each freed value to `on_free`, and clears
    /// the marks for the next collection.
    fn sweep(&mut self, mut on_free: impl FnMut(&T)) {
        for slot in 0..self.values.len() {
            if std::mem::take(&mut self.marked[slot]) {
                continue;
            }
            if let Some(value) = self.values[slot].take() {
                on_free(&value);
                self.free.push(slot);
            }
        }
    }
}

/// A fast hasher for the integer and pointer keys of the slot tables: one
/// multiplication instead of SipHash, which showed up in profiles of loops
/// over big integers. Colliding keys only cost time, never correctness.
#[derive(Default)]
struct SlotKeyHasher(u64);

impl std::hash::Hasher for SlotKeyHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(byte as u64);
        }
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        // Fibonacci hashing: the high bits, which the table uses, depend on
        // every bit of the key.
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }

    #[inline]
    fn write_i64(&mut self, value: i64) {
        self.write_u64(value as u64);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

type SlotMap<K> = HashMap<K, usize, std::hash::BuildHasherDefault<SlotKeyHasher>>;

#[derive(Default)]
struct BoxedValues {
    ints: Slots<i64>,
    /// Equal integers share a slot, so integers still compare by their bits.
    int_slots: SlotMap<i64>,
    strings: Slots<Rc<String>>,
    /// The same string reuses its slot instead of growing the table.
    string_slots: SlotMap<*const String>,
    ranges: Slots<Range>,
}

thread_local! {
    static BOXED: RefCell<BoxedValues> = RefCell::new(BoxedValues::default());
}

/// Inline integers use 46 value bits plus a flag bit that marks boxed ones.
const INLINE_INT_MIN: i64 = -(1 << 45);
const INLINE_INT_MAX: i64 = (1 << 45) - 1;

fn box_int(value: i64) -> usize {
    BOXED.with(|boxed| {
        let boxed = &mut *boxed.borrow_mut();
        match boxed.int_slots.entry(value) {
            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                *entry.insert(boxed.ints.insert(value))
            }
        }
    })
}

fn box_string(value: &Rc<String>) -> usize {
    BOXED.with(|boxed| {
        let boxed = &mut *boxed.borrow_mut();
        match boxed.string_slots.entry(Rc::as_ptr(value)) {
            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                *entry.insert(boxed.strings.insert(Rc::clone(value)))
            }
        }
    })
}

fn box_range(value: &Range) -> usize {
    BOXED.with(|boxed| boxed.borrow_mut().ranges.insert(value.clone()))
}

/// Live slots in the big-integer, string and range tables (for diagnostics
/// and leak tests).
pub fn boxed_value_counts() -> (usize, usize, usize) {
    BOXED.with(|boxed| {
        let boxed = boxed.borrow();
        (boxed.ints.live(), boxed.strings.live(), boxed.ranges.live())
    })
}

/// Frees table slots not marked since the last sweep. Called after a
/// collection that marked every register of the VM.
pub fn sweep_boxed_values() {
    BOXED.with(|boxed| {
        let mut boxed = boxed.borrow_mut();
        let boxed = &mut *boxed;
        let int_slots = &mut boxed.int_slots;
        boxed.ints.sweep(|value| {
            int_slots.remove(value);
        });
        let string_slots = &mut boxed.string_slots;
        boxed.strings.sweep(|value| {
            string_slots.remove(&Rc::as_ptr(value));
        });
        boxed.ranges.sweep(|_| {});
    });
}

/// Optimized 64-bit representation of Pleno values using NaN boxing.
///
/// Layout:
/// - Any bit pattern without all of the exponent bits (62-52) and the quiet
///   bit (51) set is a standard IEEE 754 f64, which includes the infinities.
/// - NaNs are stored as the canonical quiet NaN `QNAN` (tag 0, no payload).
/// - Every other pattern with those bits set is a tagged value.
/// - Tag bits (5 bits total): Bit 63 (sign) and bits 50-47.
/// - Payload/Pointer (47 bits): Bits 46-0.
///
/// This layout works on 64-bit Linux because user-space pointers are always within [0, 2^47-1].
/// Thus bit 47 is always 0, and we use bits 51-47 for tags safely. Bit 51 is kept 1 for QNAN.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct NanValue(u64);

/// The payload of a native function: its address with the low bit set to tell
/// it apart from a bytecode function index. Code addresses on 64-bit targets
/// are even and fit in 47 bits, so the bit is free. On 32-bit targets such as
/// WebAssembly, function "addresses" are table indices and can be odd, so the
/// address moves up a bit instead; 33 bits still fit the payload.
fn native_payload(address: usize) -> u64 {
    if cfg!(target_pointer_width = "32") {
        ((address as u64) << 1) | 0x1
    } else {
        let address = address as u64;
        debug_assert!(
            address & 0x1 == 0 && address <= PAYLOAD_MASK,
            "native function address {address:#x} cannot be NaN-boxed"
        );
        address | 0x1
    }
}

/// The address stored by `native_payload`.
fn native_address(payload: u64) -> usize {
    if cfg!(target_pointer_width = "32") {
        (payload >> 1) as usize
    } else {
        (payload & !0x1) as usize
    }
}

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
const TAG_PARTIAL: u64 = 23;
const TAG_BOUND_METHOD: u64 = 24;
const TAG_STATIC_METHOD: u64 = 25;
const TAG_BOUND_NATIVE: u64 = 26;
const TAG_BOUND_FUNC: u64 = 27;
const TAG_CELL: u64 = 28;
const TAG_ITERATOR: u64 = 21;
const TAG_DATABASE: u64 = 29;
const TAG_RESOURCE: u64 = 30;
const TAG_VARIANT_COMBINED: u64 = 31;

const PAYLOAD_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;

impl NanValue {
    /// Every NaN is stored as the one canonical quiet NaN (`QNAN`), which no
    /// tagged value uses, so a NaN's payload cannot be mistaken for a tag.
    #[inline]
    pub fn from_f64(f: f64) -> Self {
        if f.is_nan() {
            Self(QNAN)
        } else {
            Self(f.to_bits())
        }
    }

    #[inline]
    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Tagged values always have the exponent and the quiet bit set. Of the
    /// floats, only the canonical NaN does too; infinities do not.
    #[inline]
    pub fn is_f64(self) -> bool {
        (self.0 & QNAN) != QNAN || self.0 == QNAN
    }

    #[inline]
    pub fn tag(self) -> u64 {
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

    /// Integers in the 46-bit range are stored inline, shifted left by one;
    /// larger ones are boxed, with the low payload bit set and a slot index.
    #[inline]
    pub fn from_int(i: i64) -> Self {
        if (INLINE_INT_MIN..=INLINE_INT_MAX).contains(&i) {
            Self::encode(TAG_INT, (i << 1) as u64)
        } else {
            Self::encode(TAG_INT, ((box_int(i) as u64) << 1) | 1)
        }
    }

    #[inline]
    pub fn as_int(self) -> Option<i64> {
        if self.tag() != TAG_INT {
            return None;
        }
        let payload = self.0 & PAYLOAD_MASK;
        if payload & 1 == 1 {
            let slot = (payload >> 1) as usize;
            return Some(BOXED.with(|boxed| *boxed.borrow().ints.get(slot)));
        }
        // Sign-extend the 47-bit payload, then drop the flag bit.
        Some((((payload << 17) as i64) >> 17) >> 1)
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

    /// The index of a bytecode function (not a native one).
    #[inline]
    pub fn as_function_index(self) -> Option<usize> {
        if self.tag() != TAG_FUNC {
            return None;
        }
        let payload = self.0 & PAYLOAD_MASK;
        if payload & 0x1 != 0 {
            None
        } else {
            Some((payload >> 1) as usize)
        }
    }

    #[inline]
    pub fn is_closure(self) -> bool {
        self.tag() == TAG_CLOSURE
    }

    #[inline]
    pub fn is_thunk(self) -> bool {
        self.tag() == TAG_THUNK
    }

    #[inline]
    pub fn as_ptr<T>(self) -> Option<*const T> {
        if self.tag() >= TAG_STRING {
            // All tags >= TAG_STRING are pointers
            // Mask out the discriminator bit for combined tags if necessary
            let ptr = (self.0 & PAYLOAD_MASK) & !0x1;
            Some(ptr as usize as *const T)
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
                TAG_FUNC => {
                    let payload = self.0 & PAYLOAD_MASK;
                    if (payload & 0x1) != 0 {
                        let ptr = native_address(payload) as *const ();
                        Value::NativeFunction(unsafe {
                            std::mem::transmute::<*const (), crate::NativeFunction>(ptr)
                        })
                    } else {
                        Value::Function((payload >> 1) as usize)
                    }
                }
                TAG_STRING => {
                    let slot = (self.0 & PAYLOAD_MASK) as usize;
                    Value::String(BOXED.with(|boxed| Rc::clone(boxed.borrow().strings.get(slot))))
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
                    // The low pointer bit distinguishes a specialized generic class.
                    let is_bound = (self.0 & 0x1) != 0;
                    let ptr = self.as_ptr::<()>().unwrap();
                    if is_bound {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(
                                ptr as *const std::cell::RefCell<crate::BoundClassState>,
                            ))
                        };
                        Value::BoundClass(GcRc {
                            inner: Rc::clone(&rc),
                        })
                    } else {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<Class>))
                        };
                        Value::Class(GcRc {
                            inner: Rc::clone(&rc),
                        })
                    }
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
                    // The low pointer bit distinguishes a specialized generic interface.
                    let is_bound = (self.0 & 0x1) != 0;
                    let ptr = self.as_ptr::<()>().unwrap();
                    if is_bound {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(
                                ptr as *const std::cell::RefCell<crate::BoundInterfaceState>,
                            ))
                        };
                        Value::BoundInterface(GcRc {
                            inner: Rc::clone(&rc),
                        })
                    } else {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(
                                ptr as *const std::cell::RefCell<Interface>,
                            ))
                        };
                        Value::Interface(GcRc {
                            inner: Rc::clone(&rc),
                        })
                    }
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
                    let slot = (self.0 & PAYLOAD_MASK) as usize;
                    Value::Range(BOXED.with(|boxed| boxed.borrow().ranges.get(slot).clone()))
                }
                TAG_PARTIAL => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<PartialState>,
                        ))
                    };
                    Value::Partial(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_BOUND_METHOD => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<BoundMethodState>,
                        ))
                    };
                    Value::BoundMethod(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_STATIC_METHOD => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<StaticMethodState>,
                        ))
                    };
                    Value::StaticMethod(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_BOUND_NATIVE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<BoundNativeMethodState>,
                        ))
                    };
                    Value::BoundNativeMethod(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_BOUND_FUNC => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<BoundFunctionState>,
                        ))
                    };
                    Value::BoundFunction(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_CELL => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(ptr as *const std::cell::RefCell<CellState>))
                    };
                    Value::Cell(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_ITERATOR => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::IteratorState>,
                        ))
                    };
                    Value::Iterator(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_DATABASE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::DatabaseState>,
                        ))
                    };
                    Value::Database(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_RESOURCE => {
                    let ptr = self.as_ptr::<()>().unwrap();
                    let rc = unsafe {
                        ManuallyDrop::new(Rc::from_raw(
                            ptr as *const std::cell::RefCell<crate::NativeResource>,
                        ))
                    };
                    Value::Resource(GcRc {
                        inner: Rc::clone(&rc),
                    })
                }
                TAG_VARIANT_COMBINED => {
                    let is_constructor = (self.0 & 0x1) != 0;
                    let ptr = self.as_ptr::<()>().unwrap();
                    if is_constructor {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(
                                ptr as *const std::cell::RefCell<crate::VariantConstructorState>,
                            ))
                        };
                        Value::VariantConstructor(GcRc {
                            inner: Rc::clone(&rc),
                        })
                    } else {
                        let rc = unsafe {
                            ManuallyDrop::new(Rc::from_raw(
                                ptr as *const std::cell::RefCell<crate::VariantState>,
                            ))
                        };
                        Value::Variant(GcRc {
                            inner: Rc::clone(&rc),
                        })
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
            Value::Function(idx) => Self::encode(TAG_FUNC, (*idx as u64) << 1),
            Value::NativeFunction(f) => Self::encode(TAG_FUNC, native_payload(*f as usize)),
            Value::String(s) => Self::encode(TAG_STRING, box_string(s) as u64),
            Value::Array(a) => Self::encode(TAG_ARRAY, a.as_refcell_ptr() as usize as u64),
            Value::Map(m) => Self::encode(TAG_MAP, m.as_refcell_ptr() as usize as u64),
            Value::Set(s) => Self::encode(TAG_SET, s.as_refcell_ptr() as usize as u64),
            Value::Object(o) => Self::encode(TAG_OBJECT, o.as_refcell_ptr() as usize as u64),
            Value::Class(c) => Self::encode(TAG_CLASS, c.as_refcell_ptr() as usize as u64),
            Value::BoundClass(c) => {
                Self::encode(TAG_CLASS, (c.as_refcell_ptr() as usize as u64) | 0x1)
            }
            Value::Closure(c) => Self::encode(TAG_CLOSURE, c.as_refcell_ptr() as usize as u64),
            Value::Thunk(t) => Self::encode(TAG_THUNK, t.as_refcell_ptr() as usize as u64),
            Value::Tuple(t) => Self::encode(TAG_TUPLE, t.as_refcell_ptr() as usize as u64),
            Value::Enum(e) => Self::encode(TAG_ENUM, e.as_refcell_ptr() as usize as u64),
            Value::Interface(i) => Self::encode(TAG_INTERFACE, i.as_refcell_ptr() as usize as u64),
            Value::BoundInterface(i) => {
                Self::encode(TAG_INTERFACE, (i.as_refcell_ptr() as usize as u64) | 0x1)
            }
            Value::File(f) => Self::encode(TAG_FILE, f.as_refcell_ptr() as usize as u64),
            Value::Mutex(m) => Self::encode(TAG_MUTEX, m.as_refcell_ptr() as usize as u64),
            Value::Channel(c) => Self::encode(TAG_CHANNEL, c.as_refcell_ptr() as usize as u64),
            Value::Range(r) => Self::encode(TAG_RANGE, box_range(r) as u64),
            Value::Partial(p) => Self::encode(TAG_PARTIAL, p.as_refcell_ptr() as usize as u64),
            Value::BoundMethod(m) => {
                Self::encode(TAG_BOUND_METHOD, m.as_refcell_ptr() as usize as u64)
            }
            Value::StaticMethod(m) => {
                Self::encode(TAG_STATIC_METHOD, m.as_refcell_ptr() as usize as u64)
            }
            Value::BoundNativeMethod(m) => {
                Self::encode(TAG_BOUND_NATIVE, m.as_refcell_ptr() as usize as u64)
            }
            Value::BoundFunction(f) => {
                Self::encode(TAG_BOUND_FUNC, f.as_refcell_ptr() as usize as u64)
            }
            Value::Cell(c) => Self::encode(TAG_CELL, c.as_refcell_ptr() as usize as u64),
            Value::Iterator(i) => Self::encode(TAG_ITERATOR, i.as_refcell_ptr() as usize as u64),
            Value::Database(d) => Self::encode(TAG_DATABASE, d.as_refcell_ptr() as usize as u64),
            Value::Resource(r) => Self::encode(TAG_RESOURCE, r.as_refcell_ptr() as usize as u64),
            Value::Variant(v) => {
                Self::encode(TAG_VARIANT_COMBINED, v.as_refcell_ptr() as usize as u64)
            }
            Value::VariantConstructor(v) => Self::encode(
                TAG_VARIANT_COMBINED,
                (v.as_refcell_ptr() as usize as u64) | 0x1,
            ),
            v => {
                panic!("Unhandled value type in NanValue::from_value: {:?}", v);
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
                TAG_FUNC => {
                    if (self.0 & 0x1) != 0 {
                        "NativeFunction"
                    } else {
                        "Function"
                    }
                }
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
        if self.is_f64() {
            return;
        }
        let payload = self.0 & PAYLOAD_MASK;
        match self.tag() {
            TAG_INT if payload & 1 == 1 => {
                BOXED.with(|boxed| boxed.borrow_mut().ints.mark((payload >> 1) as usize));
                return;
            }
            TAG_STRING => {
                BOXED.with(|boxed| boxed.borrow_mut().strings.mark(payload as usize));
                return;
            }
            TAG_RANGE => {
                BOXED.with(|boxed| boxed.borrow_mut().ranges.mark(payload as usize));
                return;
            }
            _ => {}
        }
        // Mark through `Value` so the referenced object itself is marked, not only
        // its children; otherwise objects reachable only from registers are swept.
        if !self.is_f64() && self.tag() >= TAG_STRING {
            GcObject::mark(&self.to_value(), gc);
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
                t if t >= TAG_STRING => write!(f, "{}", self.to_value()),
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
    fn infinities_and_every_nan_stay_floats() {
        for f in [f64::INFINITY, f64::NEG_INFINITY, f64::MAX, f64::MIN, -0.0] {
            let v = NanValue::from_f64(f);
            assert!(v.is_f64(), "{f}");
            assert_eq!(v.as_f64().to_bits(), f.to_bits());
            assert_eq!(v.tag(), 0);
        }
        // Quiet, signalling, negative and payload-carrying NaNs.
        for bits in [
            0x7ff8_0000_0000_0000u64,
            0xfff8_0000_0000_0000,
            0x7ff0_0000_0000_0001,
            0x7ff8_0000_dead_beef,
            0xffff_ffff_ffff_ffff,
        ] {
            let v = NanValue::from_f64(f64::from_bits(bits));
            assert!(v.is_f64(), "{bits:#x}");
            assert!(v.as_f64().is_nan());
            assert_eq!(v.tag(), 0);
        }
        // Tagged values are never read as floats.
        assert!(!NanValue::from_int(0).is_f64());
        assert!(!NanValue::from_bool(false).is_f64());
        assert!(!NanValue::null().is_f64());
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

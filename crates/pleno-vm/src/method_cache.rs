//! A memo of the built-in methods that the VM's `builtin_resolver` finds, so
//! that reading `items.push` or `text.length` does not search the resolver's
//! table of every built-in method each time.

use pleno_value::NativeFunction;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

/// The resolver's type: a type name and a method name to the native method.
pub type BuiltinResolver = fn(&str, &str) -> Option<NativeFunction>;

/// Hashes the few machine words of a key with a multiply.
#[derive(Default)]
struct WordHasher(u64);

impl Hasher for WordHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

/// The key of a lookup: the type name (address and length of the `'static`
/// string that `Value::type_name` returns) and the address of the method
/// name's constant.
type Key = (usize, usize, usize);

#[derive(Default)]
pub struct BuiltinMethodCache {
    /// The resolver the entries came from; another one empties the cache.
    resolver: Option<usize>,
    /// Each entry keeps its name alive, so that no other string can take the
    /// address in its key while it is cached.
    entries: HashMap<Key, (Rc<String>, Option<NativeFunction>), BuildHasherDefault<WordHasher>>,
}

impl BuiltinMethodCache {
    /// `resolver(ty, name)`, remembered for this type and name constant.
    #[inline]
    pub fn get(
        &mut self,
        resolver: Option<BuiltinResolver>,
        ty: &'static str,
        name: &Rc<String>,
    ) -> Option<NativeFunction> {
        let resolver = resolver?;
        if self.resolver != Some(resolver as usize) {
            self.entries.clear();
            self.resolver = Some(resolver as usize);
        }
        let key = (ty.as_ptr() as usize, ty.len(), Rc::as_ptr(name) as usize);
        if let Some((_, function)) = self.entries.get(&key) {
            return *function;
        }
        let function = resolver(ty, name);
        self.entries.insert(key, (Rc::clone(name), function));
        function
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pleno_value::{RuntimeError, VMProvider, Value};

    fn first(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::Int(1))
    }

    fn second(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
        Ok(Value::Int(2))
    }

    fn resolve_first(ty: &str, name: &str) -> Option<NativeFunction> {
        (ty == "Array" && name == "push").then_some(first as NativeFunction)
    }

    fn resolve_second(ty: &str, name: &str) -> Option<NativeFunction> {
        (ty == "Array" && name == "push").then_some(second as NativeFunction)
    }

    #[test]
    fn remembers_hits_and_misses_per_resolver() {
        let mut cache = BuiltinMethodCache::default();
        let push = Rc::new("push".to_string());
        let pop = Rc::new("pop".to_string());
        let found = cache.get(Some(resolve_first), "Array", &push);
        assert_eq!(
            found.map(|f| f as usize),
            Some(first as NativeFunction as usize)
        );
        // A second lookup is answered from the cache.
        let again = cache.get(Some(resolve_first), "Array", &push);
        assert_eq!(
            again.map(|f| f as usize),
            Some(first as NativeFunction as usize)
        );
        assert!(cache.get(Some(resolve_first), "Array", &pop).is_none());
        assert!(cache.get(Some(resolve_first), "String", &push).is_none());
        assert!(cache.get(None, "Array", &push).is_none());
        // Another resolver does not see the first one's entries.
        let other = cache.get(Some(resolve_second), "Array", &push);
        assert_eq!(
            other.map(|f| f as usize),
            Some(second as NativeFunction as usize)
        );
    }
}

//! GC (Garbage Collection) module for manual memory management hints

use pleno_value::{RuntimeError, VMProvider, Value};

/// Trigger a garbage collection cycle
pub fn gc_collect(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    vm.collect();
    Ok(Value::Null)
}

/// Get the current heap size (number of managed objects)
pub fn gc_heap_size(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    Ok(Value::Int(gc.current_allocation as i64))
}

/// Check if GC is enabled (placeholder - always returns true for now)
pub fn gc_is_enabled(_vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    // GC cannot currently be disabled, so always return true
    Ok(Value::Bool(true))
}

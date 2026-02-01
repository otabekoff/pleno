# Pleno Technical Debt & V1 Audit Results

This document consolidates identified technical debt and implementation gaps found during the V1 audit.

## Type System

- [x] **Type Annotations**: Enforced at runtime for function parameters, local variable declarations (`let x: Ty`), assignments, and class properties (instance and static).
- [x] **Interface Properties**: Interface property definitions are now parsed, compiled, and enforced by the VM.
- [x] **Character Type**: `Char` type is fully integrated into the VM, with support for character literals and built-in methods (`isDigit`, `isAlpha`, `toUpperCase`, `toLowerCase`, `toInt`, `toString`).

## VM & Runtime

- [x] **Set Iteration**: `Set` iteration is performed by converting the entire set to an array. (Fixed: Now uses `IndexSet` with O(1) iterator startup).
- [x] **Static Methods/Fields**: Static methods and fields are fully implemented and tested (uncommented tests in `vm_exec.rs`).
- [x] **Privacy**: Static field privacy is fully implemented and enforced by the VM.

## Syntax & Parser

- [x] **Named Arguments**: Parser supports named arguments (`f(x: 1)`), and they are correctly handled by the compiler and VM.
- [x] **Keyword-only Arguments**: Parser, compiler, and VM support for keyword-only arguments (`fn f(a, *, b)`) is fully implemented and tested.
- [ ] **Generic Constraints**: `T: Comparable` is parsed but not enforced.

## Stdlib

- [ ] **Rich API**: Collections (Map, Set, Array) need more comprehensive built-in methods (currently relying heavily on VM-level opcodes).

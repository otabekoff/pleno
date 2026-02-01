# Pleno Technical Debt & V1 Audit Results

This document consolidates identified technical debt and implementation gaps found during the V1 audit.

## Type System

- [ ] **Type Annotations**: Currently, `let x: Ty` and `fn f(x: Ty)` are parsed but ignored by the compiler. No static or runtime type checking is enforced by default.
- [ ] **Interface Properties**: Interface property definitions are not compiled/enforced (`TODO` in `pleno-compiler/src/lib.rs:752`).
- [ ] **Character Type**: `Char` type exists in AST/VM but needs full integration (literals, stdlib methods).

## VM & Runtime

- [x] **Set Iteration**: `Set` iteration is performed by converting the entire set to an array. (Fixed: Now uses `IndexSet` with O(1) iterator startup).
- [ ] **Static Methods/Fields**: Several cases of static method/field implementation are incomplete or untested (`TODO`s in `vm_exec.rs`).
- [ ] **Privacy**: Static field privacy is not fully implemented.

## Syntax & Parser

- [x] **Named Arguments**: Parser supports named arguments (`f(x: 1)`), and they are correctly handled by the compiler and VM.
- [ ] **Keyword-only Arguments**: Parser support is missing.
- [ ] **Generic Constraints**: `T: Comparable` is parsed but not enforced.

## Stdlib

- [ ] **Rich API**: Collections (Map, Set, Array) need more comprehensive built-in methods (currently relying heavily on VM-level opcodes).

# Technical Debt - Pleno V1

## Architectural Redundancy

- [ ] **Dual Native Registration**: Some modules (Math, OS, etc.) are registered both as `__native_` globals and as `std:` native modules. This creates confusion and maintenance overhead.
  - _Decision Required_: Standardize on one approach. Preferred: `std:` modules for everything, with `.pln` wrappers only if they add value (like class-based APIs).
- [ ] **Naming Inconsistency**: Mix of `__native_` prefixed and unprefixed names in the global scope.

## Missing Documentat- [ ] **Module Specs**: `specs.md` is missing details for:
  - `std:reflect`
  - `std:security` (Sandbox)
  - `std:gc`
  - `std:net`
  - `std:test` (the unit testing framework)
  - `std:concurrency` (Mutex, Channel) - Note: `Channel` is mentioned but `Mutex` is not.
- [ ] **Architecture Doc**: No `ARCHITECTURE.md` exists to explain the crate structure and VM execution model.

## Implementation Gaps

- [ ] **Upvalue Checks**: `roadmap.md` mentions that variable reassignment is verified, but upvalue captures currently bypass checks.
- [ ] **Error Handling**: Some VM errors are difficult to trigger and are effectively "untested" in `vm_errors.rs`.
- [ ] **LSP Performance**: `pleno-lsp` has some "hacks" (identified via grep) that need review.
- [ ] **Project Housekeeping**: Root directory has some temporary folders/files that should be cleaned up or gitignored.

## Code Quality

- [ ] **Children-like Coding**: Review `value_ops.rs` and `lib.rs` in VM for non-optimal or redundant code.
- [ ] **Compiler Warnings**: Run `cargo clippy` across the workspace and fix all warnings.

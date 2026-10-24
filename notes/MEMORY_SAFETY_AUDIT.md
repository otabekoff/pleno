# Memory safety audit

Review of every `unsafe` block and of the garbage collector, completed 2026-10-24.

## Scope

| Location | `unsafe` blocks | Purpose |
| --- | --- | --- |
| `crates/pleno-value/src/nan_value.rs` | 27 | Decoding NaN-boxed pointers back into `GcRc` handles and native function pointers |
| `crates/pleno-stdlib/src/dotenv.rs` | 1 | `std::env::set_var` (unsafe since Rust 2024) |
| Tests (`local_registry.rs`, `stdlib_process_tests.rs`) | 2 | `std::env::set_var` in single-test binaries |

All other crates are safe Rust.

## How NaN boxing stays sound

A `NanValue` is a `Copy` 64-bit word. For heap objects its payload is a raw pointer, and it holds **no reference count**. Decoding (`to_value`) rebuilds an `Rc` with `ManuallyDrop::new(Rc::from_raw(..))` and clones it, which is sound only if:

1. **The tag matches the pointee type.** Each tag maps to one type; tags shared by two types use the pointer's low bit (`Class`/`BoundClass`, `Interface`/`BoundInterface`, `Variant`/`VariantConstructor`, function index/native function). `RefCell<T>` allocations are at least 8-byte aligned, so the bit is free.
2. **The object is still alive.** Every heap object is allocated through the GC, whose heap holds a strong reference. NaN-boxed values are stored only in `VM.registers` (verified: no other field holds a `NanValue`), and every collection marks all registers.
3. **Collections happen between instructions.** Automatic collection runs at the start of `run_step`, and `std:gc` collection runs from a native call. Instructions that hold a `NanValue` in a local across a native call copy it from a register, which keeps it marked.

## Findings and fixes

| # | Finding | Severity | Status |
| --- | --- | --- | --- |
| 1 | `NanValue::mark` marked an object's children but not the object, so the sweep freed objects held only in registers (use-after-free) | Critical | Fixed: marking goes through `Value::mark` |
| 2 | `BoundClass` and `BoundInterface` had no encoding; the VM panicked, and the GC read them as the wrong type | High | Fixed: low-bit tags with matching decode and mark |
| 3 | Encoding a string incremented its reference count and never decremented it: every string passing through a register leaked | High | Fixed: strings live in a slot table freed by the GC |
| 4 | A range was encoded as a pointer to the `Range` inside a temporary `Value`, which dangled immediately (undefined behaviour) | Critical | Fixed: ranges live in the slot table |
| 5 | Integers were 47-bit: values beyond ±2^46 wrapped silently, contradicting the 64-bit `Int` in the spec (nanosecond timestamps were wrong) | High | Fixed: large integers live in the slot table |
| 6 | Native function pointers are tagged with their low bit, which assumes 2-byte alignment | Low | Holds on x86-64 and AArch64; now checked with a `debug_assert!` |
| 7 | Sandboxed VMs share the thread's slot tables with the parent VM | Medium | Handled: sandbox VMs do not sweep the tables (`GCManager::sweeps_boxed_values = false`) |
| 8 | `std::env::set_var` in `dotenv` is unsafe when other threads read the environment | Low | Acceptable: the VM is single-threaded; documented at the call site |

Regression tests: `crates/pleno-vm/tests/boxed_values.rs` (slot tables are freed), `opcode_proptests.rs` (full-range integers), and the GC stress probes in the test suite.

## Remaining design risk

The soundness of `to_value` rests on invariant 2. A future change that stores a `NanValue` anywhere other than `VM.registers` (for example in a native resource or a cache) without marking it would reintroduce use-after-free. Code reviews of `pleno-vm` should check for new `NanValue` fields.

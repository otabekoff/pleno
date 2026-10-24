# Technical Debt - Pleno V1

Status checked against the code on 2026-10-24. Resolved items are removed and logged in `tasks/done.md`.

## Code Quality

- [ ] **Split the VM's `lib.rs`**. Status: `crates/pleno-vm/src/lib.rs` is about 6,100 lines: dispatch, calls, classes, exceptions, generics checks and iteration in one file. `value_ops.rs` has been reviewed and cleaned up, and duplicated interface-bound checks were merged, but the split into modules is not done.

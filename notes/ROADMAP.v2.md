# Pleno V2 Roadmap

Open items toward Pleno V2. Status checked against the code on 2026-10-24; finished items are removed and logged in `tasks/done.md`.

---

## Stability & Hardening

- **[ ] Arena allocator for the AST**. Status: not started. AST nodes are `Box`ed.

## Standard Library & Ecosystem

- **[ ] std:db PostgreSQL driver**. Status: `std:db` supports SQLite (`sqlite:path`) only.
- **[ ] Central package registry**. Status: partly built. `crates/pleno-registry` is a registry server (Axum + SQLite) and `plm` can publish to and install from a registry URL, but no public registry is deployed and the protocol is undocumented.

## Performance & Runtime

- **[ ] JIT compiler** (Cranelift, x86-64/ARM64). Status: not started.
- **[ ] Generational GC**. Status: not started. The collector is single-generation mark-and-sweep.

## Developer Experience

- **[ ] LSP refactorings**. Status: definition, references, rename, hover, completion, signature help, inlay hints, semantic tokens, folding, formatting, code lens, call hierarchy and code actions are implemented. Missing: refactorings other than rename, workspace symbol search, and integration tests.
- **[ ] WASM distribution**. Status: `crates/pleno-wasm` builds for `wasm32-unknown-unknown` and exports `compile`, `run` and `run_source` through wasm-bindgen. There is no npm package or browser playground yet.

---

_Features and timelines are living targets and will evolve based on community feedback._

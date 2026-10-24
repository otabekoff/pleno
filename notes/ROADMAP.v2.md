# Pleno V2 Roadmap

Open items toward Pleno V2. Status checked against the code on 2026-10-24; finished items are removed and logged in `tasks/done.md`.

---

## Stability & Hardening

- **[ ] Fuzzing in CI**. Status: three cargo-fuzz targets (`fuzz_lexer`, `fuzz_parser`, `fuzz_compile`) and `.github/workflows/fuzz.yml` (weekly, on demand, and on pull requests that touch the lexer, parser or compiler). Remove this item once the workflow has run green on GitHub.
- **[ ] Benchmark gating in CI**. Status: the `benchmark` job in `ci.yml` compares each pull request against its base branch on the same runner and fails on a slowdown over 15%. Remove this item once the job has run green on GitHub.
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

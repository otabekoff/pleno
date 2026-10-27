# Pleno source tests

Pleno programs that test the language and the standard library from the outside, as a user runs
them. The Rust tests of each crate are in `crates/*/tests` and in the crates' sources.

`scripts/run_pln_tests.sh` runs every `.pln` file here and in `examples/`, each from its own
folder, and checks it against comments at the top of the file:

```text
// expect-error: <text>   must exit non-zero with <text> in its output
// args: <flags>          extra flags for `pleno run`
// stdin: <text>          fed to the script; `\n` separates lines
// skip: <reason>         not run (for example, a module other tests import)
// requires: network      run only when PLENO_NETWORK_TESTS=1
```

A file without them must run and exit with status 0.

```bash
cargo build --bin pleno
bash scripts/run_pln_tests.sh                       # uses target/debug/pleno
bash scripts/run_pln_tests.sh target/release/pleno  # or another build
```

Folders: `sources_tests/` (language features), `class_module_rules/` (classes, modules and
imports), `performance/` (longer programs) and `fixtures/` (files other tests read).

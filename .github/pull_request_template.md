## Summary

<!-- What changes and why. Link the issue: Closes #123 -->

## Checks

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `bash scripts/run_pln_tests.sh` and, for language or docs changes, `python3 scripts/check-doc-examples.py target/debug/pleno docs`
- [ ] A line in `CHANGELOG.md` under "Unreleased", for a change users notice
- [ ] The docs (`docs/`) say what the change does

## Notes

Tests/build/release/debug

Test: cargo test --workspace
Build: cargo build
Release: cargo build --release
Debug: RUST_BACKTRACE=1 cargo test -p pleno-vm --test vm_exec -- --nocapture

cargo run -p pleno-repl is to run the CLI tool.
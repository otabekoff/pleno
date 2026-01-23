Tests/build/release/debug

Test: cargo test --workspace
Build: cargo build
Release: cargo build --release
Debug: RUST_BACKTRACE=1 cargo test -p pleno-vm --test vm_exec -- --nocapture

cargo run -p pleno-repl is to run the CLI tool.


cd /home/otabek/Projects/plang/pleno && timeout 10s cargo run simple_test.pln

cargo test -p pleno-gc

cargo run -p pleno-cli -- run sources/test_array_methods.pln

cargo build -p pleno-repl

cargo run -p pleno-cli -- sources/test_stdlib.pln
#!/usr/bin/env bash
# Builds the WebAssembly version of Pleno that the docs playground and
# tutorial run, into docs/.vitepress/theme/wasm/ (not committed).
#
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

cargo build --profile wasm -p pleno-wasm --target wasm32-unknown-unknown
wasm-bindgen --target web \
    --out-dir docs/.vitepress/theme/wasm \
    --out-name pleno \
    target/wasm32-unknown-unknown/wasm/pleno_wasm.wasm

echo "Built docs/.vitepress/theme/wasm/pleno_bg.wasm ($(du -h docs/.vitepress/theme/wasm/pleno_bg.wasm | cut -f1))"

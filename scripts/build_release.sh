#!/bin/bash
set -e

# Pleno Release Build Script
# Builds the release binaries for this machine and packs them as release.yml
# does (Linux and macOS; on Windows the binaries end in .exe).

VERSION=$(grep '^version' Cargo.toml | head -n 1 | cut -d '"' -f 2)
TARGET_DIR="target/release"
DIST_DIR="dist"

echo "Building Pleno v$VERSION..."

# Build workspace in release mode
cargo build --release --locked --workspace

# Create distribution directory
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Copy every tool a release archive has
for tool in pleno plm pleno-lsp pleno-dap pleno-repl; do
    cp "$TARGET_DIR/$tool" "$DIST_DIR/"
done

# Name it as release.yml does: pleno-vX.Y.Z-<Rust target>.tar.gz
HOST=$(rustc -vV | sed -n 's/^host: //p')
TARBALL="pleno-v$VERSION-$HOST.tar.gz"

echo "Packaging into $TARBALL..."
tar -czf "$TARBALL" -C "$DIST_DIR" .

echo "Build complete: $TARBALL"

#!/bin/bash
set -e

# Pleno Release Build Script
# This script builds optimized binaries and packages them for distribution.

VERSION=$(grep '^version' Cargo.toml | head -n 1 | cut -d '"' -f 2)
TARGET_DIR="target/release"
DIST_DIR="dist"

echo "Building Pleno v$VERSION..."

# Build workspace in release mode
cargo build --release --workspace

# Create distribution directory
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Copy binaries
cp "$TARGET_DIR/pleno" "$DIST_DIR/"
cp "$TARGET_DIR/plm" "$DIST_DIR/"

# Create tarball
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
TARBALL="pleno-v$VERSION-$ARCH-$PLATFORM.tar.gz"

echo "Packaging into $TARBALL..."
tar -czf "$TARBALL" -C "$DIST_DIR" .

echo "Build complete: $TARBALL"

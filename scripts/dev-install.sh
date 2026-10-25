#!/bin/sh
# Installs Pleno from this checkout: builds the release binaries (pleno, plm,
# pleno-lsp, pleno-dap, pleno-repl) into ~/.pleno/bin, the same folder the
# release installer uses, and installs the VS Code extension from ide/vscode.
# Run it again after pulling to update.
#
# A toolchain that the extension's "Install Toolchain" command downloaded is
# removed, because the extension would use it instead of this build.
#
# Usage: scripts/dev-install.sh [--skip-tools] [--skip-extension] [--keep-extension-toolchain]
# Needs: Rust (cargo); for the extension also Node.js (npm) and the `code` command.
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INSTALL_DIR="$HOME/.pleno/bin"
TOOLS="pleno plm pleno-lsp pleno-dap pleno-repl"
SKIP_TOOLS=0
SKIP_EXTENSION=0
KEEP_EXTENSION_TOOLCHAIN=0

for arg in "$@"; do
    case "$arg" in
        --skip-tools) SKIP_TOOLS=1 ;;
        --skip-extension) SKIP_EXTENSION=1 ;;
        --keep-extension-toolchain) KEEP_EXTENSION_TOOLCHAIN=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

step() { printf '\033[0;36m==> %s\033[0m\n' "$1"; }
warn() { printf '\033[0;33mwarning: %s\033[0m\n' "$1"; }
require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "'$1' was not found. $2" >&2
        exit 1
    fi
}

# Two runs at once would build into the same folders and break each other.
mkdir -p "$ROOT/target"
LOCK="$ROOT/target/.dev-install.lock"
if ! mkdir "$LOCK" 2>/dev/null; then
    echo "another dev-install is running from $ROOT; wait for it to finish (or remove $LOCK)" >&2
    exit 1
fi
trap 'rmdir "$LOCK"' EXIT

step "Installing Pleno from $ROOT ($(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null), $(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null))"

if [ "$SKIP_TOOLS" = 0 ]; then
    require cargo "Install Rust from https://rustup.rs"
    step "Building release binaries"
    cargo build --release --locked --manifest-path "$ROOT/Cargo.toml" \
        -p pleno-cli -p pleno-lsp -p pleno-dap -p pleno-repl

    step "Copying binaries to $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
    for tool in $TOOLS; do
        # Replace instead of overwriting, so a running language server keeps
        # its old file.
        cp "$ROOT/target/release/$tool" "$INSTALL_DIR/$tool.new"
        chmod +x "$INSTALL_DIR/$tool.new"
        mv -f "$INSTALL_DIR/$tool.new" "$INSTALL_DIR/$tool"
    done

    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *) warn "$INSTALL_DIR is not on PATH; add: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
    esac

    # Another copy found first on PATH would hide this one.
    for tool in $TOOLS; do
        found="$(command -v "$tool" 2>/dev/null || true)"
        if [ -n "$found" ] && [ "$found" != "$INSTALL_DIR/$tool" ]; then
            warn "$found comes before $INSTALL_DIR/$tool on PATH; remove it (for cargo installs: cargo uninstall pleno-cli pleno-lsp pleno-dap pleno-repl)"
        fi
    done

    # The extension prefers a toolchain it downloaded itself over PATH.
    for own in "$HOME/.config/Code/User/globalStorage/pleno.pleno-language/toolchain" \
        "$HOME/Library/Application Support/Code/User/globalStorage/pleno.pleno-language/toolchain"; do
        if [ -d "$own" ]; then
            if [ "$KEEP_EXTENSION_TOOLCHAIN" = 1 ]; then
                warn "the extension uses its own toolchain in $own instead of $INSTALL_DIR"
            else
                step "Removing the toolchain the extension downloaded, so it uses this build"
                rm -rf "$own"
            fi
        fi
    done
fi

if [ "$SKIP_EXTENSION" = 0 ]; then
    require npm "Install Node.js from https://nodejs.org, or pass --skip-extension"
    require code "Install VS Code and its 'code' command, or pass --skip-extension"
    step "Building the VS Code extension"
    VSIX="$(mktemp -d)/pleno-language-dev.vsix"
    (
        cd "$ROOT/ide/vscode"
        npm ci --no-audit --no-fund
        # vsce builds the extension itself (the `vscode:prepublish` script). Node's
        # deprecation notices come from vsce and VS Code's CLI, not from this script.
        NODE_OPTIONS=--no-deprecation npx --yes @vscode/vsce@3.2.1 package --no-dependencies --out "$VSIX"
    )
    step "Installing the extension"
    NODE_OPTIONS=--no-deprecation code --install-extension "$VSIX" --force
    rm -f "$VSIX"
fi

step "Installed"
if [ "$SKIP_TOOLS" = 0 ]; then
    printf '  %-10s %s\n' pleno "$("$INSTALL_DIR/pleno" --version)"
    printf '  %-10s %s\n' pleno-lsp "$("$INSTALL_DIR/pleno-lsp" --version)"
fi
if [ "$SKIP_EXTENSION" = 0 ]; then
    echo "  Reload VS Code (Developer: Reload Window) to use the new extension and language server."
fi

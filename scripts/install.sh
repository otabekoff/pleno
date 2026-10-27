#!/bin/sh
set -e

# Pleno Installer
# Detects your OS and architecture, downloads a release from GitHub, checks
# its SHA-256 checksum and installs pleno, plm, pleno-lsp, pleno-dap and
# pleno-repl into $PLENO_HOME/bin (~/.pleno/bin by default).
#
#   curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
#
# Environment variables:
#   PLENO_HOME            where to install (default: ~/.pleno); plm keeps its
#                         package cache there too
#   PLENO_VERSION         a release tag such as v0.3.0 (default: the latest release)
#   PLENO_NO_MODIFY_PATH  set to 1 to leave your shell profile alone

GITHUB_REPO="otabekoff/pleno"
PLENO_DIR="${PLENO_HOME:-$HOME/.pleno}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { printf "${BLUE}%s${NC}\n" "$1"; }
fail() { printf "${RED}Error: %s${NC}\n" "$1" >&2; exit 1; }

info "Installing Pleno..."

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux) OS_NAME="unknown-linux-gnu" ;;
    Darwin) OS_NAME="apple-darwin" ;;
    *) fail "Unsupported OS: $OS (on Windows, use scripts/install.ps1)" ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64) ARCH_NAME="x86_64" ;;
    arm64|aarch64) ARCH_NAME="aarch64" ;;
    i386|i686)
        [ "$OS_NAME" = "unknown-linux-gnu" ] || fail "Unsupported architecture: $ARCH"
        ARCH_NAME="i686" ;;
    *) fail "Unsupported architecture: $ARCH" ;;
esac

TARGET="${ARCH_NAME}-${OS_NAME}"

VERSION="${PLENO_VERSION:-}"
if [ -z "$VERSION" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    [ -n "$VERSION" ] || fail "Could not determine the latest version."
fi
case "$VERSION" in
    v*) ;;
    *) VERSION="v$VERSION" ;;
esac

info "Version: $VERSION"
info "Target: $TARGET"

DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/pleno-$VERSION-$TARGET.tar.gz"

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

info "Downloading binaries..."
curl -fL -o "$TMP_DIR/pleno.tar.gz" "$DOWNLOAD_URL" || fail "Could not download $DOWNLOAD_URL"

# Verify checksum against the published .sha256 file
info "Verifying checksum..."
EXPECTED_HASH=$(curl -fsL "$DOWNLOAD_URL.sha256" | cut -d ' ' -f 1)
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH=$(sha256sum "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
else
    ACTUAL_HASH=$(shasum -a 256 "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
fi
if [ -z "$EXPECTED_HASH" ] || [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
    fail "Checksum verification failed."
fi

info "Extracting..."
tar -xzf "$TMP_DIR/pleno.tar.gz" -C "$TMP_DIR"

# Install binaries
mkdir -p "$PLENO_DIR/bin"
# The language server, debugger and REPL are needed by editors; older
# releases may not include all of them.
for tool in pleno plm pleno-lsp pleno-dap pleno-repl; do
    if [ -f "$TMP_DIR/$tool" ]; then
        mv -f "$TMP_DIR/$tool" "$PLENO_DIR/bin/"
        chmod +x "$PLENO_DIR/bin/$tool"
    fi
done
printf "${GREEN}Installed Pleno %s in %s${NC}\n" "$VERSION" "$PLENO_DIR/bin"

case ":$PATH:" in
    *":$PLENO_DIR/bin:"*)
        info "Try running: pleno --version"
        exit 0
        ;;
esac

if [ "${PLENO_NO_MODIFY_PATH:-0}" = 1 ]; then
    info "Add $PLENO_DIR/bin to your PATH to use it."
    exit 0
fi

case "$SHELL" in
    */bash) SHELL_PROFILE="$HOME/.bashrc" ;;
    */zsh) SHELL_PROFILE="$HOME/.zshrc" ;;
    *) SHELL_PROFILE="$HOME/.profile" ;;
esac
PATH_LINE="export PATH=\"\$PATH:$PLENO_DIR/bin\""
if [ -f "$SHELL_PROFILE" ] && grep -qxF "$PATH_LINE" "$SHELL_PROFILE"; then
    :
else
    echo "$PATH_LINE" >> "$SHELL_PROFILE"
    printf "${GREEN}Added %s to PATH in %s${NC}\n" "$PLENO_DIR/bin" "$SHELL_PROFILE"
fi
info "Restart your terminal or run: . $SHELL_PROFILE"
info "Then try: pleno --version"

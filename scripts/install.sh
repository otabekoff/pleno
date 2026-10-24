#!/bin/sh
set -e

# Pleno Installer
# This script detects your OS and architecture, downloads the latest release from GitHub,
# and installs pleno, plm, pleno-lsp, pleno-dap and pleno-repl.

GITHUB_REPO="otabekoff/pleno"
INSTALL_DIR="/usr/local/bin"
PLENO_DIR="$HOME/.pleno"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

printf "${BLUE}Installing Pleno...${NC}\n"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux) OS_NAME="unknown-linux-gnu" ;;
    Darwin) OS_NAME="apple-darwin" ;;
    *) printf "${RED}Error: Unsupported OS: $OS${NC}\n"; exit 1 ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64) ARCH_NAME="x86_64" ;;
    arm64|aarch64) ARCH_NAME="aarch64" ;;
    *) printf "${RED}Error: Unsupported architecture: $ARCH${NC}\n"; exit 1 ;;
esac

TARGET="${ARCH_NAME}-${OS_NAME}"

# Fetch latest version from GitHub API
VERSION=$(curl -s https://api.github.com/repos/$GITHUB_REPO/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    printf "${RED}Error: Could not determine latest version.${NC}\n"
    exit 1
fi

printf "${BLUE}Version: $VERSION${NC}\n"
printf "${BLUE}Target: $TARGET${NC}\n"

DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/pleno-$VERSION-$TARGET.tar.gz"

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

printf "${BLUE}Downloading binaries...${NC}\n"
curl -fL -o "$TMP_DIR/pleno.tar.gz" "$DOWNLOAD_URL"

# Verify checksum against the published .sha256 file
printf "${BLUE}Verifying checksum...${NC}\n"
EXPECTED_HASH=$(curl -fsL "$DOWNLOAD_URL.sha256" | cut -d ' ' -f 1)
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_HASH=$(sha256sum "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
else
    ACTUAL_HASH=$(shasum -a 256 "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
fi
if [ -z "$EXPECTED_HASH" ] || [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
    printf "${RED}Error: Checksum verification failed.${NC}\n"
    exit 1
fi

printf "${BLUE}Extracting...${NC}\n"
tar -xzf "$TMP_DIR/pleno.tar.gz" -C "$TMP_DIR"

# Install binaries
mkdir -p "$PLENO_DIR/bin"
# The language server, debugger and REPL are needed by editors; older
# releases may not include all of them.
for tool in pleno plm pleno-lsp pleno-dap pleno-repl; do
    if [ -f "$TMP_DIR/$tool" ]; then
        mv "$TMP_DIR/$tool" "$PLENO_DIR/bin/"
        chmod +x "$PLENO_DIR/bin/$tool"
    fi
done

# Suggest PATH update
if ! echo "$PATH" | grep -q "$PLENO_DIR/bin"; then
    printf "${BLUE}Updating PATH...${NC}\n"
    SHELL_PROFILE=""
    case "$SHELL" in
        */bash) SHELL_PROFILE="$HOME/.bashrc" ;;
        */zsh) SHELL_PROFILE="$HOME/.zshrc" ;;
        *) SHELL_PROFILE="$HOME/.profile" ;;
    esac

    if [ -f "$SHELL_PROFILE" ]; then
        echo "export PATH=\"\$PATH:$PLENO_DIR/bin\"" >> "$SHELL_PROFILE"
        printf "${GREEN}Added $PLENO_DIR/bin to $SHELL_PROFILE${NC}\n"
    fi
fi

printf "${GREEN}Pleno $VERSION installed successfully!${NC}\n"
printf "${BLUE}Restart your terminal or run: source $SHELL_PROFILE${NC}\n"
printf "${BLUE}Try running: pleno --version${NC}\n"

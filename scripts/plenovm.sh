#!/bin/bash
# Pleno Version Manager (plenovm)
# Manages multiple Pleno installations

set -e

PLENO_HOME="${PLENO_HOME:-$HOME/.pleno}"
PLENO_VERSIONS="$PLENO_HOME/versions"
PLENO_BIN="$PLENO_HOME/bin"
GITHUB_REPO="otabekoff/pleno"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

mkdir -p "$PLENO_VERSIONS"
mkdir -p "$PLENO_BIN"

usage() {
    echo "Pleno Version Manager (plenovm)"
    echo
    echo "Usage: plenovm <command> [args]"
    echo
    echo "Commands:"
    echo "  install <version>    Install a specific version"
    echo "  use <version>        Switch to a specific version"
    echo "  list                 List installed versions"
    echo "  list-remote          List available versions from GitHub"
    echo "  uninstall <version>  Remove a specific version"
    echo "  version              Show plenovm version"
    echo
}

detect_target() {
    OS="$(uname -s)"
    case "$OS" in
        Linux) OS_NAME="unknown-linux-gnu" ;;
        Darwin) OS_NAME="apple-darwin" ;;
        *) echo "Error: Unsupported OS: $OS"; exit 1 ;;
    esac

    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64|amd64) ARCH_NAME="x86_64" ;;
        arm64|aarch64) ARCH_NAME="aarch64" ;;
        i386|i686) ARCH_NAME="i686" ;;
        *) echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    echo "${ARCH_NAME}-${OS_NAME}"
}

command_install() {
    VERSION="$1"
    if [ -z "$VERSION" ]; then
        echo "Error: version is required"
        exit 1
    fi

    if [ "$VERSION" == "latest" ]; then
        VERSION=$(curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    fi

    TARGET=$(detect_target)
    VERSION_DIR="$PLENO_VERSIONS/$VERSION"

    if [ -d "$VERSION_DIR" ]; then
        echo "Version $VERSION is already installed"
        exit 0
    fi

    echo -e "${BLUE}Installing Pleno $VERSION for $TARGET...${NC}"

    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/pleno-$VERSION-$TARGET.tar.gz"
    
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    if ! curl -L -f -o "$TMP_DIR/pleno.tar.gz" "$DOWNLOAD_URL"; then
        echo -e "${RED}Error: Failed to download version $VERSION${NC}"
        exit 1
    fi

    EXPECTED_HASH=$(curl -fsL "$DOWNLOAD_URL.sha256" | cut -d ' ' -f 1)
    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL_HASH=$(sha256sum "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
    else
        ACTUAL_HASH=$(shasum -a 256 "$TMP_DIR/pleno.tar.gz" | cut -d ' ' -f 1)
    fi
    if [ -z "$EXPECTED_HASH" ] || [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
        echo -e "${RED}Error: Checksum verification failed for $VERSION${NC}"
        exit 1
    fi

    mkdir -p "$VERSION_DIR"
    tar -xzf "$TMP_DIR/pleno.tar.gz" -C "$VERSION_DIR"

    echo -e "${GREEN}Pleno $VERSION installed to $VERSION_DIR${NC}"
}

command_use() {
    VERSION="$1"
    if [ -z "$VERSION" ]; then
        echo "Error: version is required"
        exit 1
    fi

    VERSION_DIR="$PLENO_VERSIONS/$VERSION"
    if [ ! -d "$VERSION_DIR" ]; then
        echo "Error: Version $VERSION is not installed. Run 'plenovm install $VERSION' first."
        exit 1
    fi

    echo -e "${BLUE}Switching to Pleno $VERSION...${NC}"

    # Point every tool the version has at it (older releases lack some).
    for tool in pleno plm pleno-lsp pleno-dap pleno-repl; do
        if [ -f "$VERSION_DIR/$tool" ]; then
            rm -f "$PLENO_BIN/$tool"
            ln -s "$VERSION_DIR/$tool" "$PLENO_BIN/$tool"
        fi
    done

    echo "$VERSION" > "$PLENO_HOME/current_version"
    echo -e "${GREEN}Now using Pleno $VERSION${NC}"
}

command_list() {
    CURRENT=$(cat "$PLENO_HOME/current_version" 2>/dev/null || echo "")
    echo "Installed versions:"
    for v in "$PLENO_VERSIONS"/*; do
        if [ -d "$v" ]; then
            V_NAME=$(basename "$v")
            if [ "$V_NAME" == "$CURRENT" ]; then
                echo -e "  * ${GREEN}$V_NAME${NC} (current)"
            else
                echo "    $V_NAME"
            fi
        fi
    done
}

command_list_remote() {
    echo -e "${BLUE}Available versions from GitHub:${NC}"
    curl -s "https://api.github.com/repos/$GITHUB_REPO/releases" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

case "$1" in
    install) command_install "$2" ;;
    use) command_use "$2" ;;
    list) command_list ;;
    list-remote) command_list_remote ;;
    uninstall)
        VERSION="$2"
        # Without a version this would remove $PLENO_VERSIONS itself.
        if [ -z "$VERSION" ] || [ ! -d "$PLENO_VERSIONS/$VERSION" ]; then
            echo "Error: give an installed version (see 'plenovm list')"
            exit 1
        fi
        rm -rf "${PLENO_VERSIONS:?}/$VERSION"
        echo "Uninstalled $VERSION"
        ;;
    version) echo "plenovm 0.1.0" ;;
    *) usage ;;
esac

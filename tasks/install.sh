#!/bin/sh
set -e

# Detect OS and Arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        PLATFORM="linux"
        EXT="tar.gz"
        ;;
    Darwin)
        PLATFORM="macos"
        EXT="tar.gz"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64)
        ARCH="x86_64"
        ;;
    arm64|aarch64)
        if [ "$OS" = "Darwin" ]; then
            ARCH="aarch64"
        else
            echo "Unsupported Architecture: $ARCH"
            exit 1
        fi
        ;;
    *)
        echo "Unsupported Architecture: $ARCH"
        exit 1
        ;;
esac

# Define installation path
INSTALL_DIR="$HOME/.pleno/bin"
mkdir -p "$INSTALL_DIR"

# Download URL (Replace with your actual repo)
REPO="otabekoff/pleno"
# Get latest tag
TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$TAG" ]; then
    echo "Failed to find latest release."
    exit 1
fi

ASSET_NAME="pleno-${TAG}-${PLATFORM}-${ARCH}.${EXT}"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"

echo "Downloading Pleno $TAG for $OS-$ARCH..."
curl -L -o "/tmp/$ASSET_NAME" "$DOWNLOAD_URL"

# Extract
echo "Installing to $INSTALL_DIR..."
if [ "$EXT" = "zip" ]; then
    unzip -o "/tmp/$ASSET_NAME" -d "$INSTALL_DIR"
else
    tar -xzf "/tmp/$ASSET_NAME" -C "$INSTALL_DIR"
fi

# Cleanup
rm "/tmp/$ASSET_NAME"

# Path setup suggestion
echo "Pleno installed successfully!"
echo "Please add $INSTALL_DIR to your PATH:"
echo "  export PATH=\"\$PATH:$INSTALL_DIR\""

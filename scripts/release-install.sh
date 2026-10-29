#!/bin/sh
# Installs or updates Pleno from a GitHub release: the tools (pleno, plm, pleno-lsp, pleno-dap,
# pleno-repl) into $PLENO_HOME/bin (~/.pleno/bin by default) and the VS Code extension from the
# same release. It is the release counterpart of dev-install.sh, which builds this checkout; both
# install into the same folder, so each replaces what the other installed.
# Run it again to update: it does nothing when the release is already installed.
#
# The tools come from install.sh (checksums checked), the extension's .vsix is checked against
# the release's SHA256SUMS.txt. A toolchain that the extension's "Install Toolchain" command
# downloaded is removed, because the extension would use it instead of these tools.
#
# Usage: scripts/release-install.sh [--version v0.4.0] [--force] [--skip-tools] [--skip-extension]
#                                   [--keep-extension-toolchain]
#   --version  a release tag (default: $PLENO_VERSION, else the latest release); an older tag
#              installs that version
#   --force    install even if that version is already installed
# Needs: curl, tar; for the extension, VS Code's `code` command (without it the extension is
# skipped). PLENO_HOME and PLENO_NO_MODIFY_PATH work as in install.sh.
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="otabekoff/pleno"
INSTALL_DIR="${PLENO_HOME:-$HOME/.pleno}/bin"
TOOLS="pleno plm pleno-lsp pleno-dap pleno-repl"
EXTENSION_ID="pleno.pleno-language"
TAG="${PLENO_VERSION:-}"
FORCE=0
SKIP_TOOLS=0
SKIP_EXTENSION=0
KEEP_EXTENSION_TOOLCHAIN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || { echo "--version needs a tag, such as v0.4.0" >&2; exit 2; }
            TAG="$2"; shift ;;
        --version=*) TAG="${1#--version=}" ;;
        --force) FORCE=1 ;;
        --skip-tools) SKIP_TOOLS=1 ;;
        --skip-extension) SKIP_EXTENSION=1 ;;
        --keep-extension-toolchain) KEEP_EXTENSION_TOOLCHAIN=1 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

step() { printf '\033[0;36m==> %s\033[0m\n' "$1"; }
warn() { printf '\033[0;33mwarning: %s\033[0m\n' "$1"; }

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# The version the installed pleno reports ("pleno 0.4.0" gives "0.4.0"), or nothing.
installed_version() {
    [ -x "$INSTALL_DIR/pleno" ] || return 0
    "$INSTALL_DIR/pleno" --version 2>/dev/null | head -n 1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+[^ ]*' | head -n 1 || true
}

if [ -z "$TAG" ]; then
    # github.com/<repo>/releases/latest redirects to .../releases/tag/<tag>. Unlike the GitHub
    # API, it has no hourly limit for unauthenticated requests.
    latest="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" ||
        { echo "Could not reach https://github.com/$REPO/releases/latest" >&2; exit 1; }
    case "$latest" in
        */releases/tag/*) TAG="${latest##*/}" ;;
        *) echo "Could not find the latest release (got $latest)" >&2; exit 1 ;;
    esac
fi
case "$TAG" in
    v*) ;;
    *) TAG="v$TAG" ;;
esac
WANTED="${TAG#v}"
BASE="https://github.com/$REPO/releases/download/$TAG"

INSTALLED="$(installed_version)"
step "Pleno $WANTED from the release $TAG (installed in $INSTALL_DIR: ${INSTALLED:-not installed})"

if [ "$SKIP_TOOLS" = 0 ]; then
    if [ "$INSTALLED" = "$WANTED" ] && [ "$FORCE" = 0 ]; then
        step "The tools are already $WANTED (pass --force to install them again)"
    else
        if [ -n "$INSTALLED" ]; then
            step "Replacing the tools $INSTALLED with $WANTED"
        else
            step "Installing the tools"
        fi
        PLENO_VERSION="$TAG" sh "$ROOT/scripts/install.sh"
        now="$(installed_version)"
        if [ "$now" != "$WANTED" ]; then
            echo "$INSTALL_DIR/pleno reports '${now:-nothing}', not $WANTED" >&2
            exit 1
        fi
    fi

    # Another copy found first on PATH would hide this one: one warning per folder, with the
    # command that removes it when the folder belongs to a package manager.
    shadowing=""
    for tool in $TOOLS; do
        found="$(command -v "$tool" 2>/dev/null || true)"
        if [ -n "$found" ] && [ "$found" != "$INSTALL_DIR/$tool" ]; then
            dir="$(dirname "$found")"
            case " $shadowing " in
                *" $dir "*) continue ;;
            esac
            shadowing="$shadowing $dir"
            case "$dir" in
                */.cargo/*) how="cargo uninstall pleno-cli pleno-lsp pleno-dap pleno-repl" ;;
                /opt/homebrew/*|/usr/local/bin|/home/linuxbrew/*) how="brew uninstall pleno (if Homebrew installed it)" ;;
                /usr/bin) how="sudo apt remove pleno (if the .deb installed it)" ;;
                *) how="remove them from that folder, or put $INSTALL_DIR first on PATH" ;;
            esac
            warn "Pleno tools in $dir come before $INSTALL_DIR on PATH, so that copy runs instead; to use this one: $how"
        fi
    done

    # The extension prefers a toolchain it downloaded itself over PATH.
    for own in "$HOME/.config/Code/User/globalStorage/$EXTENSION_ID/toolchain" \
        "$HOME/Library/Application Support/Code/User/globalStorage/$EXTENSION_ID/toolchain"; do
        if [ -d "$own" ]; then
            if [ "$KEEP_EXTENSION_TOOLCHAIN" = 1 ]; then
                warn "the extension uses its own toolchain in $own instead of $INSTALL_DIR"
            else
                step "Removing the toolchain the extension downloaded, so it uses these tools"
                rm -rf "$own"
            fi
        fi
    done
fi

EXTENSION_DONE=0
if [ "$SKIP_EXTENSION" = 0 ]; then
    if ! command -v code >/dev/null 2>&1; then
        warn "VS Code's 'code' command was not found, so the extension was not installed (pass --skip-extension to leave it out)"
    else
        current="$(NODE_OPTIONS=--no-deprecation code --list-extensions --show-versions 2>/dev/null |
            grep "^$EXTENSION_ID@" | head -n 1 | cut -d @ -f 2 || true)"
        if [ "$current" = "$WANTED" ] && [ "$FORCE" = 0 ]; then
            step "The VS Code extension is already $WANTED"
        else
            vsix="pleno-language-$TAG.vsix"
            work="$(mktemp -d)"
            trap 'rm -rf "$work"' EXIT
            step "Downloading the VS Code extension $vsix"
            curl -fsSL -o "$work/$vsix" "$BASE/$vsix" || { echo "Could not download $BASE/$vsix" >&2; exit 1; }
            curl -fsSL -o "$work/SHA256SUMS.txt" "$BASE/SHA256SUMS.txt" || { echo "Could not download $BASE/SHA256SUMS.txt" >&2; exit 1; }
            # Lines are "<hash>  ./<file>" (or "*<file>" from sha256sum -b).
            expected="$(awk -v f="$vsix" '{ name = $2; sub(/^(\.\/|\*)/, "", name); if (name == f) print $1 }' "$work/SHA256SUMS.txt")"
            [ -n "$expected" ] || { echo "$vsix is not in the release's SHA256SUMS.txt" >&2; exit 1; }
            actual="$(sha256 "$work/$vsix")"
            if [ "$actual" != "$expected" ]; then
                echo "Checksum mismatch for $vsix (expected $expected, got $actual)" >&2
                exit 1
            fi
            if [ -n "$current" ]; then
                step "Replacing the extension $current with $WANTED"
            else
                step "Installing the extension"
            fi
            NODE_OPTIONS=--no-deprecation code --install-extension "$work/$vsix" --force
            EXTENSION_DONE=1
        fi
    fi
fi

step "Installed"
if [ "$SKIP_TOOLS" = 0 ]; then
    printf '  %-10s %s\n' pleno "$("$INSTALL_DIR/pleno" --version)"
    printf '  %-10s %s\n' pleno-lsp "$("$INSTALL_DIR/pleno-lsp" --version)"
fi
if [ "$EXTENSION_DONE" = 1 ]; then
    echo "  Reload VS Code (Developer: Reload Window) to use the new extension and language server."
fi

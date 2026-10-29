#!/usr/bin/env bash
# Tests scripts/release-install.sh against the real GitHub releases, in a temporary HOME and
# PLENO_HOME, with a stand-in for VS Code's `code` command that records what it installs:
#   1. a fresh install gets the latest release's tools and extension, and removes a toolchain
#      the extension downloaded;
#   2. running it again changes nothing;
#   3. --version v0.3.0 installs that older release, tools and extension;
#   4. running it again updates both to the latest, while the old language server is running.
#
# Usage: scripts/test_release_install.sh   (needs network access to github.com)
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
lsp_pid=""
cleanup() {
    [ -n "$lsp_pid" ] && kill "$lsp_pid" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

export HOME="$work/home" PLENO_HOME="$work/pleno" PLENO_NO_MODIFY_PATH=1
unset PLENO_VERSION
mkdir -p "$HOME" "$work/shim"
toolchain="$HOME/.config/Code/User/globalStorage/pleno.pleno-language/toolchain"
[ "$(uname -s)" = Darwin ] && toolchain="$HOME/Library/Application Support/Code/User/globalStorage/pleno.pleno-language/toolchain"
mkdir -p "$toolchain"

# The stand-in `code`: --list-extensions prints what it installed, --install-extension checks
# that the .vsix is a real extension (a zip with extension/package.json) and records its version.
cat > "$work/shim/code" <<'EOF'
#!/bin/sh
state="$(dirname "$0")/installed.txt"
case "$1" in
    --list-extensions) cat "$state" 2>/dev/null; echo "other.extension@1.0.0" ;;
    --install-extension)
        unzip -p "$2" extension/package.json > /dev/null || { echo "not a .vsix: $2" >&2; exit 1; }
        version=$(unzip -p "$2" extension/package.json | sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' | head -n 1)
        echo "pleno.pleno-language@$version" > "$state"
        echo "install $version" >> "$(dirname "$0")/log.txt" ;;
    *) exit 2 ;;
esac
EOF
chmod +x "$work/shim/code"
export PATH="$work/shim:$PATH"

latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' https://github.com/otabekoff/pleno/releases/latest)"
latest="${latest_url##*/v}"
older=0.3.0
[ "$latest" != "$older" ] || { echo "the latest release must be newer than $older for this test" >&2; exit 1; }

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
tools_are() {
    for tool in pleno plm pleno-lsp pleno-dap pleno-repl; do
        got="$("$PLENO_HOME/bin/$tool" --version 2>/dev/null | head -n 1 || true)"
        [ "$got" = "$tool $1" ] || fail "$2: $tool --version is '$got', expected '$tool $1'"
    done
}
extension_is() {
    got="$(cat "$work/shim/installed.txt" 2>/dev/null || true)"
    [ "$got" = "pleno.pleno-language@$1" ] || fail "$2: the extension is '$got', expected $1"
}
installs() { wc -l < "$work/shim/log.txt" 2>/dev/null | tr -d ' ' || echo 0; }
run() { bash "$repo/scripts/release-install.sh" "$@" > "$work/out.txt" 2>&1 || { cat "$work/out.txt" >&2; fail "release-install.sh $* failed"; }; }

echo "latest release: $latest"

run
tools_are "$latest" "fresh install"
extension_is "$latest" "fresh install"
[ ! -d "$toolchain" ] || fail "fresh install: the extension's own toolchain was not removed"
[ "$(installs)" = 1 ] || fail "fresh install: the extension was installed $(installs) times"

run
grep -q "already $latest" "$work/out.txt" || fail "second run: no 'already $latest' message"
[ "$(installs)" = 1 ] || fail "second run: the extension was installed again"
tools_are "$latest" "second run"

run --version "v$older"
tools_are "$older" "--version v$older"
extension_is "$older" "--version v$older"

# The language server keeps running while it is replaced, as when VS Code has it open.
sleep 600 | "$PLENO_HOME/bin/pleno-lsp" > /dev/null 2>&1 &
lsp_pid=$!
sleep 1
run
tools_are "$latest" "update with the language server running"
extension_is "$latest" "update with the language server running"
kill -0 "$lsp_pid" 2>/dev/null || fail "the running language server stopped during the update"

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "release-install.sh: all checks passed"

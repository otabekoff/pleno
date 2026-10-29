#!/usr/bin/env bash
# Tests scripts/update-packaging.sh on a copy of packaging/:
#   1. it moves every manifest to a new version, with each checksum where it belongs;
#   2. moving back to the current version with the current checksums gives the committed files
#      byte for byte, so the script writes exactly the format of the hand-made manifests;
#   3. a missing checksum or a bad version fails and changes nothing.
#
# Usage: scripts/test_update_packaging.sh
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/scripts"
cp "$repo/scripts/update-packaging.sh" "$repo/scripts/homebrew-formula.sh" \
    "$repo/scripts/scoop-manifest.sh" "$work/scripts/"
cp -r "$repo/packaging" "$work/packaging"
pkg="$work/packaging"

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
contains() { grep -qF -- "$2" "$pkg/$1" || fail "$1 does not contain: $2"; }
lacks() { if grep -qF -- "$2" "$pkg/$1"; then fail "$1 still contains: $2"; fi; }

files=(homebrew/pleno.rb scoop/pleno.json winget/Pleno.Pleno.yaml winget/Pleno.Pleno.installer.yaml
    winget/Pleno.Pleno.locale.en-US.yaml chocolatey/pleno.nuspec chocolatey/tools/chocolateyinstall.ps1
    aur/pleno-bin/PKGBUILD aur/pleno-bin/.SRCINFO)

# What the committed manifests point at, read back from them.
old=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$pkg/scoop/pleno.json")
old_date=$(sed -n 's/^ReleaseDate: \([0-9-]*\).*/\1/p' "$pkg/winget/Pleno.Pleno.installer.yaml")
old_license=$(sed -n "s/^sha256sums=('\([0-9a-f]*\)').*/\1/p" "$pkg/aur/pleno-bin/PKGBUILD")
{
    # The formula lists each archive's URL followed by its checksum.
    awk '/url "/ { split($0, u, "\""); n = split(u[2], p, "/"); name = p[n] }
         /sha256 "/ { split($0, s, "\""); print s[2] "  ./" name }' "$pkg/homebrew/pleno.rb"
    python3 -c 'import json,sys; a=json.load(open(sys.argv[1]))["architecture"]["64bit"]; print(a["hash"] + "  ./" + a["url"].rsplit("/", 1)[1])' \
        "$pkg/scoop/pleno.json"
} > "$work/old-sums.txt"
[ "$(wc -l < "$work/old-sums.txt")" -eq 5 ] || fail "could not read 5 checksums from the manifests"
[ -n "$old_date" ] && [ -n "$old_license" ] || fail "could not read the release date and LICENSE checksum"

# 1. Forward to a new version with made-up checksums, one per archive.
new=9.8.7
fake() { printf '%s' "$1" | sha256sum | cut -d' ' -f1; }
for target in x86_64-pc-windows-msvc.zip x86_64-unknown-linux-gnu.tar.gz aarch64-unknown-linux-gnu.tar.gz \
    x86_64-apple-darwin.tar.gz aarch64-apple-darwin.tar.gz i686-unknown-linux-gnu.tar.gz; do
    echo "$(fake "$target")  ./pleno-v$new-$target"
done > "$work/new-sums.txt"
echo "$(fake deb)  ./pleno_v${new}_amd64.deb" >> "$work/new-sums.txt"
license=$(fake license)

PLENO_LICENSE_SHA256="$license" bash "$work/scripts/update-packaging.sh" "$new" "$work/new-sums.txt" 2030-01-02 > /dev/null

for f in "${files[@]}"; do
    lacks "$f" "$old"
    contains "$f" "$new"
done
win=$(fake x86_64-pc-windows-msvc.zip)
x64=$(fake x86_64-unknown-linux-gnu.tar.gz)
arm=$(fake aarch64-unknown-linux-gnu.tar.gz)
contains homebrew/pleno.rb "sha256 \"$(fake aarch64-apple-darwin.tar.gz)\""
contains homebrew/pleno.rb "sha256 \"$(fake x86_64-apple-darwin.tar.gz)\""
contains homebrew/pleno.rb "sha256 \"$x64\""
contains homebrew/pleno.rb "sha256 \"$arm\""
contains scoop/pleno.json "\"hash\": \"$win\""
contains winget/Pleno.Pleno.installer.yaml "InstallerSha256: ${win^^}"
contains winget/Pleno.Pleno.installer.yaml "ReleaseDate: 2030-01-02"
contains winget/Pleno.Pleno.installer.yaml "InstallerUrl: https://github.com/otabekoff/pleno/releases/download/v$new/pleno-v$new-x86_64-pc-windows-msvc.zip"
contains winget/Pleno.Pleno.locale.en-US.yaml "ReleaseNotesUrl: https://github.com/otabekoff/pleno/releases/tag/v$new"
contains chocolatey/pleno.nuspec "<version>$new</version>"
contains chocolatey/tools/chocolateyinstall.ps1 "checksum64     = '$win'"
contains aur/pleno-bin/PKGBUILD "pkgver=$new"
contains aur/pleno-bin/PKGBUILD "sha256sums=('$license')"
contains aur/pleno-bin/PKGBUILD "sha256sums_x86_64=('$x64')"
contains aur/pleno-bin/PKGBUILD "sha256sums_aarch64=('$arm')"
contains aur/pleno-bin/.SRCINFO "	sha256sums = $license"
contains aur/pleno-bin/.SRCINFO "	sha256sums_x86_64 = $x64"
contains aur/pleno-bin/.SRCINFO "	sha256sums_aarch64 = $arm"
contains aur/pleno-bin/.SRCINFO "	source_aarch64 = pleno-v$new-aarch64.tar.gz::https://github.com/otabekoff/pleno/releases/download/v$new/pleno-v$new-aarch64-unknown-linux-gnu.tar.gz"
if grep -q $'\r' "$pkg/winget/Pleno.Pleno.installer.yaml" && grep -q $'[^\r]$' "$pkg/winget/Pleno.Pleno.installer.yaml"; then
    fail "winget/Pleno.Pleno.installer.yaml mixes line endings"
fi

# 2. Back to the committed version: the files must come out as they are in the repository.
PLENO_LICENSE_SHA256="$old_license" bash "$work/scripts/update-packaging.sh" "$old" "$work/old-sums.txt" "$old_date" > /dev/null
diff -r --strip-trailing-cr "$repo/packaging" "$pkg" || fail "moving back to $old did not give the committed files"

# 3. Errors change nothing.
cp -r "$pkg" "$work/before"
grep -v windows "$work/new-sums.txt" > "$work/no-windows.txt"
if PLENO_LICENSE_SHA256="$license" bash "$work/scripts/update-packaging.sh" "$new" "$work/no-windows.txt" 2> "$work/err.txt"; then
    fail "a missing Windows checksum was accepted"
fi
grep -q "pleno-v$new-x86_64-pc-windows-msvc.zip" "$work/err.txt" || fail "the error does not name the missing archive: $(cat "$work/err.txt")"
diff -r "$work/before" "$pkg" > /dev/null || fail "a failed run changed packaging/"
if bash "$work/scripts/update-packaging.sh" "v$new" "$work/new-sums.txt" 2> /dev/null; then
    fail "a version with a leading v was accepted"
fi
diff -r "$work/before" "$pkg" > /dev/null || fail "a rejected version changed packaging/"

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "update-packaging.sh: all checks passed"

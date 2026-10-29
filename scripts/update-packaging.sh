#!/usr/bin/env bash
# Points every manifest in packaging/ at a release: Homebrew, Scoop, WinGet, Chocolatey and the
# AUR. Versions, URLs and checksums come from the release's SHA256SUMS.txt; the AUR's LICENSE
# checksum comes from LICENSE at the release's tag. The files are rewritten in place, so the
# result is one commit (or pull request) to review.
#
# Usage: scripts/update-packaging.sh <version, e.g. 1.2.3> <path to SHA256SUMS.txt> [release date]
#
#   gh release download v1.2.3 --pattern SHA256SUMS.txt --dir release
#   scripts/update-packaging.sh 1.2.3 release/SHA256SUMS.txt
#
# The release date (YYYY-MM-DD, for WinGet) defaults to today in UTC. Set PLENO_LICENSE_SHA256 to
# give the LICENSE checksum instead of reading LICENSE at tag v<version>.
set -euo pipefail

usage="usage: update-packaging.sh <version> <SHA256SUMS.txt> [YYYY-MM-DD]"
version="${1:?$usage}"
sums="${2:?$usage}"
date="${3:-$(date -u +%Y-%m-%d)}"

if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "update-packaging.sh: '$version' is not a version such as 1.2.3 (no leading v; package managers take no prereleases)" >&2
    exit 1
fi
if ! [[ "$date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
    echo "update-packaging.sh: '$date' is not a date such as 2026-10-29" >&2
    exit 1
fi
if [ ! -f "$sums" ]; then
    echo "update-packaging.sh: $sums does not exist" >&2
    exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
packaging="$root/packaging"

if [ -n "${PLENO_LICENSE_SHA256:-}" ]; then
    license_sha="$PLENO_LICENSE_SHA256"
elif license=$(git -C "$root" show "v$version:LICENSE" 2>/dev/null); then
    license_sha=$(printf '%s\n' "$license" | sha256sum | cut -d' ' -f1)
else
    echo "update-packaging.sh: there is no tag v$version with a LICENSE (fetch the tags, or set PLENO_LICENSE_SHA256)" >&2
    exit 1
fi

# Every file is worked out before any is written, so a run that fails changes nothing. Homebrew
# and Scoop use the two generators the release workflow uses for the tap and the bucket.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
bash "$root/scripts/homebrew-formula.sh" "$version" "$sums" > "$tmp/pleno.rb"
bash "$root/scripts/scoop-manifest.sh" "$version" "$sums" > "$tmp/pleno.json"

python3 - "$packaging" "$version" "$sums" "$date" "$license_sha" "$tmp" <<'PY'
import pathlib, re, sys

packaging, version, sums_path, date, license_sha, generated = sys.argv[1:]
packaging = pathlib.Path(packaging)
generated = pathlib.Path(generated)
writes = {}
base = f"https://github.com/otabekoff/pleno/releases/download/v{version}"
notes = f"https://github.com/otabekoff/pleno/releases/tag/v{version}"

sums = {}
for line in pathlib.Path(sums_path).read_text(encoding="utf-8").splitlines():
    parts = line.split()
    if len(parts) == 2:
        sums[parts[1].removeprefix("./").removeprefix("*")] = parts[0].lower()

def sha(target, ext):
    name = f"pleno-v{version}-{target}.{ext}"
    if name not in sums:
        sys.exit(f"update-packaging.sh: {name} is not in {sums_path}")
    return sums[name]

windows = sha("x86_64-pc-windows-msvc", "zip")
linux_x64 = sha("x86_64-unknown-linux-gnu", "tar.gz")
linux_arm = sha("aarch64-unknown-linux-gnu", "tar.gz")
windows_zip = f"{base}/pleno-v{version}-x86_64-pc-windows-msvc.zip"

def rewrite(relative, replacements):
    """Applies (pattern, replacement) pairs to a file; each pattern must match at least once."""
    path = packaging / relative
    if path in writes:
        text = writes[path]
    else:
        # newline="" keeps the file's line endings as they are.
        with open(path, encoding="utf-8", newline="") as f:
            text = f.read()
    for pattern, replacement in replacements:
        # A checkout with CRLF line endings: the value stops before the CR, which is kept.
        pattern = pattern.replace(".*$", r"[^\r\n]*(?=\r?$)")
        text, count = re.subn(pattern, lambda _: replacement, text, flags=re.MULTILINE)
        if count == 0:
            sys.exit(f"update-packaging.sh: {relative} has no match for {pattern!r}; update the script")
    writes[path] = text

for name in ["Pleno.Pleno.yaml", "Pleno.Pleno.installer.yaml", "Pleno.Pleno.locale.en-US.yaml"]:
    rewrite(f"winget/{name}", [(r"^PackageVersion: .*$", f"PackageVersion: {version}")])
rewrite("winget/Pleno.Pleno.installer.yaml", [
    (r"^ReleaseDate: .*$", f"ReleaseDate: {date}"),
    (r"(?<=InstallerUrl: ).*$", windows_zip),
    (r"(?<=InstallerSha256: ).*$", windows.upper()),
])
rewrite("winget/Pleno.Pleno.locale.en-US.yaml", [(r"(?<=^ReleaseNotesUrl: ).*$", notes)])

rewrite("chocolatey/pleno.nuspec", [
    (r"(?<=<version>)[^<]*(?=</version>)", version),
    (r"(?<=<releaseNotes>)[^<]*(?=</releaseNotes>)", notes),
])
rewrite("chocolatey/tools/chocolateyinstall.ps1", [
    (r"(?<=url64bit       = ')[^']*", windows_zip),
    (r"(?<=checksum64     = ')[^']*", windows),
])

# PKGBUILD builds its URLs from $pkgver; .SRCINFO spells them out, as makepkg --printsrcinfo does.
rewrite("aur/pleno-bin/PKGBUILD", [
    (r"^pkgver=.*$", f"pkgver={version}"),
    (r"^pkgrel=.*$", "pkgrel=1"),
    (r"^sha256sums=\('[^']*'\)", f"sha256sums=('{license_sha}')"),
    (r"^sha256sums_x86_64=\('[^']*'\)", f"sha256sums_x86_64=('{linux_x64}')"),
    (r"^sha256sums_aarch64=\('[^']*'\)", f"sha256sums_aarch64=('{linux_arm}')"),
])
license_url = f"https://raw.githubusercontent.com/otabekoff/pleno/v{version}/LICENSE"
rewrite("aur/pleno-bin/.SRCINFO", [
    (r"(?<=^\tpkgver = ).*$", version),
    (r"(?<=^\tpkgrel = ).*$", "1"),
    (r"(?<=^\tsource = ).*$", f"LICENSE-{version}::{license_url}"),
    (r"(?<=^\tsha256sums = ).*$", license_sha),
    (r"(?<=^\tsource_x86_64 = ).*$",
     f"pleno-v{version}-x86_64.tar.gz::{base}/pleno-v{version}-x86_64-unknown-linux-gnu.tar.gz"),
    (r"(?<=^\tsha256sums_x86_64 = ).*$", linux_x64),
    (r"(?<=^\tsource_aarch64 = ).*$",
     f"pleno-v{version}-aarch64.tar.gz::{base}/pleno-v{version}-aarch64-unknown-linux-gnu.tar.gz"),
    (r"(?<=^\tsha256sums_aarch64 = ).*$", linux_arm),
])
for name, relative in [("pleno.rb", "homebrew/pleno.rb"), ("pleno.json", "scoop/pleno.json")]:
    writes[packaging / relative] = (generated / name).read_text(encoding="utf-8")
for path, text in writes.items():
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)
PY

echo "packaging/ now points at v$version. Review with: git diff -- packaging"

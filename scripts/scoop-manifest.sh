#!/usr/bin/env bash
# Writes the Scoop manifest for a release to standard output: packaging/scoop/pleno.json with
# the version, the Windows archive's URL and its checksum from the release's SHA256SUMS.txt.
#
# Usage: scripts/scoop-manifest.sh <version, e.g. 1.2.3> <path to SHA256SUMS.txt>
set -euo pipefail

version="${1:?usage: scoop-manifest.sh <version> <SHA256SUMS.txt>}"
sums="${2:?usage: scoop-manifest.sh <version> <SHA256SUMS.txt>}"
archive="pleno-v${version}-x86_64-pc-windows-msvc.zip"

hash=$(awk -v name="$archive" '{ file = $2; sub(/^\.\//, "", file) } file == name { print $1 }' "$sums")
if [ -z "$hash" ]; then
    echo "scoop-manifest.sh: $archive is not in $sums" >&2
    exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$root/packaging/scoop/pleno.json" "$version" "$archive" "$hash" <<'PY'
import json, sys

path, version, archive, sha256 = sys.argv[1:]
with open(path, encoding="utf-8") as f:
    manifest = json.load(f)
manifest["version"] = version
manifest["architecture"]["64bit"]["url"] = (
    f"https://github.com/otabekoff/pleno/releases/download/v{version}/{archive}"
)
manifest["architecture"]["64bit"]["hash"] = sha256
print(json.dumps(manifest, indent=4, ensure_ascii=False))
PY

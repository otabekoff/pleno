#!/usr/bin/env bash
# Prints the Homebrew formula for a release, using the prebuilt archives and
# the checksums in the release's SHA256SUMS.txt.
#
# Usage: scripts/homebrew-formula.sh <version, e.g. 0.2.0> <path to SHA256SUMS.txt>
set -euo pipefail

version="$1"
sums="$2"
base="https://github.com/otabekoff/pleno/releases/download/v${version}"

sha() {
    local file="pleno-v${version}-$1.tar.gz"
    local hash
    hash="$(awk -v f="$file" '{ name = $2; sub(/^\.\//, "", name); if (name == f) print $1 }' "$sums")"
    if [ -z "$hash" ]; then
        echo "no checksum for $file in $sums" >&2
        exit 1
    fi
    echo "$hash"
}

cat <<EOF
class Pleno < Formula
  desc "Readable language with optional checked types and async built in"
  homepage "https://pleno.mohirlab.uz"
  version "${version}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "${base}/pleno-v${version}-aarch64-apple-darwin.tar.gz"
      sha256 "$(sha aarch64-apple-darwin)"
    else
      url "${base}/pleno-v${version}-x86_64-apple-darwin.tar.gz"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "${base}/pleno-v${version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    else
      url "${base}/pleno-v${version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pleno --version")
  end
end
EOF

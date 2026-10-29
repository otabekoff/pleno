class Pleno < Formula
  desc "Readable language with optional checked types and async built in"
  homepage "https://pleno.mohirlab.uz"
  version "0.4.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.4.0/pleno-v0.4.0-aarch64-apple-darwin.tar.gz"
      sha256 "210e15b18cb6119e877c957a0c0486c428f3a359bfd178d99218de14fb5da1d8"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.4.0/pleno-v0.4.0-x86_64-apple-darwin.tar.gz"
      sha256 "e805116c900cba8ee884e2889a2c3b830c80857617b2d4dc1df67d2161e05a6f"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.4.0/pleno-v0.4.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "018f93f3127becc48785f4e90e56d73d5de93a6f670efe8cb45c0c74489f0d4f"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.4.0/pleno-v0.4.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "8d769b1f55b99adbb4c8234c25779a110a588c2d59ab94142936fda88a436b32"
    end
  end

  def install
    bin.install "pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pleno --version")
  end
end

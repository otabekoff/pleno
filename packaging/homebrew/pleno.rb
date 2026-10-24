class Pleno < Formula
  desc "Readable language with optional checked types and async built in"
  homepage "https://pleno.mohirlab.uz"
  version "0.2.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.2.0/pleno-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "ef419e40eca1552359e39543bab0441850ce85d678c956380fc2b49ba29cb12b"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.2.0/pleno-v0.2.0-x86_64-apple-darwin.tar.gz"
      sha256 "7dd483a7a50f13a01e2ab7bb98ecb5cc68623f8e2b9bc1ac159382b5b523630c"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.2.0/pleno-v0.2.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "1534f90b9cc844caa7bbdd2c7c2c6f05169e4b813fba5c0dfd53952283edd29a"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.2.0/pleno-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "a22dec76f1758063a6d7816926322d5fccb639c0a9ad840a244f9ac010678b21"
    end
  end

  def install
    bin.install "pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pleno --version")
  end
end

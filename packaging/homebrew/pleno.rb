class Pleno < Formula
  desc "Readable language with optional checked types and async built in"
  homepage "https://pleno.mohirlab.uz"
  version "0.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.3.0/pleno-v0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "70ea122891110b9c26a973349b3e9f7a478a49dc37bef5f88e57991059c4d5af"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.3.0/pleno-v0.3.0-x86_64-apple-darwin.tar.gz"
      sha256 "69498be59014b5178c5a5dca9a90f1c4d70d4c0478809dd6ffd010d33d3c73b0"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/otabekoff/pleno/releases/download/v0.3.0/pleno-v0.3.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "7711e241920d1b5c49c1f5717562881e9d0eb9d62de5173ca5011d7d101321cb"
    else
      url "https://github.com/otabekoff/pleno/releases/download/v0.3.0/pleno-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "12fee4bbf53905e5916b245ecf3905c5afdea9ff7c376a3424f6de4db5e0b28f"
    end
  end

  def install
    bin.install "pleno", "plm", "pleno-lsp", "pleno-dap", "pleno-repl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pleno --version")
  end
end

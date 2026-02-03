# Homebrew Formula for Pleno
# To install: brew install pleno (after tap is set up)
# Or: brew install --build-from-source ./packaging/homebrew/pleno.rb

class Pleno < Formula
  desc "Modern programming language with clarity, safety, and expressiveness"
  homepage "https://otabekoff.github.io/pleno/"
  license "MIT"
  head "https://github.com/otabekoff/pleno.git", branch: "main"

  # For stable releases, uncomment and update:
  # url "https://github.com/otabekoff/pleno/archive/refs/tags/v1.0.0.tar.gz"
  # sha256 "YOUR_SHA256_HERE"
  # version "1.0.0"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--workspace"
    bin.install "target/release/pleno"
    bin.install "target/release/plm"
    
    # Install shell completions (if available)
    # generate_completions_from_executable(bin/"pleno", "completions")
  end

  test do
    # Test that pleno runs
    assert_match "Pleno", shell_output("#{bin}/pleno --version")
    
    # Test a simple program
    (testpath/"hello.pln").write <<~EOS
      print("Hello from Pleno!");
    EOS
    assert_match "Hello from Pleno!", shell_output("#{bin}/pleno run #{testpath}/hello.pln")
  end
end

class RustSwift < Formula
  desc "Deterministic, qualified Rust-to-Swift compiler and SwiftUI generator"
  homepage "https://github.com/zachshallbetter/rust-swift"
  url "https://github.com/zachshallbetter/rust-swift/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "MIT"
  head "https://github.com/zachshallbetter/rust-swift.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/rust-swift-cli")
  end

  test do
    assert_match "rust-swift", shell_output("#{bin}/rust-swift --help")
    assert_match "diff-test", shell_output("#{bin}/rust-swift --help")
  end
end

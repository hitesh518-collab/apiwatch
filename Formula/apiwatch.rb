class Apiwatch < Formula
  desc "Lock, diff, and verify external API contracts"
  homepage "https://github.com/hitesh518-collab/apiwatch"
  url "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz"
  sha256 "243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "apiwatch", shell_output("#{bin}/apiwatch --help")
  end
end

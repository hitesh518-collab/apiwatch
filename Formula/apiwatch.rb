class Apiwatch < Formula
  desc "Lock, diff, and verify external API contracts"
  homepage "https://github.com/hitesh518-collab/apiwatch"
  url "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.7.0.tar.gz"
  sha256 "a42b7bf0e5f4559add1d856da18e6dda60613549f9203f4c8f3d0eee7d1d1ebe"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "apiwatch", shell_output("#{bin}/apiwatch --help")
  end
end

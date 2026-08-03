class Apiwatch < Formula
  desc "Lock, diff, and verify external API contracts"
  homepage "https://github.com/hitesh518-collab/apiwatch"
  url "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v1.0.2.tar.gz"
  sha256 "fca9bc66eba610854a451dd1c10ecbd4c7603c11c6bc3042ea85efa91534486c"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "apiwatch", shell_output("#{bin}/apiwatch --help")
  end
end

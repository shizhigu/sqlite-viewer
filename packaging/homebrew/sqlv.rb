##
# Homebrew formula for the `sqlv` CLI.
#
# Install via a personal tap until we have a homebrew-core entry:
#
#   brew tap shizhigu/sqlv https://github.com/shizhigu/homebrew-sqlv
#   brew install sqlv
#
# To keep this formula current on each release:
#
#   1. Push a tag `vX.Y.Z` — the release workflow uploads
#      `sqlv-vX.Y.Z-<target>.tar.gz` per platform.
#   2. Update `version` below to X.Y.Z.
#   3. For each `url` line, replace the sha256 with
#      `shasum -a 256 <the downloaded tarball>` output.
#   4. Commit + push to the homebrew tap repo.
#
# If you want `brew` to fetch shas automatically, you can skip step 3 and
# use `brew create --set-version X.Y.Z <tarball-url>` as a starting point.
#
# Requires macOS 12+ / Linux with glibc 2.31+ (matches our CI release
# targets).
class Sqlv < Formula
  desc     "Agent-friendly SQLite viewer — CLI, MCP server, and live GUI sync"
  homepage "https://github.com/shizhigu/sqlite-viewer"
  version  "0.1.0"
  license  "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url     "https://github.com/shizhigu/sqlite-viewer/releases/download/v#{version}/sqlv-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256  "REPLACE_WITH_ARM64_MAC_SHA256"
    else
      url     "https://github.com/shizhigu/sqlite-viewer/releases/download/v#{version}/sqlv-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256  "REPLACE_WITH_X86_64_MAC_SHA256"
    end
  end

  on_linux do
    url     "https://github.com/shizhigu/sqlite-viewer/releases/download/v#{version}/sqlv-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256  "REPLACE_WITH_LINUX_SHA256"
  end

  def install
    # Release tarballs unpack to `sqlv-vX.Y.Z-<target>/` containing the binary
    # plus README.md and LICENSE.
    bin.install "sqlv"
    doc.install "README.md" if File.exist?("README.md")
    # The MCP server ships alongside the CLI when included in the release.
    bin.install "sqlv-mcp" if File.exist?("sqlv-mcp")
  end

  test do
    assert_match(/sqlv \d+\.\d+\.\d+/, shell_output("#{bin}/sqlv --version"))
    # Smoke: create a tiny DB and query it end-to-end.
    system "#{bin}/sqlv", "exec", "--db", "t.sqlite", "--write",
           "CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (42);"
    output = shell_output("#{bin}/sqlv query --db t.sqlite 'SELECT x FROM t'")
    assert_match(/42/, output)
  end
end

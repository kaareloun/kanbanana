class Kanbanana < Formula
  desc "Kanban board for your Linear issues, with Claude Code built in"
  homepage "https://github.com/kaareloun/kanbanana"
  url "https://github.com/kaareloun/kanbanana/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_THE_TAG_TARBALL"
  license "MIT"
  head "https://github.com/kaareloun/kanbanana.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/app")
  end

  test do
    # The MCP server answers the handshake without a window or a Linear key.
    ENV["KANBANANA_DB"] = (testpath/"cache.sqlite3").to_s
    request = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}'
    output = pipe_output("#{bin}/kanbanana mcp", "#{request}\n")
    assert_match "\"name\":\"kanbanana\"", output
  end
end

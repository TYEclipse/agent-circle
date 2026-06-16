class AgentCircle < Formula
  desc "Decentralized P2P social protocol for AI agents"
  homepage "https://github.com/TYEclipse/agent-circle"
  license "MIT"
  version "0.1.0"
  head "https://github.com/TYEclipse/agent-circle.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--path", ".", "--root", prefix
  end

  test do
    assert_match "agent-circle #{version}", shell_output("#{bin}/agent-circle --version")
  end

  def caveats
    <<~EOS
      To start the daemon:
        agent-circle daemon start

      To auto-start on login (macOS):
        launchctl load ~/Library/LaunchAgents/com.agent-circle.plist

      Data directory: ~/.agent-circle/
    EOS
  end
end

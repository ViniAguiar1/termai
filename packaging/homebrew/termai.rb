# Homebrew Cask for termAI.
#
# This file is the source of truth; it is published to the tap repository
# (ViniAguiar1/homebrew-termai, path Casks/termai.rb) by the release workflow,
# which bumps `version` and `sha256` to match the new GitHub Release.
#
# Users install with:
#   brew tap viniaguiar1/termai
#   brew install --cask termai
# and update with:
#   brew upgrade --cask termai
cask "termai" do
  version "0.1.0"
  sha256 :no_check # replaced with the real DMG sha256 by the release workflow

  url "https://github.com/ViniAguiar1/termai/releases/download/v#{version}/termAI-#{version}-macos-arm64.dmg"
  name "termAI"
  desc "GPU-accelerated terminal emulator with built-in AI assistance"
  homepage "https://github.com/ViniAguiar1/termai"

  # Apple Silicon only for now (the release pipeline builds an arm64 DMG).
  depends_on arch: :arm64

  app "termAI.app"

  zap trash: [
    "~/.config/termai",
    "~/Library/Caches/com.termai.app",
    "~/Library/Saved Application State/com.termai.app.savedState",
  ]
end

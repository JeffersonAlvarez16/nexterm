# typed: false
# frozen_string_literal: true

# ============================================================================
# NexTerm Homebrew Cask
#
# This cask lives in a CUSTOM TAP (JeffersonAlvarez16/homebrew-tap), not in
# homebrew/homebrew-cask. The official homebrew-cask requires a project to meet
# a notability threshold (significant GitHub stars, press coverage, etc.).
# A custom tap has no such requirement and is the standard approach for
# distributing signed+notarized macOS apps that don't yet qualify.
#
# Install:
#   brew tap JeffersonAlvarez16/tap
#   brew install --cask nexterm
#
# ============================================================================
#
# ARCH-SPECIFIC DMGs (current release model):
#
# The NexTerm release workflow produces two separate DMGs per version:
#   NexTerm_<version>_aarch64.dmg  — Apple Silicon (arm64)
#   NexTerm_<version>_x86_64.dmg  — Intel (x86_64)
#
# This cask uses on_arm / on_intel blocks accordingly.
#
# UNIVERSAL DMG alternative (if you ever ship a single universal binary):
# Replace the two on_arm/on_intel blocks with:
#
#   url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_universal.dmg"
#   sha256 "<PLACEHOLDER: shasum -a 256 NexTerm_<version>_universal.dmg>"
#
# ============================================================================

cask "nexterm" do
  version "0.3.0"

  # --------------------------------------------------------------------------
  # SHA-256 PLACEHOLDERS
  #
  # After downloading the release DMGs, compute each digest with:
  #   shasum -a 256 NexTerm_0.3.0_aarch64.dmg
  #   shasum -a 256 NexTerm_0.3.0_x86_64.dmg
  #
  # Then replace the placeholder strings below with the hex output.
  # --------------------------------------------------------------------------

  on_arm do
    url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_aarch64.dmg"
    # PLACEHOLDER: replace with: shasum -a 256 NexTerm_#{version}_aarch64.dmg
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  on_intel do
    url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_x86_64.dmg"
    # PLACEHOLDER: replace with: shasum -a 256 NexTerm_#{version}_x86_64.dmg
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  name "NexTerm"
  desc "Modern SSH and connection manager with integrated password vault"
  homepage "https://github.com/JeffersonAlvarez16/nexterm"

  # The app ships with an in-app auto-updater; Homebrew should not try to
  # upgrade it via cask reinstall on every `brew upgrade` run.
  auto_updates true

  # NexTerm is signed and notarized by Apple; no quarantine override needed.
  depends_on macos: ">= :ventura"

  app "NexTerm.app"

  # Check GitHub Releases for the latest version tag.
  livecheck do
    url :url
    strategy :github_latest
  end

  # --------------------------------------------------------------------------
  # zap: remove all user data on `brew uninstall --zap --cask nexterm`
  #
  # Covers:
  #   - App Support dir: SSH profiles, password vault (passwords.json),
  #     SSH vault (vault.json), profiles.json
  #   - Preferences plist
  #   - Caches
  #   - Tauri WebView data (WKWebView storage)
  # --------------------------------------------------------------------------
  zap trash: [
    "~/Library/Application Support/com.jeffersonalvarez.nexterm",
    "~/Library/Preferences/com.jeffersonalvarez.nexterm.plist",
    "~/Library/Caches/com.jeffersonalvarez.nexterm",
    "~/Library/WebKit/com.jeffersonalvarez.nexterm",
    "~/Library/Saved Application State/com.jeffersonalvarez.nexterm.savedState",
  ]
end

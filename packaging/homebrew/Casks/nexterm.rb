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
# UNIVERSAL DMG (current release model):
#
# The release workflow builds `universal-apple-darwin` — a single DMG that runs
# natively on BOTH Apple Silicon (arm64) and Intel (x86_64):
#   NexTerm_<version>_universal.dmg
#
# So this cask uses a single url + sha256 (no on_arm / on_intel split).
# NOTE: confirm the exact DMG filename on the first universal release and adjust
# the url if Tauri names it differently.
# ============================================================================

cask "nexterm" do
  version "0.3.0"

  # --------------------------------------------------------------------------
  # SHA-256 PLACEHOLDER
  #
  # After downloading the release DMG, compute its digest with:
  #   shasum -a 256 NexTerm_0.3.0_universal.dmg
  # Then replace the placeholder string below with the hex output.
  # --------------------------------------------------------------------------

  url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_universal.dmg"
  # PLACEHOLDER: replace with: shasum -a 256 NexTerm_#{version}_universal.dmg
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

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

# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning where practical.

## [Unreleased]

## [0.4.2] - 2026-06-09

### Fixed
- "Update now" in the update dialog silently did nothing: the pending update was stored in a per-component ref, so the dialog that found the update and the dialog that installs it never shared it. The pending update is now module-scoped.

## [0.4.1] - 2026-06-09

### Changed
- Maintenance release validating the in-app auto-update pipeline end to end (no user-facing changes).

## [0.4.0] - 2026-06-09

### Added
- Password manager promoted to a top-level, full-screen section reachable from a pinned entry in the left sidebar, independent of any SSH session (it previously lived in the per-session side panel).
- Live search filter over saved passwords (matches title, username, URL, and category).
- Bitwarden-compatible import (unencrypted `.json` or `.csv`) and plaintext CSV export of the password vault. Export is gated behind a master-password re-authentication and a plaintext warning.
- About dialog (bottom status bar) showing the app version, a link to the project on GitHub, and a manual "Check for updates" action.
- Proxmox QEMU virtual-machine management (`qm`) alongside the existing LXC containers (`pct`): list, start/stop/reboot, and snapshots, with the same injection-safe VMID and snapshot-name validation.
- Homebrew custom-tap cask for macOS distribution (`packaging/homebrew/`).

### Changed
- The independent password store now auto-locks after 1 minute of inactivity by default (was 5 minutes); existing stores keep their saved timeout.
- Re-enabled the Tauri auto-updater: macOS release builds are now code-signed and notarized in CI, the macOS artifact is a single universal (arm64 + Intel) DMG, and `latest.json` is published. See `docs/RELEASE_SIGNING.md` for the required secrets.

### Fixed
- macOS: closing the main window (red traffic-light / Cmd-W) now quits the app instead of leaving a headless process holding the single-instance lock.
- "View project on GitHub" now opens the browser (the opener capability was missing the default-URL scope).
- Hardened CSV import/export per an adversarial security review: record-aware CSV parsing (multiline fields no longer corrupt on reimport), real Bitwarden JSON with `"uri": null` now imports, formula-injection neutralization (`=`/`+`/`-`/`@`) on export, a clear error for encrypted Bitwarden exports, plaintext buffers zeroized, and rollback on partial import failure.

## [0.3.0] - 2026-06-08

### Added
- Secure password manager in the right panel: an independent, AES-256-GCM + Argon2id encrypted second vault (`passwords.json`) with its own master password, separate from the SSH credential vault.
- Per-entry opaque ids with metadata and secret encrypted separately, so listing entries never decrypts the password.
- Reveal and copy gated by a single-use, id-bound re-authentication grant; the password store has its own 5-minute auto-lock, independent of the SSH vault.
- Metadata-only edits plus an explicit set/replace-password flow, a configurable password generator (length and character classes) with a live entropy estimate, and a settings panel for rotating the master password and the idle timeout.
- Resizable left sidebar: drag from its right edge, persisted across sessions and keyboard accessible.

### Security
- Memory-lock (`mlock` / `VirtualLock`) the derived encryption keys for both the password store and the SSH vault to reduce swap and hibernation exposure (best effort).
- Tightened the webview CSP `connect-src` to `ipc:` only, removing the GitHub origins; the updater runs Rust-side and is unaffected.
- Clear the password reveal grant on window blur, and use constant-time comparison on the re-authentication path.

### Fixed
- Password dialogs now close only via the close button (no backdrop or Escape dismissal), preventing accidental loss of typed input; restored proper Cancel and Save button styling.
- Fixed the revealed password rendering one character per line in the entry list.
- Disabled Tauri updater artifact generation until a valid updater signing key is configured, so release workflows can publish installers for every OS target instead of failing during updater signing.
- Fixed Windows `fs_secure` Win32 API bindings for `windows` crate 0.61 so the Windows release job can compile.
- Made auto-lock timer tests portable on Windows by avoiding `Instant` subtraction underflow.

## [0.2.3] - 2026-06-06

### Security
- Hardened local-file handling so file reads/writes go through Rust-backed commands instead of broad renderer filesystem IPC.
- Hardened release security around frozen dependency installs and signed updater artifacts.

### Changed
- Bumped application metadata to `0.2.3` for the tagged release.

## [0.2.2] - 2026-06-05

### Added
- Frontend test infrastructure with Vitest and CI checks for typecheck, tests, Rust formatting, clippy and Rust tests.
- Terminal find/search with match counts, copy-on-select, smart key handling and right-click paste support.
- Terminal snippets with NexTerm `{{var}}` grammar, variable prompts, live preview, password masking and preview-before-execute safeguards.
- Collapsible and resizable terminal side panel for SFTP, tunnels and command history.
- Split terminal panes with keyboard navigation, focus states and pane lifecycle management.
- Input broadcast across panes with safety guards and warning UI.
- Command history capture with opt-in storage, filtering and action buttons.
- WebGL terminal rendering with DOM fallback.
- Profile folders with folder-first sidebar grouping.
- Theme presets, persisted theme store, theme picker, FOUC-safe theme application and LAMPLIGHT UI/theme work.
- SFTP conflict detection and sequential conflict resolution for upload/download batches.
- Dynamic SOCKS5 tunnels.
- SSH key generation for Ed25519, RSA and ECDSA keys.
- SSH exec primitives and Tauri command support.
- Remote monitoring with metrics parsing, sampler task, frontend panel, sparklines and kill confirmation.
- Docker panel and SSH-backed Docker command primitives.
- Proxmox LXC panel and SSH-backed Proxmox command primitives.
- Full-area SFTP file view with drag-and-drop transfers in both directions.
- In-app local and remote file editor.

### Changed
- Upgraded `russh` from `0.48` to `0.50` and negotiated RSA SHA-2 for RSA keys.
- Centralized SSH client configuration for connection and test-connection keepalive behavior.
- Reworked layout so terminal state stays mounted while SFTP/tunnels can dock in the side panel.
- Improved startup command preview/confirmation and dialog focus styling.

### Fixed
- Restored interactive SSH editor controls.
- Treated host-key algorithm changes as dangerous.
- Prevented credentials from being sent before host-key verification succeeds.
- Rejected unsafe entry names in recursive SFTP downloads.
- Matched OpenSSH hashed `known_hosts` entries and honored known-host revocations.
- Closed empty-vault password bypass and hardened vault format handling.
- Zeroized plaintext credentials across auth hops.
- Defaulted local tunnel binds to loopback.
- Fixed terminal hook ordering, xterm cleanup on disconnect and split-pane layout timing.
- Fixed snippet and monitoring blank-screen loops caused by unstable object selectors.
- Fixed private-key loading for PEM/PKCS#1 and PKCS#8 keys.
- Fixed SFTP conflict decision leakage across operations and same-pane drag/drop handling.

### Accessibility
- Added tablist roles, accessible dialog names, `html[lang]` synchronization, keyboard/focus improvements and measured contrast corrections for theme presets.

## [0.2.1] - 2026-04-29

### Added
- SFTP folder download: right-click a remote folder (or select + use the toolbar / drag-drop) to download it recursively. Reports a single aggregate transfer with combined byte progress instead of one entry per file.
- Persisted workspace snapshots keyed by stable identity (`profileId + userId`) so the app remembers the last active workspace per connection context.

### Changed
- Restored the active feature (`terminal`, `sftp`, or `tunnel`) when switching back to a previously used session workspace.
- Restored SFTP navigation context including local path, remote path, back/forward history, split position, and search state.
- Applied `startupDirectory` as the initial SFTP remote fallback when no saved workspace snapshot exists.

### Notes
- Folder download skips symlinks (logs a warning) to avoid loops; partial files are left on disk on failure for inspection.
- Live SSH handles, PTYs, and dead terminal processes are intentionally not persisted; only safe UI/workspace state is restored.

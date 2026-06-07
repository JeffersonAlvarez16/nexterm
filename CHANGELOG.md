# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning where practical.

## [Unreleased]

### Fixed
- Disabled Tauri updater artifact generation until a valid updater signing key is configured, so release workflows can publish installers for every OS target instead of failing during updater signing.
- Fixed Windows `fs_secure` Win32 API bindings for `windows` crate 0.61 so the Windows release job can compile.

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

# NexTerm Homebrew Tap

NexTerm is distributed via a **custom Homebrew tap** instead of the official
[homebrew/homebrew-cask](https://github.com/Homebrew/homebrew-cask) repository.

## Why a custom tap?

The official `homebrew-cask` project requires applications to meet a
[notability threshold](https://docs.brew.sh/Cask-Cookbook#naming-policy) —
typically hundreds of GitHub stars, significant press coverage, or widespread
adoption. A custom tap has no such requirements and is the standard approach
for distributing signed + notarized macOS apps that don't yet qualify for the
official index.

NexTerm is **fully signed and notarized by Apple**, so macOS Gatekeeper will
trust it regardless of which tap it comes from.

---

## Installing NexTerm

```sh
brew tap JeffersonAlvarez16/tap
brew install --cask nexterm
```

To upgrade:

```sh
brew upgrade --cask nexterm
```

> **Note:** NexTerm also ships with an in-app auto-updater. The cask sets
> `auto_updates true`, so `brew upgrade` will skip it unless you pass `--greedy`.

To uninstall (app only):

```sh
brew uninstall --cask nexterm
```

To uninstall **and delete all user data** (SSH profiles, password vault,
preferences):

```sh
brew uninstall --zap --cask nexterm
```

---

## Setting up the tap repository

The tap repository must be named **`homebrew-tap`** under the
`JeffersonAlvarez16` GitHub org/user so Homebrew can resolve `brew tap
JeffersonAlvarez16/tap`.

1. Create the repository: `https://github.com/JeffersonAlvarez16/homebrew-tap`
2. Make it public.
3. Inside the repo, create a `Casks/` directory.
4. Copy `packaging/homebrew/Casks/nexterm.rb` from this repo into
   `Casks/nexterm.rb` in the tap repo.

The resulting structure must be:

```
homebrew-tap/
└── Casks/
    └── nexterm.rb
```

---

## Updating the cask for a new release

After every release you must update two fields in `Casks/nexterm.rb`:
`version` and the `sha256` digests.

### 1. Download the release DMGs

```sh
VERSION="0.3.0"   # replace with the new version
curl -LO "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v${VERSION}/NexTerm_${VERSION}_aarch64.dmg"
curl -LO "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v${VERSION}/NexTerm_${VERSION}_x86_64.dmg"
```

### 2. Compute the sha256 digests

```sh
shasum -a 256 "NexTerm_${VERSION}_aarch64.dmg"
shasum -a 256 "NexTerm_${VERSION}_x86_64.dmg"
```

Each command prints a 64-character hex digest followed by the filename.

### 3. Update `Casks/nexterm.rb` in the tap repo

Replace `version` and both `sha256` placeholders:

```ruby
version "0.3.0"   # ← new version

on_arm do
  url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_aarch64.dmg"
  sha256 "<paste arm64 digest here>"
end

on_intel do
  url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_x86_64.dmg"
  sha256 "<paste x86_64 digest here>"
end
```

### 4. Commit and push to the tap repo

```sh
git add Casks/nexterm.rb
git commit -m "chore: bump nexterm to v${VERSION}"
git push
```

Users who run `brew upgrade --cask nexterm` (or `brew upgrade --greedy --cask
nexterm`) will pick up the new version automatically.

### 5. Validate the cask syntax (optional but recommended)

If you have a local Homebrew install:

```sh
brew audit --cask Casks/nexterm.rb
brew style Casks/nexterm.rb
```

---

## DMG filename convention

Tauri v2's default artifact naming for macOS DMGs is:

| Arch | Filename |
|---|---|
| Apple Silicon (arm64) | `NexTerm_<version>_aarch64.dmg` |
| Intel (x86_64) | `NexTerm_<version>_x86_64.dmg` |

If a universal binary is ever shipped, the filename would be
`NexTerm_<version>_universal.dmg` and the cask would use a single `url`/`sha256`
pair instead of the `on_arm`/`on_intel` blocks (see the comment at the top of
`nexterm.rb`).

---

## Files

| Path | Purpose |
|---|---|
| `packaging/homebrew/Casks/nexterm.rb` | Homebrew Cask definition |
| `packaging/homebrew/README.md` | This file |

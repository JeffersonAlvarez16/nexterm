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
`version` and the `sha256` digest. The release ships a single **universal** DMG
(arm64 + x86_64 in one file), so there is only one digest to update.

### 1. Download the release DMG

```sh
VERSION="0.3.0"   # replace with the new version
curl -LO "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v${VERSION}/NexTerm_${VERSION}_universal.dmg"
```

### 2. Compute the sha256 digest

```sh
shasum -a 256 "NexTerm_${VERSION}_universal.dmg"
```

The command prints a 64-character hex digest followed by the filename.

### 3. Update `Casks/nexterm.rb` in the tap repo

Replace `version` and the `sha256` placeholder:

```ruby
version "0.3.0"   # ← new version

url "https://github.com/JeffersonAlvarez16/nexterm/releases/download/v#{version}/NexTerm_#{version}_universal.dmg"
sha256 "<paste universal digest here>"
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

The release workflow builds a single **universal** macOS binary
(`--target universal-apple-darwin`), so there is one DMG per release:

| Build | Filename |
|---|---|
| Universal (arm64 + x86_64) | `NexTerm_<version>_universal.dmg` |

Confirm the exact filename on the first universal release (Tauri's artifact
naming) and adjust the cask `url` if it differs.

---

## Files

| Path | Purpose |
|---|---|
| `packaging/homebrew/Casks/nexterm.rb` | Homebrew Cask definition |
| `packaging/homebrew/README.md` | This file |

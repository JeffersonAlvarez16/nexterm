# Release Signing Setup

This document describes every GitHub repository secret required by
`.github/workflows/release.yml` and explains how to obtain or generate each
one. **Never commit secret values to the repository.**

---

## 1. Tauri Updater Signing

These secrets enable the in-app auto-updater to verify downloaded bundles.

| Secret | Description |
|--------|-------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the minisign private key file |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password used when generating the key (empty string if none) |

### How to obtain

A fresh keypair was generated for this project with:

```sh
CI=true pnpm tauri signer generate \
  -w ~/.tauri/nexterm-updater.key \
  -p "" \
  --force
```

- **Private key location:** `~/.tauri/nexterm-updater.key`
- **Public key location:** `~/.tauri/nexterm-updater.key.pub`

Set `TAURI_SIGNING_PRIVATE_KEY` to the **full contents** of
`~/.tauri/nexterm-updater.key` (the whole file, including the header line).

The corresponding public key is already committed in
`src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

If no password was set during generation, add `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
as an empty secret (or omit it — tauri-action treats a missing value as empty).

---

## 2. macOS Code Signing (Apple Developer ID)

These secrets allow the runner's ephemeral keychain to sign `.app` bundles.

| Secret | Description |
|--------|-------------|
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting the `.p12` file |
| `APPLE_SIGNING_IDENTITY` | Full identity string, e.g. `Developer ID Application: Your Name (RFMR899Y3Z)` |
| `APPLE_TEAM_ID` | Apple Developer Team ID, e.g. `RFMR899Y3Z` |

### How to obtain

1. Export the **Developer ID Application** certificate from Keychain Access
   (or `security export`) as a `.p12` file with a strong password.
2. Base64-encode it:
   ```sh
   base64 -i /path/to/cert.p12 | pbcopy   # macOS — copies to clipboard
   # or
   base64 -i /path/to/cert.p12 > cert.p12.b64
   ```
3. Set `APPLE_CERTIFICATE` to the base64 output (no newlines required —
   GitHub Secrets handles multi-line values fine).
4. Set `APPLE_CERTIFICATE_PASSWORD` to the password chosen during export.
5. Set `APPLE_SIGNING_IDENTITY` to the full identity string shown in
   Keychain Access or via:
   ```sh
   security find-identity -v -p codesigning
   ```
6. Set `APPLE_TEAM_ID` to your 10-character Team ID (visible at
   <https://developer.apple.com/account> or in the identity string above).

---

## 3. macOS Notarization (App Store Connect API key)

Notarization submits the signed `.app` to Apple for malware scanning before
stapling a notarization ticket. The workflow uses an API key (not password)
for non-interactive CI use.

| Secret | Description |
|--------|-------------|
| `APPLE_API_ISSUER` | Issuer ID (UUID) from App Store Connect |
| `APPLE_API_KEY` | Key ID (10-char alphanumeric) from App Store Connect |
| `APPLE_API_KEY_CONTENT` | Raw contents of the `.p8` private key file |

### How to obtain

1. Go to **App Store Connect → Users and Access → Integrations → App Store Connect API**.
2. Create a key with **Developer** role (minimum required for notarization).
3. Download the `.p8` file — **this can only be downloaded once**.
4. Note the **Issuer ID** (shown at the top of the API Keys page) and the
   **Key ID** (shown next to the key name, e.g. `AB12CD34EF`).
5. Set `APPLE_API_ISSUER` to the Issuer ID UUID.
6. Set `APPLE_API_KEY` to the Key ID (not the file name — just the ID).
7. Set `APPLE_API_KEY_CONTENT` to the raw contents of the `.p8` file:
   ```sh
   cat AuthKey_AB12CD34EF.p8 | pbcopy   # copies to clipboard
   ```
   The workflow writes this to
   `~/.private_keys/AuthKey_<KEY_ID>.p8` at build time, which is the
   standard location searched by `xcrun notarytool`.

---

## Summary Table

| Secret | Used by |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | All platforms — updater `.sig` generation |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | All platforms — updater key decryption |
| `APPLE_CERTIFICATE` | macOS only — code signing |
| `APPLE_CERTIFICATE_PASSWORD` | macOS only — code signing |
| `APPLE_SIGNING_IDENTITY` | macOS only — code signing |
| `APPLE_TEAM_ID` | macOS only — code signing |
| `APPLE_API_ISSUER` | macOS only — notarization |
| `APPLE_API_KEY` | macOS only — notarization |
| `APPLE_API_KEY_CONTENT` | macOS only — notarization |

Add all nine secrets at:
**GitHub repo → Settings → Secrets and variables → Actions → New repository secret**

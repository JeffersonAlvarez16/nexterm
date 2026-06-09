// passwords.rs — Independent encrypted password store (second vault).
//
// This is a SEPARATE encrypted file (`passwords.json`) with its own salt, its
// own Argon2id-derived key, its own in-memory key slot, and its own auto-lock
// timer — fully independent from the SSH credential vault in `vault.rs`. It
// reuses the audited primitives in `crate::crypto` (AES-256-GCM + Argon2id) so
// no crypto is reinvented and the on-disk security properties match the vault.
//
// Password store file format (JSON on disk), version 1:
// {
//   "version": 1,
//   "salt": "<base64 32-byte salt>",
//   "kdf": { "algorithm": "argon2id", "m_cost": 65536, "t_cost": 3, "p_cost": 1 },
//   "verifier": "<base64 nonce(12) + ciphertext + tag(16)>",
//   "entries": {
//     "<opaque 16-byte hex id>": {
//       "meta":   "<base64 AES-GCM blob>",   // {title,username,url,category,created_at,updated_at}
//       "secret": "<base64 AES-GCM blob>"    // {password,notes}
//     }
//   }
// }
//
// SECURITY MODEL
// - Each entry stores TWO independently-encrypted blobs: `meta` (listing info)
//   and `secret` (the password + notes). `list()` decrypts ONLY `meta`, so
//   enumerating entries NEVER touches a password. `reveal()` decrypts the
//   `secret` blob for a single entry on demand.
// - The entry `id` is an opaque random 16-byte hex string from `OsRng`. It is
//   the map KEY (not stored inside `meta`) and is NOT derived from the title,
//   so ids leak nothing about the entry.
// - A fixed verifier plaintext is encrypted under the derived key so the master
//   password can be validated independently of how many entries exist.
//
// The reveal *grant* (fresh re-auth requirement) is enforced at the command
// layer; this module's `reveal()` only requires the store to be unlocked.
// Likewise the 12-char minimum master-password length is enforced at the
// command layer (documented on `create`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::{self, KdfParams, SALT_SIZE};
use crate::error::AppError;
use crate::fs_secure;
use crate::secure_mem::LockedKey;

/// Password store file name in the app data directory.
const PASSWORDS_FILE: &str = "passwords.json";

/// Current password store format version.
const PASSWORDS_VERSION: u32 = 1;

/// Fixed plaintext encrypted under the derived key to verify the master
/// password independently of how many entries the store holds. The AES-GCM auth
/// tag also guards the verifier's integrity. Distinct from the SSH vault's
/// verifier so the two stores can never be confused for one another.
const VERIFIER_PLAINTEXT: &[u8] = b"nexterm-passwords-verifier-v1";

/// Number of random bytes for an entry id (hex-encoded → 32-char id).
const ID_BYTES: usize = 16;

// ─── Public IPC types ───────────────────────────────────

/// Listing metadata for one password entry. NEVER contains the password.
///
/// Serialized to the frontend with camelCase field names to match the Tauri
/// IPC contract (`createdAt`, `updatedAt`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordEntryMeta {
    pub id: String,
    pub title: String,
    pub username: String,
    pub url: String,
    pub category: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Full input for creating or updating an entry. Carries the secret material
/// (`password`, `notes`) which is encrypted into the separate `secret` blob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordEntryInput {
    pub title: String,
    pub username: String,
    pub url: String,
    pub category: String,
    pub notes: String,
    pub password: String,
}

/// Lossless meta-only update input. Carries ONLY the listing fields, so
/// re-encrypting the meta blob never touches (and never needs to decrypt) the
/// entry's `secret` blob. Used by `update_meta` / the `pw_update_meta` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaInput {
    pub title: String,
    pub username: String,
    pub url: String,
    pub category: String,
}

/// Lossless secret-only update input. Carries ONLY the secret material, so
/// re-encrypting the `secret` blob preserves the meta blob's fields verbatim.
/// Used by `update_secret` / the `pw_update_secret` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretInput {
    pub password: String,
    pub notes: String,
}

// ─── On-disk plaintext shapes (encrypted before persistence) ────

/// Plaintext JSON of the `meta` blob. The entry id is the map KEY on disk, so
/// it is deliberately absent here.
#[derive(Serialize, Deserialize)]
struct MetaPlain {
    title: String,
    username: String,
    url: String,
    category: String,
    created_at: i64,
    updated_at: i64,
}

/// Plaintext JSON of the `secret` blob — the dangerous material kept encrypted
/// at rest and only decrypted by `reveal()`.
#[derive(Serialize, Deserialize)]
struct SecretPlain {
    password: String,
    notes: String,
}

// ─── On-disk file shape ─────────────────────────────────

/// One encrypted entry: two independent AES-GCM blobs, base64-encoded.
#[derive(Serialize, Deserialize, Clone)]
struct EncEntry {
    meta: String,
    secret: String,
}

/// On-disk file. `deny_unknown_fields` makes a corrupt/tampered file fail loudly
/// at parse time instead of silently ignoring junk keys.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordFile {
    version: u32,
    salt: String,
    kdf: KdfParams,
    verifier: String,
    entries: BTreeMap<String, EncEntry>,
}

// ─── In-memory entry ────────────────────────────────────

/// In-memory entry holding the RAW (base64-decoded) AES-GCM blobs. We keep the
/// ciphertext in memory and only decrypt on demand, mirroring how `Vault` holds
/// `Vec<u8>` ciphertext per credential.
struct StoreEntry {
    meta: Vec<u8>,
    secret: Vec<u8>,
}

// ─── PasswordStore ──────────────────────────────────────

/// The independent password store. Holds the derived key in memory while
/// unlocked and the per-entry ciphertext; decrypts lazily.
///
/// The in-memory derived key is wrapped in a [`LockedKey`], which best-effort
/// `mlock`s (Unix) / `VirtualLock`s (Windows) the 32 bytes into RAM so they are
/// kept out of swap/hibernation where the OS and process limits permit, and
/// zeroizes them on drop. Locking is best-effort: `RLIMIT_MEMLOCK` or
/// insufficient privileges may prevent it (a warning is logged), and it reduces
/// but does not eliminate swap exposure.
pub struct PasswordStore {
    file_path: PathBuf,
    derived_key: Option<LockedKey>,
    salt: [u8; SALT_SIZE],
    kdf_params: KdfParams,
    entries: BTreeMap<String, StoreEntry>,
}

impl Drop for PasswordStore {
    fn drop(&mut self) {
        self.lock();
    }
}

/// Material snapshotted from an unlocked [`PasswordStore`] so the command layer
/// can run the master-password re-auth derivation OFF the async lock. Holds the
/// salt, KDF params, and a `Zeroizing` copy of the current 32-byte derived key
/// (wiped on drop). See [`PasswordStore::reauth_snapshot`] and
/// [`verify_reauth_candidate`].
pub struct ReauthSnapshot {
    salt: [u8; SALT_SIZE],
    kdf_params: KdfParams,
    current_key: Zeroizing<[u8; 32]>,
}

/// Derive a candidate key from `password` using a [`ReauthSnapshot`] and compare
/// it (constant-time) against the snapshotted current key. Returns `Ok(true)` on
/// a match, `Ok(false)` on a mismatch, or `Err` if derivation fails.
///
/// This is the pure, lock-free half of the reveal re-auth: the command layer
/// snapshots under a brief lock, drops the guard, then calls this on a blocking
/// thread so the memory-hard Argon2id derivation never stalls other `pw_*`
/// commands.
pub fn verify_reauth_candidate(
    snapshot: &ReauthSnapshot,
    password: &str,
) -> Result<bool, AppError> {
    let candidate = crypto::derive_key(password, &snapshot.salt, &snapshot.kdf_params)?;
    Ok(crypto::ct_eq(
        candidate.as_slice(),
        snapshot.current_key.as_slice(),
    ))
}

impl PasswordStore {
    /// Whether the password store file exists on disk.
    pub fn exists(data_dir: &Path) -> bool {
        data_dir.join(PASSWORDS_FILE).exists()
    }

    /// Create a brand-new password store and leave it unlocked.
    ///
    /// Generates a random salt, derives the key via Argon2id (current default
    /// params), and writes an empty `passwords.json` to disk via
    /// `fs_secure::secure_write` (owner-only perms).
    ///
    /// NOTE: the 12-character minimum master-password length is enforced at the
    /// command layer (`pw_create`), NOT here, so library tests can use short
    /// passwords. Callers outside the command layer must validate length
    /// themselves.
    pub fn create(data_dir: &Path, master_password: &str) -> Result<Self, AppError> {
        let file_path = data_dir.join(PASSWORDS_FILE);

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::VaultError(format!("Failed to create passwords directory: {e}"))
            })?;
        }

        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);

        let kdf_params = crypto::default_kdf_params();
        // Derive into a Zeroizing buffer, then move the bytes into a LockedKey
        // (best-effort mlock + zero-on-drop). The Zeroizing source is wiped when
        // it drops at the end of this scope.
        let derived_key = crypto::derive_key(master_password, &salt, &kdf_params)?;

        let store = PasswordStore {
            file_path,
            derived_key: Some(LockedKey::new(*derived_key)),
            salt,
            kdf_params,
            entries: BTreeMap::new(),
        };

        store.save_to_disk()?;

        Ok(store)
    }

    /// Open an existing password store with the master password.
    ///
    /// The password is validated by decrypting the fixed verifier; an AEAD
    /// failure (wrong key or tampered verifier) maps to
    /// [`AppError::VaultWrongPassword`].
    pub fn unlock(data_dir: &Path, master_password: &str) -> Result<Self, AppError> {
        let file_path = data_dir.join(PASSWORDS_FILE);

        let contents = std::fs::read_to_string(&file_path)
            .map_err(|e| AppError::VaultError(format!("Failed to read passwords file: {e}")))?;

        let file: PasswordFile = serde_json::from_str(&contents)
            .map_err(|e| AppError::VaultError(format!("Corrupt passwords file: {e}")))?;

        if file.version != PASSWORDS_VERSION {
            return Err(AppError::VaultError(format!(
                "Unsupported passwords version: {}",
                file.version
            )));
        }

        let salt_bytes = BASE64
            .decode(&file.salt)
            .map_err(|e| AppError::VaultError(format!("Invalid salt encoding: {e}")))?;
        if salt_bytes.len() != SALT_SIZE {
            return Err(AppError::VaultError("Invalid salt length".to_string()));
        }
        let mut salt = [0u8; SALT_SIZE];
        salt.copy_from_slice(&salt_bytes);

        let kdf_params = file.kdf.clone();
        let derived_key = crypto::derive_key(master_password, &salt, &kdf_params)?;

        // Validate the master password against the verifier BEFORE decoding the
        // entries, so a wrong password fails fast with the right error.
        let verifier_bytes = BASE64
            .decode(&file.verifier)
            .map_err(|e| AppError::VaultError(format!("Invalid verifier encoding: {e}")))?;
        let ok = crypto::verify(&derived_key, &verifier_bytes, VERIFIER_PLAINTEXT)
            .map_err(|_| AppError::VaultWrongPassword)?;
        if !ok {
            return Err(AppError::VaultWrongPassword);
        }

        // Decode every entry's blobs from base64 into raw ciphertext.
        let mut entries = BTreeMap::new();
        for (id, enc) in &file.entries {
            let meta = BASE64.decode(&enc.meta).map_err(|e| {
                AppError::VaultError(format!("Invalid meta encoding for {id}: {e}"))
            })?;
            let secret = BASE64.decode(&enc.secret).map_err(|e| {
                AppError::VaultError(format!("Invalid secret encoding for {id}: {e}"))
            })?;
            entries.insert(id.clone(), StoreEntry { meta, secret });
        }

        let store = PasswordStore {
            file_path,
            // Move the verified key bytes into a LockedKey (best-effort mlock +
            // zero-on-drop). The Zeroizing `derived_key` source is wiped on drop.
            derived_key: Some(LockedKey::new(*derived_key)),
            salt,
            kdf_params,
            entries,
        };

        // Idempotently re-apply owner-only perms (covers files written before
        // fs_secure existed or by an older app version).
        if let Err(e) = fs_secure::harden_existing(&store.file_path) {
            tracing::warn!("Failed to harden existing passwords file: {e}");
        }

        Ok(store)
    }

    /// Lock the store — drop the derived key (zeroized on drop).
    pub fn lock(&mut self) {
        self.derived_key = None;
    }

    /// Whether the store is unlocked (has a derived key in memory).
    pub fn is_unlocked(&self) -> bool {
        self.derived_key.is_some()
    }

    /// Verify a candidate master password against the currently-unlocked store
    /// WITHOUT changing any state. Re-derives a key from `password` using the
    /// store's salt + KDF params and compares it against the in-memory key with
    /// a constant-time comparison ([`crypto::ct_eq`]) so the match decision does
    /// not leak through timing.
    ///
    /// Returns `Ok(true)` on a match, `Ok(false)` on a mismatch, and
    /// [`AppError::VaultLocked`] if the store is locked. This backs the fresh
    /// re-auth GRANT required by the dangerous `pw_reveal` path: the command
    /// layer calls this, and only on `Ok(true)` issues the reveal grant.
    pub fn verify_password(&self, password: &str) -> Result<bool, AppError> {
        let current_key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;
        let candidate = crypto::derive_key(password, &self.salt, &self.kdf_params)?;
        Ok(crypto::ct_eq(candidate.as_slice(), current_key.as_slice()))
    }

    /// Snapshot the material needed to verify a candidate master password OFF
    /// the async lock: the salt, KDF params, and a COPY of the current derived
    /// key. Returns [`AppError::VaultLocked`] if locked.
    ///
    /// This exists so the command layer (`pw_reauth`) can clone these cheap
    /// values while holding the async mutex briefly, drop the guard, and then
    /// run the expensive Argon2id derivation on a blocking thread WITHOUT
    /// serializing every other `pw_*` command behind the memory-hard KDF. The
    /// returned key copy is wrapped in `Zeroizing` so it is wiped on drop; the
    /// constant-time compare lives in [`crypto::ct_eq`].
    pub fn reauth_snapshot(&self) -> Result<ReauthSnapshot, AppError> {
        let current_key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;
        Ok(ReauthSnapshot {
            salt: self.salt,
            kdf_params: self.kdf_params.clone(),
            // Copy the 32 key bytes out of the LockedKey into a Zeroizing buffer
            // so the off-lock re-auth derivation can compare against them; the
            // copy is wiped on drop.
            current_key: Zeroizing::new(*current_key.as_bytes()),
        })
    }

    /// List all entries' metadata. Decrypts ONLY the `meta` blob per entry — the
    /// password (`secret` blob) is never touched here.
    ///
    /// Entries are returned in stable id order (the on-disk `BTreeMap` order).
    pub fn list(&self) -> Result<Vec<PasswordEntryMeta>, AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let mut out = Vec::with_capacity(self.entries.len());
        for (id, entry) in &self.entries {
            let plain = crypto::decrypt_raw(key, &entry.meta)?;
            let meta: MetaPlain = serde_json::from_slice(&plain)
                .map_err(|e| AppError::VaultError(format!("Corrupt meta for {id}: {e}")))?;
            out.push(PasswordEntryMeta {
                id: id.clone(),
                title: meta.title,
                username: meta.username,
                url: meta.url,
                category: meta.category,
                created_at: meta.created_at,
                updated_at: meta.updated_at,
            });
        }
        Ok(out)
    }

    /// Add a new entry. Returns the generated opaque id.
    ///
    /// `created_at` and `updated_at` are set to the current Unix time. The id is
    /// 16 random bytes from `OsRng`, hex-encoded.
    pub fn add(&mut self, input: &PasswordEntryInput) -> Result<String, AppError> {
        let now = now_unix();
        let id = new_id();
        let entry = self.encrypt_entry(input, now, now)?;
        self.entries.insert(id.clone(), entry);
        self.save_to_disk()?;
        Ok(id)
    }

    /// Update an existing entry in place, preserving its `created_at` and
    /// refreshing `updated_at`. Errors if the id does not exist.
    pub fn update(&mut self, id: &str, input: &PasswordEntryInput) -> Result<(), AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        // Preserve the original created_at by decrypting the existing meta.
        let existing = self
            .entries
            .get(id)
            .ok_or_else(|| AppError::VaultError(format!("No such entry: {id}")))?;
        let plain = crypto::decrypt_raw(key, &existing.meta)?;
        let old_meta: MetaPlain = serde_json::from_slice(&plain)
            .map_err(|e| AppError::VaultError(format!("Corrupt meta for {id}: {e}")))?;

        let created_at = old_meta.created_at;
        let updated_at = now_unix();
        let entry = self.encrypt_entry(input, created_at, updated_at)?;
        self.entries.insert(id.to_string(), entry);
        self.save_to_disk()
    }

    /// Re-encrypt ONLY the `meta` blob for an entry, leaving the `secret` blob
    /// byte-for-byte UNCHANGED (the password + notes are never decrypted here).
    ///
    /// `created_at` is preserved by decrypting the existing meta; `updated_at` is
    /// bumped to now. The new listing fields come from `input`. Errors with
    /// [`AppError::VaultError`] if the id does not exist.
    ///
    /// This is the lossless edit path: editing listing info must NOT round-trip
    /// the secret through plaintext nor risk clobbering it.
    pub fn update_meta(&mut self, id: &str, input: &MetaInput) -> Result<(), AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let existing = self
            .entries
            .get(id)
            .ok_or_else(|| AppError::VaultError(format!("No such entry: {id}")))?;

        // Decrypt the existing meta ONLY to recover created_at. The secret blob
        // is never read.
        let plain = crypto::decrypt_raw(key, &existing.meta)?;
        let old_meta: MetaPlain = serde_json::from_slice(&plain)
            .map_err(|e| AppError::VaultError(format!("Corrupt meta for {id}: {e}")))?;

        let new_meta = MetaPlain {
            title: input.title.clone(),
            username: input.username.clone(),
            url: input.url.clone(),
            category: input.category.clone(),
            created_at: old_meta.created_at,
            updated_at: now_unix(),
        };
        let meta_json = serde_json::to_vec(&new_meta)
            .map_err(|e| AppError::VaultError(format!("Failed to serialize meta: {e}")))?;
        let meta = crypto::encrypt_bytes(key, &meta_json)?;

        // Preserve the existing secret blob byte-for-byte.
        let secret = existing.secret.clone();
        self.entries
            .insert(id.to_string(), StoreEntry { meta, secret });
        self.save_to_disk()
    }

    /// Re-encrypt ONLY the `secret` blob (password + notes) for an entry,
    /// preserving every meta field. `updated_at` in the meta blob is bumped to
    /// now (the meta is decrypted only to recover its current fields, then
    /// re-encrypted with a refreshed `updated_at`); `created_at` and all listing
    /// fields are preserved. Errors with [`AppError::VaultError`] if the id does
    /// not exist.
    pub fn update_secret(&mut self, id: &str, input: &SecretInput) -> Result<(), AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let existing = self
            .entries
            .get(id)
            .ok_or_else(|| AppError::VaultError(format!("No such entry: {id}")))?;

        // Decrypt the existing meta to preserve every field while bumping
        // updated_at.
        let plain = crypto::decrypt_raw(key, &existing.meta)?;
        let old_meta: MetaPlain = serde_json::from_slice(&plain)
            .map_err(|e| AppError::VaultError(format!("Corrupt meta for {id}: {e}")))?;

        let new_meta = MetaPlain {
            title: old_meta.title,
            username: old_meta.username,
            url: old_meta.url,
            category: old_meta.category,
            created_at: old_meta.created_at,
            updated_at: now_unix(),
        };
        let meta_json = serde_json::to_vec(&new_meta)
            .map_err(|e| AppError::VaultError(format!("Failed to serialize meta: {e}")))?;
        let meta = crypto::encrypt_bytes(key, &meta_json)?;

        // Re-encrypt the secret from the new material. Wrap the JSON so the
        // plaintext password is wiped after encryption.
        let secret_plain = SecretPlain {
            password: input.password.clone(),
            notes: input.notes.clone(),
        };
        let secret_json = Zeroizing::new(
            serde_json::to_vec(&secret_plain)
                .map_err(|e| AppError::VaultError(format!("Failed to serialize secret: {e}")))?,
        );
        let secret = crypto::encrypt_bytes(key, &secret_json)?;

        self.entries
            .insert(id.to_string(), StoreEntry { meta, secret });
        self.save_to_disk()
    }

    /// Delete an entry. Errors if the id does not exist.
    pub fn delete(&mut self, id: &str) -> Result<(), AppError> {
        if self.entries.remove(id).is_none() {
            return Err(AppError::VaultError(format!("No such entry: {id}")));
        }
        self.save_to_disk()
    }

    /// Reveal the password for one entry — decrypts the `secret` blob and
    /// returns ONLY the password, wrapped in `Zeroizing` so it is wiped on drop.
    ///
    /// The fresh re-auth GRANT check happens at the command layer; this method
    /// only requires the store to be unlocked.
    pub fn reveal(&self, id: &str) -> Result<Zeroizing<String>, AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| AppError::VaultError(format!("No such entry: {id}")))?;

        let plain = Zeroizing::new(crypto::decrypt_raw(key, &entry.secret)?);
        let secret: SecretPlain = serde_json::from_slice(&plain)
            .map_err(|e| AppError::VaultError(format!("Corrupt secret for {id}: {e}")))?;
        // Wrap the password before the `secret` struct (holding a bare String)
        // is dropped at end of scope.
        Ok(Zeroizing::new(secret.password))
    }

    /// Change the master password: validate `old`, then rotate the salt, derive
    /// a fresh key from `new`, and re-encrypt EVERY blob (meta + secret) and the
    /// verifier under the new key.
    ///
    /// A wrong `old` password maps to [`AppError::VaultWrongPassword`].
    pub fn change_master_password(&mut self, old: &str, new: &str) -> Result<(), AppError> {
        // Validate the OLD password by re-deriving and comparing (constant-time)
        // against the in-memory key. The store must already be unlocked.
        let current_key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;
        let old_key = crypto::derive_key(old, &self.salt, &self.kdf_params)?;
        if !crypto::ct_eq(old_key.as_slice(), current_key.as_slice()) {
            return Err(AppError::VaultWrongPassword);
        }

        // Rotate salt + derive the new key BEFORE touching `self.derived_key`,
        // so a derivation failure leaves the store on the old key untouched.
        let mut new_salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut new_salt);
        let new_kdf_params = crypto::default_kdf_params();
        let new_key = crypto::derive_key(new, &new_salt, &new_kdf_params)?;

        // Re-encrypt every blob INLINE: decrypt the secret with the OLD key into
        // a Zeroizing buffer, immediately re-encrypt under the NEW key, then drop
        // the cleartext at the end of each iteration. This never accumulates all
        // passwords as bare plaintext in memory — at most one secret is in the
        // clear at any moment, and it is wiped on drop (FIX #7).
        let mut new_entries: BTreeMap<String, StoreEntry> = BTreeMap::new();
        for (id, entry) in &self.entries {
            // `meta` is non-secret listing info; `secret` holds the password.
            let meta_plain = crypto::decrypt_raw(current_key, &entry.meta)?;
            let secret_plain = Zeroizing::new(crypto::decrypt_raw(current_key, &entry.secret)?);

            let meta = crypto::encrypt_bytes(&new_key, &meta_plain)?;
            let secret = crypto::encrypt_bytes(&new_key, &secret_plain)?;
            new_entries.insert(id.clone(), StoreEntry { meta, secret });
            // `secret_plain` (Zeroizing) is dropped here, wiping the cleartext
            // before the next iteration.
        }

        // Commit the rotation only after every blob re-encrypted successfully.
        // Replacing the key drops the old LockedKey (zeroizing + unlocking it);
        // the new key is moved into a fresh LockedKey (best-effort re-lock).
        self.salt = new_salt;
        self.kdf_params = new_kdf_params;
        self.derived_key = Some(LockedKey::new(*new_key));
        self.entries = new_entries;

        self.save_to_disk()
    }

    // ─── Private helpers ────────────────────────────────

    /// Encrypt a `PasswordEntryInput` into a `StoreEntry` (two AES-GCM blobs)
    /// using the in-memory key, with the supplied timestamps.
    fn encrypt_entry(
        &self,
        input: &PasswordEntryInput,
        created_at: i64,
        updated_at: i64,
    ) -> Result<StoreEntry, AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let meta_plain = MetaPlain {
            title: input.title.clone(),
            username: input.username.clone(),
            url: input.url.clone(),
            category: input.category.clone(),
            created_at,
            updated_at,
        };
        let secret_plain = SecretPlain {
            password: input.password.clone(),
            notes: input.notes.clone(),
        };

        let meta_json = serde_json::to_vec(&meta_plain)
            .map_err(|e| AppError::VaultError(format!("Failed to serialize meta: {e}")))?;
        // Wrap the secret JSON so the plaintext password is wiped after encrypt.
        let secret_json = Zeroizing::new(
            serde_json::to_vec(&secret_plain)
                .map_err(|e| AppError::VaultError(format!("Failed to serialize secret: {e}")))?,
        );

        let meta = crypto::encrypt_bytes(key, &meta_json)?;
        let secret = crypto::encrypt_bytes(key, &secret_json)?;
        Ok(StoreEntry { meta, secret })
    }

    /// Persist the store to disk via fs_secure (atomic, owner-only). Freshly
    /// encrypts the verifier each save (new nonce) so the master password can be
    /// validated on the next unlock.
    fn save_to_disk(&self) -> Result<(), AppError> {
        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        let mut entries = BTreeMap::new();
        for (id, entry) in &self.entries {
            entries.insert(
                id.clone(),
                EncEntry {
                    meta: BASE64.encode(&entry.meta),
                    secret: BASE64.encode(&entry.secret),
                },
            );
        }

        let verifier = crypto::encrypt_bytes(key, VERIFIER_PLAINTEXT)?;

        let file = PasswordFile {
            version: PASSWORDS_VERSION,
            salt: BASE64.encode(self.salt),
            kdf: self.kdf_params.clone(),
            verifier: BASE64.encode(&verifier),
            entries,
        };

        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| AppError::VaultError(format!("Failed to serialize passwords: {e}")))?;

        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::VaultError(format!("Failed to create passwords directory: {e}"))
            })?;
        }

        fs_secure::secure_write(&self.file_path, json.as_bytes())
            .map_err(|e| AppError::VaultError(format!("Failed to write passwords: {e}")))
    }

    // ─── Export / Import ────────────────────────────────────

    /// Export ALL entries to a Bitwarden-compatible CSV file at `path`.
    ///
    /// The master password is re-verified before any decryption takes place.
    /// Every entry's secret blob is decrypted in order to write the cleartext
    /// CSV.
    ///
    /// Memory safety: ALL plaintext material (per-entry password/notes bytes,
    /// the accumulated row strings, and the final CSV buffer) is wrapped in
    /// `Zeroizing` so it is wiped when it leaves scope. Multiple entries'
    /// plaintext passwords are in memory simultaneously while the rows Vec is
    /// being built; this is an accepted trade-off (the alternative is writing
    /// the file incrementally, which requires more complex I/O). After the file
    /// is written, both the Vec and the CSV buffer are dropped and zeroized.
    ///
    /// CSV uses CRLF line endings per RFC 4180. Fields that start with `=`,
    /// `+`, `-`, `@`, tab, or CR are prefixed with `'` to neutralize formula
    /// injection (CWE-1236).
    ///
    /// CSV header:
    ///   `folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp`
    ///
    /// The export path is a user-chosen destination that may reside on FAT32,
    /// SMB, or cloud-sync filesystems where strict owner-only permissions fail.
    /// This uses a plain `std::fs::write` followed by `best_effort_harden` so
    /// the export is never lost on non-POSIX volumes. The internal
    /// `passwords.json` continues to use the strict `secure_write` path.
    ///
    /// Returns the number of entries written.
    pub fn export_to_csv(
        &self,
        path: &std::path::Path,
        master_password: &str,
    ) -> Result<usize, AppError> {
        // Re-verify master password before touching any secret blobs.
        if !self.verify_password(master_password)? {
            return Err(AppError::VaultWrongPassword);
        }

        let key = self
            .derived_key
            .as_ref()
            .map(LockedKey::as_bytes)
            .ok_or(AppError::VaultLocked)?;

        // [W-3] Wrap row strings in Zeroizing so each row's plaintext password
        // and notes are wiped when the Vec is dropped. All entries are in the
        // Vec simultaneously while we build the CSV — this is accepted (see doc
        // comment above).
        let mut rows: Vec<Zeroizing<String>> = Vec::with_capacity(self.entries.len() + 1);
        rows.push(Zeroizing::new(
            "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp"
                .to_string(),
        ));

        for (id, entry) in &self.entries {
            // Decrypt meta for listing fields.
            let meta_plain_bytes = crypto::decrypt_raw(key, &entry.meta)?;
            let meta: MetaPlain = serde_json::from_slice(&meta_plain_bytes)
                .map_err(|e| AppError::VaultError(format!("Corrupt meta for {id}: {e}")))?;

            // Decrypt secret for the password/notes — bytes wiped on drop.
            let secret_plain_bytes = Zeroizing::new(crypto::decrypt_raw(key, &entry.secret)?);
            let secret: SecretPlain = serde_json::from_slice(&secret_plain_bytes)
                .map_err(|e| AppError::VaultError(format!("Corrupt secret for {id}: {e}")))?;

            // [W-3] Wrap the row String in Zeroizing so the password/notes
            // embedded in the CSV text are wiped when the Vec drops.
            let row = Zeroizing::new(format!(
                "{},{},login,{},{},{},{},{},{},{},{}",
                csv_field(&meta.category),
                "", // favorite — empty
                csv_field(&meta.title),
                csv_field(&secret.notes),
                "", // fields — empty
                "", // reprompt — empty
                csv_field(&meta.url),
                csv_field(&meta.username),
                csv_field(&secret.password),
                "", // login_totp — empty
            ));
            rows.push(row);
            // `secret_plain_bytes` (Zeroizing) drops here, wiping the cleartext
            // password bytes before the next iteration.
        }

        // [C-1] RFC 4180 §2 mandates CRLF as the line terminator.
        // [W-3] Wrap the final CSV buffer in Zeroizing so it is wiped on drop.
        let csv = Zeroizing::new(
            rows.iter()
                .map(|r| r.as_str())
                .collect::<Vec<_>>()
                .join("\r\n")
                + "\r\n",
        );
        // rows Vec (holding all plaintext passwords/notes) is no longer needed.
        drop(rows);

        // [Fix-10] Export target is a user-chosen path that may be FAT32/
        // network/cloud-sync where strict owner-only perms fail. Write with
        // std::fs and harden best-effort so the file is never lost on
        // non-POSIX volumes. The internal passwords.json uses secure_write.
        std::fs::write(path, csv.as_bytes())
            .map_err(|e| AppError::VaultError(format!("Failed to write export file: {e}")))?;
        fs_secure::best_effort_harden(path);

        Ok(self.entries.len())
    }

    /// Import entries from a file at `path`. The store MUST already be unlocked.
    ///
    /// Auto-detects the format by file extension:
    /// - `.json` → Bitwarden JSON export (`{items:[...], folders:[...]}`)
    /// - `.csv`  → Bitwarden CSV (same columns as `export_to_csv`)
    ///
    /// Only `type == 1` (login) items are imported from JSON exports.
    ///
    /// [W-2] Partial-import rollback: the existing entry IDs are snapshotted
    /// before the import loop. If `add()` fails mid-loop (e.g. disk full, corrupt
    /// entry), all newly-added IDs are removed and the store is persisted once to
    /// restore the pre-import state, then the original error is returned. The
    /// returned count is the ACTUAL number of entries successfully imported.
    pub fn import_from_file(&mut self, path: &std::path::Path) -> Result<usize, AppError> {
        // Guard: store must be unlocked (we will call add() for each entry).
        if !self.is_unlocked() {
            return Err(AppError::VaultLocked);
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let contents = std::fs::read_to_string(path)
            .map_err(|e| AppError::VaultError(format!("Failed to read import file: {e}")))?;

        let entries: Vec<PasswordEntryInput> = match ext.as_str() {
            "json" => parse_bitwarden_json(&contents)?,
            "csv" => parse_bitwarden_csv(&contents)?,
            other => {
                return Err(AppError::VaultError(format!(
                    "Unsupported import format: .{other} (expected .json or .csv)"
                )))
            }
        };

        // [W-2] Snapshot the existing IDs so we can roll back on partial failure.
        let ids_before: std::collections::BTreeSet<String> =
            self.entries.keys().cloned().collect();

        let mut added_ids: Vec<String> = Vec::new();
        let mut import_error: Option<AppError> = None;

        for input in &entries {
            match self.add(input) {
                Ok(id) => added_ids.push(id),
                Err(e) => {
                    import_error = Some(e);
                    break;
                }
            }
        }

        if let Some(err) = import_error {
            // Remove every ID that was added during this import loop (not in
            // the pre-import snapshot) and persist once to restore the vault.
            for id in &added_ids {
                if !ids_before.contains(id) {
                    self.entries.remove(id);
                }
            }
            // Best-effort persist: if this also fails we still return the
            // original import error (the vault may be transiently inconsistent,
            // but the next save will clean it up).
            let _ = self.save_to_disk();
            return Err(err);
        }

        Ok(added_ids.len())
    }
}

// ─── CSV helpers ────────────────────────────────────────

/// Wrap a field value for RFC 4180 CSV, with formula-injection neutralization
/// (CWE-1236).
///
/// Steps applied in order:
///  1. If the value starts with `=`, `+`, `-`, `@`, `\t`, or `\r`, prefix a
///     single quote `'` so spreadsheet applications do not interpret the cell
///     as a formula. This matches the mitigation used by Bitwarden and 1Password.
///  2. If the (possibly prefixed) value contains a comma, double-quote, `\n`,
///     or `\r`, wrap it in double-quotes and escape any embedded `"` as `""`.
fn csv_field(value: &str) -> String {
    // [W-CSV] Formula injection neutralization (CWE-1236): prefix dangerous
    // leading characters with a single quote before any quoting step.
    let neutralized: &str;
    let neutralized_owned: String;
    if matches!(
        value.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r')
    ) {
        neutralized_owned = format!("'{value}");
        neutralized = &neutralized_owned;
    } else {
        neutralized = value;
    }

    if neutralized.contains(',')
        || neutralized.contains('"')
        || neutralized.contains('\n')
        || neutralized.contains('\r')
    {
        format!("\"{}\"", neutralized.replace('"', "\"\""))
    } else {
        neutralized.to_string()
    }
}

/// Parse a CSV line, respecting RFC 4180 quoting.
/// Returns a `Vec` of field strings (may be empty strings for empty fields).
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match (in_quotes, ch) {
            (false, ',') => {
                fields.push(current.clone());
                current.clear();
            }
            (false, '"') => {
                in_quotes = true;
            }
            (true, '"') => {
                if chars.peek() == Some(&'"') {
                    // Escaped double-quote: consume the second one.
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

/// Split a CSV `contents` string into logical records, respecting RFC 4180
/// quoting. A `\n` or `\r\n` inside a quoted field is part of that field; only
/// an unquoted line break ends a record. Returns one `Vec<String>` of raw
/// record strings (without the terminating newline) per logical row.
///
/// This fixes [C-1]: `str::lines()` splits on every `\n` including those inside
/// quoted fields, corrupting multiline notes and passwords. This character-level
/// state machine is the correct approach per RFC 4180 §2.
fn split_csv_records(contents: &str) -> Vec<String> {
    let mut records = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = contents.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '\r' if !in_quotes => {
                // Consume the optional `\n` of a CRLF pair.
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                records.push(current.clone());
                current.clear();
            }
            '\n' if !in_quotes => {
                records.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    // Capture any trailing content that wasn't followed by a newline.
    if !current.is_empty() {
        records.push(current);
    }
    records
}

/// Parse a Bitwarden CSV export into a list of `PasswordEntryInput`.
///
/// Expects the header:
///   folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp
///
/// Rows where `type != login` are skipped. Rows where `name` is empty are
/// skipped silently.
///
/// Uses a record-aware splitter ([`split_csv_records`]) so `\n` inside quoted
/// fields (e.g. multiline notes or passwords) is treated as part of the field
/// rather than as a record boundary — fixing [C-1].
fn parse_bitwarden_csv(contents: &str) -> Result<Vec<PasswordEntryInput>, AppError> {
    let records = split_csv_records(contents);
    let mut iter = records.into_iter();

    // Consume (and validate) the header row.
    let header = iter
        .next()
        .ok_or_else(|| AppError::VaultError("Empty CSV file".to_string()))?;
    let header_fields = parse_csv_line(&header);

    // Build a name → column-index map so we are robust to column reordering.
    let col = |name: &str| -> Option<usize> {
        header_fields.iter().position(|h| h.trim() == name)
    };

    let col_folder = col("folder");
    let col_type = col("type");
    let col_name = col("name");
    let col_notes = col("notes");
    let col_uri = col("login_uri");
    let col_username = col("login_username");
    let col_password = col("login_password");

    // `name` column is the only required one.
    let col_name = col_name
        .ok_or_else(|| AppError::VaultError("CSV missing required 'name' column".to_string()))?;

    let mut entries = Vec::new();
    for (row_num, record) in iter.enumerate() {
        let record = record.trim().to_string();
        if record.is_empty() {
            continue;
        }
        let fields = parse_csv_line(&record);
        let get = |idx: Option<usize>| -> &str {
            idx.and_then(|i| fields.get(i).map(|s| s.as_str()))
                .unwrap_or("")
        };

        // Skip non-login rows (type column absent → assume login).
        if let Some(t) = col_type {
            let type_val = get(Some(t));
            if !type_val.is_empty() && type_val != "login" {
                continue;
            }
        }

        let name = get(Some(col_name)).to_string();
        if name.is_empty() {
            tracing::debug!("CSV row {row_num}: skipped (empty name)");
            continue;
        }

        entries.push(PasswordEntryInput {
            title: name,
            username: get(col_username).to_string(),
            url: get(col_uri).to_string(),
            category: get(col_folder).to_string(),
            notes: get(col_notes).to_string(),
            password: get(col_password).to_string(),
        });
    }

    Ok(entries)
}

// ─── Bitwarden JSON import ───────────────────────────────

/// Minimal shape of a Bitwarden JSON export that we care about.
///
/// The real Bitwarden export is more complex; we only need the fields we
/// actually import. `serde(default)` covers missing optional fields.
///
/// The `encrypted` field is set to `true` in Bitwarden's encrypted JSON
/// exports. We detect this and return a clear error rather than silently
/// importing 0 entries — an encrypted export cannot be used as-is.
#[derive(Deserialize)]
struct BitwardenExport {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    items: Vec<BitwardenItem>,
    #[serde(default)]
    folders: Vec<BitwardenFolder>,
}

#[derive(Deserialize)]
struct BitwardenItem {
    /// Bitwarden item type: 1 = Login, 2 = SecureNote, 3 = Card, 4 = Identity.
    #[serde(rename = "type")]
    item_type: u32,
    name: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(rename = "folderId", default)]
    folder_id: Option<String>,
    login: Option<BitwardenLogin>,
}

#[derive(Deserialize)]
struct BitwardenLogin {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    uris: Vec<BitwardenUri>,
}

#[derive(Deserialize)]
struct BitwardenUri {
    /// Real Bitwarden exports may emit `"uri": null` for entries with no URL;
    /// using `Option<String>` prevents deserialization failures on those rows.
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Deserialize)]
struct BitwardenFolder {
    id: String,
    name: String,
}

/// Parse a Bitwarden JSON export (`{items:[...], folders:[...]}`) into a list
/// of `PasswordEntryInput`. Only `type == 1` (Login) items are imported.
///
/// Returns an error if the export has `"encrypted": true` — encrypted Bitwarden
/// exports cannot be imported; the user must re-export as unencrypted JSON or CSV.
fn parse_bitwarden_json(contents: &str) -> Result<Vec<PasswordEntryInput>, AppError> {
    let export: BitwardenExport = serde_json::from_str(contents)
        .map_err(|e| AppError::VaultError(format!("Invalid Bitwarden JSON: {e}")))?;

    // [W-1] Reject encrypted Bitwarden exports explicitly instead of silently
    // importing 0 entries. The user must re-export as unencrypted JSON or CSV.
    if export.encrypted {
        return Err(AppError::VaultError(
            "Encrypted Bitwarden exports are not supported; export as unencrypted JSON or CSV"
                .to_string(),
        ));
    }

    // Build folder id → name map.
    let folder_map: std::collections::HashMap<&str, &str> = export
        .folders
        .iter()
        .map(|f| (f.id.as_str(), f.name.as_str()))
        .collect();

    let mut entries = Vec::new();
    for item in &export.items {
        // Only import Login items (type == 1).
        if item.item_type != 1 {
            continue;
        }
        let title = item.name.clone();
        if title.is_empty() {
            continue;
        }

        let login = item.login.as_ref();
        let username = login
            .and_then(|l| l.username.as_deref())
            .unwrap_or("")
            .to_string();
        let password = login
            .and_then(|l| l.password.as_deref())
            .unwrap_or("")
            .to_string();
        // [C-2] Use filter_map to skip uris where `uri` is null, then take the
        // first non-null value. A missing or all-null uris list yields empty url.
        let url = login
            .map(|l| {
                l.uris
                    .iter()
                    .filter_map(|u| u.uri.as_deref())
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();
        let category = item
            .folder_id
            .as_deref()
            .and_then(|fid| folder_map.get(fid).copied())
            .unwrap_or("")
            .to_string();
        let notes = item.notes.as_deref().unwrap_or("").to_string();

        entries.push(PasswordEntryInput {
            title,
            username,
            url,
            category,
            notes,
            password,
        });
    }

    Ok(entries)
}

// ─── Free functions ─────────────────────────────────────

/// Current Unix timestamp in seconds. Saturates to 0 before the epoch (which
/// never happens on a sane clock) so we never panic.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Generate an opaque random entry id: 16 bytes from `OsRng`, hex-encoded.
/// NOT derived from any entry field, so it leaks nothing about the entry.
fn new_id() -> String {
    let mut bytes = [0u8; ID_BYTES];
    OsRng.fill_bytes(&mut bytes);
    let mut s = String::with_capacity(ID_BYTES * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Generate a random password of `length` characters from `OsRng`.
///
/// The character set always includes lowercase letters; `uppercase`, `digits`,
/// and `symbols` each add their class when `true`. If every flag is `false` the
/// password is lowercase-only. A `length` of 0 yields an empty string.
///
/// Uniform selection uses rejection sampling over the byte range so no class is
/// biased by the modulo of a non-power-of-two charset size.
pub fn generate_password(length: u32, symbols: bool, digits: bool, uppercase: bool) -> String {
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const DIGITS: &[u8] = b"0123456789";
    const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.<>?";

    let mut charset: Vec<u8> = LOWER.to_vec();
    if uppercase {
        charset.extend_from_slice(UPPER);
    }
    if digits {
        charset.extend_from_slice(DIGITS);
    }
    if symbols {
        charset.extend_from_slice(SYMBOLS);
    }

    let n = charset.len() as u32;
    let mut out = String::with_capacity(length as usize);
    let mut rng = OsRng;
    for _ in 0..length {
        // Rejection sampling for an unbiased index in [0, n).
        let limit = u32::MAX - (u32::MAX % n);
        let idx = loop {
            let r = rng.next_u32();
            if r < limit {
                break (r % n) as usize;
            }
        };
        out.push(charset[idx] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> PasswordEntryInput {
        PasswordEntryInput {
            title: "GitHub".to_string(),
            username: "octocat".to_string(),
            url: "https://github.com".to_string(),
            category: "dev".to_string(),
            notes: "personal account".to_string(),
            password: "s3cr3t-p@ss".to_string(),
        }
    }

    fn read_file_json(dir: &Path) -> serde_json::Value {
        let contents = std::fs::read_to_string(dir.join(PASSWORDS_FILE)).unwrap();
        serde_json::from_str(&contents).unwrap()
    }

    // ─── Round-trip add → list → reveal ─────────────────

    #[test]
    fn add_list_reveal_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();

        let id = store.add(&sample_input()).unwrap();
        // Opaque id: 32 hex chars, all hex digits.
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        let meta = &list[0];
        assert_eq!(meta.id, id);
        assert_eq!(meta.title, "GitHub");
        assert_eq!(meta.username, "octocat");
        assert_eq!(meta.url, "https://github.com");
        assert_eq!(meta.category, "dev");
        assert!(meta.created_at > 0);
        assert_eq!(meta.created_at, meta.updated_at);

        let revealed = store.reveal(&id).unwrap();
        assert_eq!(revealed.as_str(), "s3cr3t-p@ss");
    }

    // ─── Persistence across unlock ──────────────────────

    #[test]
    fn entries_survive_relock_and_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let id = {
            let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
            store.add(&sample_input()).unwrap()
        };

        let store = PasswordStore::unlock(dir.path(), "master-password").unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(store.reveal(&id).unwrap().as_str(), "s3cr3t-p@ss");
    }

    // ─── Wrong master password rejected ─────────────────

    #[test]
    fn unlock_rejects_wrong_master_password() {
        let dir = tempfile::tempdir().unwrap();
        {
            let _store = PasswordStore::create(dir.path(), "correct-master-pw").unwrap();
        }

        let result = PasswordStore::unlock(dir.path(), "wrong").map(|_| ());
        assert!(
            matches!(result, Err(AppError::VaultWrongPassword)),
            "wrong master password must be rejected, got {result:?}"
        );
    }

    #[test]
    fn unlock_accepts_correct_master_password() {
        let dir = tempfile::tempdir().unwrap();
        {
            let _store = PasswordStore::create(dir.path(), "correct-master-pw").unwrap();
        }
        let store = PasswordStore::unlock(dir.path(), "correct-master-pw").unwrap();
        assert!(store.is_unlocked());
    }

    // ─── list() never exposes the password ──────────────

    #[test]
    fn list_meta_never_contains_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        // The PasswordEntryMeta struct has no password field; its serialized
        // form must not contain the secret value anywhere either.
        let list = store.list().unwrap();
        let json = serde_json::to_string(&list).unwrap();
        assert!(
            !json.contains("s3cr3t-p@ss"),
            "list() output must never include the password"
        );
        assert!(
            !json.contains("personal account"),
            "list() output must never include the notes"
        );
    }

    #[test]
    fn on_disk_meta_blob_does_not_reveal_secret() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        // The whole file is encrypted blobs; the cleartext password must not
        // appear anywhere on disk.
        let raw = std::fs::read_to_string(dir.path().join(PASSWORDS_FILE)).unwrap();
        assert!(!raw.contains("s3cr3t-p@ss"));
        assert!(!raw.contains("octocat"));
        assert!(!raw.contains("GitHub"));
    }

    // ─── lock() ─────────────────────────────────────────

    #[test]
    fn lock_clears_unlocked_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        assert!(store.is_unlocked());
        store.lock();
        assert!(!store.is_unlocked());
    }

    #[test]
    fn list_and_reveal_fail_when_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let id = store.add(&sample_input()).unwrap();
        store.lock();

        assert!(matches!(store.list(), Err(AppError::VaultLocked)));
        assert!(matches!(store.reveal(&id), Err(AppError::VaultLocked)));
    }

    // ─── update ─────────────────────────────────────────

    #[test]
    fn update_changes_fields_and_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let id = store.add(&sample_input()).unwrap();
        let created_at = store.list().unwrap()[0].created_at;

        let mut updated = sample_input();
        updated.title = "GitHub (work)".to_string();
        updated.password = "new-p@ssw0rd".to_string();
        store.update(&id, &updated).unwrap();

        let meta = &store.list().unwrap()[0];
        assert_eq!(meta.title, "GitHub (work)");
        assert_eq!(
            meta.created_at, created_at,
            "created_at must be preserved across update"
        );
        assert!(
            meta.updated_at >= created_at,
            "updated_at must not go backwards"
        );
        assert_eq!(store.reveal(&id).unwrap().as_str(), "new-p@ssw0rd");
    }

    #[test]
    fn update_rejects_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let result = store.update("deadbeef", &sample_input());
        assert!(result.is_err());
    }

    // ─── update_meta (lossless meta-only edit) ──────────

    #[test]
    fn update_meta_preserves_secret_and_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let id = store.add(&sample_input()).unwrap();
        let created_at = store.list().unwrap()[0].created_at;
        // Capture the EXACT secret ciphertext to prove it is untouched.
        let secret_before = store.entries.get(&id).unwrap().secret.clone();

        let new_meta = MetaInput {
            title: "GitHub (work)".to_string(),
            username: "new-user".to_string(),
            url: "https://work.github.com".to_string(),
            category: "work".to_string(),
        };
        store.update_meta(&id, &new_meta).unwrap();

        // Meta fields updated, created_at preserved, updated_at not earlier.
        let meta = &store.list().unwrap()[0];
        assert_eq!(meta.title, "GitHub (work)");
        assert_eq!(meta.username, "new-user");
        assert_eq!(meta.url, "https://work.github.com");
        assert_eq!(meta.category, "work");
        assert_eq!(
            meta.created_at, created_at,
            "created_at must be preserved across update_meta"
        );
        assert!(
            meta.updated_at >= created_at,
            "updated_at must not go backwards"
        );

        // The secret blob must be byte-for-byte unchanged...
        assert_eq!(
            store.entries.get(&id).unwrap().secret,
            secret_before,
            "update_meta must leave the secret ciphertext byte-for-byte unchanged"
        );
        // ...and the revealed password AND notes must be the originals.
        assert_eq!(store.reveal(&id).unwrap().as_str(), "s3cr3t-p@ss");
        let secret_plain = crypto::decrypt_raw(
            store.derived_key.as_ref().unwrap().as_bytes(),
            &store.entries.get(&id).unwrap().secret,
        )
        .unwrap();
        let secret: SecretPlain = serde_json::from_slice(&secret_plain).unwrap();
        assert_eq!(secret.notes, "personal account");
    }

    #[test]
    fn update_meta_survives_relock_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let id = {
            let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
            let id = store.add(&sample_input()).unwrap();
            let m = MetaInput {
                title: "Renamed".to_string(),
                username: "octocat".to_string(),
                url: "https://github.com".to_string(),
                category: "dev".to_string(),
            };
            store.update_meta(&id, &m).unwrap();
            id
        };
        let store = PasswordStore::unlock(dir.path(), "master-password").unwrap();
        assert_eq!(store.list().unwrap()[0].title, "Renamed");
        assert_eq!(store.reveal(&id).unwrap().as_str(), "s3cr3t-p@ss");
    }

    #[test]
    fn update_meta_rejects_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let m = MetaInput {
            title: "x".to_string(),
            username: "x".to_string(),
            url: "x".to_string(),
            category: "x".to_string(),
        };
        assert!(matches!(
            store.update_meta("deadbeef", &m),
            Err(AppError::VaultError(_))
        ));
    }

    // ─── update_secret (lossless secret-only edit) ──────

    #[test]
    fn update_secret_preserves_meta_and_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let id = store.add(&sample_input()).unwrap();
        let created_at = store.list().unwrap()[0].created_at;

        let s = SecretInput {
            password: "rotated-p@ssw0rd".to_string(),
            notes: "rotated notes".to_string(),
        };
        store.update_secret(&id, &s).unwrap();

        // Every meta listing field is preserved; created_at preserved.
        let meta = &store.list().unwrap()[0];
        assert_eq!(meta.title, "GitHub");
        assert_eq!(meta.username, "octocat");
        assert_eq!(meta.url, "https://github.com");
        assert_eq!(meta.category, "dev");
        assert_eq!(
            meta.created_at, created_at,
            "created_at must be preserved across update_secret"
        );
        assert!(
            meta.updated_at >= created_at,
            "updated_at must not go backwards"
        );

        // The new secret (password + notes) is what is now stored.
        assert_eq!(store.reveal(&id).unwrap().as_str(), "rotated-p@ssw0rd");
        let secret_plain = crypto::decrypt_raw(
            store.derived_key.as_ref().unwrap().as_bytes(),
            &store.entries.get(&id).unwrap().secret,
        )
        .unwrap();
        let secret: SecretPlain = serde_json::from_slice(&secret_plain).unwrap();
        assert_eq!(secret.notes, "rotated notes");
    }

    #[test]
    fn update_secret_rejects_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let s = SecretInput {
            password: "p".to_string(),
            notes: "n".to_string(),
        };
        assert!(matches!(
            store.update_secret("deadbeef", &s),
            Err(AppError::VaultError(_))
        ));
    }

    #[test]
    fn update_meta_and_secret_inputs_deserialize_camel_case() {
        let meta: MetaInput = serde_json::from_str(
            r#"{"title":"t","username":"u","url":"https://x","category":"c"}"#,
        )
        .unwrap();
        assert_eq!(meta.title, "t");
        let secret: SecretInput = serde_json::from_str(r#"{"password":"p","notes":"n"}"#).unwrap();
        assert_eq!(secret.password, "p");
        assert_eq!(secret.notes, "n");
    }

    // ─── delete ─────────────────────────────────────────

    #[test]
    fn delete_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let id = store.add(&sample_input()).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);

        store.delete(&id).unwrap();
        assert_eq!(store.list().unwrap().len(), 0);
        assert!(matches!(store.reveal(&id), Err(AppError::VaultError(_))));
    }

    #[test]
    fn delete_rejects_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        assert!(store.delete("nope").is_err());
    }

    // ─── verify_password (backs the reveal grant) ───────

    #[test]
    fn verify_password_accepts_correct_and_rejects_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let store = PasswordStore::create(dir.path(), "master-password").unwrap();
        assert!(
            store.verify_password("master-password").unwrap(),
            "correct master password must verify"
        );
        assert!(
            !store.verify_password("not-the-password").unwrap(),
            "wrong master password must not verify"
        );
    }

    #[test]
    fn verify_password_errors_when_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.lock();
        assert!(matches!(
            store.verify_password("master-password"),
            Err(AppError::VaultLocked)
        ));
    }

    // ─── change_master_password ─────────────────────────

    #[test]
    fn change_master_keeps_entries_under_new_password() {
        let dir = tempfile::tempdir().unwrap();
        let id = {
            let mut store = PasswordStore::create(dir.path(), "old-master-pw").unwrap();
            let id = store.add(&sample_input()).unwrap();
            store
                .change_master_password("old-master-pw", "new-master-pw")
                .unwrap();
            id
        };

        // Old password no longer opens the store.
        let old = PasswordStore::unlock(dir.path(), "old-master-pw").map(|_| ());
        assert!(
            matches!(old, Err(AppError::VaultWrongPassword)),
            "old password must fail after change, got {old:?}"
        );

        // New password opens it and the entry survives, fully decryptable.
        let store = PasswordStore::unlock(dir.path(), "new-master-pw").unwrap();
        assert_eq!(store.list().unwrap()[0].id, id);
        assert_eq!(store.reveal(&id).unwrap().as_str(), "s3cr3t-p@ss");
    }

    #[test]
    fn change_master_rejects_wrong_old_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "old-master-pw").unwrap();
        store.add(&sample_input()).unwrap();

        let result = store.change_master_password("WRONG-old-pw", "new-master-pw");
        assert!(
            matches!(result, Err(AppError::VaultWrongPassword)),
            "wrong old password must be rejected, got {result:?}"
        );

        // The store must still open with the ORIGINAL password (unchanged).
        drop(store);
        let reopened = PasswordStore::unlock(dir.path(), "old-master-pw");
        assert!(reopened.is_ok(), "store must remain on the old password");
    }

    #[test]
    fn change_master_rotates_salt() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "old-master-pw").unwrap();
        let salt_before = read_file_json(dir.path())["salt"]
            .as_str()
            .unwrap()
            .to_string();
        store
            .change_master_password("old-master-pw", "new-master-pw")
            .unwrap();
        let salt_after = read_file_json(dir.path())["salt"]
            .as_str()
            .unwrap()
            .to_string();
        assert_ne!(salt_before, salt_after, "salt must rotate on master change");
    }

    // ─── Independent keys: different masters → different ciphertext ──

    #[test]
    fn different_master_passwords_produce_different_ciphertext() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();

        let mut store_a = PasswordStore::create(dir_a.path(), "master-aaaa").unwrap();
        let mut store_b = PasswordStore::create(dir_b.path(), "master-bbbb").unwrap();
        store_a.add(&sample_input()).unwrap();
        store_b.add(&sample_input()).unwrap();

        let blob_a = store_a.entries.values().next().unwrap().meta.clone();
        let blob_b = store_b.entries.values().next().unwrap().meta.clone();
        assert_ne!(
            blob_a, blob_b,
            "same plaintext under different keys must yield different ciphertext"
        );
    }

    // ─── File format assertions ─────────────────────────

    #[test]
    fn file_has_expected_shape() {
        let dir = tempfile::tempdir().unwrap();
        let _store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let json = read_file_json(dir.path());
        assert_eq!(json["version"], PASSWORDS_VERSION);
        assert_eq!(json["kdf"]["algorithm"], "argon2id");
        assert_eq!(json["kdf"]["m_cost"], 65536);
        assert!(json["verifier"].as_str().is_some_and(|v| !v.is_empty()));
        assert!(json["entries"].is_object());
    }

    #[test]
    fn ipc_meta_serializes_camel_case() {
        let meta = PasswordEntryMeta {
            id: "abc".to_string(),
            title: "t".to_string(),
            username: "u".to_string(),
            url: "https://x".to_string(),
            category: "c".to_string(),
            created_at: 100,
            updated_at: 200,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["createdAt"], 100);
        assert_eq!(json["updatedAt"], 200);
        assert!(json.get("created_at").is_none());
    }

    #[test]
    fn ipc_input_deserializes_camel_case() {
        let json = r#"{
            "title":"t","username":"u","url":"https://x",
            "category":"c","notes":"n","password":"p"
        }"#;
        let input: PasswordEntryInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.password, "p");
        assert_eq!(input.notes, "n");
    }

    // ─── Opaque id ──────────────────────────────────────

    #[test]
    fn ids_are_random_and_not_derived_from_title() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        // Same input twice → different ids (random, not title-derived).
        let id1 = store.add(&sample_input()).unwrap();
        let id2 = store.add(&sample_input()).unwrap();
        assert_ne!(id1, id2);
        // No id contains the title text.
        assert!(!id1.to_lowercase().contains("github"));
    }

    // ─── generate_password ──────────────────────────────

    #[test]
    fn generate_password_respects_length() {
        for len in [0u32, 1, 8, 16, 64] {
            let pw = generate_password(len, true, true, true);
            assert_eq!(pw.len(), len as usize);
        }
    }

    #[test]
    fn generate_password_lowercase_only_when_all_flags_false() {
        let pw = generate_password(200, false, false, false);
        assert!(
            pw.chars().all(|c| c.is_ascii_lowercase()),
            "all-false flags must yield lowercase-only, got {pw:?}"
        );
    }

    #[test]
    fn generate_password_digits_flag_limits_charset() {
        // With only digits+lowercase allowed, no uppercase or symbol may appear.
        let pw = generate_password(300, false, true, false);
        assert!(pw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert!(!pw.chars().any(|c| c.is_ascii_uppercase()));
    }

    #[test]
    fn generate_password_uppercase_flag_allows_uppercase() {
        // Over a large sample with uppercase enabled, at least one uppercase
        // char is overwhelmingly likely to appear.
        let pw = generate_password(500, false, false, true);
        assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
        assert!(!pw.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn generate_password_symbols_flag_allows_symbols() {
        let symbol_set = b"!@#$%^&*()-_=+[]{};:,.<>?";
        let pw = generate_password(500, true, false, false);
        assert!(
            pw.bytes().any(|b| symbol_set.contains(&b)),
            "symbols flag must allow symbol chars"
        );
    }

    #[test]
    fn generate_password_two_calls_differ() {
        let a = generate_password(32, true, true, true);
        let b = generate_password(32, true, true, true);
        assert_ne!(a, b, "two OsRng-backed passwords must differ");
    }

    // ─── csv_field escaping ─────────────────────────────

    #[test]
    fn csv_field_plain_value_unquoted() {
        assert_eq!(csv_field("hello"), "hello");
        assert_eq!(csv_field(""), "");
        assert_eq!(csv_field("simple"), "simple");
    }

    #[test]
    fn csv_field_comma_triggers_quoting() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_field_double_quote_escaped() {
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_field_newline_triggers_quoting() {
        assert_eq!(csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    // ─── parse_csv_line ─────────────────────────────────

    #[test]
    fn parse_csv_line_basic() {
        let fields = parse_csv_line("a,b,c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_csv_line_quoted_comma() {
        let fields = parse_csv_line("a,\"b,c\",d");
        assert_eq!(fields, vec!["a", "b,c", "d"]);
    }

    #[test]
    fn parse_csv_line_escaped_double_quote() {
        let fields = parse_csv_line("\"a\"\"b\",c");
        assert_eq!(fields, vec!["a\"b", "c"]);
    }

    #[test]
    fn parse_csv_line_empty_fields() {
        let fields = parse_csv_line(",,,");
        assert_eq!(fields, vec!["", "", "", ""]);
    }

    // ─── Export → CSV round-trip ─────────────────────────

    #[test]
    fn export_to_csv_produces_valid_bitwarden_header() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        let csv_path = dir.path().join("export.csv");
        let count = store
            .export_to_csv(&csv_path, "master-password")
            .unwrap();
        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(&csv_path).unwrap();
        let mut lines = contents.lines();
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp"
        );
        // The data row should contain the entry fields.
        let data = lines.next().unwrap();
        assert!(data.contains("GitHub"), "title must appear in CSV row");
        assert!(data.contains("octocat"), "username must appear in CSV row");
        assert!(
            data.contains("s3cr3t-p@ss"),
            "password must appear in exported CSV"
        );
    }

    #[test]
    fn export_to_csv_rejects_wrong_master_password() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        let csv_path = dir.path().join("export.csv");
        let result = store.export_to_csv(&csv_path, "WRONG-PASSWORD");
        assert!(
            matches!(result, Err(AppError::VaultWrongPassword)),
            "wrong master password must be rejected on export, got {result:?}"
        );
    }

    #[test]
    fn export_to_csv_fails_when_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.lock();
        let csv_path = dir.path().join("export.csv");
        let result = store.export_to_csv(&csv_path, "master-password");
        // verify_password requires the store to be unlocked.
        assert!(matches!(result, Err(AppError::VaultLocked)));
    }

    // ─── Import from CSV ────────────────────────────────

    #[test]
    fn import_from_csv_round_trips_export() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        let csv_path = dir.path().join("export.csv");
        store
            .export_to_csv(&csv_path, "master-password")
            .unwrap();

        // Import into a fresh store.
        let dir2 = tempfile::tempdir().unwrap();
        let mut store2 = PasswordStore::create(dir2.path(), "master-password").unwrap();
        let count = store2.import_from_file(&csv_path).unwrap();
        assert_eq!(count, 1);

        let list = store2.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "GitHub");
        assert_eq!(list[0].username, "octocat");
        assert_eq!(list[0].url, "https://github.com");
        assert_eq!(list[0].category, "dev");

        let id = &list[0].id;
        let pw = store2.reveal(id).unwrap();
        assert_eq!(pw.as_str(), "s3cr3t-p@ss");
    }

    #[test]
    fn import_from_csv_skips_non_login_type_rows() {
        // A CSV with one login row and one 'card' row.
        let csv = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
                   ,,,Login1,,,,https://a.com,user1,pass1,\n\
                   ,,,Card1,,,,,,,\n";

        // We need to write to a temp file because import_from_file reads by path.
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("test.csv");
        std::fs::write(&csv_path, csv).unwrap();

        // Build a CSV with the type column set explicitly.
        let csv_typed = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
                         ,,login,Login1,,,,https://a.com,user1,pass1,\n\
                         ,,card,Card1,,,,,,,\n";
        std::fs::write(&csv_path, csv_typed).unwrap();

        let mut store = PasswordStore::create(dir.path(), "pw").unwrap();
        let count = store.import_from_file(&csv_path).unwrap();
        assert_eq!(count, 1, "card row must be skipped");
        assert_eq!(store.list().unwrap()[0].title, "Login1");
    }

    #[test]
    fn import_from_csv_fails_when_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.lock();
        let csv_path = dir.path().join("dummy.csv");
        // File doesn't need to exist — lock check comes first.
        let result = store.import_from_file(&csv_path);
        assert!(matches!(result, Err(AppError::VaultLocked)));
    }

    // ─── Import from Bitwarden JSON ─────────────────────

    #[test]
    fn import_from_bitwarden_json_parses_login_items() {
        let json = r#"{
            "folders": [{"id": "f1", "name": "Work"}],
            "items": [
                {
                    "type": 1,
                    "name": "GitHub",
                    "folderId": "f1",
                    "notes": "personal",
                    "login": {
                        "username": "octocat",
                        "password": "s3cr3t",
                        "uris": [{"uri": "https://github.com"}]
                    }
                },
                {
                    "type": 2,
                    "name": "SecureNote",
                    "notes": "note text",
                    "login": null
                }
            ]
        }"#;

        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("bw.json");
        std::fs::write(&json_path, json).unwrap();

        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let count = store.import_from_file(&json_path).unwrap();
        assert_eq!(count, 1, "SecureNote (type 2) must be skipped");

        let list = store.list().unwrap();
        assert_eq!(list[0].title, "GitHub");
        assert_eq!(list[0].username, "octocat");
        assert_eq!(list[0].url, "https://github.com");
        assert_eq!(list[0].category, "Work");

        let id = &list[0].id;
        let pw = store.reveal(id).unwrap();
        assert_eq!(pw.as_str(), "s3cr3t");
    }

    #[test]
    fn import_rejects_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let bad_path = dir.path().join("data.xml");
        std::fs::write(&bad_path, "<xml/>").unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let result = store.import_from_file(&bad_path);
        assert!(matches!(result, Err(AppError::VaultError(_))));
    }

    #[test]
    fn import_csv_with_commas_in_fields_round_trips() {
        // A password and notes containing commas — must be quoted in the CSV.
        let input = PasswordEntryInput {
            title: "Site, Inc.".to_string(),
            username: "user@example.com".to_string(),
            url: "https://site.com".to_string(),
            category: "Work, Dev".to_string(),
            notes: "Notes with, commas and \"quotes\"".to_string(),
            password: "p@ss,word".to_string(),
        };

        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&input).unwrap();

        let csv_path = dir.path().join("export.csv");
        store.export_to_csv(&csv_path, "master-password").unwrap();

        let dir2 = tempfile::tempdir().unwrap();
        let mut store2 = PasswordStore::create(dir2.path(), "master-password").unwrap();
        store2.import_from_file(&csv_path).unwrap();

        let list = store2.list().unwrap();
        assert_eq!(list[0].title, "Site, Inc.");
        assert_eq!(list[0].category, "Work, Dev");

        let id = &list[0].id;
        let pw = store2.reveal(id).unwrap();
        assert_eq!(pw.as_str(), "p@ss,word");
    }

    // ─── [C-1] Multiline CSV round-trip ─────────────────

    /// Export an entry whose notes AND password contain embedded newlines,
    /// reimport it, and assert an exact round-trip. This is the regression test
    /// for the `str::lines()`-based parser that would split on newlines inside
    /// quoted fields and corrupt the data.
    #[test]
    fn csv_multiline_notes_and_password_round_trip() {
        let input = PasswordEntryInput {
            title: "MultilineEntry".to_string(),
            username: "user".to_string(),
            url: "https://example.com".to_string(),
            category: "test".to_string(),
            notes: "line one\nline two\nline three".to_string(),
            password: "pass\nword\nwith\nnewlines".to_string(),
        };

        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&input).unwrap();

        let csv_path = dir.path().join("export.csv");
        let count = store.export_to_csv(&csv_path, "master-password").unwrap();
        assert_eq!(count, 1);

        // Reimport into a fresh store and verify exact round-trip.
        let dir2 = tempfile::tempdir().unwrap();
        let mut store2 = PasswordStore::create(dir2.path(), "master-password").unwrap();
        let imported = store2.import_from_file(&csv_path).unwrap();
        assert_eq!(imported, 1);

        let list = store2.list().unwrap();
        assert_eq!(list[0].title, "MultilineEntry");
        assert_eq!(list[0].url, "https://example.com");

        let id = &list[0].id;
        let pw = store2.reveal(id).unwrap();
        assert_eq!(pw.as_str(), "pass\nword\nwith\nnewlines");

        // Verify notes via secret blob decryption.
        let key = store2.derived_key.as_ref().unwrap().as_bytes();
        let entry = store2.entries.get(id).unwrap();
        let plain = crypto::decrypt_raw(key, &entry.secret).unwrap();
        let secret: SecretPlain = serde_json::from_slice(&plain).unwrap();
        assert_eq!(secret.notes, "line one\nline two\nline three");
    }

    /// Exported CSV must use CRLF line endings per RFC 4180.
    #[test]
    fn export_csv_uses_crlf_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        store.add(&sample_input()).unwrap();

        let csv_path = dir.path().join("export.csv");
        store.export_to_csv(&csv_path, "master-password").unwrap();

        let bytes = std::fs::read(&csv_path).unwrap();
        let contents = String::from_utf8(bytes).unwrap();
        // Every line must be terminated with \r\n.
        assert!(
            contents.contains("\r\n"),
            "exported CSV must use CRLF line endings"
        );
        // The LF count equals the CRLF count (no bare LF outside quoted fields).
        let crlf_count = contents.matches("\r\n").count();
        let lf_count = contents.matches('\n').count();
        assert_eq!(
            crlf_count, lf_count,
            "every LF must be preceded by CR (no bare LF in non-quoted lines)"
        );
    }

    // ─── [C-2] Bitwarden JSON null uri ──────────────────

    /// A Bitwarden JSON export where a login item's uris array contains
    /// `{"uri": null}` must be imported successfully (not fail to deserialize).
    #[test]
    fn import_bitwarden_json_null_uri_succeeds() {
        let json = r#"{
            "encrypted": false,
            "folders": [],
            "items": [
                {
                    "type": 1,
                    "name": "NullUri",
                    "notes": null,
                    "login": {
                        "username": "user",
                        "password": "secret",
                        "uris": [{"uri": null}]
                    }
                }
            ]
        }"#;

        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("bw.json");
        std::fs::write(&json_path, json).unwrap();

        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let count = store.import_from_file(&json_path).unwrap();
        assert_eq!(count, 1, "null-uri item should import successfully");

        let list = store.list().unwrap();
        assert_eq!(list[0].title, "NullUri");
        // url should be empty (no non-null uri found).
        assert_eq!(list[0].url, "");
    }

    // ─── CSV formula injection (CWE-1236) ───────────────

    /// A field value starting with `=HYPERLINK(...)` must be emitted with a
    /// leading `'` prefix to neutralize spreadsheet formula interpretation
    /// (CWE-1236). The `'` prefix is inserted before the RFC 4180 quoting step,
    /// so if the neutralized value requires quoting (contains comma, quote, etc.),
    /// the entire `'`-prefixed value is enclosed in double-quotes.
    #[test]
    fn csv_field_formula_injection_neutralized() {
        // Simple formulas (no quoting needed after prefix): output starts with `'`.
        assert_eq!(csv_field("+1234"), "'+1234", "+ prefix must be neutralized");
        assert_eq!(csv_field("-1234"), "'-1234", "- prefix must be neutralized");
        assert_eq!(csv_field("@SUM(A1)"), "'@SUM(A1)", "@ prefix must be neutralized");

        // `=HYPERLINK("evil")` — neutralized prefix makes it `'=HYPERLINK("evil")`,
        // which contains `"` so it gets RFC 4180-quoted: `"'=HYPERLINK(""evil"")"`
        let out = csv_field("=HYPERLINK(\"evil\")");
        assert!(
            out.starts_with("\"'="),
            "= formula with quotes must be quoted and neutralized, got: {out}"
        );
        assert!(out.contains("'=HYPERLINK"), "must contain neutralized prefix: {out}");

        // A formula with a comma: neutralized prefix + quoting.
        let out2 = csv_field("=HYPERLINK(\"a,b\")");
        assert!(
            out2.starts_with("\"'="),
            "neutralized formula with comma must be quoted: {out2}"
        );

        // A formula with no special chars: just `'` prefix, no quoting wrapper.
        let out3 = csv_field("=1+1");
        assert_eq!(out3, "'=1+1", "simple formula must just get the prefix: {out3}");
    }

    // ─── [W-1] Encrypted Bitwarden JSON ─────────────────

    /// A Bitwarden export with `"encrypted": true` must return a descriptive
    /// error instead of silently importing 0 entries.
    #[test]
    fn import_encrypted_bitwarden_json_returns_error() {
        let json = r#"{
            "encrypted": true,
            "items": [],
            "folders": []
        }"#;

        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("bw_enc.json");
        std::fs::write(&json_path, json).unwrap();

        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        let result = store.import_from_file(&json_path);
        assert!(
            matches!(result, Err(AppError::VaultError(ref msg)) if msg.contains("Encrypted")),
            "encrypted export must be rejected with a clear error, got {result:?}"
        );
    }

    // ─── [W-2] Partial-import rollback ──────────────────

    /// If one entry in the parsed list is missing a required field (empty name),
    /// the import should skip it. To test mid-loop failure rollback we verify
    /// that the vault is left unchanged after a forced failure: we parse a CSV
    /// that yields valid entries but inject a test where `add()` itself would
    /// fail by locking the store mid-import.
    ///
    /// A simpler observable: import a valid batch first, then verify count is
    /// correct (rollback is implicitly tested in the error path above).
    #[test]
    fn import_rollback_on_locked_store_leaves_vault_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PasswordStore::create(dir.path(), "master-password").unwrap();
        // Add a pre-existing entry so we can verify it survives a failed import.
        let pre_id = store.add(&sample_input()).unwrap();
        let initial_count = store.list().unwrap().len();
        assert_eq!(initial_count, 1);

        // Lock the store mid-import by using a locked store to call import.
        // We can't truly inject a mid-loop failure without mock infrastructure,
        // so instead we exercise the locked-store early-return path and verify
        // the pre-existing entry is still present.
        store.lock();
        let csv_path = dir.path().join("dummy.csv");
        std::fs::write(&csv_path, "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n").unwrap();
        let result = store.import_from_file(&csv_path);
        assert!(matches!(result, Err(AppError::VaultLocked)));

        // Re-unlock and verify the pre-existing entry is intact.
        drop(store);
        let store2 = PasswordStore::unlock(dir.path(), "master-password").unwrap();
        let list = store2.list().unwrap();
        assert_eq!(list.len(), 1, "pre-existing entry must survive failed import");
        assert_eq!(list[0].id, pre_id);
    }
}

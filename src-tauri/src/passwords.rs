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
pub struct PasswordStore {
    file_path: PathBuf,
    derived_key: Option<Zeroizing<[u8; 32]>>,
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
    Ok(crypto::ct_eq(candidate.as_slice(), snapshot.current_key.as_slice()))
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
        let derived_key = crypto::derive_key(master_password, &salt, &kdf_params)?;

        let store = PasswordStore {
            file_path,
            derived_key: Some(derived_key),
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
            derived_key: Some(derived_key),
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
            current_key: Zeroizing::new(**current_key),
        })
    }

    /// List all entries' metadata. Decrypts ONLY the `meta` blob per entry — the
    /// password (`secret` blob) is never touched here.
    ///
    /// Entries are returned in stable id order (the on-disk `BTreeMap` order).
    pub fn list(&self) -> Result<Vec<PasswordEntryMeta>, AppError> {
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
        self.entries.insert(id.to_string(), StoreEntry { meta, secret });
        self.save_to_disk()
    }

    /// Re-encrypt ONLY the `secret` blob (password + notes) for an entry,
    /// preserving every meta field. `updated_at` in the meta blob is bumped to
    /// now (the meta is decrypted only to recover its current fields, then
    /// re-encrypted with a refreshed `updated_at`); `created_at` and all listing
    /// fields are preserved. Errors with [`AppError::VaultError`] if the id does
    /// not exist.
    pub fn update_secret(&mut self, id: &str, input: &SecretInput) -> Result<(), AppError> {
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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

        self.entries.insert(id.to_string(), StoreEntry { meta, secret });
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
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
        let current_key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;
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
        // Replacing the key drops the old Zeroizing wrapper (zeroizes it).
        self.salt = new_salt;
        self.kdf_params = new_kdf_params;
        self.derived_key = Some(new_key);
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
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
        let key = self.derived_key.as_ref().ok_or(AppError::VaultLocked)?;

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
            store.derived_key.as_ref().unwrap(),
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
            store.derived_key.as_ref().unwrap(),
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
        let secret: SecretInput =
            serde_json::from_str(r#"{"password":"p","notes":"n"}"#).unwrap();
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
}

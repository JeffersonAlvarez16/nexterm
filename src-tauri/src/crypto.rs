// crypto.rs — Shared, audited crypto primitives for the encrypted stores.
//
// This module centralizes the AES-256-GCM + Argon2id building blocks so that
// BOTH the SSH credential vault (`vault.rs`) and the separate password store
// (`passwords.rs`) derive keys and encrypt/decrypt identically. The logic here
// is the exact behavior previously inlined in `vault.rs` — extracted verbatim
// so the on-disk format and security properties are unchanged.
//
// On-disk blob shape for every ciphertext produced here:
//   nonce(12) + ciphertext + tag(16)
// The random nonce is prepended; AES-GCM appends the 16-byte auth tag.
//
// All items are `pub(crate)`: callers inside the crate share these primitives,
// but nothing crypto-related is exposed across the crate boundary.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::AppError;

/// AES-256-GCM nonce size in bytes.
pub(crate) const NONCE_SIZE: usize = 12;

/// AES-256-GCM authentication tag size in bytes.
pub(crate) const TAG_SIZE: usize = 16;

/// Salt size in bytes for Argon2id.
pub(crate) const SALT_SIZE: usize = 32;

// ─── KDF Parameters ─────────────────────────────────────

/// KDF parameters persisted with an encrypted store so future reads use the
/// exact settings the file was written with.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct KdfParams {
    pub algorithm: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

/// Default Argon2id parameters for newly created / migrated stores:
/// m_cost = 64 MiB, t_cost = 3, p_cost = 1.
pub(crate) fn default_kdf_params() -> KdfParams {
    KdfParams {
        algorithm: "argon2id".to_string(),
        m_cost: 65536,
        t_cost: 3,
        p_cost: 1,
    }
}

/// Build an Argon2id hasher from persisted KDF params. Shared by the vault,
/// the password store, and the profile-export path so they derive keys
/// identically.
pub(crate) fn argon2_from_params(p: &KdfParams) -> Result<Argon2<'static>, AppError> {
    if p.algorithm != "argon2id" {
        return Err(AppError::VaultError(format!(
            "Unsupported KDF algorithm: {}",
            p.algorithm
        )));
    }
    let params = Params::new(p.m_cost, p.t_cost, p.p_cost, None)
        .map_err(|e| AppError::VaultError(format!("Invalid KDF params: {e}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Derive a 32-byte key from password + salt using Argon2id with the supplied
/// KDF params. The key is wrapped in `Zeroizing` so it is wiped from memory on
/// drop.
pub(crate) fn derive_key(
    password: &str,
    salt: &[u8; SALT_SIZE],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; 32]>, AppError> {
    let mut key = Zeroizing::new([0u8; 32]);
    argon2_from_params(params)?
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| AppError::VaultError(format!("Key derivation failed: {e}")))?;
    Ok(key)
}

// ─── AEAD: AES-256-GCM (nonce-prepended) ────────────────

/// Encrypt raw bytes under `key` → nonce(12) + ciphertext + tag(16).
///
/// A fresh random nonce is generated on every call and prepended to the
/// output. The 32-byte `key` is the Argon2id-derived store key.
pub(crate) fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::VaultError(format!("Cipher init failed: {e}")))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::VaultError(format!("Encryption failed: {e}")))?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt nonce(12) + ciphertext + tag(16) under `key` → raw plaintext bytes.
///
/// An AEAD failure (wrong key or tampered ciphertext) maps to a generic
/// `VaultError("Decryption failed")` so the error text never reveals whether
/// the key or the data was the problem.
pub(crate) fn decrypt_raw(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, AppError> {
    if data.len() < NONCE_SIZE + TAG_SIZE {
        return Err(AppError::VaultError("Ciphertext too short".to_string()));
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::VaultError(format!("Cipher init failed: {e}")))?;

    let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
    let ciphertext = &data[NONCE_SIZE..];

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::VaultError("Decryption failed".to_string()))
}

/// Encrypt `plaintext` and verify it round-trips back to the same bytes under
/// `key`. This is the building block for the fixed-verifier scheme: a store
/// encrypts a known constant under the derived key at save time, then on the
/// next unlock decrypts it and compares — validating the master password
/// independently of how many entries the store holds.
///
/// Returns the verifier blob (nonce-prepended ciphertext) to persist.
// Consumed by the password store in step 2; the vault keeps its own inline
// verifier path for now. `allow(dead_code)` keeps the step-1 build warning-free.
#[allow(dead_code)]
pub(crate) fn encrypt_verifier(
    key: &[u8; 32],
    verifier_plaintext: &[u8],
) -> Result<Vec<u8>, AppError> {
    encrypt_bytes(key, verifier_plaintext)
}

/// Constant-time equality for two byte slices (dependency-free).
///
/// Returns `false` immediately if the lengths differ (length is not secret
/// here — both inputs are fixed-size 32-byte derived keys). For equal-length
/// inputs the comparison time depends only on the length, never on WHERE the
/// first differing byte is: every byte is XOR-ed and the differences are
/// OR-accumulated into a single byte, so no early exit leaks a match position.
/// The accumulator is collapsed to a bool with a single `== 0`, avoiding the
/// branch-on-first-mismatch that `==`/`!=` on slices performs.
///
/// This backs the master-key comparisons in `verify_password` and
/// `change_master_password`, where a timing side channel on the derived key
/// could otherwise leak information across repeated re-auth attempts.
pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Decrypt a verifier blob and confirm it matches the expected plaintext.
///
/// Returns `Ok(true)` when the decrypted bytes equal `expected`, `Ok(false)`
/// when they decrypt but differ (a corrupt/tampered verifier), and `Err` when
/// the blob is malformed or AEAD decryption fails (wrong password). Callers map
/// the failure to `AppError::VaultWrongPassword`.
// Consumed by the password store's unlock path in step 2.
#[allow(dead_code)]
pub(crate) fn verify(key: &[u8; 32], blob: &[u8], expected: &[u8]) -> Result<bool, AppError> {
    let plaintext = decrypt_raw(key, blob)?;
    Ok(plaintext == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let salt = [7u8; SALT_SIZE];
        let derived = derive_key("correct-horse", &salt, &default_kdf_params()).unwrap();
        *derived
    }

    #[test]
    fn encrypt_then_decrypt_roundtrips() {
        let key = test_key();
        let plaintext = b"super secret value";
        let blob = encrypt_bytes(&key, plaintext).unwrap();
        // nonce(12) + ciphertext + tag(16): always strictly larger than input.
        assert!(blob.len() >= NONCE_SIZE + TAG_SIZE + plaintext.len());
        let back = decrypt_raw(&key, &blob).unwrap();
        assert_eq!(back, plaintext);
    }

    #[test]
    fn encrypt_uses_fresh_nonce_each_call() {
        let key = test_key();
        let a = encrypt_bytes(&key, b"same").unwrap();
        let b = encrypt_bytes(&key, b"same").unwrap();
        // Random nonce ⇒ the two blobs (and their prepended nonces) must differ.
        assert_ne!(a, b);
        assert_ne!(a[..NONCE_SIZE], b[..NONCE_SIZE]);
    }

    #[test]
    fn decrypt_rejects_wrong_key() {
        let key = test_key();
        let blob = encrypt_bytes(&key, b"value").unwrap();
        let wrong = [0u8; 32];
        assert!(decrypt_raw(&wrong, &blob).is_err());
    }

    #[test]
    fn decrypt_rejects_truncated_blob() {
        let key = test_key();
        // Anything shorter than nonce + tag must be rejected before AEAD.
        let result = decrypt_raw(&key, &[0u8; NONCE_SIZE + TAG_SIZE - 1]);
        assert!(result.is_err());
    }

    #[test]
    fn verifier_roundtrips_and_detects_wrong_password() {
        let key = test_key();
        let blob = encrypt_verifier(&key, b"verifier-constant").unwrap();
        assert!(verify(&key, &blob, b"verifier-constant").unwrap());

        // A different key (wrong password) fails to decrypt the verifier.
        let wrong = [1u8; 32];
        assert!(verify(&wrong, &blob, b"verifier-constant").is_err());
    }

    #[test]
    fn ct_eq_matches_equal_slices() {
        assert!(ct_eq(&[0u8; 32], &[0u8; 32]));
        let a = [1u8, 2, 3, 4, 5];
        let b = [1u8, 2, 3, 4, 5];
        assert!(ct_eq(&a, &b));
    }

    #[test]
    fn ct_eq_rejects_differing_slices() {
        let a = [1u8, 2, 3, 4, 5];
        // Differ in the first byte (would be the earliest-exit case for `==`).
        let mut first = a;
        first[0] = 99;
        assert!(!ct_eq(&a, &first));
        // Differ in the last byte (latest-exit case).
        let mut last = a;
        last[4] = 99;
        assert!(!ct_eq(&a, &last));
    }

    #[test]
    fn ct_eq_rejects_different_lengths() {
        assert!(!ct_eq(&[0u8; 32], &[0u8; 31]));
        assert!(!ct_eq(&[], &[0u8]));
    }

    #[test]
    fn ct_eq_distinguishes_derived_keys() {
        // Real 32-byte keys: same password matches, different password does not.
        let salt = [9u8; SALT_SIZE];
        let p = default_kdf_params();
        let k1 = derive_key("right-password", &salt, &p).unwrap();
        let k2 = derive_key("right-password", &salt, &p).unwrap();
        let k3 = derive_key("wrong-password", &salt, &p).unwrap();
        assert!(ct_eq(k1.as_slice(), k2.as_slice()));
        assert!(!ct_eq(k1.as_slice(), k3.as_slice()));
    }

    #[test]
    fn argon2_rejects_non_argon2id_algorithm() {
        let params = KdfParams {
            algorithm: "scrypt".to_string(),
            m_cost: 65536,
            t_cost: 3,
            p_cost: 1,
        };
        assert!(argon2_from_params(&params).is_err());
    }

    #[test]
    fn default_params_match_expected_costs() {
        let p = default_kdf_params();
        assert_eq!(p.algorithm, "argon2id");
        assert_eq!(p.m_cost, 65536);
        assert_eq!(p.t_cost, 3);
        assert_eq!(p.p_cost, 1);
    }
}

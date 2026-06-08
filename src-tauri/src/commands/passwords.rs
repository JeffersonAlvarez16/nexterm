// commands/passwords.rs — Independent password-store Tauri commands.
//
// This is the COMMAND layer for the second, fully independent vault implemented
// in `crate::passwords`. It is deliberately separate from `commands/vault.rs`
// (the SSH credential vault): a different master password, a different on-disk
// file (`passwords.json`), a different in-memory slot (`state.passwords`), and a
// different auto-lock clock (`state.passwords_auto_lock`, 5-min default). SSH
// activity must never keep this store unlocked, and vice-versa.
//
// Command/IPC conventions (must match the frontend contract exactly):
// - Command names are snake_case (`pw_status`, `pw_unlock`, …).
// - Arguments arrive camelCase over IPC (Tauri's serde default), so a Rust arg
//   `master_password` is `masterPassword` on the wire.
// - Every command that reads or writes the store locks `state.passwords`, gates
//   on `unlocked` via `ok_or(AppError::VaultLocked)`, and records activity on
//   the password store's OWN auto-lock on success.
//
// SECURITY — the reveal path:
// - `pw_reveal` is the most dangerous command: it returns a plaintext password.
//   It requires a FRESH re-auth grant issued by `pw_reauth` (master password
//   re-entered), valid for `REVEAL_GRANT_TTL`. Without an active, unexpired
//   grant, `pw_reveal` errors with `AppError::RevealNotAuthorized` — it never
//   falls back to "unlocked is enough".
// - `pw_lock` and `pw_reset` clear the grant so a revealed-then-locked store
//   can never be revealed again without a new re-auth.

use std::time::{Duration, Instant};

use tauri::{Manager, State};

use crate::error::AppError;
use crate::passwords::{
    generate_password, verify_reauth_candidate, MetaInput, PasswordEntryInput, PasswordEntryMeta,
    PasswordStore, SecretInput,
};
use crate::state::AppState;

/// Minimum master-password length enforced at the command layer for the
/// password store. The library `PasswordStore::create` deliberately does NOT
/// enforce this (so unit tests can use short passwords); the policy lives here.
const MIN_MASTER_PASSWORD_LEN: usize = 12;

/// How long a fresh re-auth grant remains valid for `pw_reveal`, in seconds.
/// Short by design: revealing a password requires the master password to have
/// been re-entered within this window.
const REVEAL_GRANT_TTL_SECS: u64 = 30;

/// Status payload for the password store, mirroring the SSH vault's status but
/// scoped to the independent password store and its own auto-lock clock.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PwStatus {
    /// Whether `passwords.json` exists on disk.
    pub exists: bool,
    /// Whether the store is currently unlocked in memory.
    pub unlocked: bool,
    /// Configured idle auto-lock timeout in seconds (0 = auto-lock disabled).
    pub idle_timeout_secs: u64,
    /// Seconds remaining before idle auto-lock fires, or `null` when auto-lock
    /// is disabled (timeout 0). Only meaningful while unlocked.
    pub seconds_until_lock: Option<u64>,
}

/// Resolve the app data directory from the Tauri app handle.
fn get_app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|_| AppError::VaultError("Cannot determine app data directory".into()))
}

// ─── Lifecycle commands ─────────────────────────────────

#[tauri::command]
pub async fn pw_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<PwStatus, AppError> {
    let data_dir = get_app_data_dir(&app)?;
    let exists = PasswordStore::exists(&data_dir);

    let guard = state.passwords.lock().await;
    let unlocked = guard.as_ref().map(|s| s.is_unlocked()).unwrap_or(false);
    // Release the store mutex before reading the independent auto-lock state.
    drop(guard);

    Ok(PwStatus {
        exists,
        unlocked,
        idle_timeout_secs: state.passwords_auto_lock.idle_timeout_secs(),
        seconds_until_lock: if unlocked {
            state.passwords_auto_lock.seconds_until_lock()
        } else {
            None
        },
    })
}

#[tauri::command]
pub async fn pw_create(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    master_password: String,
) -> Result<(), AppError> {
    if master_password.len() < MIN_MASTER_PASSWORD_LEN {
        return Err(AppError::VaultError(format!(
            "Master password must be at least {MIN_MASTER_PASSWORD_LEN} characters"
        )));
    }

    let data_dir = get_app_data_dir(&app)?;
    if PasswordStore::exists(&data_dir) {
        return Err(AppError::VaultError("Password store already exists".into()));
    }

    // Argon2id derivation happens inside `create`. Run it on a blocking thread
    // so the expensive (memory-hard) KDF never holds the async `passwords`
    // mutex — otherwise all pw_* commands serialize behind it (local DoS).
    let store =
        tokio::task::spawn_blocking(move || PasswordStore::create(&data_dir, &master_password))
            .await
            .map_err(|e| {
                AppError::VaultError(format!("Password store creation task failed: {e}"))
            })??;

    // Only NOW take the lock — a trivial slot write, no derivation under it.
    let mut guard = state.passwords.lock().await;
    *guard = Some(store);
    drop(guard);

    // Creating leaves the store unlocked — start its idle clock fresh.
    state.passwords_auto_lock.record_activity();
    Ok(())
}

#[tauri::command]
pub async fn pw_unlock(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    master_password: String,
) -> Result<(), AppError> {
    let data_dir = get_app_data_dir(&app)?;

    // `unlock` reads the file and runs Argon2id internally. Do both on a
    // blocking thread so the memory-hard KDF never holds the async `passwords`
    // mutex (local DoS prevention).
    let store =
        tokio::task::spawn_blocking(move || PasswordStore::unlock(&data_dir, &master_password))
            .await
            .map_err(|e| {
                AppError::VaultError(format!("Password store unlock task failed: {e}"))
            })??;

    let mut guard = state.passwords.lock().await;
    *guard = Some(store);
    drop(guard);

    // Unlocking is activity — reset the idle clock so the store does not
    // immediately auto-lock against a stale `last_activity`.
    state.passwords_auto_lock.record_activity();
    Ok(())
}

#[tauri::command]
pub async fn pw_lock(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut guard = state.passwords.lock().await;
    if let Some(ref mut store) = *guard {
        store.lock(); // zeroize the derived key
    }
    *guard = None;
    drop(guard);

    // Locking MUST also clear any outstanding reveal grant.
    *state.pw_reveal_grant.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn pw_set_idle_timeout(state: State<'_, AppState>, secs: u64) -> Result<(), AppError> {
    state.passwords_auto_lock.set_idle_timeout_secs(secs);
    Ok(())
}

#[tauri::command]
pub async fn pw_reset(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let data_dir = get_app_data_dir(&app)?;

    // 1. Delete passwords.json (and any lingering atomic-write temp file).
    //    NEVER touch vault.json — the SSH vault is fully independent.
    let pw_path = data_dir.join("passwords.json");
    if pw_path.exists() {
        std::fs::remove_file(&pw_path)
            .map_err(|e| AppError::VaultError(format!("Failed to delete passwords file: {e}")))?;
    }
    let tmp_path = pw_path.with_extension("json.tmp");
    if tmp_path.exists() {
        let _ = std::fs::remove_file(&tmp_path);
    }

    // 2. Clear the in-memory store (zeroize key) and the reveal grant.
    let mut guard = state.passwords.lock().await;
    if let Some(ref mut store) = *guard {
        store.lock();
    }
    *guard = None;
    drop(guard);

    *state.pw_reveal_grant.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn pw_change_master(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), AppError> {
    if new_password.len() < MIN_MASTER_PASSWORD_LEN {
        return Err(AppError::VaultError(format!(
            "Master password must be at least {MIN_MASTER_PASSWORD_LEN} characters"
        )));
    }

    // `change_master_password` derives BOTH the old key (to validate) and a
    // fresh new key via Argon2id, then re-encrypts every blob. To keep that
    // memory-hard work off the async mutex (local DoS), take the store OUT of
    // the slot, run the rotation on a blocking thread, then put it back.
    let mut store = {
        let mut guard = state.passwords.lock().await;
        guard.take().ok_or(AppError::VaultLocked)?
    };

    let result = tokio::task::spawn_blocking(move || {
        let res = store.change_master_password(&old_password, &new_password);
        // Return the store regardless of outcome so the caller can reinstate it
        // (a wrong old password leaves the store fully intact on the old key).
        (store, res)
    })
    .await;

    let (store, res) = match result {
        Ok(pair) => pair,
        Err(e) => {
            // The blocking task panicked: the store moved into it and is lost.
            // The slot stays empty (locked); surface the failure.
            return Err(AppError::VaultError(format!(
                "Change-master task failed: {e}"
            )));
        }
    };

    // Reinstate the store (mutated on success, untouched on failure).
    let mut guard = state.passwords.lock().await;
    *guard = Some(store);
    drop(guard);

    res?;

    // Rotating the master password counts as use; also invalidate any reveal
    // grant since the key the grant was issued under is gone.
    state.passwords_auto_lock.record_activity();
    *state.pw_reveal_grant.lock().unwrap() = None;
    Ok(())
}

// ─── Entry commands ─────────────────────────────────────

#[tauri::command]
pub async fn pw_list(state: State<'_, AppState>) -> Result<Vec<PasswordEntryMeta>, AppError> {
    let guard = state.passwords.lock().await;
    let store = guard.as_ref().ok_or(AppError::VaultLocked)?;
    let metas = store.list()?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(metas)
}

#[tauri::command]
pub async fn pw_add(
    state: State<'_, AppState>,
    entry: PasswordEntryInput,
) -> Result<String, AppError> {
    let mut guard = state.passwords.lock().await;
    let store = guard.as_mut().ok_or(AppError::VaultLocked)?;
    let id = store.add(&entry)?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(id)
}

#[tauri::command]
pub async fn pw_update(
    state: State<'_, AppState>,
    id: String,
    entry: PasswordEntryInput,
) -> Result<(), AppError> {
    let mut guard = state.passwords.lock().await;
    let store = guard.as_mut().ok_or(AppError::VaultLocked)?;
    store.update(&id, &entry)?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(())
}

/// Lossless meta-only edit: re-encrypt ONLY the meta blob (title, username,
/// url, category), preserving `created_at`, bumping `updated_at`, and leaving
/// the secret blob (password + notes) byte-for-byte UNCHANGED. Requires the
/// store to be unlocked. The UI uses this instead of `pw_update` for editing
/// listing fields so a metadata edit never round-trips the password.
#[tauri::command]
pub async fn pw_update_meta(
    state: State<'_, AppState>,
    id: String,
    meta: MetaInput,
) -> Result<(), AppError> {
    let mut guard = state.passwords.lock().await;
    let store = guard.as_mut().ok_or(AppError::VaultLocked)?;
    store.update_meta(&id, &meta)?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(())
}

/// Lossless secret-only edit: re-encrypt ONLY the secret blob (password +
/// notes), preserving every meta field and `created_at` while bumping
/// `updated_at`. Requires the store to be unlocked.
#[tauri::command]
pub async fn pw_update_secret(
    state: State<'_, AppState>,
    id: String,
    secret: SecretInput,
) -> Result<(), AppError> {
    let mut guard = state.passwords.lock().await;
    let store = guard.as_mut().ok_or(AppError::VaultLocked)?;
    store.update_secret(&id, &secret)?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(())
}

#[tauri::command]
pub async fn pw_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let mut guard = state.passwords.lock().await;
    let store = guard.as_mut().ok_or(AppError::VaultLocked)?;
    store.delete(&id)?;
    drop(guard);

    state.passwords_auto_lock.record_activity();
    Ok(())
}

// ─── Reveal path (fresh re-auth grant) ──────────────────

/// Decide whether a reveal grant authorizes revealing `requested_id` as of
/// `now`. The grant authorizes ONLY when it exists, is bound to the exact
/// requested id, AND is still within [`REVEAL_GRANT_TTL_SECS`] of its issue
/// time. Pure so the id-binding + freshness security logic is unit-testable
/// without a Tauri `State` harness; `pw_reveal` is the sole caller.
fn grant_authorizes(grant: Option<&(String, Instant)>, requested_id: &str, now: Instant) -> bool {
    grant
        .map(|(grant_id, issued)| {
            grant_id == requested_id
                && now.saturating_duration_since(*issued)
                    < Duration::from_secs(REVEAL_GRANT_TTL_SECS)
        })
        .unwrap_or(false)
}

/// Re-authenticate to mint a fresh reveal grant for ONE specific entry.
/// Verifies `master_password` against the unlocked store (re-derives the key and
/// constant-time-compares it) and, on success, records `(id, now)` in
/// `state.pw_reveal_grant`. Returns the grant TTL in seconds so the frontend can
/// show/track the countdown.
///
/// The grant is BOTH single-use and id-bound: `pw_reveal` consumes it and
/// accepts ONLY this exact `id` while fresh (see [`pw_reveal`]), so one re-auth
/// can never dump the whole vault in a loop.
///
/// FIX #6: the memory-hard Argon2id derivation runs on a blocking thread and is
/// NOT performed while the async `passwords` mutex is held. We lock only briefly
/// to snapshot (salt, kdf, current-key copy), drop the guard, derive + compare
/// off-lock, then re-lock only to record the grant.
///
/// A wrong password maps to [`AppError::VaultWrongPassword`] and does NOT touch
/// the existing grant.
#[tauri::command]
pub async fn pw_reauth(
    state: State<'_, AppState>,
    master_password: String,
    id: String,
) -> Result<u64, AppError> {
    // Brief lock: snapshot the cheap material, confirm unlocked, then release.
    let snapshot = {
        let guard = state.passwords.lock().await;
        let store = guard.as_ref().ok_or(AppError::VaultLocked)?;
        store.reauth_snapshot()?
    };

    // Expensive derive + constant-time compare OFF the async lock.
    let ok =
        tokio::task::spawn_blocking(move || verify_reauth_candidate(&snapshot, &master_password))
            .await
            .map_err(|e| AppError::VaultError(format!("Re-auth task failed: {e}")))??;

    if !ok {
        return Err(AppError::VaultWrongPassword);
    }

    // Bind the grant to this exact entry id and stamp it now (single-use:
    // consumed by the next successful pw_reveal).
    *state.pw_reveal_grant.lock().unwrap() = Some((id, Instant::now()));
    state.passwords_auto_lock.record_activity();
    Ok(REVEAL_GRANT_TTL_SECS)
}

/// Reveal one entry's password. Requires, in order:
///   1. an active, unexpired reveal grant (from `pw_reauth`) that is BOTH fresh
///      AND bound to THIS exact `id`, else [`AppError::RevealNotAuthorized`];
///   2. the store to be unlocked, else [`AppError::VaultLocked`].
///
/// FIX #3: the grant is SINGLE-USE and id-bound. After a successful
/// `store.reveal`, the grant is cleared (`= None`), so every reveal needs a
/// fresh `pw_reauth`; and the grant only authorizes the entry id it was minted
/// for, so one re-auth cannot loop-dump the whole vault within the TTL.
///
/// FIX #5 (accepted residual): the plaintext crosses the IPC boundary exactly
/// once. The store hands back a `Zeroizing<String>` (wiped on drop); we move
/// that single owned value into the returned `String` with no extra clones. The
/// one returned `String` is owned by Tauri's IPC layer for serialization — that
/// lone copy is unavoidable and accepted.
#[tauri::command]
pub async fn pw_reveal(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    // 1. Grant check FIRST — never reveal without a fresh, id-bound re-auth.
    {
        let grant = state.pw_reveal_grant.lock().unwrap();
        if !grant_authorizes(grant.as_ref(), &id, Instant::now()) {
            return Err(AppError::RevealNotAuthorized);
        }
    }

    // 2. Store must be unlocked. `reveal` returns a Zeroizing<String>; move its
    //    single owned String out for IPC. `std::mem::take` swaps the inner
    //    String out for an empty one, so there is NO extra clone of the
    //    plaintext — the password exists in exactly one owned String, which is
    //    handed to Tauri. The now-empty Zeroizing wrapper is dropped immediately.
    let guard = state.passwords.lock().await;
    let store = guard.as_ref().ok_or(AppError::VaultLocked)?;
    let mut revealed = store.reveal(&id)?;
    let password = std::mem::take(&mut *revealed);
    drop(revealed);
    drop(guard);

    // 3. Consume the grant — single-use. Even if the same id is requested again,
    //    a fresh pw_reauth is required.
    *state.pw_reveal_grant.lock().unwrap() = None;

    state.passwords_auto_lock.record_activity();
    Ok(password)
}

// ─── Generator ──────────────────────────────────────────

/// Generate a random password with `OsRng`. Stateless — does not touch the
/// store, so it needs no unlock gate and records no activity.
#[tauri::command]
pub async fn pw_generate(
    length: u32,
    symbols: bool,
    digits: bool,
    uppercase: bool,
) -> Result<String, AppError> {
    Ok(generate_password(length, symbols, digits, uppercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Reveal grant authorization (FIX #3: single-use + id-bound) ──

    #[test]
    fn grant_none_never_authorizes() {
        assert!(!grant_authorizes(None, "abc", Instant::now()));
    }

    #[test]
    fn grant_authorizes_matching_fresh_id() {
        let now = Instant::now();
        let grant = ("abc".to_string(), now);
        assert!(grant_authorizes(Some(&grant), "abc", now));
    }

    #[test]
    fn grant_rejects_mismatched_id_even_when_fresh() {
        // Intent binding: a grant minted for "abc" must NOT reveal "xyz",
        // so one re-auth cannot loop-dump other entries within the TTL.
        let now = Instant::now();
        let grant = ("abc".to_string(), now);
        assert!(!grant_authorizes(Some(&grant), "xyz", now));
    }

    #[test]
    fn grant_rejects_expired_even_when_id_matches() {
        // Issued just past the TTL → not fresh, must be rejected.
        let issued = Instant::now();
        let now = issued + Duration::from_secs(REVEAL_GRANT_TTL_SECS + 1);
        let grant = ("abc".to_string(), issued);
        assert!(!grant_authorizes(Some(&grant), "abc", now));
    }

    #[test]
    fn grant_authorizes_within_ttl_boundary() {
        // Still inside the window (strictly less than TTL).
        let issued = Instant::now();
        let now = issued + Duration::from_secs(REVEAL_GRANT_TTL_SECS - 1);
        let grant = ("abc".to_string(), issued);
        assert!(grant_authorizes(Some(&grant), "abc", now));
    }
}

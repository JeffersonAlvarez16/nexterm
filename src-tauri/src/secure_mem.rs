// secure_mem — Best-effort memory locking for in-memory derived keys.
//
// A derived encryption key (32 bytes from Argon2id) sits in process memory for
// as long as a store is unlocked. By default the OS is free to page that memory
// out to swap, or write it to the hibernation file on suspend — leaving the key
// material on disk where it outlives the process and survives a reboot. This
// module pins a key in physical RAM with the platform memory-lock primitive so
// the kernel will not page it out:
//   - Unix-family: `mlock` / `munlock` (libc).
//   - Windows:     `VirtualLock` / `VirtualUnlock` (Win32, via the `windows` crate).
//
// SECURITY NOTE — best-effort by design:
//   Locking is NOT guaranteed. `mlock` is bounded by `RLIMIT_MEMLOCK` (often a
//   small per-process limit for unprivileged users) and `VirtualLock` is bounded
//   by the process working-set minimum / privileges. When the lock call fails we
//   log a warning and continue with an UNLOCKED-but-still-zeroized buffer rather
//   than panicking — a running app with a possibly-swappable key is strictly
//   better than a crash. This reduces, but does NOT eliminate, swap/hibernation
//   exposure: memory locked while the process runs can still be captured by a
//   privileged attacker, a core dump, or a cold-boot attack on RAM.
//
// On Drop the bytes are always zeroized FIRST (so the cleartext key never lingers
// after the type goes away) and the lock is then released. Zeroization happens
// unconditionally, independent of whether the lock succeeded.

use zeroize::Zeroize;

/// Size in bytes of a derived key. Matches the Argon2id output used by the
/// encrypted stores (`crypto::derive_key` → `[u8; 32]`).
pub const KEY_LEN: usize = 32;

/// A heap-pinned, memory-locked, zero-on-drop 32-byte derived key.
///
/// The bytes live in a `Box<[u8; KEY_LEN]>` so the allocation has a stable
/// address for the whole lifetime of the value (a stack array would move on
/// every `LockedKey` move, invalidating any page lock taken on it). On
/// construction we best-effort lock that page in RAM; on drop we zeroize then
/// unlock.
///
/// Use [`LockedKey::as_bytes`] / [`LockedKey::as_slice`] to read the key for
/// crypto operations. The type intentionally exposes no owned-copy accessor so
/// callers cannot accidentally clone the key into an unlocked, non-zeroized
/// buffer.
pub struct LockedKey {
    /// Heap-pinned key bytes. `Box` guarantees a stable heap address so the
    /// platform lock stays valid for the whole lifetime (no realloc, no move of
    /// the underlying bytes when the `LockedKey` itself moves).
    key: Box<[u8; KEY_LEN]>,
    /// Whether the platform lock call succeeded. Drives whether we attempt the
    /// matching unlock on drop (we never unlock a page we did not lock).
    locked: bool,
}

impl LockedKey {
    /// Build a `LockedKey` from a 32-byte derived key, taking ownership of the
    /// material and best-effort locking it into RAM.
    ///
    /// `bytes` is moved into a heap `Box` (the pinned allocation that gets
    /// locked and is zeroized on drop). The caller remains responsible for any
    /// *other* copy of the key it still holds — e.g. the `Zeroizing` source it
    /// derived from, which wipes itself on scope exit. Locking failure is logged
    /// and swallowed (the value is still usable and still zero-on-drop).
    pub fn new(bytes: [u8; KEY_LEN]) -> Self {
        let key = Box::new(bytes);
        let ptr = key.as_ptr();
        let locked = lock_memory(ptr, KEY_LEN);
        if !locked {
            tracing::warn!(
                "secure_mem: failed to lock {} bytes of key material into RAM \
                 (RLIMIT_MEMLOCK / privileges may forbid it); continuing with a \
                 possibly-swappable but still zero-on-drop key",
                KEY_LEN
            );
        }
        LockedKey { key, locked }
    }

    /// Borrow the key bytes as a fixed-size array reference.
    ///
    /// This is the canonical accessor for crypto calls that take `&[u8; 32]`
    /// (e.g. `crypto::encrypt_bytes` / `crypto::decrypt_raw`).
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    /// Borrow the key bytes as a slice. Convenience for APIs taking `&[u8]`
    /// (e.g. `crypto::ct_eq`).
    pub fn as_slice(&self) -> &[u8] {
        &self.key[..]
    }
}

impl Drop for LockedKey {
    fn drop(&mut self) {
        // Always wipe the cleartext key first, regardless of lock state, so the
        // key never lingers in the freed allocation.
        self.key.zeroize();
        // Only release a lock we actually took.
        if self.locked {
            let ptr = self.key.as_ptr();
            unlock_memory(ptr, KEY_LEN);
        }
    }
}

// ─── Platform memory-lock primitives ────────────────────

/// Best-effort lock `len` bytes at `ptr` into physical RAM. Returns `true` on
/// success, `false` on failure (caller logs and continues).
#[cfg(unix)]
fn lock_memory(ptr: *const u8, len: usize) -> bool {
    // SAFETY: `ptr`/`len` describe a live heap allocation owned by the caller
    // (the `Box<[u8; KEY_LEN]>` in `LockedKey`). `mlock` only pins those pages;
    // it neither reads nor writes the bytes.
    unsafe { libc::mlock(ptr as *const libc::c_void, len) == 0 }
}

/// Best-effort unlock `len` bytes at `ptr`. Failures are ignored (the page is
/// freed on drop regardless).
#[cfg(unix)]
fn unlock_memory(ptr: *const u8, len: usize) {
    // SAFETY: same allocation that was passed to `mlock` in `lock_memory`.
    unsafe {
        let _ = libc::munlock(ptr as *const libc::c_void, len);
    }
}

/// Best-effort lock `len` bytes at `ptr` into the process working set via
/// `VirtualLock`. Returns `true` on success.
#[cfg(windows)]
fn lock_memory(ptr: *const u8, len: usize) -> bool {
    use windows::Win32::System::Memory::VirtualLock;
    // SAFETY: `ptr`/`len` describe a live heap allocation owned by the caller.
    // `VirtualLock` only pins the pages in the working set.
    unsafe { VirtualLock(ptr as *const core::ffi::c_void, len).is_ok() }
}

/// Best-effort unlock `len` bytes at `ptr` via `VirtualUnlock`. Failures ignored.
#[cfg(windows)]
fn unlock_memory(ptr: *const u8, len: usize) {
    use windows::Win32::System::Memory::VirtualUnlock;
    // SAFETY: same allocation that was passed to `VirtualLock`.
    unsafe {
        let _ = VirtualUnlock(ptr as *const core::ffi::c_void, len);
    }
}

/// Fallback for exotic targets with neither `mlock` nor `VirtualLock`: report
/// "not locked" so the warning fires and zero-on-drop still applies.
#[cfg(not(any(unix, windows)))]
fn lock_memory(_ptr: *const u8, _len: usize) -> bool {
    false
}

#[cfg(not(any(unix, windows)))]
fn unlock_memory(_ptr: *const u8, _len: usize) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_from_array_and_reads_back() {
        let mut raw = [0u8; KEY_LEN];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = i as u8;
        }
        let key = LockedKey::new(raw);

        // as_bytes / as_slice expose the same material we put in.
        assert_eq!(key.as_bytes(), &raw);
        assert_eq!(key.as_slice(), &raw[..]);
        assert_eq!(key.as_slice().len(), KEY_LEN);
    }

    #[test]
    fn as_bytes_and_as_slice_agree() {
        let raw = [0xABu8; KEY_LEN];
        let key = LockedKey::new(raw);
        assert_eq!(key.as_slice(), &key.as_bytes()[..]);
    }

    #[test]
    fn drop_runs_without_panic() {
        // Construct and immediately drop many keys: zeroize + unlock on drop
        // must never panic, even when locking was best-effort/failed.
        for _ in 0..256 {
            let key = LockedKey::new([0x5Au8; KEY_LEN]);
            // Touch the bytes so the optimizer keeps the allocation.
            assert_eq!(key.as_bytes()[0], 0x5A);
            drop(key);
        }
    }

    #[test]
    fn distinct_values_are_independent() {
        let a = LockedKey::new([1u8; KEY_LEN]);
        let b = LockedKey::new([2u8; KEY_LEN]);
        assert_ne!(a.as_slice(), b.as_slice());
        assert_eq!(a.as_bytes()[0], 1);
        assert_eq!(b.as_bytes()[0], 2);
    }
}

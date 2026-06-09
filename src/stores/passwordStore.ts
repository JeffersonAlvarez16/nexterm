// stores/passwordStore.ts — Zustand store for the independent password manager
//
// This is the SECOND vault: a password manager with its OWN master password,
// its OWN encrypted file (passwords.json), and its OWN auto-lock — fully
// independent from the SSH credential vault. Actions wrap the backend `pw_*`
// Tauri commands via the tauriInvoke wrapper.
//
// SECURITY — non-negotiable invariants:
//   - A revealed password is NEVER stored in this store. reveal() returns the
//     plaintext to the caller (transient UI use) and the store keeps nothing.
//   - Listing entries NEVER decrypts a password: `entries` holds metadata only.
//   - Reveal requires a fresh re-auth grant (pw_reauth). The grant is now
//     SINGLE-USE and ID-BOUND on the backend: each reveal/copy of a row must
//     call pw_reauth(masterPassword, id) for THAT row immediately before
//     pw_reveal(id). The backend is the sole authority; the client tracks no
//     cross-row grant. reauth() is therefore (id, masterPassword) -> ttl.
//   - lock() / resetStore() clear all transient state.

import { create } from "zustand";
import { tauriInvoke } from "../lib/tauri";

// ─── Types (mirror the frozen Tauri command contract) ──────────────────────────

/** Metadata for a single entry. NEVER includes the password. */
export interface PasswordEntryMeta {
  id: string;
  title: string;
  username: string;
  url: string;
  category: string;
  createdAt: number;
  updatedAt: number;
}

/** Input payload for creating/updating an entry. Includes the secret. */
export interface PasswordEntryInput {
  title: string;
  username: string;
  url: string;
  category: string;
  notes: string;
  password: string;
}

/**
 * Metadata-only edit payload (lossless): updates the listing fields without
 * ever touching the encrypted secret blob (password + notes).
 */
export interface PasswordMetaInput {
  title: string;
  username: string;
  url: string;
  category: string;
}

/**
 * Secret-only edit payload (lossless): REPLACES the encrypted secret blob
 * (password + notes) without touching the meta fields.
 */
export interface PasswordSecretInput {
  password: string;
  notes: string;
}

/** Result of pw_status(). */
export interface PasswordStatus {
  exists: boolean;
  unlocked: boolean;
  idleTimeoutSecs: number;
  /** Seconds until the backend auto-locks, or null when locked / no file. */
  secondsUntilLock: number | null;
}

// ─── Store shape ───────────────────────────────────────────────────────────────

interface PasswordStoreState {
  status: PasswordStatus | null;
  entries: PasswordEntryMeta[];
  unlocked: boolean;
  loading: boolean;
  error: string | null;

  // ── Lifecycle / gating ──
  refreshStatus: () => Promise<PasswordStatus>;
  create: (masterPassword: string) => Promise<void>;
  unlock: (masterPassword: string) => Promise<void>;
  lock: () => Promise<void>;
  resetStore: () => Promise<void>;
  changeMaster: (oldPassword: string, newPassword: string) => Promise<void>;
  setIdleTimeout: (secs: number) => Promise<void>;

  // ── Entries ──
  list: () => Promise<void>;
  add: (entry: PasswordEntryInput) => Promise<string>;
  /**
   * @deprecated Full-entry update. The UI no longer uses this for edits because
   * it re-encrypts the secret blob from the form, silently wiping the stored
   * password/notes on a metadata-only edit. Prefer updateMeta / updateSecret.
   */
  update: (id: string, entry: PasswordEntryInput) => Promise<void>;
  /** Lossless metadata-only edit (never touches the secret). */
  updateMeta: (id: string, meta: PasswordMetaInput) => Promise<void>;
  /** Lossless secret-only edit (REPLACES password + notes). */
  updateSecret: (id: string, secret: PasswordSecretInput) => Promise<void>;
  remove: (id: string) => Promise<void>;

  // ── Reveal flow (grant is single-use + id-bound on the backend) ──
  reauth: (id: string, masterPassword: string) => Promise<number>;
  reveal: (id: string) => Promise<string>;

  // ── Tools ──
  generate: (
    length: number,
    symbols: boolean,
    digits: boolean,
    uppercase: boolean,
  ) => Promise<string>;

  // ── Import / Export ──
  /**
   * Export all entries to a Bitwarden-compatible CSV at `path`.
   * Requires master password re-authentication (enforced on the backend).
   * Returns the number of entries written.
   */
  exportToFile: (path: string, masterPassword: string) => Promise<number>;
  /**
   * Import entries from a Bitwarden JSON or CSV file at `path`.
   * The store must be unlocked. Returns the number of entries imported.
   */
  importFromFile: (path: string) => Promise<number>;

  clearError: () => void;
}

export const usePasswordStore = create<PasswordStoreState>((set, get) => ({
  status: null,
  entries: [],
  unlocked: false,
  loading: false,
  error: null,

  refreshStatus: async () => {
    try {
      const status = await tauriInvoke<PasswordStatus>("pw_status");
      // If the backend reports locked (e.g. auto-lock fired), drop all
      // transient secret-adjacent state so the UI re-gates correctly.
      if (!status.unlocked) {
        set({ status, unlocked: false, entries: [] });
      } else {
        set({ status, unlocked: true });
      }
      return status;
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  create: async (masterPassword: string) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_create", { masterPassword });
      // pw_create leaves the store unlocked.
      set({ unlocked: true });
      await get().refreshStatus();
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  unlock: async (masterPassword: string) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_unlock", { masterPassword });
      set({ unlocked: true });
      await get().refreshStatus();
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  lock: async () => {
    try {
      await tauriInvoke<void>("pw_lock");
    } catch (err) {
      // Even if the backend call fails, locally re-gate: never keep secrets
      // or entries visible when the user asked to lock.
      set({ error: String(err) });
    } finally {
      set({
        unlocked: false,
        entries: [],
      });
      // Refresh status best-effort so secondsUntilLock etc. reflect reality.
      try {
        await get().refreshStatus();
      } catch {
        /* ignore — already locked locally */
      }
    }
  },

  resetStore: async () => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_reset");
    } catch (err) {
      set({ error: String(err) });
      throw err;
    } finally {
      set({
        unlocked: false,
        entries: [],
      });
      try {
        await get().refreshStatus();
      } catch {
        /* ignore */
      }
    }
  },

  changeMaster: async (oldPassword: string, newPassword: string) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_change_master", { oldPassword, newPassword });
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  setIdleTimeout: async (secs: number) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_set_idle_timeout", { secs });
      await get().refreshStatus();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  list: async () => {
    set({ loading: true, error: null });
    try {
      const entries = await tauriInvoke<PasswordEntryMeta[]>("pw_list");
      set({ entries, loading: false });
    } catch (err) {
      set({ loading: false, error: String(err) });
      throw err;
    }
  },

  add: async (entry: PasswordEntryInput) => {
    set({ error: null });
    try {
      const id = await tauriInvoke<string>("pw_add", { entry });
      await get().list();
      return id;
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  update: async (id: string, entry: PasswordEntryInput) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_update", { id, entry });
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  updateMeta: async (id: string, meta: PasswordMetaInput) => {
    set({ error: null });
    try {
      // Lossless: re-encrypts ONLY the meta blob. The secret (password + notes)
      // is left byte-for-byte unchanged — a title edit never re-types the
      // password and can never wipe it.
      await tauriInvoke<void>("pw_update_meta", { id, meta });
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  updateSecret: async (id: string, secret: PasswordSecretInput) => {
    set({ error: null });
    try {
      // Lossless for the meta side: re-encrypts ONLY the secret blob,
      // REPLACING the stored password + notes while preserving all meta fields.
      await tauriInvoke<void>("pw_update_secret", { id, secret });
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  remove: async (id: string) => {
    set({ error: null });
    try {
      await tauriInvoke<void>("pw_delete", { id });
      await get().list();
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  reauth: async (id: string, masterPassword: string) => {
    set({ error: null });
    try {
      // Mints a SINGLE-USE grant bound to THIS exact entry id. The next
      // pw_reveal(id) consumes it; it never authorizes any other row. We track
      // no client-side expiry — the backend is the sole authority.
      return await tauriInvoke<number>("pw_reauth", { masterPassword, id });
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  reveal: async (id: string) => {
    set({ error: null });
    try {
      // Returns the plaintext to the CALLER only. We deliberately do NOT store
      // it anywhere in the store — the caller is responsible for transient,
      // short-lived display and must not persist it either.
      const password = await tauriInvoke<string>("pw_reveal", { id });
      return password;
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  generate: async (
    length: number,
    symbols: boolean,
    digits: boolean,
    uppercase: boolean,
  ) => {
    set({ error: null });
    try {
      return await tauriInvoke<string>("pw_generate", {
        length,
        symbols,
        digits,
        uppercase,
      });
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  exportToFile: async (path: string, masterPassword: string) => {
    set({ error: null });
    try {
      return await tauriInvoke<number>("pw_export_to_file", { path, masterPassword });
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  importFromFile: async (path: string) => {
    set({ error: null });
    try {
      const count = await tauriInvoke<number>("pw_import_from_file", { path });
      // Refresh the entry list so the UI reflects the newly imported entries.
      await get().list();
      return count;
    } catch (err) {
      set({ error: String(err) });
      throw err;
    }
  },

  clearError: () => set({ error: null }),
}));

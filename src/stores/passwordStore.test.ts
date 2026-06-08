// stores/passwordStore.test.ts — Security-critical invariants for the password
// manager store.
//
// Verifies the two non-negotiable behaviors:
//   1. reveal() returns the plaintext to the caller but NEVER persists it in
//      store state.
//   2. lock() clears entries and unlocked (re-gates the UI).

import { describe, it, expect, vi, beforeEach } from "vitest";

// ── Mocks ─────────────────────────────────────────────────────────────────────

const { mockTauriInvoke } = vi.hoisted(() => ({
  mockTauriInvoke: vi.fn(),
}));

vi.mock("../lib/tauri", () => ({
  tauriInvoke: mockTauriInvoke,
}));

import {
  usePasswordStore,
  type PasswordEntryMeta,
  type PasswordStatus,
} from "./passwordStore";

// ── Helpers ─────────────────────────────────────────────────────────────────────

function resetStore() {
  usePasswordStore.setState({
    status: null,
    entries: [],
    unlocked: false,
    loading: false,
    error: null,
  });
}

const META: PasswordEntryMeta = {
  id: "deadbeef",
  title: "GitHub",
  username: "octocat",
  url: "https://github.com",
  category: "Work",
  createdAt: 1,
  updatedAt: 2,
};

const UNLOCKED_STATUS: PasswordStatus = {
  exists: true,
  unlocked: true,
  idleTimeoutSecs: 300,
  secondsUntilLock: 300,
};

const LOCKED_STATUS: PasswordStatus = {
  exists: true,
  unlocked: false,
  idleTimeoutSecs: 300,
  secondsUntilLock: null,
};

describe("passwordStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("reveal() returns the plaintext to the caller but never persists it in state", async () => {
    const SECRET = "s3cr3t-plaintext-never-stored";
    mockTauriInvoke.mockResolvedValueOnce(SECRET); // pw_reveal

    // Seed a plausible unlocked state with one entry.
    usePasswordStore.setState({ unlocked: true, entries: [META] });

    const returned = await usePasswordStore.getState().reveal(META.id);

    // The caller gets the plaintext.
    expect(returned).toBe(SECRET);
    expect(mockTauriInvoke).toHaveBeenCalledWith("pw_reveal", { id: META.id });

    // The store must NOT contain the secret anywhere.
    const state = usePasswordStore.getState();
    const serialized = JSON.stringify({
      status: state.status,
      entries: state.entries,
      unlocked: state.unlocked,
      loading: state.loading,
      error: state.error,
    });
    expect(serialized).not.toContain(SECRET);

    // And there is no rogue field holding it on the entry either.
    expect(JSON.stringify(state.entries)).not.toContain(SECRET);
  });

  it("lock() clears entries and unlocked", async () => {
    // First call: pw_lock resolves. Second call (refreshStatus): pw_status.
    mockTauriInvoke
      .mockResolvedValueOnce(undefined) // pw_lock
      .mockResolvedValueOnce(LOCKED_STATUS); // pw_status

    // Seed an unlocked store with entries.
    usePasswordStore.setState({
      unlocked: true,
      entries: [META],
      status: UNLOCKED_STATUS,
    });

    await usePasswordStore.getState().lock();

    const state = usePasswordStore.getState();
    expect(mockTauriInvoke).toHaveBeenCalledWith("pw_lock");
    expect(state.unlocked).toBe(false);
    expect(state.entries).toEqual([]);
  });

  it("reauth(id, master) passes an id-bound payload and returns the TTL", async () => {
    mockTauriInvoke.mockResolvedValueOnce(30); // pw_reauth returns TTL secs

    const ttl = await usePasswordStore.getState().reauth(META.id, "master-pass");

    // The grant is single-use + id-bound: the payload carries the exact id.
    expect(ttl).toBe(30);
    expect(mockTauriInvoke).toHaveBeenCalledWith("pw_reauth", {
      masterPassword: "master-pass",
      id: META.id,
    });
  });

  it("updateMeta() calls pw_update_meta with meta only, never the secret", async () => {
    mockTauriInvoke
      .mockResolvedValueOnce(undefined) // pw_update_meta
      .mockResolvedValueOnce([META]); // pw_list refresh

    await usePasswordStore.getState().updateMeta(META.id, {
      title: "GitHub",
      username: "octocat",
      url: "https://github.com",
      category: "Work",
    });

    const call = mockTauriInvoke.mock.calls.find((c) => c[0] === "pw_update_meta");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({
      id: META.id,
      meta: {
        title: "GitHub",
        username: "octocat",
        url: "https://github.com",
        category: "Work",
      },
    });
    // No password/notes anywhere in the meta payload — no silent-wipe path.
    expect(JSON.stringify(call![1])).not.toContain("password");
    expect(JSON.stringify(call![1])).not.toContain("notes");
  });

  it("updateSecret() calls pw_update_secret with the secret blob only", async () => {
    mockTauriInvoke
      .mockResolvedValueOnce(undefined) // pw_update_secret
      .mockResolvedValueOnce([META]); // pw_list refresh

    await usePasswordStore.getState().updateSecret(META.id, {
      password: "new-secret",
      notes: "new notes",
    });

    const call = mockTauriInvoke.mock.calls.find((c) => c[0] === "pw_update_secret");
    expect(call).toBeTruthy();
    expect(call![1]).toEqual({
      id: META.id,
      secret: { password: "new-secret", notes: "new notes" },
    });
  });

  it("refreshStatus() re-gates locally when backend reports locked", async () => {
    mockTauriInvoke.mockResolvedValueOnce(LOCKED_STATUS); // pw_status

    // Simulate a previously-unlocked UI with entries.
    usePasswordStore.setState({
      unlocked: true,
      entries: [META],
    });

    await usePasswordStore.getState().refreshStatus();

    const state = usePasswordStore.getState();
    expect(state.unlocked).toBe(false);
    expect(state.entries).toEqual([]);
  });
});

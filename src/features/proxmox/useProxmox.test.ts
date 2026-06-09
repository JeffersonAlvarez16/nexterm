// features/proxmox/useProxmox.test.ts — TDD: Proxmox hook lifecycle (LXC + VMs)

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

// ── Mocks ─────────────────────────────────────────────────────────────────────

const { mockTauriInvoke } = vi.hoisted(() => ({
  mockTauriInvoke: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  tauriInvoke: mockTauriInvoke,
}));

// ── Store reset ───────────────────────────────────────────────────────────────

import { useProxmoxStore } from "../../stores/proxmoxStore";

function resetStore() {
  useProxmoxStore.setState({
    containers: new Map(),
    vms: new Map(),
    snapshots: new Map(),
    availability: new Map(),
    vmAvailability: new Map(),
    loading: new Map(),
  });
}

// ── Hook import (after mocks) ──────────────────────────────────────────────────

import { useProxmox } from "./useProxmox";

const SESSION_ID = "test-session-proxmox-1";

function makeLxcResult(available = true) {
  return {
    containers: [{ vmid: 100, status: "running", name: "debian-dev" }],
    pctUnavailable: !available,
  };
}

function makeVmResult(available = true) {
  return {
    vms: [{ vmid: 200, name: "windows-server", status: "stopped" }],
    qmUnavailable: !available,
  };
}

describe("useProxmox", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    // Default: both lxc and vm calls succeed.
    mockTauriInvoke.mockImplementation((cmd: string) => {
      if (cmd === "proxmox_list_lxc") return Promise.resolve(makeLxcResult());
      if (cmd === "proxmox_list_vms") return Promise.resolve(makeVmResult());
      return Promise.resolve({});
    });
  });

  it("calls proxmox_list_lxc on mount", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(mockTauriInvoke).toHaveBeenCalledWith(
      "proxmox_list_lxc",
      expect.objectContaining({ sessionId: SESSION_ID }),
    );
  });

  it("calls proxmox_list_vms on mount", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(mockTauriInvoke).toHaveBeenCalledWith(
      "proxmox_list_vms",
      expect.objectContaining({ sessionId: SESSION_ID }),
    );
  });

  it("populates store with containers after mount", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    const containers = useProxmoxStore.getState().containers.get(SESSION_ID);
    expect(containers).toHaveLength(1);
    expect(containers![0]!.vmid).toBe(100);
  });

  it("populates store with vms after mount", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    const vms = useProxmoxStore.getState().vms.get(SESSION_ID);
    expect(vms).toHaveLength(1);
    expect(vms![0]!.vmid).toBe(200);
    expect(vms![0]!.name).toBe("windows-server");
  });

  it("sets availability=true when pct is available", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(useProxmoxStore.getState().availability.get(SESSION_ID)).toBe(true);
  });

  it("sets vmAvailability=true when qm is available", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(useProxmoxStore.getState().vmAvailability.get(SESSION_ID)).toBe(true);
  });

  it("sets availability=false when pctUnavailable=true", async () => {
    mockTauriInvoke.mockImplementation((cmd: string) => {
      if (cmd === "proxmox_list_lxc")
        return Promise.resolve({ containers: [], pctUnavailable: true });
      if (cmd === "proxmox_list_vms") return Promise.resolve(makeVmResult());
      return Promise.resolve({});
    });
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(useProxmoxStore.getState().availability.get(SESSION_ID)).toBe(false);
  });

  it("sets vmAvailability=false when qmUnavailable=true", async () => {
    mockTauriInvoke.mockImplementation((cmd: string) => {
      if (cmd === "proxmox_list_lxc") return Promise.resolve(makeLxcResult());
      if (cmd === "proxmox_list_vms")
        return Promise.resolve({ vms: [], qmUnavailable: true });
      return Promise.resolve({});
    });
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(useProxmoxStore.getState().vmAvailability.get(SESSION_ID)).toBe(false);
  });

  it("sets loading=false after fetch completes", async () => {
    renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    expect(useProxmoxStore.getState().loading.get(SESSION_ID)).toBe(false);
  });

  it("does not call proxmox_list_lxc when sessionId is empty", async () => {
    renderHook(() => useProxmox(""));
    await act(async () => {});
    expect(mockTauriInvoke).not.toHaveBeenCalledWith(
      "proxmox_list_lxc",
      expect.anything(),
    );
  });

  it("does not call proxmox_list_vms when sessionId is empty", async () => {
    renderHook(() => useProxmox(""));
    await act(async () => {});
    expect(mockTauriInvoke).not.toHaveBeenCalledWith(
      "proxmox_list_vms",
      expect.anything(),
    );
  });

  it("refresh re-fetches both containers and vms", async () => {
    const { result } = renderHook(() => useProxmox(SESSION_ID));
    await act(async () => {});
    mockTauriInvoke.mockClear();
    await act(async () => {
      result.current.refresh();
    });
    await act(async () => {});
    expect(mockTauriInvoke).toHaveBeenCalledWith(
      "proxmox_list_lxc",
      expect.objectContaining({ sessionId: SESSION_ID }),
    );
    expect(mockTauriInvoke).toHaveBeenCalledWith(
      "proxmox_list_vms",
      expect.objectContaining({ sessionId: SESSION_ID }),
    );
  });
});

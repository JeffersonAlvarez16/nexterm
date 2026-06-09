// features/proxmox/useProxmox.ts — Proxmox LXC + QEMU VM management hook
//
// Lifecycle:
//   - On mount (and when sessionId changes): call proxmox_list_lxc AND proxmox_list_vms
//   - Populates proxmoxStore with containers, vms, and availability state
//   - Exposes refresh() for manual re-fetch (refreshes both LXC and VMs)
//   - Polls every 10 s while the panel is open
//
// Poll is stateless — no background Rust task. Each poll is a new
// one-shot call. Mirrors useDocker exactly.

import { useCallback, useEffect, useRef } from "react";

import { tauriInvoke } from "../../lib/tauri";
import { useProxmoxStore } from "../../stores/proxmoxStore";
import type { LxcRow, VmRow } from "../../stores/proxmoxStore";

// ─── Response shapes ──────────────────────────────────────────────────────────

interface ListLxcResult {
  containers: LxcRow[];
  pctUnavailable: boolean;
}

interface ListVmsResult {
  vms: VmRow[];
  qmUnavailable: boolean;
}

// ─── Constants ────────────────────────────────────────────────────────────────

/** Slow poll interval (ms) while the panel is open. */
const POLL_INTERVAL_MS = 10_000;

// ─── Hook ─────────────────────────────────────────────────────────────────────

/**
 * Load and optionally poll Proxmox LXC + VM data for a session.
 *
 * Call from ProxmoxPanel. The hook:
 *   - Fetches both proxmox_list_lxc and proxmox_list_vms on mount
 *   - Sets proxmoxStore containers, vms, availability, and vmAvailability
 *   - Polls every 10 s while mounted
 *   - Cleans up on unmount
 *
 * @param sessionId - Active session ID. Pass empty string to skip.
 */
export function useProxmox(sessionId: string) {
  const setLxc = useProxmoxStore((s) => s.setLxc);
  const setVms = useProxmoxStore((s) => s.setVms);
  const setAvailability = useProxmoxStore((s) => s.setAvailability);
  const setVmAvailability = useProxmoxStore((s) => s.setVmAvailability);
  const setLoading = useProxmoxStore((s) => s.setLoading);

  const mountedRef = useRef(false);

  const fetchAll = useCallback(async () => {
    if (!sessionId) return;

    setLoading(sessionId, true);
    try {
      // Fetch LXC containers and QEMU VMs in parallel.
      const [lxcResult, vmResult] = await Promise.allSettled([
        tauriInvoke<ListLxcResult>("proxmox_list_lxc", { sessionId }),
        tauriInvoke<ListVmsResult>("proxmox_list_vms", { sessionId }),
      ]);

      // LXC result
      if (lxcResult.status === "fulfilled") {
        if (lxcResult.value.pctUnavailable) {
          setAvailability(sessionId, false);
          setLxc(sessionId, []);
        } else {
          setAvailability(sessionId, true);
          setLxc(sessionId, lxcResult.value.containers);
        }
      } else {
        console.error("[useProxmox] proxmox_list_lxc failed:", lxcResult.reason);
      }

      // VM result
      if (vmResult.status === "fulfilled") {
        if (vmResult.value.qmUnavailable) {
          setVmAvailability(sessionId, false);
          setVms(sessionId, []);
        } else {
          setVmAvailability(sessionId, true);
          setVms(sessionId, vmResult.value.vms);
        }
      } else {
        console.error("[useProxmox] proxmox_list_vms failed:", vmResult.reason);
      }
    } finally {
      setLoading(sessionId, false);
    }
  }, [sessionId, setLxc, setVms, setAvailability, setVmAvailability, setLoading]);

  useEffect(() => {
    if (!sessionId) return;

    mountedRef.current = true;
    void fetchAll();

    const timer = setInterval(() => {
      if (mountedRef.current) {
        void fetchAll();
      }
    }, POLL_INTERVAL_MS);

    return () => {
      mountedRef.current = false;
      clearInterval(timer);
    };
  }, [sessionId, fetchAll]);

  return { refresh: fetchAll };
}

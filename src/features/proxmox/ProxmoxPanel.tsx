// features/proxmox/ProxmoxPanel.tsx — Proxmox LXC + QEMU VM management panel
//
// Renders two clearly separated sections:
//   - "Containers" — LXC table with VMID, name, status badges, per-row action buttons
//   - "Virtual Machines" — QEMU VM table with the same action set
//
// Actions per row (both LXC and VM):
//   - Lifecycle: Start / Stop / Reboot (routed with the correct GuestKind)
//   - Interactive shell: pct enter <vmid> for LXC (Shell button, LXC only)
//   - Snapshots: list / create / rollback (confirm) / delete (confirm)
//
// The `kind` parameter ("lxc" or "vm") is always passed to every tauriInvoke
// call so the Rust backend selects the right CLI tool (pct vs qm).
//
// Shell injection uses the validated VMID (numeric string), never the name.

import React, { useState } from "react";

import { useI18n } from "../../lib/i18n";
import { tauriInvoke } from "../../lib/tauri";
import { useProxmoxStore, snapshotKey } from "../../stores/proxmoxStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useProxmox } from "./useProxmox";
import { SnapshotConfirmDialog } from "./SnapshotConfirmDialog";
import type { LxcRow, VmRow, SnapshotRow, GuestKind } from "../../stores/proxmoxStore";

// ─── Types ───────────────────────────────────────────────────────────────────

type SnapshotConfirmState = {
  vmid: number;
  action: "rollback" | "delete";
  snapshotName: string;
} | null;

// ─── Status badge ─────────────────────────────────────────────────────────────

function statusBadgeClass(status: string): string {
  switch (status.toLowerCase()) {
    case "running":
      return "proxmox-status-badge--running";
    case "stopped":
      return "proxmox-status-badge--stopped";
    case "paused":
      return "proxmox-status-badge--paused";
    default:
      return "proxmox-status-badge--unknown";
  }
}

// ─── RefreshIcon ─────────────────────────────────────────────────────────────

function RefreshIcon() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M13.5 2.5A6.5 6.5 0 1 1 4.5 14"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        fill="none"
      />
      <polyline
        points="4,10 4,14 0,14"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

// ─── Shell command helper ─────────────────────────────────────────────────────

/**
 * Build the `pct enter <vmid>` command string for the interactive shell action.
 * Accepts the validated VMID string (NOT the container name) so a future
 * refactor cannot accidentally swap the arguments without breaking the test.
 *
 * The trailing `\n` submits the command to the PTY.
 */
export function buildPctEnterCommand(vmid: string): string {
  return `pct enter ${vmid}\n`;
}

// ─── GuestRow — common shape accepted by shared components ───────────────────

/** Minimal shared shape for a guest row (LXC or VM). */
interface GuestRow {
  vmid: number;
  name: string;
  status: string;
}

// ─── SnapshotSubView ─────────────────────────────────────────────────────────

interface SnapshotSubViewProps {
  sessionId: string;
  guest: GuestRow;
  /** Guest kind: "lxc" → pct commands, "vm" → qm commands. */
  kind: GuestKind;
  onClose: () => void;
}

function SnapshotSubView({ sessionId, guest, kind, onClose }: SnapshotSubViewProps) {
  const { t } = useI18n();
  const [newSnapName, setNewSnapName] = useState("");
  const [creating, setCreating] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [confirmState, setConfirmState] = useState<SnapshotConfirmState>(null);

  const key = snapshotKey(sessionId, guest.vmid);
  const setSnapshots = useProxmoxStore((s) => s.setSnapshots);
  const snapshotsOrUndef = useProxmoxStore((s) => s.snapshots.get(key));
  const snapshots: SnapshotRow[] = snapshotsOrUndef ?? [];

  async function loadSnapshots() {
    try {
      const result = await tauriInvoke<{ snapshots: SnapshotRow[] }>(
        "proxmox_list_snapshots",
        { sessionId, vmid: String(guest.vmid), kind },
      );
      setSnapshots(sessionId, guest.vmid, result.snapshots);
    } catch (err) {
      console.error("[ProxmoxPanel] proxmox_list_snapshots failed:", err);
    }
  }

  async function handleCreate() {
    const name = newSnapName.trim();
    if (!name) return;
    setCreating(true);
    setActionError(null);
    try {
      await tauriInvoke("proxmox_create_snapshot", {
        sessionId,
        vmid: String(guest.vmid),
        snapshotName: name,
        kind,
      });
      setNewSnapName("");
      await loadSnapshots();
    } catch (err) {
      setActionError(typeof err === "string" ? err : String(err));
    } finally {
      setCreating(false);
    }
  }

  async function handleRollback(snapshotName: string) {
    setConfirmState(null);
    setActionError(null);
    try {
      await tauriInvoke("proxmox_rollback_snapshot", {
        sessionId,
        vmid: String(guest.vmid),
        snapshotName,
        kind,
      });
      await loadSnapshots();
    } catch (err) {
      setActionError(typeof err === "string" ? err : String(err));
    }
  }

  async function handleDelete(snapshotName: string) {
    setConfirmState(null);
    setActionError(null);
    try {
      await tauriInvoke("proxmox_delete_snapshot", {
        sessionId,
        vmid: String(guest.vmid),
        snapshotName,
        kind,
      });
      await loadSnapshots();
    } catch (err) {
      setActionError(typeof err === "string" ? err : String(err));
    }
  }

  return (
    <tr>
      <td
        colSpan={4}
        className="proxmox-snapshot-subview"
        aria-label={`${t("proxmox.snapshot.title")}: ${guest.name}`}
      >
        <div className="proxmox-snapshot-header">
          <span className="proxmox-snapshot-title">
            {t("proxmox.snapshot.title")}: <strong>{guest.name}</strong>
          </span>
          <button
            type="button"
            className="proxmox-snapshot-close-btn"
            onClick={onClose}
            aria-label={t("proxmox.snapshot.close")}
          >
            ✕
          </button>
        </div>
        {actionError && (
          <div className="proxmox-action-error" role="alert">
            {actionError}
          </div>
        )}
        {snapshots.length === 0 ? (
          <div className="proxmox-snapshot-empty" role="status">
            {t("proxmox.snapshot.empty")}
          </div>
        ) : (
          <ul className="proxmox-snapshot-list">
            {snapshots.map((snap) =>
              confirmState?.snapshotName === snap.name ? (
                <li key={snap.name} className="proxmox-snapshot-item">
                  <span className="proxmox-snapshot-item-name">{snap.name}</span>
                  <SnapshotConfirmDialog
                    action={confirmState.action}
                    snapshotName={snap.name}
                    onConfirm={
                      confirmState.action === "rollback"
                        ? handleRollback
                        : handleDelete
                    }
                    onCancel={() => setConfirmState(null)}
                  />
                </li>
              ) : (
                <li key={snap.name} className="proxmox-snapshot-item">
                  <span className="proxmox-snapshot-item-name">{snap.name}</span>
                  <div className="proxmox-snapshot-actions">
                    <button
                      type="button"
                      className="proxmox-action-btn"
                      onClick={() =>
                        setConfirmState({
                          vmid: guest.vmid,
                          action: "rollback",
                          snapshotName: snap.name,
                        })
                      }
                      aria-label={`Rollback to ${snap.name}`}
                    >
                      {t("proxmox.snapshot.rollback.arm")}
                    </button>
                    <button
                      type="button"
                      className="proxmox-action-btn proxmox-action-btn--danger"
                      onClick={() =>
                        setConfirmState({
                          vmid: guest.vmid,
                          action: "delete",
                          snapshotName: snap.name,
                        })
                      }
                      aria-label={`Delete snapshot ${snap.name}`}
                    >
                      {t("proxmox.snapshot.delete.arm")}
                    </button>
                  </div>
                </li>
              ),
            )}
          </ul>
        )}
        {/* Create snapshot */}
        <div className="proxmox-snapshot-create">
          <input
            type="text"
            className="proxmox-snapshot-create-input"
            value={newSnapName}
            onChange={(e) => setNewSnapName(e.target.value)}
            placeholder={t("proxmox.snapshot.create.label")}
            aria-label={t("proxmox.snapshot.create.label")}
            maxLength={40}
          />
          <button
            type="button"
            className="proxmox-action-btn"
            onClick={handleCreate}
            disabled={creating || !newSnapName.trim()}
            aria-label={t("proxmox.snapshot.create.btn")}
          >
            {t("proxmox.snapshot.create.btn")}
          </button>
        </div>
      </td>
    </tr>
  );
}

// ─── GuestTable — shared table renderer for both LXC and VM rows ─────────────

interface GuestTableProps {
  sessionId: string;
  guests: GuestRow[];
  kind: GuestKind;
  /** Whether the Shell button should be shown (LXC only). */
  showShell: boolean;
  expandedVmid: number | null;
  onLifecycle: (vmid: number, action: "start" | "stop" | "reboot", kind: GuestKind) => void;
  onShell: (guest: GuestRow) => void;
  onSnapshotsToggle: (guest: GuestRow, kind: GuestKind) => void;
  onSnapshotClose: () => void;
}

function GuestTable({
  sessionId,
  guests,
  kind,
  showShell,
  expandedVmid,
  onLifecycle,
  onShell,
  onSnapshotsToggle,
  onSnapshotClose,
}: GuestTableProps) {
  const { t } = useI18n();
  return (
    <table className="proxmox-container-table">
      <thead>
        <tr>
          <th scope="col">{t("proxmox.col.vmid")}</th>
          <th scope="col">{t("proxmox.col.name")}</th>
          <th scope="col">{t("proxmox.col.status")}</th>
          <th scope="col">{t("proxmox.col.actions")}</th>
        </tr>
      </thead>
      <tbody>
        {guests.map((g) => (
          <React.Fragment key={g.vmid}>
            <tr>
              <td className="proxmox-container-vmid">{g.vmid}</td>
              <td className="proxmox-container-name">{g.name}</td>
              <td>
                <span className={`proxmox-status-badge ${statusBadgeClass(g.status)}`}>
                  {g.status}
                </span>
              </td>
              <td className="proxmox-container-actions">
                <div className="proxmox-action-group">
                  {g.status !== "running" && (
                    <button
                      type="button"
                      className="proxmox-action-btn"
                      onClick={() => onLifecycle(g.vmid, "start", kind)}
                      aria-label={`${t("proxmox.action.start")} ${g.name}`}
                    >
                      {t("proxmox.action.start")}
                    </button>
                  )}
                  {g.status === "running" && (
                    <button
                      type="button"
                      className="proxmox-action-btn"
                      onClick={() => onLifecycle(g.vmid, "stop", kind)}
                      aria-label={`${t("proxmox.action.stop")} ${g.name}`}
                    >
                      {t("proxmox.action.stop")}
                    </button>
                  )}
                  <button
                    type="button"
                    className="proxmox-action-btn"
                    onClick={() => onLifecycle(g.vmid, "reboot", kind)}
                    aria-label={`${t("proxmox.action.reboot")} ${g.name}`}
                  >
                    {t("proxmox.action.reboot")}
                  </button>
                  {showShell && g.status === "running" && (
                    <button
                      type="button"
                      className="proxmox-action-btn"
                      onClick={() => onShell(g)}
                      aria-label={`${t("proxmox.action.shell")} ${g.name}`}
                    >
                      {t("proxmox.action.shell")}
                    </button>
                  )}
                  <button
                    type="button"
                    className={`proxmox-action-btn${expandedVmid === g.vmid ? " proxmox-action-btn--active" : ""}`}
                    onClick={() => onSnapshotsToggle(g, kind)}
                    aria-label={`${t("proxmox.action.snapshots")} ${g.name}`}
                    aria-expanded={expandedVmid === g.vmid}
                  >
                    {t("proxmox.action.snapshots")}
                  </button>
                </div>
              </td>
            </tr>
            {expandedVmid === g.vmid && (
              <SnapshotSubView
                key={`snap-${g.vmid}`}
                sessionId={sessionId}
                guest={g}
                kind={kind}
                onClose={onSnapshotClose}
              />
            )}
          </React.Fragment>
        ))}
      </tbody>
    </table>
  );
}

// ─── ProxmoxPanel ─────────────────────────────────────────────────────────────

interface ProxmoxPanelProps {
  sessionId: string;
}

export function ProxmoxPanel({ sessionId }: ProxmoxPanelProps) {
  const { t } = useI18n();
  const { refresh } = useProxmox(sessionId);

  const [actionError, setActionError] = useState<string | null>(null);
  // Expanded snapshot sub-view: tracks (vmid, kind) so LXC 100 and VM 100
  // don't collide if a Proxmox host has both with the same VMID.
  const [expandedVmid, setExpandedVmid] = useState<number | null>(null);
  const [expandedKind, setExpandedKind] = useState<GuestKind | null>(null);

  const containersOrUndef = useProxmoxStore((s) => s.containers.get(sessionId));
  const containers: LxcRow[] = containersOrUndef ?? [];
  const vmsOrUndef = useProxmoxStore((s) => s.vms.get(sessionId));
  const vms: VmRow[] = vmsOrUndef ?? [];

  const availability = useProxmoxStore((s) => s.availability.get(sessionId));
  const loading = useProxmoxStore((s) => s.loading.get(sessionId) ?? false);

  // ── pct not available (LXC side only — show warning, VMs may still work) ──

  if (availability === false && vms.length === 0) {
    return (
      <div className="proxmox-panel proxmox-unavailable" role="status">
        {t("proxmox.unavailable")}
      </div>
    );
  }

  // ── initial loading ──────────────────────────────────────────────────────

  if (availability === undefined && loading) {
    return (
      <div className="proxmox-panel proxmox-loading" role="status">
        {t("proxmox.loading")}
      </div>
    );
  }

  // ── Lifecycle action handler ─────────────────────────────────────────────

  async function handleLifecycle(
    vmid: number,
    action: "start" | "stop" | "reboot",
    kind: GuestKind,
  ) {
    setActionError(null);
    try {
      await tauriInvoke("proxmox_lifecycle_action", {
        sessionId,
        vmid: String(vmid),
        action,
        kind,
      });
      void refresh();
    } catch (err) {
      console.error(`[ProxmoxPanel] ${action} failed:`, err);
      setActionError(typeof err === "string" ? err : String(err));
    }
  }

  // ── Interactive shell handler (LXC only) — mirrors DockerPanel.handleShell ─

  async function handleShell(guest: GuestRow) {
    const isReady = (id: string | null | undefined): id is string =>
      !!id && !id.startsWith("pending-");

    let terminalId =
      useSessionStore.getState().sessions.get(sessionId)?.activeTerminalId;

    for (let i = 0; i < 50 && !isReady(terminalId); i++) {
      await new Promise((r) => setTimeout(r, 100));
      terminalId =
        useSessionStore.getState().sessions.get(sessionId)?.activeTerminalId;
    }

    if (!isReady(terminalId)) {
      console.warn("[ProxmoxPanel] No ready terminal for shell injection");
      return;
    }

    // Use the validated numeric vmid (from the row), never the name.
    const cmd = buildPctEnterCommand(String(guest.vmid));
    try {
      await tauriInvoke("write_terminal", {
        sessionId,
        terminalId,
        data: Array.from(new TextEncoder().encode(cmd)),
      });
    } catch (err) {
      console.error("[ProxmoxPanel] write_terminal failed:", err);
    }
  }

  // ── Snapshot sub-view toggle ─────────────────────────────────────────────

  async function handleSnapshotsToggle(guest: GuestRow, kind: GuestKind) {
    // If same (vmid, kind) is already expanded, collapse it.
    if (expandedVmid === guest.vmid && expandedKind === kind) {
      setExpandedVmid(null);
      setExpandedKind(null);
      return;
    }
    setExpandedVmid(guest.vmid);
    setExpandedKind(kind);
    // Load snapshots on expand.
    try {
      const result = await tauriInvoke<{ snapshots: SnapshotRow[] }>(
        "proxmox_list_snapshots",
        { sessionId, vmid: String(guest.vmid), kind },
      );
      useProxmoxStore
        .getState()
        .setSnapshots(sessionId, guest.vmid, result.snapshots);
    } catch (err) {
      console.error("[ProxmoxPanel] proxmox_list_snapshots failed:", err);
    }
  }

  function handleSnapshotClose() {
    setExpandedVmid(null);
    setExpandedKind(null);
  }

  const hasContainers = containers.length > 0;
  const hasVms = vms.length > 0;
  const isEmpty = !hasContainers && !hasVms && !loading;

  // ── Panel ────────────────────────────────────────────────────────────────

  return (
    <div className="proxmox-panel">
      {/* Header: refresh + optional error */}
      <div className="proxmox-panel-header">
        <button
          type="button"
          className="proxmox-refresh-btn"
          onClick={refresh}
          aria-label={t("proxmox.refresh")}
          title={t("proxmox.refresh")}
          disabled={loading}
        >
          <RefreshIcon />
        </button>
        {actionError && (
          <span className="proxmox-action-error" role="alert">
            {actionError}
          </span>
        )}
      </div>

      {/* Empty state */}
      {isEmpty && (
        <div className="proxmox-empty" role="status">
          {t("proxmox.empty")}
        </div>
      )}

      {/* LXC Containers section */}
      {hasContainers && (
        <section
          className="proxmox-containers"
          aria-label={t("proxmox.containers")}
        >
          <h3 className="proxmox-section-heading">{t("proxmox.containers")}</h3>
          <GuestTable
            sessionId={sessionId}
            guests={containers}
            kind="lxc"
            showShell={true}
            expandedVmid={expandedKind === "lxc" ? expandedVmid : null}
            onLifecycle={handleLifecycle}
            onShell={handleShell}
            onSnapshotsToggle={handleSnapshotsToggle}
            onSnapshotClose={handleSnapshotClose}
          />
        </section>
      )}

      {/* QEMU Virtual Machines section */}
      {hasVms && (
        <section
          className="proxmox-vms"
          aria-label={t("proxmox.virtualMachines")}
        >
          <h3 className="proxmox-section-heading">{t("proxmox.virtualMachines")}</h3>
          <GuestTable
            sessionId={sessionId}
            guests={vms}
            kind="vm"
            showShell={false}
            expandedVmid={expandedKind === "vm" ? expandedVmid : null}
            onLifecycle={handleLifecycle}
            onShell={handleShell}
            onSnapshotsToggle={handleSnapshotsToggle}
            onSnapshotClose={handleSnapshotClose}
          />
        </section>
      )}
    </div>
  );
}

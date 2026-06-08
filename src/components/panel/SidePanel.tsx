// components/panel/SidePanel.tsx — Collapsible terminal side panel
//
// Docks SftpBrowser and TunnelManager beside the live terminal in a flex-row
// layout. The terminal area is never unmounted when this panel is open.
//
// Architecture:
//   - Icon rail (role="toolbar"): two toggle buttons (SFTP, Tunnels)
//   - Content pane: conditionally mounts SftpBrowser or TunnelManager
//   - Width transition 200ms so ResizeObserver/FitAddon fires after CSS ends
//   - Drag handle on the left edge of the content pane for resizing

import { useCallback, useRef, useState } from "react";
import { useI18n } from "../../lib/i18n";
import {
  useWorkspaceStore,
  buildWorkspaceKey,
  PANEL_WIDTH_MIN,
  PANEL_WIDTH_MAX,
  PANEL_WIDTH_DEFAULT,
} from "../../stores/workspaceStore";
import { useSessionStore } from "../../stores/sessionStore";
import { TunnelManager } from "../../features/tunnel/TunnelManager";
import { HistoryPanel } from "../../features/history/HistoryPanel";
import { MonitoringPanel } from "../../features/monitoring/MonitoringPanel";
import { DockerPanel } from "../../features/docker/DockerPanel";
import { ProxmoxPanel } from "../../features/proxmox/ProxmoxPanel";
import { PasswordsPanel } from "../../features/passwords/PasswordsPanel";
import type { PanelSection } from "../../stores/workspaceStore";

// ── SVG icons (inline, no external dep) ─────────────────────────────────────

function FolderIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M1.5 3.5A1 1 0 0 1 2.5 2.5h3.293l1.5 1.5H13.5a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-8z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );
}

function TunnelIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M2 8c0-3.314 2.686-6 6-6s6 2.686 6 6-2.686 6-6 6"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      <path
        d="M5 8c0-1.657.672-3 1.5-3S8 6.343 8 8s-.672 3-1.5 3"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      <line
        x1="2"
        y1="8"
        x2="8"
        y2="8"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

function HistoryIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.2" />
      <path
        d="M8 5v3.5l2 1.5"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M4.5 4L3 2.5M4.5 4l.5-1.5"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

function MonitoringIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      {/* CPU activity line */}
      <polyline
        points="1,10 3,10 4,6 5,12 6,8 7,10 9,10"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      {/* Desktop monitor outline */}
      <rect
        x="1"
        y="2"
        width="14"
        height="9"
        rx="1"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      <line x1="6" y1="11" x2="6" y2="14" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
      <line x1="10" y1="11" x2="10" y2="14" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
      <line x1="4.5" y1="14" x2="11.5" y2="14" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function DockerIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      {/* Docker whale body */}
      <rect x="1" y="8" width="3" height="2" rx="0.5" stroke="currentColor" strokeWidth="1.1" fill="none" />
      <rect x="5" y="6" width="3" height="2" rx="0.5" stroke="currentColor" strokeWidth="1.1" fill="none" />
      <rect x="9" y="8" width="3" height="2" rx="0.5" stroke="currentColor" strokeWidth="1.1" fill="none" />
      <rect x="5" y="8" width="3" height="2" rx="0.5" stroke="currentColor" strokeWidth="1.1" fill="none" />
      <path
        d="M1 10.5c1 1.5 3 2 5.5 1.5s5 0 6.5-2"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
        fill="none"
      />
      <path
        d="M12.5 7.5c.5-1 .5-2 .2-2.5"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  );
}

function ProxmoxIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      {/* Proxmox VE: stylized server stack with a highlight bar */}
      <rect
        x="1"
        y="2"
        width="14"
        height="4"
        rx="1"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      <rect
        x="1"
        y="7"
        width="14"
        height="4"
        rx="1"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      <circle cx="12" cy="4" r="0.7" fill="currentColor" />
      <circle cx="12" cy="9" r="0.7" fill="currentColor" />
      <line
        x1="3"
        y1="4"
        x2="7"
        y2="4"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
      <line
        x1="3"
        y1="9"
        x2="7"
        y2="9"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
      {/* Container indicator line */}
      <line
        x1="4"
        y1="13"
        x2="12"
        y2="13"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

function PasswordsIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      {/* Padlock body */}
      <rect
        x="3"
        y="7"
        width="10"
        height="7"
        rx="1"
        stroke="currentColor"
        strokeWidth="1.2"
        fill="none"
      />
      {/* Shackle */}
      <path
        d="M5 7V5a3 3 0 0 1 6 0v2"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        fill="none"
      />
      {/* Keyhole */}
      <circle cx="8" cy="10" r="1" stroke="currentColor" strokeWidth="1.1" fill="none" />
      <line
        x1="8"
        y1="11"
        x2="8"
        y2="12.2"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M2 2l10 10M12 2L2 12"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

// ─────────────────────────────────────────────────────────────────────────────

export function SidePanel() {
  const { t } = useI18n();

  const { activeSessionId, sessions } = useSessionStore();

  const activeSession = activeSessionId
    ? sessions.get(activeSessionId)
    : undefined;

  const workspaceKey =
    activeSession
      ? buildWorkspaceKey(activeSession.profileId, activeSession.userId)
      : null;

  // Selector: subscribe only to the fields we actually render (panelOpen,
  // panelSection, setters). This avoids re-renders on unrelated store changes
  // such as sftp snapshot updates.
  const panelOpen = useWorkspaceStore((s) =>
    workspaceKey ? (s.workspaces[workspaceKey]?.panelOpen ?? false) : false,
  );
  const panelSection = useWorkspaceStore((s) =>
    workspaceKey
      ? ((s.workspaces[workspaceKey]?.panelSection ?? null) as PanelSection)
      : null,
  );
  const panelWidth = useWorkspaceStore((s) =>
    workspaceKey
      ? (s.workspaces[workspaceKey]?.panelWidth ?? PANEL_WIDTH_DEFAULT)
      : PANEL_WIDTH_DEFAULT,
  );
  const setPanelSection = useWorkspaceStore((s) => s.setPanelSection);
  const setPanelOpen = useWorkspaceStore((s) => s.setPanelOpen);
  const setPanelWidth = useWorkspaceStore((s) => s.setPanelWidth);
  const setMainView = useWorkspaceStore((s) => s.setMainView);

  // ── App-global passwords panel ──────────────────────────────────────────────
  // The password manager is session-independent: it can be opened with ZERO SSH
  // sessions active. Its open/closed state therefore lives in a GLOBAL store
  // slice, NOT in the per-workspace map keyed by `workspaceKey`.
  const passwordsPanelOpen = useWorkspaceStore((s) => s.passwordsPanelOpen);
  const setPasswordsPanelOpen = useWorkspaceStore((s) => s.setPasswordsPanelOpen);

  const sessionId = activeSession?.id ?? "";

  // The pane is open when either an SSH-scoped section is open OR the global
  // passwords panel is open. When passwords is open it is the visible section.
  const isPasswordsActive = passwordsPanelOpen;
  const contentOpen = panelOpen || passwordsPanelOpen;

  // ── Resize handle state ──────────────────────────────────────────────────────
  // isDragging suppresses the width CSS transition while drag is in progress so
  // the panel follows the pointer without fighting the animation.
  const isDraggingRef = useRef(false);
  const dragStartXRef = useRef(0);
  const dragStartWidthRef = useRef(PANEL_WIDTH_DEFAULT);
  const [isDragging, setIsDragging] = useState(false);

  const handleResizePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId);
      isDraggingRef.current = true;
      dragStartXRef.current = e.clientX;
      dragStartWidthRef.current = panelWidth;
      setIsDragging(true);
    },
    [panelWidth],
  );

  const handleResizePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!isDraggingRef.current || !workspaceKey) return;
      // Dragging left (negative delta) grows the panel; right shrinks it.
      // Panel is docked to the right, so left edge drag: delta = startX - currentX
      const delta = dragStartXRef.current - e.clientX;
      const newWidth = Math.min(
        PANEL_WIDTH_MAX,
        Math.max(PANEL_WIDTH_MIN, dragStartWidthRef.current + delta),
      );
      setPanelWidth(workspaceKey, newWidth);
    },
    [workspaceKey, setPanelWidth],
  );

  const handleResizePointerUp = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!isDraggingRef.current) return;
      isDraggingRef.current = false;
      e.currentTarget.releasePointerCapture(e.pointerId);
      setIsDragging(false);
    },
    [],
  );

  function handleToggle(section: "tunnel" | "history" | "monitoring" | "docker" | "proxmox") {
    if (!workspaceKey) return;
    const isActive = panelOpen && panelSection === section && !passwordsPanelOpen;
    if (isActive) {
      setPanelOpen(workspaceKey, false);
    } else {
      // Opening an SSH-scoped section takes over the single pane: close the
      // global passwords panel so only one section is visible at a time.
      if (passwordsPanelOpen) setPasswordsPanelOpen(false);
      setPanelSection(workspaceKey, section);
      setPanelOpen(workspaceKey, true);
    }
  }

  // Passwords is GLOBAL: it toggles independently of workspaceKey and must work
  // with zero active SSH sessions. Opening it makes it the visible section and
  // closes any open SSH-scoped section so the single pane shows one thing.
  function handlePasswordsToggle() {
    if (passwordsPanelOpen) {
      setPasswordsPanelOpen(false);
    } else {
      if (panelOpen && workspaceKey) setPanelOpen(workspaceKey, false);
      setPasswordsPanelOpen(true);
    }
  }

  function handleFilesToggle() {
    if (!workspaceKey) return;
    setMainView(workspaceKey, "files");
  }

  function handleClose() {
    if (passwordsPanelOpen) {
      setPasswordsPanelOpen(false);
      return;
    }
    if (!workspaceKey) return;
    setPanelOpen(workspaceKey, false);
  }

  // An SSH section is visible only when the workspace panel is open AND the
  // global passwords panel is NOT taking over the single pane.
  const sshSectionActive = panelOpen && !passwordsPanelOpen;

  return (
    <div className="side-panel-wrapper" data-open={contentOpen}>
      {/* Icon rail — always visible */}
      <div
        role="toolbar"
        aria-label={t("panel.sections")}
        className="side-panel-rail"
      >
        <button
          type="button"
          aria-pressed={false}
          aria-label={t("panel.sftp")}
          className="side-panel-rail-btn"
          onClick={handleFilesToggle}
          title={t("panel.sftp")}
        >
          <FolderIcon />
        </button>

        <button
          type="button"
          aria-pressed={sshSectionActive && panelSection === "tunnel"}
          aria-label={t("panel.tunnels")}
          className={`side-panel-rail-btn${sshSectionActive && panelSection === "tunnel" ? " side-panel-rail-btn-active" : ""}`}
          onClick={() => handleToggle("tunnel")}
          title={t("panel.tunnels")}
        >
          <TunnelIcon />
        </button>

        <button
          type="button"
          aria-pressed={sshSectionActive && panelSection === "history"}
          aria-label={t("panel.history")}
          className={`side-panel-rail-btn${sshSectionActive && panelSection === "history" ? " side-panel-rail-btn-active" : ""}`}
          onClick={() => handleToggle("history")}
          title={t("panel.history")}
        >
          <HistoryIcon />
        </button>

        <button
          type="button"
          aria-pressed={sshSectionActive && panelSection === "monitoring"}
          aria-label={t("panel.monitoring")}
          className={`side-panel-rail-btn${sshSectionActive && panelSection === "monitoring" ? " side-panel-rail-btn-active" : ""}`}
          onClick={() => handleToggle("monitoring")}
          title={t("panel.monitoring")}
        >
          <MonitoringIcon />
        </button>

        <button
          type="button"
          aria-pressed={sshSectionActive && panelSection === "docker"}
          aria-label={t("panel.docker")}
          className={`side-panel-rail-btn${sshSectionActive && panelSection === "docker" ? " side-panel-rail-btn-active" : ""}`}
          onClick={() => handleToggle("docker")}
          title={t("panel.docker")}
        >
          <DockerIcon />
        </button>

        <button
          type="button"
          aria-pressed={sshSectionActive && panelSection === "proxmox"}
          aria-label={t("panel.proxmox")}
          className={`side-panel-rail-btn${sshSectionActive && panelSection === "proxmox" ? " side-panel-rail-btn-active" : ""}`}
          onClick={() => handleToggle("proxmox")}
          title={t("panel.proxmox")}
        >
          <ProxmoxIcon />
        </button>

        <button
          type="button"
          aria-pressed={isPasswordsActive}
          aria-label={t("panel.passwords")}
          className={`side-panel-rail-btn${isPasswordsActive ? " side-panel-rail-btn-active" : ""}`}
          onClick={handlePasswordsToggle}
          title={t("panel.passwords")}
        >
          <PasswordsIcon />
        </button>
      </div>

      {/* Collapsible content pane */}
      <div
        className={`side-panel-content${contentOpen ? " side-panel-content-open" : ""}${isDragging ? " side-panel-content-dragging" : ""}`}
        style={contentOpen ? { width: `${panelWidth}px` } : undefined}
        aria-hidden={!contentOpen}
      >
        {contentOpen && (
          <>
            {/* Draggable resize handle — left edge of content pane.
               Resize is workspace-scoped; in passwords-only mode (no session)
               there is no per-workspace width to drag, so the handler is a no-op. */}
            <div
              className="side-panel-resize-handle"
              role="separator"
              aria-label="Resize panel"
              aria-orientation="vertical"
              onPointerDown={handleResizePointerDown}
              onPointerMove={handleResizePointerMove}
              onPointerUp={handleResizePointerUp}
            />
          </>
        )}
        {contentOpen && (
          <section
            aria-label={t("panel.region")}
            className="side-panel-section"
            style={{ width: `${panelWidth}px` }}
          >
            {/* Header with close button */}
            <div className="side-panel-header">
              <span className="side-panel-title">
                {isPasswordsActive
                  ? t("panel.passwords")
                  : panelSection === "history"
                    ? t("panel.history")
                    : panelSection === "monitoring"
                      ? t("panel.monitoring")
                      : panelSection === "docker"
                        ? t("panel.docker")
                        : panelSection === "proxmox"
                          ? t("panel.proxmox")
                          : t("panel.tunnels")}
              </span>
              <button
                type="button"
                aria-label={t("panel.close")}
                className="side-panel-close-btn"
                onClick={handleClose}
              >
                <CloseIcon />
              </button>
            </div>

            {/* Content — single pane. Passwords (global) takes precedence;
               otherwise the active SSH-scoped section is shown. */}
            <div className="side-panel-body">
              {isPasswordsActive ? (
                <PasswordsPanel />
              ) : (
                <>
                  {panelSection === "tunnel" && sessionId && (
                    <TunnelManager sessionId={sessionId} />
                  )}
                  {panelSection === "history" && sessionId && (
                    <HistoryPanel
                      sessionId={sessionId}
                      terminalId={activeSession?.activeTerminalId ?? null}
                      host={activeSession?.host ?? ""}
                    />
                  )}
                  {panelSection === "monitoring" && sessionId && (
                    <MonitoringPanel sessionId={sessionId} />
                  )}
                  {panelSection === "docker" && sessionId && (
                    <DockerPanel sessionId={sessionId} />
                  )}
                  {panelSection === "proxmox" && sessionId && (
                    <ProxmoxPanel sessionId={sessionId} />
                  )}
                </>
              )}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

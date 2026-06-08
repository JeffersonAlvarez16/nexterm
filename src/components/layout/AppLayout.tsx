// components/layout/AppLayout.tsx — Main application layout with sidebar + content

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";

interface AppLayoutProps {
  children: ReactNode;
  onConnect: (profileId: string, userId?: string) => void;
  onDisconnect: (sessionId: string) => void;
  onNewProfile: () => void;
  onEditProfile: (profileId: string) => void;
  connectingProfileId: string | null;
  connectError: string | null;
  onClearError: () => void;
  onStartTour?: () => void;
}

const SIDEBAR_COLLAPSED_STORAGE_KEY = "nexterm.sidebar.collapsed";
const SIDEBAR_WIDTH_STORAGE_KEY = "nexterm.sidebar.width";

// Resizable left sidebar bounds (px). Exported for tests.
export const SIDEBAR_WIDTH_DEFAULT = 300;
export const SIDEBAR_WIDTH_MIN = 220;
export const SIDEBAR_WIDTH_MAX = 520;

const clampSidebarWidth = (px: number) =>
  Math.min(SIDEBAR_WIDTH_MAX, Math.max(SIDEBAR_WIDTH_MIN, px));

function readStoredSidebarWidth(): number {
  if (typeof window === "undefined") return SIDEBAR_WIDTH_DEFAULT;
  const raw = window.localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY);
  const n = raw ? Number(raw) : NaN;
  // Ignore missing, non-numeric, or out-of-range persisted values.
  if (!Number.isFinite(n) || n < SIDEBAR_WIDTH_MIN || n > SIDEBAR_WIDTH_MAX) {
    return SIDEBAR_WIDTH_DEFAULT;
  }
  return n;
}

export function AppLayout({
  children,
  onConnect,
  onDisconnect,
  onNewProfile,
  onEditProfile,
  connectingProfileId,
  connectError,
  onClearError,
  onStartTour,
}: AppLayoutProps) {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY) === "true";
  });
  const [sidebarWidth, setSidebarWidth] = useState(readStoredSidebarWidth);
  const [resizing, setResizing] = useState(false);
  const dragRef = useRef<{ startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    window.localStorage.setItem(
      SIDEBAR_COLLAPSED_STORAGE_KEY,
      String(sidebarCollapsed),
    );
  }, [sidebarCollapsed]);

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  const handleResizePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      dragRef.current = { startX: e.clientX, startWidth: sidebarWidth };
      setResizing(true);
      e.currentTarget.setPointerCapture(e.pointerId);
    },
    [sidebarWidth],
  );

  const handleResizePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag) return;
      setSidebarWidth(
        clampSidebarWidth(drag.startWidth + (e.clientX - drag.startX)),
      );
    },
    [],
  );

  const endResize = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragRef.current) return;
    dragRef.current = null;
    setResizing(false);
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      // pointer was never captured (e.g. jsdom) — ignore
    }
  }, []);

  const handleResizeKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setSidebarWidth((w) => clampSidebarWidth(w - 16));
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        setSidebarWidth((w) => clampSidebarWidth(w + 16));
      }
    },
    [],
  );

  return (
    <div
      className={`app-layout ${sidebarCollapsed ? "app-layout-sidebar-collapsed" : ""}`}
      // Collapsed defers to the CSS var; expanded drives the first column from
      // the persisted px width. Transition is suppressed mid-drag for snappiness.
      style={
        sidebarCollapsed
          ? undefined
          : {
              gridTemplateColumns: `${sidebarWidth}px 1fr`,
              ...(resizing ? { transition: "none" } : null),
            }
      }
    >
      <Sidebar
        onConnect={onConnect}
        onDisconnect={onDisconnect}
        onNewProfile={onNewProfile}
        onEditProfile={onEditProfile}
        connectingProfileId={connectingProfileId}
        connectError={connectError}
        onClearError={onClearError}
        collapsed={sidebarCollapsed}
        onToggleCollapsed={() => setSidebarCollapsed((prev) => !prev)}
      />
      {!sidebarCollapsed && (
        <div
          className={`sidebar-resize-handle ${resizing ? "sidebar-resize-handle-active" : ""}`}
          role="separator"
          aria-label="Resize sidebar"
          aria-orientation="vertical"
          aria-valuenow={sidebarWidth}
          aria-valuemin={SIDEBAR_WIDTH_MIN}
          aria-valuemax={SIDEBAR_WIDTH_MAX}
          tabIndex={0}
          style={{ left: `${sidebarWidth}px` }}
          onPointerDown={handleResizePointerDown}
          onPointerMove={handleResizePointerMove}
          onPointerUp={endResize}
          onPointerCancel={endResize}
          onKeyDown={handleResizeKeyDown}
        />
      )}
      <main className="app-content">{children}</main>
      <StatusBar onStartTour={onStartTour} />
    </div>
  );
}

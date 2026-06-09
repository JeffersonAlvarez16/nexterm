// Sidebar.test.tsx — top-level passwords section entry
//
// The password manager is a top-level section (independent of any SSH session),
// surfaced as a footer entry in the left sidebar that toggles a global view
// flag. These tests cover that entry in both expanded and collapsed modes.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// ── i18n mock ─────────────────────────────────────────────────────────────────
vi.mock("../../lib/i18n", () => ({
  useI18n: () => ({
    t: (k: string) => {
      const labels: Record<string, string> = {
        "sidebar.passwords": "Passwords",
      };
      return labels[k] ?? k;
    },
  }),
}));

// ── Tauri dialog plugin (export/import) — not exercised here ─────────────────────
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

// ── profileStore mock ───────────────────────────────────────────────────────────
const mockLoadProfiles = vi.fn();
vi.mock("../../stores/profileStore", () => ({
  useProfileStore: vi.fn(() => ({
    profiles: [],
    loading: false,
    loadProfiles: mockLoadProfiles,
    deleteProfile: vi.fn(),
    reorderProfiles: vi.fn(),
    exportProfiles: vi.fn(),
    importProfiles: vi.fn(),
  })),
}));

// ── sessionStore mock ───────────────────────────────────────────────────────────
const mockSetActiveSession = vi.fn();
vi.mock("../../stores/sessionStore", () => ({
  useSessionStore: vi.fn(() => ({
    sessions: new Map(),
    activeSessionId: null,
    setActiveSession: mockSetActiveSession,
  })),
}));

// ── workspaceStore mock (selector-aware) ────────────────────────────────────────
const mockSetPasswordsViewOpen = vi.fn();
let _workspaceState: { passwordsViewOpen: boolean; setPasswordsViewOpen: typeof mockSetPasswordsViewOpen };
vi.mock("../../stores/workspaceStore", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  useWorkspaceStore: vi.fn((selector?: (s: any) => any) =>
    typeof selector === "function" ? selector(_workspaceState) : _workspaceState,
  ),
}));

import { Sidebar } from "./Sidebar";

const noop = () => {};

function renderSidebar(collapsed = false) {
  return render(
    <Sidebar
      onConnect={noop}
      onDisconnect={noop}
      onNewProfile={noop}
      onEditProfile={noop}
      connectingProfileId={null}
      connectError={null}
      onClearError={noop}
      collapsed={collapsed}
      onToggleCollapsed={noop}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  _workspaceState = {
    passwordsViewOpen: false,
    setPasswordsViewOpen: mockSetPasswordsViewOpen,
  };
});

describe("Sidebar — passwords section entry (expanded)", () => {
  it("renders a Passwords entry in the footer", () => {
    renderSidebar();
    expect(
      screen.getByRole("button", { name: "Passwords" }),
    ).toBeInTheDocument();
  });

  it("clicking the entry opens the passwords view via the global setter", () => {
    renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "Passwords" }));
    expect(mockSetPasswordsViewOpen).toHaveBeenCalledWith(true);
  });

  it("entry has aria-pressed=false when the view is closed", () => {
    renderSidebar();
    expect(
      screen.getByRole("button", { name: "Passwords" }),
    ).toHaveAttribute("aria-pressed", "false");
  });

  it("entry reflects the open view via aria-pressed and toggles it closed", () => {
    _workspaceState.passwordsViewOpen = true;
    renderSidebar();
    const btn = screen.getByRole("button", { name: "Passwords" });
    expect(btn).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(btn);
    expect(mockSetPasswordsViewOpen).toHaveBeenCalledWith(false);
  });
});

describe("Sidebar — passwords section entry (collapsed rail)", () => {
  it("renders a Passwords rail button when collapsed", () => {
    renderSidebar(true);
    expect(
      screen.getByRole("button", { name: "Passwords" }),
    ).toBeInTheDocument();
  });

  it("clicking the collapsed rail button opens the passwords view", () => {
    renderSidebar(true);
    fireEvent.click(screen.getByRole("button", { name: "Passwords" }));
    expect(mockSetPasswordsViewOpen).toHaveBeenCalledWith(true);
  });
});

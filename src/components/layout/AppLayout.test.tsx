// AppLayout.test.tsx — sidebar resize handle behaviour (Strict TDD)
//
// Verifies the LEFT sidebar is resizable by dragging its right edge:
//   - handle exists when the sidebar is expanded, absent when collapsed
//   - dragging updates the grid first column within clamp bounds
//   - the width persists to localStorage (key "nexterm.sidebar.width")

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// ── localStorage stub ─────────────────────────────────────────────────────────
const lsStore = vi.hoisted(() => {
  const store = new Map<string, string>();
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, String(v)),
      removeItem: (k: string) => void store.delete(k),
      clear: () => void store.clear(),
      key: (i: number) => [...store.keys()][i] ?? null,
      get length() {
        return store.size;
      },
    },
  });
  return store;
});

// ── jsdom pointer-capture shims ───────────────────────────────────────────────
// jsdom does not implement the Pointer Capture API; the resize handlers call it.
beforeEach(() => {
  if (!HTMLElement.prototype.setPointerCapture) {
    HTMLElement.prototype.setPointerCapture = vi.fn();
  }
  if (!HTMLElement.prototype.releasePointerCapture) {
    HTMLElement.prototype.releasePointerCapture = vi.fn();
  }
});

// ── child component mocks (not under test here) ───────────────────────────────
// The real Sidebar pulls in @dnd-kit, Tauri dialogs and several stores; mock it
// to a thin stub so this test exercises only AppLayout's resize logic.
vi.mock("./Sidebar", () => ({
  Sidebar: ({ collapsed }: { collapsed: boolean }) => (
    <aside data-testid="sidebar" data-collapsed={String(collapsed)} />
  ),
}));

vi.mock("./StatusBar", () => ({
  StatusBar: () => <div data-testid="status-bar" />,
}));

import {
  AppLayout,
  SIDEBAR_WIDTH_DEFAULT,
  SIDEBAR_WIDTH_MIN,
  SIDEBAR_WIDTH_MAX,
} from "./AppLayout";

const noop = () => {};

function renderLayout() {
  return render(
    <AppLayout
      onConnect={noop}
      onDisconnect={noop}
      onNewProfile={noop}
      onEditProfile={noop}
      connectingProfileId={null}
      connectError={null}
      onClearError={noop}
    >
      <div data-testid="content" />
    </AppLayout>,
  );
}

beforeEach(() => {
  lsStore.clear();
  vi.clearAllMocks();
});

describe("AppLayout — sidebar resize handle", () => {
  it("renders the resize handle when the sidebar is expanded", () => {
    renderLayout();
    expect(
      screen.getByRole("separator", { name: "Resize sidebar" }),
    ).toBeInTheDocument();
  });

  it("drives the grid first column from the default width on first render", () => {
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    expect(layout.style.gridTemplateColumns).toBe(
      `${SIDEBAR_WIDTH_DEFAULT}px 1fr`,
    );
  });

  it("dragging the handle to the right grows the sidebar width within clamp bounds", () => {
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    const handle = screen.getByRole("separator", { name: "Resize sidebar" });

    fireEvent.pointerDown(handle, { pointerId: 1, clientX: 300 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientX: 360 });
    fireEvent.pointerUp(handle, { pointerId: 1, clientX: 360 });

    // 300 default + 60 delta = 360, inside [220, 520]
    expect(layout.style.gridTemplateColumns).toBe("360px 1fr");
  });

  it("clamps the width to the maximum when dragged far right", () => {
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    const handle = screen.getByRole("separator", { name: "Resize sidebar" });

    fireEvent.pointerDown(handle, { pointerId: 1, clientX: 300 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientX: 2000 });
    fireEvent.pointerUp(handle, { pointerId: 1, clientX: 2000 });

    expect(layout.style.gridTemplateColumns).toBe(`${SIDEBAR_WIDTH_MAX}px 1fr`);
  });

  it("clamps the width to the minimum when dragged far left", () => {
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    const handle = screen.getByRole("separator", { name: "Resize sidebar" });

    fireEvent.pointerDown(handle, { pointerId: 1, clientX: 300 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientX: -2000 });
    fireEvent.pointerUp(handle, { pointerId: 1, clientX: -2000 });

    expect(layout.style.gridTemplateColumns).toBe(`${SIDEBAR_WIDTH_MIN}px 1fr`);
  });

  it("persists the new width to localStorage after a drag", () => {
    renderLayout();
    const handle = screen.getByRole("separator", { name: "Resize sidebar" });

    fireEvent.pointerDown(handle, { pointerId: 1, clientX: 300 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientX: 350 });
    fireEvent.pointerUp(handle, { pointerId: 1, clientX: 350 });

    expect(localStorage.getItem("nexterm.sidebar.width")).toBe("350");
  });

  it("restores the persisted width on mount", () => {
    localStorage.setItem("nexterm.sidebar.width", "420");
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    expect(layout.style.gridTemplateColumns).toBe("420px 1fr");
  });

  it("ignores out-of-range persisted widths and falls back to the default", () => {
    localStorage.setItem("nexterm.sidebar.width", "99999");
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;
    expect(layout.style.gridTemplateColumns).toBe(
      `${SIDEBAR_WIDTH_DEFAULT}px 1fr`,
    );
  });

  it("hides the resize handle and clears the inline grid width when collapsed", () => {
    localStorage.setItem("nexterm.sidebar.collapsed", "true");
    const { container } = renderLayout();
    const layout = container.querySelector(".app-layout") as HTMLElement;

    expect(
      screen.queryByRole("separator", { name: "Resize sidebar" }),
    ).not.toBeInTheDocument();
    // Collapsed must defer to the CSS var, not the persisted px width.
    expect(layout.style.gridTemplateColumns).toBe("");
  });
});

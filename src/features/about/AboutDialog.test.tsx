// features/about/AboutDialog.test.tsx — Unit tests for the About dialog

import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { AboutDialog } from "./AboutDialog";

// Polyfill HTMLDialogElement for jsdom
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
});

// ── Mock i18n ─────────────────────────────────────────────────
vi.mock("../../lib/i18n", () => ({
  useI18n: () => ({
    t: (key: string, vars?: Record<string, string>) => {
      if (vars && key === "about.version") return `Version ${vars.version}`;
      return key;
    },
  }),
}));

// ── Mock @tauri-apps/api/app ──────────────────────────────────
const { mockGetVersion } = vi.hoisted(() => ({
  mockGetVersion: vi.fn().mockResolvedValue("1.2.3"),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: mockGetVersion,
}));

// ── Mock @tauri-apps/plugin-opener ────────────────────────────
const { mockOpenUrl } = vi.hoisted(() => ({
  mockOpenUrl: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: mockOpenUrl,
}));

// ── Mock useUpdater ───────────────────────────────────────────
const { mockCheckForUpdate } = vi.hoisted(() => ({
  mockCheckForUpdate: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../updater/useUpdater", () => ({
  useUpdater: () => ({
    checkForUpdate: mockCheckForUpdate,
    downloadAndInstall: vi.fn(),
    dismissUpdate: vi.fn(),
  }),
}));

// ── Mock updateStore ─────────────────────────────────────────
const { mockGetState } = vi.hoisted(() => ({
  mockGetState: vi.fn().mockReturnValue({ status: "idle" }),
}));

vi.mock("../../stores/updateStore", () => ({
  useUpdateStore: Object.assign(vi.fn(), {
    getState: mockGetState,
  }),
}));

// ── Mock constants ───────────────────────────────────────────
vi.mock("../../lib/constants", () => ({
  APP_NAME: "NexTerm",
  REPO_URL: "https://github.com/JeffersonAlvarez16/nexterm",
}));

// ── Helpers ──────────────────────────────────────────────────
function renderDialog(open = true, onClose = vi.fn()) {
  return render(<AboutDialog open={open} onClose={onClose} />);
}

// ── Tests ────────────────────────────────────────────────────

describe("AboutDialog", () => {
  it("renders app name and version when open", async () => {
    renderDialog();
    expect(screen.getByText("NexTerm")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByText("Version 1.2.3")).toBeInTheDocument(),
    );
  });

  it("calls openUrl with REPO_URL when GitHub button is clicked", async () => {
    renderDialog();
    const btn = screen.getByText("about.viewOnGitHub");
    fireEvent.click(btn);
    await waitFor(() =>
      expect(mockOpenUrl).toHaveBeenCalledWith(
        "https://github.com/JeffersonAlvarez16/nexterm",
      ),
    );
  });

  it("calls checkForUpdate when check-updates button is clicked", async () => {
    renderDialog();
    const btn = screen.getByText("about.checkUpdates");
    fireEvent.click(btn);
    await waitFor(() => expect(mockCheckForUpdate).toHaveBeenCalled());
  });

  it("shows up-to-date message when no update is available", async () => {
    mockGetState.mockReturnValue({ status: "idle" });
    renderDialog();
    const btn = screen.getByText("about.checkUpdates");
    fireEvent.click(btn);
    await waitFor(() =>
      expect(screen.getByText("about.upToDate")).toBeInTheDocument(),
    );
  });

  it("calls onClose when update is available", async () => {
    mockGetState.mockReturnValue({ status: "available" });
    const onClose = vi.fn();
    renderDialog(true, onClose);
    const btn = screen.getByText("about.checkUpdates");
    fireEvent.click(btn);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("does not render content when closed", () => {
    renderDialog(false);
    expect(screen.queryByText("NexTerm")).not.toBeInTheDocument();
  });
});

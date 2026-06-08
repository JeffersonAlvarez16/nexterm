// features/passwords/PasswordList.test.tsx
//
// FIX #3 follow-through: the reveal grant is SINGLE-USE and ID-BOUND. Every
// reveal/copy opens a re-auth prompt and calls reauth(id, master) for THAT row
// immediately before reveal(id) — no prior grant is assumed to cover the row.
// Also covers the new "Set password" affordance (FIX #4).

import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function close() {
    this.open = false;
  };
});

vi.mock("../../lib/i18n", () => ({
  useI18n: () => ({ t: (k: string) => k, locale: "en", setLocale: vi.fn() }),
  I18nProvider: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("../../lib/clipboard", () => ({
  copySecret: vi.fn().mockResolvedValue(undefined),
}));

// Capture the registered "pw-focus-lost" handler so the test can fire it.
const focusLostHandlers: Array<(e: unknown) => void> = [];
const unlistenSpy = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: unknown) => void) => {
    if (event === "pw-focus-lost") focusLostHandlers.push(handler);
    return Promise.resolve(unlistenSpy);
  }),
}));

import { PasswordList } from "./PasswordList";
import {
  usePasswordStore,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";

const META: PasswordEntryMeta = {
  id: "row-1",
  title: "GitHub",
  username: "octocat",
  url: "https://github.com",
  category: "Work",
  createdAt: 1,
  updatedAt: 2,
};

const reauth = vi.fn().mockResolvedValue(30);
const reveal = vi.fn().mockResolvedValue("the-plaintext");
const remove = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  vi.clearAllMocks();
  focusLostHandlers.length = 0;
  usePasswordStore.setState({
    entries: [META],
    loading: false,
    reauth,
    reveal,
    remove,
  } as never);
});

describe("PasswordList — single-use, id-bound reveal", () => {
  it("reveal re-auths for THAT row then reveals it", async () => {
    render(<PasswordList onEdit={() => {}} onSetPassword={() => {}} />);

    // Click reveal on the row -> a re-auth dialog opens.
    fireEvent.click(screen.getByRole("button", { name: "passwords.reveal" }));

    const masterInput = document.getElementById("pw-reauth-password") as HTMLInputElement;
    expect(masterInput).not.toBeNull();
    fireEvent.change(masterInput, { target: { value: "master-pw" } });

    // Confirm the re-auth.
    fireEvent.click(screen.getByText("passwords.reauth.confirm"));

    await vi.waitFor(() => expect(reauth).toHaveBeenCalledTimes(1));
    // Bound to THIS row's id, in (id, master) order.
    expect(reauth).toHaveBeenCalledWith(META.id, "master-pw");
    await vi.waitFor(() => expect(reveal).toHaveBeenCalledWith(META.id));
  });

  it("'Set password' triggers the onSetPassword callback for the row", () => {
    const onSetPassword = vi.fn();
    render(<PasswordList onEdit={() => {}} onSetPassword={onSetPassword} />);

    fireEvent.click(screen.getByRole("button", { name: "passwords.setPassword" }));

    expect(onSetPassword).toHaveBeenCalledWith(META);
  });
});

describe("PasswordList — lock-on-blur hides revealed secrets", () => {
  it("clears a revealed secret when the 'pw-focus-lost' event fires", async () => {
    render(<PasswordList onEdit={() => {}} onSetPassword={() => {}} />);

    // Reveal a row: open re-auth, enter master, confirm.
    fireEvent.click(screen.getByRole("button", { name: "passwords.reveal" }));
    const masterInput = document.getElementById("pw-reauth-password") as HTMLInputElement;
    fireEvent.change(masterInput, { target: { value: "master-pw" } });
    fireEvent.click(screen.getByText("passwords.reauth.confirm"));

    // The plaintext is now visible on screen.
    await vi.waitFor(() => expect(screen.getByText("the-plaintext")).toBeInTheDocument());

    // The frontend registered a "pw-focus-lost" listener.
    expect(focusLostHandlers.length).toBeGreaterThan(0);

    // Window blur (focus lost) fires the backend event -> secret must vanish.
    focusLostHandlers.forEach((h) => h({ event: "pw-focus-lost", payload: null }));

    await vi.waitFor(() =>
      expect(screen.queryByText("the-plaintext")).not.toBeInTheDocument(),
    );
  });
});

// features/passwords/PasswordSecretDialog.test.tsx
//
// FIX #4 (set/replace password): the secret dialog REPLACES the stored password
// (and notes) via updateSecret and never via the lossy full-entry update.

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

import { PasswordSecretDialog } from "./PasswordSecretDialog";
import {
  usePasswordStore,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";

const META: PasswordEntryMeta = {
  id: "deadbeef",
  title: "GitHub",
  username: "octocat",
  url: "https://github.com",
  category: "Work",
  createdAt: 1,
  updatedAt: 2,
};

const updateSecret = vi.fn().mockResolvedValue(undefined);
const generate = vi.fn().mockResolvedValue("generated-pass");

beforeEach(() => {
  vi.clearAllMocks();
  usePasswordStore.setState({ updateSecret, generate } as never);
});

describe("PasswordSecretDialog", () => {
  it("requires a new password and does not call updateSecret when empty", async () => {
    render(<PasswordSecretDialog open entry={META} onClose={() => {}} />);

    fireEvent.click(screen.getByText("passwords.secret.save"));

    await vi.waitFor(() =>
      expect(screen.getByText("passwords.entry.passwordRequired")).toBeInTheDocument(),
    );
    expect(updateSecret).not.toHaveBeenCalled();
  });

  it("replaces the secret via updateSecret with the new password + notes", async () => {
    render(<PasswordSecretDialog open entry={META} onClose={() => {}} />);

    fireEvent.change(document.getElementById("pw-secret-password")!, {
      target: { value: "brand-new-secret" },
    });
    fireEvent.change(document.getElementById("pw-secret-notes-field")!, {
      target: { value: "rotated" },
    });
    fireEvent.click(screen.getByText("passwords.secret.save"));

    await vi.waitFor(() => expect(updateSecret).toHaveBeenCalledTimes(1));
    expect(updateSecret).toHaveBeenCalledWith(META.id, {
      password: "brand-new-secret",
      notes: "rotated",
    });
  });
});

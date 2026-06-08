// features/passwords/PasswordEntryDialog.test.tsx
//
// FIX #4 (lossless edit) regression tests:
//   - EDIT mode is METADATA-ONLY: it renders no password/notes inputs and saves
//     via updateMeta (never update / pw_update), so a title change can never
//     silently wipe the stored secret.
//   - ADD mode keeps the full form and still requires a password.

import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// jsdom does not implement the native <dialog> modal API.
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function close() {
    this.open = false;
  };
});

// i18n: return the key verbatim so assertions can match on stable keys.
vi.mock("../../lib/i18n", () => ({
  useI18n: () => ({ t: (k: string) => k, locale: "en", setLocale: vi.fn() }),
  I18nProvider: ({ children }: { children: React.ReactNode }) => children,
}));

import { PasswordEntryDialog } from "./PasswordEntryDialog";
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

const add = vi.fn().mockResolvedValue("new-id");
const update = vi.fn().mockResolvedValue(undefined);
const updateMeta = vi.fn().mockResolvedValue(undefined);
const generate = vi.fn().mockResolvedValue("generated-pass");

beforeEach(() => {
  vi.clearAllMocks();
  usePasswordStore.setState({ add, update, updateMeta, generate } as never);
});

describe("PasswordEntryDialog — edit mode is metadata-only", () => {
  it("renders NO password or notes fields in edit mode", () => {
    render(<PasswordEntryDialog open entry={META} onClose={() => {}} />);

    expect(document.getElementById("pw-entry-title")).not.toBeNull();
    // The secret inputs must not exist in edit mode.
    expect(document.getElementById("pw-entry-password")).toBeNull();
    expect(document.getElementById("pw-entry-notes-field")).toBeNull();
  });

  it("saves a title change via updateMeta and NEVER via update (no silent wipe)", async () => {
    render(<PasswordEntryDialog open entry={META} onClose={() => {}} />);

    const title = document.getElementById("pw-entry-title") as HTMLInputElement;
    fireEvent.change(title, { target: { value: "GitHub Renamed" } });
    fireEvent.click(screen.getByText("general.save"));

    await vi.waitFor(() => expect(updateMeta).toHaveBeenCalledTimes(1));
    expect(updateMeta).toHaveBeenCalledWith(META.id, {
      title: "GitHub Renamed",
      username: "octocat",
      url: "https://github.com",
      category: "Work",
    });
    // The lossy full-entry update path is never used for edits.
    expect(update).not.toHaveBeenCalled();
  });
});

describe("PasswordEntryDialog — add mode", () => {
  it("renders the password field and requires a password before saving", async () => {
    render(<PasswordEntryDialog open entry={null} onClose={() => {}} />);

    expect(document.getElementById("pw-entry-password")).not.toBeNull();

    const title = document.getElementById("pw-entry-title") as HTMLInputElement;
    fireEvent.change(title, { target: { value: "New Entry" } });
    // Submit with an empty password — must be rejected, add not called.
    fireEvent.click(screen.getByText("general.save"));

    await vi.waitFor(() =>
      expect(screen.getByText("passwords.entry.passwordRequired")).toBeInTheDocument(),
    );
    expect(add).not.toHaveBeenCalled();
  });

  it("saves the full entry via add when a password is provided", async () => {
    render(<PasswordEntryDialog open entry={null} onClose={() => {}} />);

    fireEvent.change(document.getElementById("pw-entry-title")!, {
      target: { value: "New Entry" },
    });
    fireEvent.change(document.getElementById("pw-entry-password")!, {
      target: { value: "s3cret" },
    });
    fireEvent.click(screen.getByText("general.save"));

    await vi.waitFor(() => expect(add).toHaveBeenCalledTimes(1));
    expect(add).toHaveBeenCalledWith(
      expect.objectContaining({ title: "New Entry", password: "s3cret" }),
    );
  });
});

describe("PasswordEntryDialog — generator options", () => {
  it("calls generate() with the user-chosen length and charset toggles", async () => {
    render(<PasswordEntryDialog open entry={null} onClose={() => {}} />);

    // Open the generator options popover.
    fireEvent.click(screen.getByLabelText("passwords.generator.open"));

    // Change length 20 -> 32 and disable symbols, keep digits + uppercase on.
    const length = document.getElementById("pw-gen-length") as HTMLInputElement;
    fireEvent.change(length, { target: { value: "32" } });
    fireEvent.click(document.getElementById("pw-gen-symbols")!);

    // Trigger generation with the chosen options.
    fireEvent.click(screen.getByText("passwords.generator.generate"));

    await vi.waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
    // generate(length, symbols, digits, uppercase) with the chosen args.
    expect(generate).toHaveBeenCalledWith(32, false, true, true);

    // The generated value lands in the form's password field (local state only).
    await vi.waitFor(() => {
      const pw = document.getElementById("pw-entry-password") as HTMLInputElement;
      expect(pw.value).toBe("generated-pass");
    });
  });
});

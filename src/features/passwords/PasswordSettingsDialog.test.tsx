// features/passwords/PasswordSettingsDialog.test.tsx
//
// FIX #8 (settings UI): the gear dialog wires changeMaster(old,new) and
// setIdleTimeout(secs), seeding the timeout select from status.idleTimeoutSecs.

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

// PasswordStrength uses its own useI18n; with the mock above it renders fine.

import { PasswordSettingsDialog } from "./PasswordSettingsDialog";
import { usePasswordStore } from "../../stores/passwordStore";

const changeMaster = vi.fn().mockResolvedValue(undefined);
const setIdleTimeout = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  vi.clearAllMocks();
  usePasswordStore.setState({
    changeMaster,
    setIdleTimeout,
    status: {
      exists: true,
      unlocked: true,
      idleTimeoutSecs: 300,
      secondsUntilLock: 300,
    },
  } as never);
});

describe("PasswordSettingsDialog — change master", () => {
  it("rejects a new password shorter than 12 chars and does not call changeMaster", async () => {
    render(<PasswordSettingsDialog open onClose={() => {}} />);

    fireEvent.change(document.getElementById("pw-settings-old")!, {
      target: { value: "old-master-pw" },
    });
    fireEvent.change(document.getElementById("pw-settings-new")!, {
      target: { value: "short" },
    });
    fireEvent.change(document.getElementById("pw-settings-confirm")!, {
      target: { value: "short" },
    });
    fireEvent.click(screen.getByText("passwords.settings.changeMasterAction"));

    await vi.waitFor(() =>
      expect(screen.getByText("passwords.passwordTooShort")).toBeInTheDocument(),
    );
    expect(changeMaster).not.toHaveBeenCalled();
  });

  it("calls changeMaster(old,new) when the form is valid", async () => {
    render(<PasswordSettingsDialog open onClose={() => {}} />);

    fireEvent.change(document.getElementById("pw-settings-old")!, {
      target: { value: "old-master-pw" },
    });
    fireEvent.change(document.getElementById("pw-settings-new")!, {
      target: { value: "a-brand-new-master-password" },
    });
    fireEvent.change(document.getElementById("pw-settings-confirm")!, {
      target: { value: "a-brand-new-master-password" },
    });
    fireEvent.click(screen.getByText("passwords.settings.changeMasterAction"));

    await vi.waitFor(() => expect(changeMaster).toHaveBeenCalledTimes(1));
    expect(changeMaster).toHaveBeenCalledWith(
      "old-master-pw",
      "a-brand-new-master-password",
    );
  });
});

describe("PasswordSettingsDialog — idle timeout", () => {
  it("seeds the select from status.idleTimeoutSecs (300s) and saves a changed value", async () => {
    render(<PasswordSettingsDialog open onClose={() => {}} />);

    const select = document.getElementById("pw-settings-idle") as HTMLSelectElement;
    // 300s == 5 min is the seeded current value.
    expect(select.value).toBe("300");

    // Change to 15 min (900s) and save.
    fireEvent.change(select, { target: { value: "900" } });
    fireEvent.click(screen.getByText("passwords.settings.idleAction"));

    await vi.waitFor(() => expect(setIdleTimeout).toHaveBeenCalledTimes(1));
    expect(setIdleTimeout).toHaveBeenCalledWith(900);
  });
});

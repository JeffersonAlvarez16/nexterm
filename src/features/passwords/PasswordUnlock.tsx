// features/passwords/PasswordUnlock.tsx — Second-access gate for the password
// manager (unlock-only clone of VaultScreen).
//
// Shown when passwords.json EXISTS but the manager is locked. Independent from
// the SSH vault unlock — this is a separate master password.
//
// Includes a guarded "forgot password" reset that ERASES all saved passwords
// (pw_reset), behind an explicit double-confirm warning.

import { useCallback, useState } from "react";
import { Input } from "../../components/ui/Input";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { useI18n } from "../../lib/i18n";
import { usePasswordStore } from "../../stores/passwordStore";

export function PasswordUnlock() {
  const { t } = useI18n();
  const unlock = usePasswordStore((s) => s.unlock);
  const resetStore = usePasswordStore((s) => s.resetStore);

  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [capsLock, setCapsLock] = useState(false);

  // Reset flow: a two-step confirm so an erase is never a single click.
  const [showReset, setShowReset] = useState(false);
  const [resetConfirmed, setResetConfirmed] = useState(false);
  const [resetting, setResetting] = useState(false);

  const handleCapsLock = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    setCapsLock(e.getModifierState("CapsLock"));
  }, []);

  const handleUnlock = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);
      if (!password.trim()) {
        setError(t("passwords.passwordRequired"));
        return;
      }
      setLoading(true);
      try {
        await unlock(password);
        setPassword("");
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes("Wrong master password") || msg.includes("Decryption failed")) {
          setError(t("passwords.wrongPassword"));
        } else {
          setError(msg);
        }
      } finally {
        setLoading(false);
      }
    },
    [password, unlock, t],
  );

  const closeReset = useCallback(() => {
    setShowReset(false);
    setResetConfirmed(false);
  }, []);

  const handleReset = useCallback(async () => {
    setResetting(true);
    try {
      await resetStore();
      setPassword("");
      setError(null);
      closeReset();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      closeReset();
    } finally {
      setResetting(false);
    }
  }, [resetStore, closeReset]);

  return (
    <div className="pw-gate">
      <div className="pw-gate-icon" aria-hidden="true">
        <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
          <path d="M7 11V7a5 5 0 0 1 10 0v4" />
          <circle cx="12" cy="16" r="1" />
        </svg>
      </div>

      <h2 className="pw-gate-title">{t("passwords.title")}</h2>
      <p className="pw-gate-subtitle">{t("passwords.unlockMessage")}</p>

      <form onSubmit={handleUnlock} className="pw-gate-form" autoComplete="off">
        <Input
          id="pw-unlock-password"
          type="password"
          label={t("passwords.masterPassword")}
          value={password}
          onChange={(e) => {
            setPassword(e.target.value);
            setError(null);
          }}
          onKeyUp={handleCapsLock}
          onKeyDown={handleCapsLock}
          placeholder={t("passwords.unlockPlaceholder")}
          reveal
          revealLabel={t("passwords.reveal")}
          hideLabel={t("passwords.hide")}
          autoFocus
        />

        {capsLock && (
          <p className="pw-caps-warning" role="alert">
            {t("passwords.capsLockOn")}
          </p>
        )}

        {error && <p className="pw-error" role="alert">{error}</p>}

        <Button
          type="submit"
          disabled={loading || !password.trim()}
          style={{ width: "100%", justifyContent: "center" }}
        >
          {loading ? <Spinner size={14} /> : t("passwords.unlock")}
        </Button>

        <button
          type="button"
          className="pw-forgot-link"
          onClick={() => setShowReset(true)}
        >
          {t("passwords.forgotPassword")}
        </button>
      </form>

      {/* ── Reset (erase) warning — double confirm ── */}
      {showReset && (
        <div
          className="pw-reset-backdrop"
          onClick={() => !resetting && closeReset()}
        >
          <div className="pw-reset-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="pw-reset-icon" aria-hidden="true">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
            </div>
            <h3 className="pw-reset-title">{t("passwords.reset.title")}</h3>
            <p className="pw-reset-message">{t("passwords.reset.warning")}</p>
            <p className="pw-reset-irreversible">{t("passwords.reset.irreversible")}</p>

            <label className="pw-reset-confirm-check">
              <input
                type="checkbox"
                checked={resetConfirmed}
                onChange={(e) => setResetConfirmed(e.target.checked)}
              />
              <span>{t("passwords.reset.confirmCheck")}</span>
            </label>

            <div className="pw-reset-actions">
              <Button variant="ghost" onClick={closeReset} disabled={resetting}>
                {t("general.cancel")}
              </Button>
              <Button
                variant="danger"
                onClick={() => void handleReset()}
                disabled={resetting || !resetConfirmed}
              >
                {resetting ? <Spinner size={14} /> : t("passwords.reset.confirm")}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

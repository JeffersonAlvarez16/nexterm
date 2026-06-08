// features/passwords/PasswordSettingsDialog.tsx — Password-manager settings
//
// Visible only when the password manager is unlocked. Two independent sections:
//   1. Change master password (old + new + confirm, min 12, strength meter) ->
//      changeMaster(old, new).
//   2. Auto-lock idle timeout (1 / 5 / 15 / 30 min) -> setIdleTimeout(secs),
//      seeded from the current status.idleTimeoutSecs.
//
// SECURITY: the typed passwords live ONLY in local form state and are wiped on
// close/cancel/success.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Dialog } from "../../components/ui/Dialog";
import { Input } from "../../components/ui/Input";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { PasswordStrength } from "../vault/PasswordStrength";
import { useI18n } from "../../lib/i18n";
import { usePasswordStore } from "../../stores/passwordStore";

interface PasswordSettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

/** Minimum master-password length (mirrors the create gate). */
const MIN_MASTER_PASSWORD_LENGTH = 12;

/** Selectable idle-timeout options, in minutes. */
const IDLE_TIMEOUT_MINUTES = [1, 5, 15, 30] as const;

export function PasswordSettingsDialog({ open, onClose }: PasswordSettingsDialogProps) {
  const { t } = useI18n();
  const status = usePasswordStore((s) => s.status);
  const changeMaster = usePasswordStore((s) => s.changeMaster);
  const setIdleTimeout = usePasswordStore((s) => s.setIdleTimeout);

  // ── Change-master form ──
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [masterError, setMasterError] = useState<string | null>(null);
  const [masterDone, setMasterDone] = useState(false);
  const [savingMaster, setSavingMaster] = useState(false);

  // ── Idle-timeout select ──
  const currentTimeoutSecs = status?.idleTimeoutSecs ?? IDLE_TIMEOUT_MINUTES[1] * 60;
  const [timeoutSecs, setTimeoutSecs] = useState<number>(currentTimeoutSecs);
  const [timeoutError, setTimeoutError] = useState<string | null>(null);
  const [timeoutDone, setTimeoutDone] = useState(false);
  const [savingTimeout, setSavingTimeout] = useState(false);

  // The select shows the canonical 1/5/15/30 options plus, if the backend
  // currently reports some other value, that value too so the UI never lies.
  const timeoutOptions = useMemo(() => {
    const base = IDLE_TIMEOUT_MINUTES.map((m) => m * 60);
    if (!base.includes(currentTimeoutSecs)) base.push(currentTimeoutSecs);
    return base.sort((a, b) => a - b);
  }, [currentTimeoutSecs]);

  // Reset all form state and re-seed the timeout from status on each open.
  useEffect(() => {
    if (open) {
      setOldPassword("");
      setNewPassword("");
      setConfirmPassword("");
      setMasterError(null);
      setMasterDone(false);
      setTimeoutSecs(currentTimeoutSecs);
      setTimeoutError(null);
      setTimeoutDone(false);
    }
  }, [open, currentTimeoutSecs]);

  const wipeAndClose = useCallback(() => {
    setOldPassword("");
    setNewPassword("");
    setConfirmPassword("");
    setMasterError(null);
    setMasterDone(false);
    setTimeoutError(null);
    setTimeoutDone(false);
    onClose();
  }, [onClose]);

  const handleChangeMaster = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setMasterError(null);
      setMasterDone(false);

      if (!oldPassword) {
        setMasterError(t("passwords.passwordRequired"));
        return;
      }
      if (newPassword.length < MIN_MASTER_PASSWORD_LENGTH) {
        setMasterError(t("passwords.passwordTooShort", { min: MIN_MASTER_PASSWORD_LENGTH }));
        return;
      }
      if (newPassword !== confirmPassword) {
        setMasterError(t("passwords.passwordMismatch"));
        return;
      }

      setSavingMaster(true);
      try {
        await changeMaster(oldPassword, newPassword);
        setOldPassword("");
        setNewPassword("");
        setConfirmPassword("");
        setMasterDone(true);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes("Wrong master password") || msg.includes("Decryption failed")) {
          setMasterError(t("passwords.wrongPassword"));
        } else {
          setMasterError(msg);
        }
      } finally {
        setSavingMaster(false);
      }
    },
    [oldPassword, newPassword, confirmPassword, changeMaster, t],
  );

  const handleSaveTimeout = useCallback(async () => {
    setTimeoutError(null);
    setTimeoutDone(false);
    setSavingTimeout(true);
    try {
      await setIdleTimeout(timeoutSecs);
      setTimeoutDone(true);
    } catch (err) {
      setTimeoutError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingTimeout(false);
    }
  }, [setIdleTimeout, timeoutSecs]);

  return (
    <Dialog
      open={open}
      onClose={wipeAndClose}
      dismissable={false}
      title={t("passwords.settings.title")}
      width="480px"
    >
      <div className="pw-settings">
        {/* ── Change master password ── */}
        <section className="pw-settings-section">
          <h3 className="pw-settings-heading">{t("passwords.settings.changeMasterTitle")}</h3>
          <p className="pw-settings-hint">{t("passwords.settings.changeMasterHint")}</p>

          <form className="pw-settings-form" onSubmit={handleChangeMaster} autoComplete="off">
            <Input
              id="pw-settings-old"
              type="password"
              label={t("passwords.settings.currentPassword")}
              value={oldPassword}
              onChange={(e) => {
                setOldPassword(e.target.value);
                setMasterError(null);
                setMasterDone(false);
              }}
              placeholder={t("passwords.unlockPlaceholder")}
              reveal
              revealLabel={t("passwords.reveal")}
              hideLabel={t("passwords.hide")}
            />
            <div className="pw-settings-field">
              <Input
                id="pw-settings-new"
                type="password"
                label={t("passwords.settings.newPassword")}
                value={newPassword}
                onChange={(e) => {
                  setNewPassword(e.target.value);
                  setMasterError(null);
                  setMasterDone(false);
                }}
                placeholder={t("passwords.newPasswordPlaceholder")}
                reveal
                revealLabel={t("passwords.reveal")}
                hideLabel={t("passwords.hide")}
              />
              <PasswordStrength password={newPassword} />
            </div>
            <Input
              id="pw-settings-confirm"
              type="password"
              label={t("passwords.confirmPassword")}
              value={confirmPassword}
              onChange={(e) => {
                setConfirmPassword(e.target.value);
                setMasterError(null);
                setMasterDone(false);
              }}
              placeholder={t("passwords.confirmPlaceholder")}
              reveal
              revealLabel={t("passwords.reveal")}
              hideLabel={t("passwords.hide")}
            />

            {masterError && <p className="pw-error" role="alert">{masterError}</p>}
            {masterDone && (
              <p className="pw-settings-success" role="status">
                {t("passwords.settings.changeMasterSuccess")}
              </p>
            )}

            <Button type="submit" size="sm" disabled={savingMaster}>
              {savingMaster ? <Spinner size={14} /> : t("passwords.settings.changeMasterAction")}
            </Button>
          </form>
        </section>

        {/* ── Auto-lock idle timeout ── */}
        <section className="pw-settings-section">
          <h3 className="pw-settings-heading">{t("passwords.settings.idleTitle")}</h3>
          <p className="pw-settings-hint">{t("passwords.settings.idleHint")}</p>

          <div className="pw-settings-form">
            <div className="pw-settings-field">
              <label htmlFor="pw-settings-idle" className="input-label">
                {t("passwords.settings.idleLabel")}
              </label>
              <select
                id="pw-settings-idle"
                className="input"
                value={timeoutSecs}
                onChange={(e) => {
                  setTimeoutSecs(Number(e.target.value));
                  setTimeoutError(null);
                  setTimeoutDone(false);
                }}
              >
                {timeoutOptions.map((secs) => (
                  <option key={secs} value={secs}>
                    {t("passwords.settings.idleMinutes", { minutes: Math.round(secs / 60) })}
                  </option>
                ))}
              </select>
            </div>

            {timeoutError && <p className="pw-error" role="alert">{timeoutError}</p>}
            {timeoutDone && (
              <p className="pw-settings-success" role="status">
                {t("passwords.settings.idleSuccess")}
              </p>
            )}

            <Button
              type="button"
              size="sm"
              onClick={() => void handleSaveTimeout()}
              disabled={savingTimeout || timeoutSecs === currentTimeoutSecs}
            >
              {savingTimeout ? <Spinner size={14} /> : t("passwords.settings.idleAction")}
            </Button>
          </div>
        </section>

        <div className="pw-entry-actions">
          <Button type="button" variant="secondary" onClick={wipeAndClose}>
            {t("general.close")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

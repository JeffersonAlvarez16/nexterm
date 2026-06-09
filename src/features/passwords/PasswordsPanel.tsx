// features/passwords/PasswordsPanel.tsx — Top-level password-manager section
//
// App-global side-panel section for the INDEPENDENT password manager. Gates on
// the backend pw_status:
//   - !exists            -> create-master flow (min 12, confirm, strength meter)
//   - exists && locked   -> PasswordUnlock (second-access gate)
//   - exists && unlocked -> PasswordList + add button
//
// Polls pw_status every ~10s WHILE mounted so a backend auto-lock (which emits
// no event) is detected and the panel re-gates. All transient secret state is
// cleared on unmount, lock, and section switch (which unmounts this panel).

import { useCallback, useEffect, useState } from "react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { Input } from "../../components/ui/Input";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { PasswordStrength } from "../vault/PasswordStrength";
import { Dialog } from "../../components/ui/Dialog";
import { useI18n } from "../../lib/i18n";
import {
  usePasswordStore,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";
import { PasswordUnlock } from "./PasswordUnlock";
import { PasswordList } from "./PasswordList";
import { PasswordEntryDialog } from "./PasswordEntryDialog";
import { PasswordSecretDialog } from "./PasswordSecretDialog";
import { PasswordSettingsDialog } from "./PasswordSettingsDialog";

/** Minimum master-password length when creating the password store. */
const MIN_MASTER_PASSWORD_LENGTH = 12;

/** Status poll cadence (ms) to detect backend auto-lock. */
const STATUS_POLL_MS = 10_000;

export function PasswordsPanel() {
  const { t } = useI18n();

  const status = usePasswordStore((s) => s.status);
  const unlocked = usePasswordStore((s) => s.unlocked);
  const refreshStatus = usePasswordStore((s) => s.refreshStatus);
  const create = usePasswordStore((s) => s.create);
  const lock = usePasswordStore((s) => s.lock);
  const exportToFile = usePasswordStore((s) => s.exportToFile);
  const importFromFile = usePasswordStore((s) => s.importFromFile);

  const [bootstrapping, setBootstrapping] = useState(true);

  // Create-master form state.
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [capsLock, setCapsLock] = useState(false);

  // Add/edit (metadata-only) dialog.
  const [entryDialogOpen, setEntryDialogOpen] = useState(false);
  const [editingEntry, setEditingEntry] = useState<PasswordEntryMeta | null>(null);

  // Set / replace password (secret-only) dialog.
  const [secretDialogOpen, setSecretDialogOpen] = useState(false);
  const [secretEntry, setSecretEntry] = useState<PasswordEntryMeta | null>(null);

  // Settings dialog (change master + idle timeout).
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Export dialog state.
  const [exportOpen, setExportOpen] = useState(false);
  const [exportMasterPassword, setExportMasterPassword] = useState("");
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportLoading, setExportLoading] = useState(false);

  // Inline banner (success/error toast).
  const [banner, setBanner] = useState<{ type: "success" | "error"; message: string } | null>(null);

  const handleCapsLock = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    setCapsLock(e.getModifierState("CapsLock"));
  }, []);

  // ── Initial status + polling ───────────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        await refreshStatus();
      } catch {
        /* error surfaced via store.error */
      } finally {
        if (!cancelled) setBootstrapping(false);
      }
    })();

    const interval = setInterval(() => {
      void refreshStatus().catch(() => {
        /* ignore poll errors */
      });
    }, STATUS_POLL_MS);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [refreshStatus]);

  // Clear local create-form secret state whenever we leave the create gate
  // (e.g. after creation succeeds) and on unmount.
  useEffect(() => {
    return () => {
      setNewPassword("");
      setConfirmPassword("");
    };
  }, []);

  // ── Create master ────────────────────────────────────────────────────────────
  const handleCreate = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setCreateError(null);

      if (newPassword.length < 1) {
        setCreateError(t("passwords.passwordRequired"));
        return;
      }
      if (newPassword.length < MIN_MASTER_PASSWORD_LENGTH) {
        setCreateError(t("passwords.passwordTooShort", { min: MIN_MASTER_PASSWORD_LENGTH }));
        return;
      }
      if (newPassword !== confirmPassword) {
        setCreateError(t("passwords.passwordMismatch"));
        return;
      }

      setCreating(true);
      try {
        await create(newPassword);
        setNewPassword("");
        setConfirmPassword("");
      } catch (err) {
        setCreateError(err instanceof Error ? err.message : String(err));
      } finally {
        setCreating(false);
      }
    },
    [newPassword, confirmPassword, create, t],
  );

  const handleLock = useCallback(() => {
    void lock();
  }, [lock]);

  // Auto-dismiss banner after 4 s.
  useEffect(() => {
    if (!banner) return;
    const id = setTimeout(() => setBanner(null), 4000);
    return () => clearTimeout(id);
  }, [banner]);

  const openExportDialog = useCallback(() => {
    setExportMasterPassword("");
    setExportError(null);
    setExportOpen(true);
  }, []);

  const closeExportDialog = useCallback(() => {
    setExportOpen(false);
    setExportMasterPassword("");
    setExportError(null);
  }, []);

  const handleExportConfirm = useCallback(async () => {
    if (!exportMasterPassword) {
      setExportError(t("passwords.passwordRequired"));
      return;
    }
    setExportError(null);
    setExportLoading(true);
    try {
      const path = await save({
        defaultPath: "nexterm-passwords.csv",
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) {
        setExportLoading(false);
        return;
      }
      const count = await exportToFile(path, exportMasterPassword);
      closeExportDialog();
      setBanner({ type: "success", message: t("passwords.exportSuccess", { count }) });
    } catch (err) {
      setExportError(String(err));
    } finally {
      setExportLoading(false);
    }
  }, [exportMasterPassword, exportToFile, closeExportDialog, t]);

  const handleImportClick = useCallback(async () => {
    try {
      const path = await open({
        filters: [{ name: "Bitwarden", extensions: ["csv", "json"] }],
        multiple: false,
      });
      if (!path) return;
      const filePath = path as string;
      const count = await importFromFile(filePath);
      setBanner({ type: "success", message: t("passwords.importSuccess", { count }) });
    } catch (err) {
      setBanner({ type: "error", message: t("passwords.importError") });
    }
  }, [importFromFile, t]);

  const openAdd = useCallback(() => {
    setEditingEntry(null);
    setEntryDialogOpen(true);
  }, []);

  const openEdit = useCallback((entry: PasswordEntryMeta) => {
    setEditingEntry(entry);
    setEntryDialogOpen(true);
  }, []);

  const closeEntryDialog = useCallback(() => {
    setEntryDialogOpen(false);
    setEditingEntry(null);
  }, []);

  const openSetPassword = useCallback((entry: PasswordEntryMeta) => {
    setSecretEntry(entry);
    setSecretDialogOpen(true);
  }, []);

  const closeSecretDialog = useCallback(() => {
    setSecretDialogOpen(false);
    setSecretEntry(null);
  }, []);

  // ── Render gating ──────────────────────────────────────────────────────────
  if (bootstrapping && status === null) {
    return (
      <div className="pw-panel pw-panel-loading">
        <Spinner size={18} />
      </div>
    );
  }

  const exists = status?.exists ?? false;

  // 1) No store yet -> create master flow.
  if (!exists) {
    return (
      <div className="pw-panel">
        <div className="pw-gate">
          <div className="pw-gate-icon" aria-hidden="true">
            <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
              <path d="M7 11V7a5 5 0 0 1 10 0v4" />
              <circle cx="12" cy="16" r="1" />
            </svg>
          </div>
          <h2 className="pw-gate-title">{t("passwords.title")}</h2>
          <p className="pw-gate-subtitle">{t("passwords.createMessage")}</p>

          <form onSubmit={handleCreate} className="pw-gate-form" autoComplete="off">
            <div className="pw-gate-field">
              <Input
                id="pw-create-password"
                type="password"
                label={t("passwords.newPassword")}
                value={newPassword}
                onChange={(e) => {
                  setNewPassword(e.target.value);
                  setCreateError(null);
                }}
                onKeyUp={handleCapsLock}
                onKeyDown={handleCapsLock}
                placeholder={t("passwords.newPasswordPlaceholder")}
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
              <PasswordStrength password={newPassword} />
            </div>

            <Input
              id="pw-create-confirm"
              type="password"
              label={t("passwords.confirmPassword")}
              value={confirmPassword}
              onChange={(e) => {
                setConfirmPassword(e.target.value);
                setCreateError(null);
              }}
              onKeyUp={handleCapsLock}
              onKeyDown={handleCapsLock}
              placeholder={t("passwords.confirmPlaceholder")}
              reveal
              revealLabel={t("passwords.reveal")}
              hideLabel={t("passwords.hide")}
            />

            {createError && <p className="pw-error" role="alert">{createError}</p>}

            <Button
              type="submit"
              disabled={creating || !newPassword}
              style={{ width: "100%", justifyContent: "center" }}
            >
              {creating ? <Spinner size={14} /> : t("passwords.create")}
            </Button>
          </form>
        </div>
      </div>
    );
  }

  // 2) Exists but locked -> second-access gate.
  if (!unlocked) {
    return (
      <div className="pw-panel">
        <PasswordUnlock />
      </div>
    );
  }

  // 3) Unlocked -> list + add.
  return (
    <div className="pw-panel">
      {banner && (
        <div className={`pw-banner pw-banner-${banner.type}`} role="status">
          {banner.message}
        </div>
      )}

      <div className="pw-panel-toolbar">
        <Button size="sm" onClick={openAdd}>
          {t("passwords.add")}
        </Button>
        <button
          type="button"
          className="pw-toolbar-btn btn btn-ghost btn-sm"
          onClick={openExportDialog}
          aria-label={t("passwords.exportTitle")}
          title={t("passwords.exportTitle")}
        >
          {t("passwords.export")}
        </button>
        <button
          type="button"
          className="pw-toolbar-btn btn btn-ghost btn-sm"
          onClick={() => void handleImportClick()}
          aria-label={t("passwords.import")}
          title={t("passwords.import")}
        >
          {t("passwords.import")}
        </button>
        <button
          type="button"
          className="pw-settings-btn btn btn-ghost btn-sm"
          onClick={() => setSettingsOpen(true)}
          aria-label={t("passwords.settings.title")}
          title={t("passwords.settings.title")}
        >
          {t("passwords.settings.open")}
        </button>
        <button
          type="button"
          className="pw-lock-btn btn btn-ghost btn-sm"
          onClick={handleLock}
          aria-label={t("passwords.lock")}
        >
          {t("passwords.lock")}
        </button>
      </div>

      <PasswordList onEdit={openEdit} onSetPassword={openSetPassword} />

      <PasswordEntryDialog
        open={entryDialogOpen}
        entry={editingEntry}
        onClose={closeEntryDialog}
      />

      <PasswordSecretDialog
        open={secretDialogOpen}
        entry={secretEntry}
        onClose={closeSecretDialog}
      />

      <PasswordSettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />

      {/* Export dialog — requires master password re-auth + plaintext warning */}
      <Dialog
        open={exportOpen}
        onClose={closeExportDialog}
        title={t("passwords.exportTitle")}
      >
        <div className="pw-export-dialog">
          <p className="pw-export-warning" role="alert">
            {t("passwords.exportWarning")}
          </p>

          <Input
            id="pw-export-master"
            type="password"
            label={t("passwords.exportMasterLabel")}
            value={exportMasterPassword}
            onChange={(e) => {
              setExportMasterPassword(e.target.value);
              setExportError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleExportConfirm();
            }}
            placeholder={t("passwords.exportMasterPlaceholder")}
            reveal
            revealLabel={t("passwords.reveal")}
            hideLabel={t("passwords.hide")}
            autoFocus
          />

          {exportError && (
            <p className="pw-error" role="alert">
              {exportError}
            </p>
          )}

          <div className="pw-dialog-actions">
            <Button
              variant="ghost"
              onClick={closeExportDialog}
              disabled={exportLoading}
            >
              {t("general.cancel")}
            </Button>
            <Button
              onClick={() => void handleExportConfirm()}
              disabled={exportLoading || !exportMasterPassword}
            >
              {exportLoading ? <Spinner size={14} /> : t("passwords.exportConfirm")}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}

// features/passwords/PasswordSecretDialog.tsx — Set / replace a stored secret
//
// A small, clearly-labeled dialog that REPLACES the stored password (and notes)
// for an existing entry. It collects a NEW password (+ optional notes) and saves
// via updateSecret(), which re-encrypts ONLY the secret blob and preserves every
// meta field. This is the deliberate, explicit path to change a password — the
// metadata edit dialog never touches the secret, so there is no silent wipe.
//
// SECURITY: the plaintext password lives ONLY in this dialog's local form state
// and is wiped on close/cancel/success. It is passed straight to updateSecret()
// and never lands in the store.

import { useCallback, useEffect, useState } from "react";
import { Dialog } from "../../components/ui/Dialog";
import { Input } from "../../components/ui/Input";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { useI18n } from "../../lib/i18n";
import {
  PasswordGenerator,
  type GeneratorOptions,
} from "./PasswordGenerator";
import {
  usePasswordStore,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";

interface PasswordSecretDialogProps {
  open: boolean;
  /** The entry whose secret will be replaced. */
  entry: PasswordEntryMeta | null;
  onClose: () => void;
}

export function PasswordSecretDialog({ open, entry, onClose }: PasswordSecretDialogProps) {
  const { t } = useI18n();
  const updateSecret = usePasswordStore((s) => s.updateSecret);
  const generate = usePasswordStore((s) => s.generate);

  const [password, setPassword] = useState("");
  const [notes, setNotes] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [generating, setGenerating] = useState(false);

  // Reset the form on each open. We never pre-fill the secret (it is not
  // retrievable from metadata, and pre-filling would be a hygiene risk).
  useEffect(() => {
    if (open) {
      setPassword("");
      setNotes("");
      setError(null);
    }
  }, [open, entry]);

  const wipeAndClose = useCallback(() => {
    setPassword("");
    setNotes("");
    setError(null);
    onClose();
  }, [onClose]);

  const handleGenerate = useCallback(
    async (options: GeneratorOptions) => {
      setGenerating(true);
      setError(null);
      try {
        const generated = await generate(
          options.length,
          options.symbols,
          options.digits,
          options.uppercase,
        );
        setPassword(generated);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setGenerating(false);
      }
    },
    [generate],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!password) {
        setError(t("passwords.entry.passwordRequired"));
        return;
      }

      setSaving(true);
      try {
        if (entry) {
          // Lossless for the meta side: REPLACES password + notes only.
          await updateSecret(entry.id, { password, notes });
        }
        wipeAndClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setSaving(false);
      }
    },
    [password, notes, entry, updateSecret, wipeAndClose, t],
  );

  return (
    <Dialog
      open={open}
      onClose={wipeAndClose}
      dismissable={false}
      title={t("passwords.secret.title")}
      width="460px"
    >
      <form className="pw-entry-form" onSubmit={handleSubmit} autoComplete="off">
        <p className="pw-entry-hint pw-entry-hint-warning">
          {entry
            ? t("passwords.secret.warning", { title: entry.title })
            : t("passwords.secret.warningGeneric")}
        </p>

        <div className="pw-entry-password-row">
          <Input
            id="pw-secret-password"
            type="password"
            label={t("passwords.secret.fieldNewPassword")}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={t("passwords.entry.passwordPlaceholder")}
            reveal
            revealLabel={t("passwords.reveal")}
            hideLabel={t("passwords.hide")}
            autoFocus
          />
          <PasswordGenerator
            onGenerate={(options) => void handleGenerate(options)}
            busy={generating}
          />
        </div>

        <div className="pw-entry-notes">
          <label htmlFor="pw-secret-notes-field" className="input-label">
            {t("passwords.secret.fieldNewNotes")}
          </label>
          <textarea
            id="pw-secret-notes-field"
            className="input pw-entry-notes-input"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            placeholder={t("passwords.entry.notesPlaceholder")}
            autoComplete="off"
            spellCheck={false}
            rows={3}
          />
        </div>

        {error && <p className="pw-error">{error}</p>}

        <div className="pw-entry-actions">
          <Button type="button" variant="secondary" onClick={wipeAndClose} disabled={saving}>
            {t("general.cancel")}
          </Button>
          <Button type="submit" disabled={saving}>
            {saving ? <Spinner size={14} /> : t("passwords.secret.save")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

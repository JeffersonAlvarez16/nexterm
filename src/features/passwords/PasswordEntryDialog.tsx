// features/passwords/PasswordEntryDialog.tsx — Add / edit a password entry
//
// ADD mode: a full form (title, username, url, category, notes, and the
// password itself with reveal toggle + Generate) saved via add().
//
// EDIT mode: METADATA-ONLY (title, username, url, category). It deliberately
// has NO password/notes fields and saves via updateMeta(), which re-encrypts
// only the meta blob and leaves the stored secret byte-for-byte unchanged. This
// removes the old silent-wipe path where editing a title sent an empty password
// to the backend. To change the stored password use PasswordSecretDialog.
//
// SECURITY: in ADD mode the dialog holds the plaintext password ONLY in its
// local form state, and that state is wiped on close/cancel/success. It is
// passed straight to add() and never lands in the store. EDIT mode never holds
// a secret at all.

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
  type PasswordEntryInput,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";

interface PasswordEntryDialogProps {
  open: boolean;
  /** When set, the dialog is in metadata-only EDIT mode for this entry. */
  entry: PasswordEntryMeta | null;
  onClose: () => void;
}

function emptyForm(): PasswordEntryInput {
  return { title: "", username: "", url: "", category: "", notes: "", password: "" };
}

export function PasswordEntryDialog({ open, entry, onClose }: PasswordEntryDialogProps) {
  const { t } = useI18n();
  const add = usePasswordStore((s) => s.add);
  const updateMeta = usePasswordStore((s) => s.updateMeta);
  const generate = usePasswordStore((s) => s.generate);

  const isEdit = entry !== null;

  const [form, setForm] = useState<PasswordEntryInput>(emptyForm);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [generating, setGenerating] = useState(false);

  // Seed the metadata fields when (re)opening. In EDIT mode there are no
  // password/notes inputs, so those stay blank and are never sent.
  useEffect(() => {
    if (open) {
      setForm({
        title: entry?.title ?? "",
        username: entry?.username ?? "",
        url: entry?.url ?? "",
        category: entry?.category ?? "",
        notes: "",
        password: "",
      });
      setError(null);
    }
  }, [open, entry]);

  // Wipe local secret state whenever the dialog is fully closed.
  const wipeAndClose = useCallback(() => {
    setForm(emptyForm());
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
        setForm((f) => ({ ...f, password: generated }));
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

      if (!form.title.trim()) {
        setError(t("passwords.entry.titleRequired"));
        return;
      }
      // The password is required ONLY when adding. EDIT mode is metadata-only
      // and must never validate or send a password.
      if (!isEdit && !form.password) {
        setError(t("passwords.entry.passwordRequired"));
        return;
      }

      setSaving(true);
      try {
        if (isEdit && entry) {
          // Lossless meta-only edit: the secret blob is never touched.
          await updateMeta(entry.id, {
            title: form.title,
            username: form.username,
            url: form.url,
            category: form.category,
          });
        } else {
          await add(form);
        }
        wipeAndClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setSaving(false);
      }
    },
    [form, isEdit, entry, updateMeta, add, wipeAndClose, t],
  );

  return (
    <Dialog
      open={open}
      onClose={wipeAndClose}
      dismissable={false}
      title={isEdit ? t("passwords.entry.editTitle") : t("passwords.entry.addTitle")}
      width="460px"
    >
      <form className="pw-entry-form" onSubmit={handleSubmit} autoComplete="off">
        {isEdit && (
          <p className="pw-entry-hint">{t("passwords.entry.editMetaHint")}</p>
        )}

        <Input
          id="pw-entry-title"
          type="text"
          label={t("passwords.entry.fieldTitle")}
          value={form.title}
          onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
          placeholder={t("passwords.entry.titlePlaceholder")}
          autoFocus
        />
        <Input
          id="pw-entry-username"
          type="text"
          label={t("passwords.entry.fieldUsername")}
          value={form.username}
          onChange={(e) => setForm((f) => ({ ...f, username: e.target.value }))}
          placeholder={t("passwords.entry.usernamePlaceholder")}
        />
        <Input
          id="pw-entry-url"
          type="text"
          label={t("passwords.entry.fieldUrl")}
          value={form.url}
          onChange={(e) => setForm((f) => ({ ...f, url: e.target.value }))}
          placeholder={t("passwords.entry.urlPlaceholder")}
        />
        <Input
          id="pw-entry-category"
          type="text"
          label={t("passwords.entry.fieldCategory")}
          value={form.category}
          onChange={(e) => setForm((f) => ({ ...f, category: e.target.value }))}
          placeholder={t("passwords.entry.categoryPlaceholder")}
        />

        {/* Password + notes exist ONLY in ADD mode. EDIT mode is meta-only so a
            title change can never wipe the stored secret. */}
        {!isEdit && (
          <>
            <div className="pw-entry-password-row">
              <Input
                id="pw-entry-password"
                type="password"
                label={t("passwords.entry.fieldPassword")}
                value={form.password}
                onChange={(e) => setForm((f) => ({ ...f, password: e.target.value }))}
                placeholder={t("passwords.entry.passwordPlaceholder")}
                reveal
                revealLabel={t("passwords.reveal")}
                hideLabel={t("passwords.hide")}
              />
              <PasswordGenerator
                onGenerate={(options) => void handleGenerate(options)}
                busy={generating}
              />
            </div>

            <div className="pw-entry-notes">
              <label htmlFor="pw-entry-notes-field" className="input-label">
                {t("passwords.entry.fieldNotes")}
              </label>
              <textarea
                id="pw-entry-notes-field"
                className="input pw-entry-notes-input"
                value={form.notes}
                onChange={(e) => setForm((f) => ({ ...f, notes: e.target.value }))}
                placeholder={t("passwords.entry.notesPlaceholder")}
                autoComplete="off"
                spellCheck={false}
                rows={3}
              />
            </div>
          </>
        )}

        {error && <p className="pw-error">{error}</p>}

        <div className="pw-entry-actions">
          <Button type="button" variant="secondary" onClick={wipeAndClose} disabled={saving}>
            {t("general.cancel")}
          </Button>
          <Button type="submit" disabled={saving}>
            {saving ? <Spinner size={14} /> : t("general.save")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

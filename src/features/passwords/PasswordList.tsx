// features/passwords/PasswordList.tsx — Entry list with masked secrets
//
// Lists entries (title, username, url, category). The password is masked and
// NEVER rendered until the user explicitly reveals it, and only after a fresh
// re-auth grant.
//
// Reveal / copy flow (grant is SINGLE-USE and ID-BOUND on the backend):
//   1. User clicks reveal/copy on a row.
//   2. A re-auth dialog (master password) opens for THAT row.
//   3. pw_reauth(masterPassword, id) mints a single-use grant bound to that id,
//      then pw_reveal(id) consumes it. A prior reveal never authorizes another
//      row — every reveal/copy re-auths for its own row.
//   4. Reveal shows the plaintext BRIEFLY then auto-hides.
//   5. Copy uses copySecret() which auto-clears the clipboard after a timeout.
//
// SECURITY: the revealed plaintext lives only in a short-lived local ref/state
// for the single row being revealed and is wiped on hide, on row change, on
// edit/delete, and on unmount. Nothing secret is ever pushed into the store.

import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog } from "../../components/ui/Dialog";
import { Input } from "../../components/ui/Input";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { useI18n } from "../../lib/i18n";
import { copySecret } from "../../lib/clipboard";
import {
  usePasswordStore,
  type PasswordEntryMeta,
} from "../../stores/passwordStore";

interface PasswordListProps {
  /** Open the metadata-only edit dialog (title/username/url/category). */
  onEdit: (entry: PasswordEntryMeta) => void;
  /** Open the "set / replace password" dialog (replaces the stored secret). */
  onSetPassword: (entry: PasswordEntryMeta) => void;
}

/** How long a revealed password stays visible before auto-hiding (ms). */
const REVEAL_VISIBLE_MS = 15_000;

/** A queued action that needs a grant before it can run. */
type PendingAction = { kind: "reveal" | "copy"; id: string } | null;

export function PasswordList({ onEdit, onSetPassword }: PasswordListProps) {
  const { t } = useI18n();
  const entries = usePasswordStore((s) => s.entries);
  const loading = usePasswordStore((s) => s.loading);
  const remove = usePasswordStore((s) => s.remove);
  const reveal = usePasswordStore((s) => s.reveal);
  const reauth = usePasswordStore((s) => s.reauth);

  // The single currently-revealed row: id + its transient plaintext.
  const [revealedId, setRevealedId] = useState<string | null>(null);
  const [revealedValue, setRevealedValue] = useState<string>("");
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Re-auth dialog state.
  const [reauthOpen, setReauthOpen] = useState(false);
  const [reauthPassword, setReauthPassword] = useState("");
  const [reauthError, setReauthError] = useState<string | null>(null);
  const [reauthBusy, setReauthBusy] = useState(false);
  const pendingAction = useRef<PendingAction>(null);

  const [rowError, setRowError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  // ── Secret cleanup ───────────────────────────────────────────────────────────
  const clearReveal = useCallback(() => {
    if (hideTimer.current) {
      clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    setRevealedId(null);
    setRevealedValue("");
  }, []);

  // Wipe any revealed secret when the component unmounts.
  useEffect(() => {
    return () => {
      if (hideTimer.current) clearTimeout(hideTimer.current);
      setRevealedValue("");
    };
  }, []);

  // If the entry set changes (lock, reset, edit), drop any revealed secret.
  useEffect(() => {
    clearReveal();
    // Intentionally keyed on entries identity.
  }, [entries, clearReveal]);

  // ── Actions ────────────────────────────────────────────────────────────────
  const doReveal = useCallback(
    async (id: string) => {
      setRowError(null);
      try {
        const plaintext = await reveal(id);
        setRevealedId(id);
        setRevealedValue(plaintext);
        if (hideTimer.current) clearTimeout(hideTimer.current);
        hideTimer.current = setTimeout(() => {
          setRevealedId(null);
          setRevealedValue("");
          hideTimer.current = null;
        }, REVEAL_VISIBLE_MS);
      } catch (err) {
        setRowError(err instanceof Error ? err.message : String(err));
      }
    },
    [reveal],
  );

  const doCopy = useCallback(
    async (id: string) => {
      setRowError(null);
      try {
        const plaintext = await reveal(id);
        await copySecret(plaintext);
      } catch (err) {
        setRowError(err instanceof Error ? err.message : String(err));
      }
    },
    [reveal],
  );

  const runAction = useCallback(
    (action: NonNullable<PendingAction>) => {
      if (action.kind === "reveal") void doReveal(action.id);
      else void doCopy(action.id);
    },
    [doReveal, doCopy],
  );

  // Entry point for reveal/copy buttons. The grant is single-use and id-bound,
  // so EVERY reveal/copy re-auths for its own row — we never assume a prior
  // grant covers this row.
  const requestAction = useCallback(
    (kind: "reveal" | "copy", id: string) => {
      pendingAction.current = { kind, id };
      setReauthPassword("");
      setReauthError(null);
      setReauthOpen(true);
    },
    [],
  );

  const closeReauth = useCallback(() => {
    setReauthOpen(false);
    setReauthPassword("");
    setReauthError(null);
    pendingAction.current = null;
  }, []);

  const submitReauth = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!reauthPassword.trim()) {
        setReauthError(t("passwords.passwordRequired"));
        return;
      }
      const action = pendingAction.current;
      if (!action) {
        closeReauth();
        return;
      }
      setReauthBusy(true);
      setReauthError(null);
      try {
        // Mint a single-use grant bound to THIS exact row, then run the action
        // (reveal/copy) which immediately consumes it via pw_reveal(id).
        await reauth(action.id, reauthPassword);
        // Wipe the typed master password from local state immediately.
        setReauthPassword("");
        setReauthOpen(false);
        pendingAction.current = null;
        runAction(action);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes("Wrong master password") || msg.includes("Decryption failed")) {
          setReauthError(t("passwords.wrongPassword"));
        } else {
          setReauthError(msg);
        }
      } finally {
        setReauthBusy(false);
      }
    },
    [reauthPassword, reauth, runAction, closeReauth, t],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      clearReveal();
      try {
        await remove(id);
        setConfirmDeleteId(null);
      } catch (err) {
        setRowError(err instanceof Error ? err.message : String(err));
      }
    },
    [remove, clearReveal],
  );

  // ── Render ───────────────────────────────────────────────────────────────────
  if (loading && entries.length === 0) {
    return (
      <div className="pw-list-loading">
        <Spinner size={18} />
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="pw-list-empty">
        <p>{t("passwords.empty")}</p>
        <p className="pw-list-empty-hint">{t("passwords.emptyHint")}</p>
      </div>
    );
  }

  return (
    <>
      {rowError && <p className="pw-error" role="alert">{rowError}</p>}

      <ul className="pw-list" aria-label={t("passwords.title")}>
        {entries.map((entry) => {
          const isRevealed = revealedId === entry.id;
          return (
            <li key={entry.id} className="pw-entry">
              <div className="pw-entry-info">
                <span className="pw-entry-title">{entry.title}</span>
                {entry.username && (
                  <span className="pw-entry-username">{entry.username}</span>
                )}
                {entry.url && <span className="pw-entry-url">{entry.url}</span>}
                {entry.category && (
                  <span className="pw-entry-category">{entry.category}</span>
                )}
                <span className="pw-entry-secret" aria-live="polite">
                  {isRevealed ? (
                    <code className="pw-entry-secret-value">{revealedValue}</code>
                  ) : (
                    <span className="pw-entry-secret-mask" aria-hidden="true">
                      ••••••••••••
                    </span>
                  )}
                </span>
              </div>

              <div className="pw-entry-actions">
                <button
                  type="button"
                  aria-label={isRevealed ? t("passwords.hide") : t("passwords.reveal")}
                  aria-pressed={isRevealed}
                  onClick={() =>
                    isRevealed ? clearReveal() : requestAction("reveal", entry.id)
                  }
                >
                  {isRevealed ? t("passwords.hide") : t("passwords.reveal")}
                </button>
                <button
                  type="button"
                  aria-label={t("passwords.copy")}
                  onClick={() => requestAction("copy", entry.id)}
                >
                  {t("passwords.copy")}
                </button>
                <button
                  type="button"
                  aria-label={t("passwords.edit")}
                  onClick={() => {
                    clearReveal();
                    onEdit(entry);
                  }}
                >
                  {t("passwords.edit")}
                </button>
                <button
                  type="button"
                  aria-label={t("passwords.setPassword")}
                  title={t("passwords.setPassword")}
                  onClick={() => {
                    clearReveal();
                    onSetPassword(entry);
                  }}
                >
                  {t("passwords.setPassword")}
                </button>
                {confirmDeleteId === entry.id ? (
                  <button
                    type="button"
                    className="pw-entry-delete-confirm"
                    aria-label={t("passwords.deleteConfirm")}
                    onClick={() => void handleDelete(entry.id)}
                  >
                    {t("passwords.deleteConfirm")}
                  </button>
                ) : (
                  <button
                    type="button"
                    aria-label={t("passwords.delete")}
                    onClick={() => setConfirmDeleteId(entry.id)}
                  >
                    {t("passwords.delete")}
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>

      {/* ── Re-auth dialog — required before any reveal/copy ── */}
      <Dialog
        open={reauthOpen}
        onClose={closeReauth}
        title={t("passwords.reauth.title")}
        width="420px"
      >
        <form className="pw-reauth-form" onSubmit={submitReauth} autoComplete="off">
          <p className="pw-reauth-message">{t("passwords.reauth.message")}</p>
          <Input
            id="pw-reauth-password"
            type="password"
            label={t("passwords.masterPassword")}
            value={reauthPassword}
            onChange={(e) => {
              setReauthPassword(e.target.value);
              setReauthError(null);
            }}
            placeholder={t("passwords.unlockPlaceholder")}
            reveal
            revealLabel={t("passwords.reveal")}
            hideLabel={t("passwords.hide")}
            autoFocus
          />
          {reauthError && <p className="pw-error" role="alert">{reauthError}</p>}
          <div className="pw-reauth-actions">
            <Button type="button" variant="ghost" onClick={closeReauth} disabled={reauthBusy}>
              {t("general.cancel")}
            </Button>
            <Button type="submit" disabled={reauthBusy || !reauthPassword.trim()}>
              {reauthBusy ? <Spinner size={14} /> : t("passwords.reauth.confirm")}
            </Button>
          </div>
        </form>
      </Dialog>
    </>
  );
}

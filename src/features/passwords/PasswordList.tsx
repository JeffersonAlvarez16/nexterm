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
import { listen } from "@tauri-apps/api/event";
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

// ── Compact row-action icons (15px, stroke, currentColor) ──────────────────────
const iconProps = {
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.4,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
};
const EyeIcon = () => (
  <svg {...iconProps}>
    <path d="M1.5 8S3.8 3.6 8 3.6 14.5 8 14.5 8 12.2 12.4 8 12.4 1.5 8 1.5 8Z" />
    <circle cx="8" cy="8" r="2" />
  </svg>
);
const EyeOffIcon = () => (
  <svg {...iconProps}>
    <path d="M6.4 3.8A6.7 6.7 0 0 1 8 3.6c4.2 0 6.5 4.4 6.5 4.4a11 11 0 0 1-1.7 2.2M3.9 4.7A10.8 10.8 0 0 0 1.5 8S3.8 12.4 8 12.4a6.5 6.5 0 0 0 2.6-.5" />
    <path d="M6.6 6.6a2 2 0 0 0 2.8 2.8" />
    <path d="M2 2l12 12" />
  </svg>
);
const CopyIcon = () => (
  <svg {...iconProps}>
    <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
    <path d="M3.4 10.5A1.5 1.5 0 0 1 2.5 9V4a1.5 1.5 0 0 1 1.5-1.5H9a1.5 1.5 0 0 1 1.4 1" />
  </svg>
);
const EditIcon = () => (
  <svg {...iconProps}>
    <path d="M11.6 2.4a1.4 1.4 0 0 1 2 2L5.2 12.8l-3 .8.8-3 8.6-8.2Z" />
  </svg>
);
const KeyIcon = () => (
  <svg {...iconProps}>
    <circle cx="5.5" cy="10.5" r="2.5" />
    <path d="M7.3 8.7 13 3M10.8 5.2 12.4 6.8M9.3 6.7 10.6 8" />
  </svg>
);
const TrashIcon = () => (
  <svg {...iconProps}>
    <path d="M2.5 4.5h11M6 4.5V3a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5M4.1 4.5l.5 8a1 1 0 0 0 1 .95h4.8a1 1 0 0 0 1-.95l.5-8" />
  </svg>
);
const CheckIcon = () => (
  <svg {...iconProps} strokeWidth={1.7}>
    <path d="M3 8.4 6.4 12 13 4.6" />
  </svg>
);

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

  // ── Lock-on-blur ──────────────────────────────────────────────────────────────
  // The backend emits "pw-focus-lost" on window blur (and clears its reveal grant
  // server-side). On that event — or a raw window blur as a fallback — immediately
  // HIDE any revealed secret, cancel the auto-hide timer, and tear down any
  // in-progress re-auth dialog. This shrinks the shoulder-surfing window when the
  // user alt-tabs away. The 15s auto-hide and wipe-on-unmount/lock stay intact.
  useEffect(() => {
    const panicHide = () => {
      // Drop any revealed plaintext and cancel its pending auto-hide timer.
      clearReveal();
      // Reset any in-progress reveal/re-auth dialog and wipe the typed master.
      pendingAction.current = null;
      setReauthOpen(false);
      setReauthPassword("");
      setReauthError(null);
    };

    let unlisten: (() => void) | undefined;
    void listen("pw-focus-lost", panicHide).then((fn) => {
      unlisten = fn;
    });
    // Raw window blur as a defense-in-depth fallback (e.g. event not wired).
    window.addEventListener("blur", panicHide);

    return () => {
      unlisten?.();
      window.removeEventListener("blur", panicHide);
    };
  }, [clearReveal]);

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
              <div className="pw-entry-head">
                <div className="pw-entry-ident">
                  <span className="pw-entry-title">{entry.title}</span>
                  {(entry.username || entry.url) && (
                    <span className="pw-entry-sub">
                      {entry.username && (
                        <span className="pw-entry-username">{entry.username}</span>
                      )}
                      {entry.username && entry.url && (
                        <span className="pw-entry-sep" aria-hidden="true">
                          ·
                        </span>
                      )}
                      {entry.url && <span className="pw-entry-url">{entry.url}</span>}
                    </span>
                  )}
                </div>

                <div className="pw-entry-row-actions">
                  <button
                    type="button"
                    className="pw-icon-btn"
                    aria-label={isRevealed ? t("passwords.hide") : t("passwords.reveal")}
                    title={isRevealed ? t("passwords.hide") : t("passwords.reveal")}
                    aria-pressed={isRevealed}
                    onClick={() =>
                      isRevealed ? clearReveal() : requestAction("reveal", entry.id)
                    }
                  >
                    {isRevealed ? <EyeOffIcon /> : <EyeIcon />}
                  </button>
                  <button
                    type="button"
                    className="pw-icon-btn"
                    aria-label={t("passwords.copy")}
                    title={t("passwords.copy")}
                    onClick={() => requestAction("copy", entry.id)}
                  >
                    <CopyIcon />
                  </button>
                  <button
                    type="button"
                    className="pw-icon-btn"
                    aria-label={t("passwords.edit")}
                    title={t("passwords.edit")}
                    onClick={() => {
                      clearReveal();
                      onEdit(entry);
                    }}
                  >
                    <EditIcon />
                  </button>
                  <button
                    type="button"
                    className="pw-icon-btn"
                    aria-label={t("passwords.setPassword")}
                    title={t("passwords.setPassword")}
                    onClick={() => {
                      clearReveal();
                      onSetPassword(entry);
                    }}
                  >
                    <KeyIcon />
                  </button>
                  {confirmDeleteId === entry.id ? (
                    <button
                      type="button"
                      className="pw-icon-btn pw-icon-btn-danger"
                      aria-label={t("passwords.deleteConfirm")}
                      title={t("passwords.deleteConfirm")}
                      onClick={() => void handleDelete(entry.id)}
                    >
                      <CheckIcon />
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="pw-icon-btn"
                      aria-label={t("passwords.delete")}
                      title={t("passwords.delete")}
                      onClick={() => setConfirmDeleteId(entry.id)}
                    >
                      <TrashIcon />
                    </button>
                  )}
                </div>
              </div>

              {entry.category && (
                <span className="pw-entry-category">{entry.category}</span>
              )}

              <div className="pw-entry-secret" aria-live="polite">
                {isRevealed ? (
                  <code className="pw-entry-secret-value">{revealedValue}</code>
                ) : (
                  <span className="pw-entry-secret-mask" aria-hidden="true">
                    ••••••••••••
                  </span>
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

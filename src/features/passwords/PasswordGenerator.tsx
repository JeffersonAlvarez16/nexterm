// features/passwords/PasswordGenerator.tsx — Shared generator controls
//
// A SMALL shared control that exposes the password-generator options (length +
// charset toggles) used by both PasswordEntryDialog and PasswordSecretDialog.
//
// It owns ONLY the option UI state (length, symbols, digits, uppercase). It does
// NOT call generate() or hold any plaintext — when the user clicks Generate it
// invokes the parent-supplied onGenerate(options) callback, and the parent
// (which owns the form) calls the store generate() and sets the result into its
// own local form state. This keeps the generated secret confined to the dialog's
// local state and never routes plaintext through this component.
//
// Lowercase is always-on server-side, so it is shown as a static note rather
// than a toggle.

import { useState } from "react";
import { Button } from "../../components/ui/Button";
import { Spinner } from "../../components/ui/Spinner";
import { useI18n } from "../../lib/i18n";

/** Options the user can tune for password generation (lowercase is always on). */
export interface GeneratorOptions {
  length: number;
  symbols: boolean;
  digits: boolean;
  uppercase: boolean;
}

/** Default generator settings (matches the previous hardcoded behavior). */
export const DEFAULT_GENERATOR_OPTIONS: GeneratorOptions = {
  length: 20,
  symbols: true,
  digits: true,
  uppercase: true,
};

/** Allowed length range for the generator. */
export const GEN_LENGTH_MIN = 8;
export const GEN_LENGTH_MAX = 64;

interface PasswordGeneratorProps {
  /**
   * Called with the chosen options when the user requests generation. The
   * parent is responsible for calling the store generate() and placing the
   * resulting plaintext into its own local form state.
   */
  onGenerate: (options: GeneratorOptions) => void;
  /** True while a generation is in flight (disables the trigger). */
  busy?: boolean;
}

/**
 * Inline popover with the generator options. Reuses the shared Button primitive
 * and native checkbox inputs (no dedicated toggle primitive exists in ui/).
 */
export function PasswordGenerator({ onGenerate, busy = false }: PasswordGeneratorProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<GeneratorOptions>(DEFAULT_GENERATOR_OPTIONS);

  const clampLength = (value: number) => {
    if (Number.isNaN(value)) return GEN_LENGTH_MIN;
    return Math.min(GEN_LENGTH_MAX, Math.max(GEN_LENGTH_MIN, Math.round(value)));
  };

  const handleGenerate = () => {
    onGenerate(options);
  };

  return (
    <div className="pw-generator">
      <Button
        type="button"
        variant="secondary"
        size="sm"
        aria-expanded={open}
        aria-label={t("passwords.generator.open")}
        title={t("passwords.generator.open")}
        onClick={() => setOpen((v) => !v)}
        disabled={busy}
      >
        {busy ? <Spinner size={14} /> : t("passwords.entry.generate")}
      </Button>

      {open && (
        <div className="pw-generator-popover" role="group" aria-label={t("passwords.generator.title")}>
          <div className="pw-generator-row">
            <label htmlFor="pw-gen-length" className="input-label">
              {t("passwords.generator.length")}
            </label>
            <div className="pw-generator-length">
              <input
                id="pw-gen-length-range"
                type="range"
                min={GEN_LENGTH_MIN}
                max={GEN_LENGTH_MAX}
                value={options.length}
                aria-label={t("passwords.generator.length")}
                onChange={(e) =>
                  setOptions((o) => ({ ...o, length: clampLength(e.target.valueAsNumber) }))
                }
              />
              <input
                id="pw-gen-length"
                type="number"
                className="input pw-generator-length-input"
                min={GEN_LENGTH_MIN}
                max={GEN_LENGTH_MAX}
                value={options.length}
                aria-label={t("passwords.generator.length")}
                onChange={(e) =>
                  setOptions((o) => ({ ...o, length: clampLength(e.target.valueAsNumber) }))
                }
              />
            </div>
          </div>

          <label className="pw-generator-toggle" htmlFor="pw-gen-symbols">
            <input
              id="pw-gen-symbols"
              type="checkbox"
              checked={options.symbols}
              onChange={(e) => setOptions((o) => ({ ...o, symbols: e.target.checked }))}
            />
            <span>{t("passwords.generator.symbols")}</span>
          </label>

          <label className="pw-generator-toggle" htmlFor="pw-gen-digits">
            <input
              id="pw-gen-digits"
              type="checkbox"
              checked={options.digits}
              onChange={(e) => setOptions((o) => ({ ...o, digits: e.target.checked }))}
            />
            <span>{t("passwords.generator.digits")}</span>
          </label>

          <label className="pw-generator-toggle" htmlFor="pw-gen-uppercase">
            <input
              id="pw-gen-uppercase"
              type="checkbox"
              checked={options.uppercase}
              onChange={(e) => setOptions((o) => ({ ...o, uppercase: e.target.checked }))}
            />
            <span>{t("passwords.generator.uppercase")}</span>
          </label>

          <p className="pw-generator-note">{t("passwords.generator.lowercaseNote")}</p>

          <Button
            type="button"
            size="sm"
            onClick={handleGenerate}
            disabled={busy}
            style={{ width: "100%", justifyContent: "center" }}
          >
            {busy ? <Spinner size={14} /> : t("passwords.generator.generate")}
          </Button>
        </div>
      )}
    </div>
  );
}

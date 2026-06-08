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
// Lowercase is always-on server-side, so it is shown as a locked chip rather
// than a toggle.

import { useEffect, useRef, useState } from "react";
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

// Character-pool sizes used to estimate entropy (lowercase is always included).
const POOL = { lower: 26, upper: 26, digits: 10, symbols: 30 } as const;

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
 * Inline popover with the generator options. Reuses the shared Button primitive;
 * charset options are pressable chips and the length is a slider + numeric field.
 */
export function PasswordGenerator({ onGenerate, busy = false }: PasswordGeneratorProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<GeneratorOptions>(DEFAULT_GENERATOR_OPTIONS);
  const rootRef = useRef<HTMLDivElement>(null);

  // Close on Escape or click outside while the popover is open.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    const onPointer = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [open]);

  const clampLength = (value: number) => {
    if (Number.isNaN(value)) return GEN_LENGTH_MIN;
    return Math.min(GEN_LENGTH_MAX, Math.max(GEN_LENGTH_MIN, Math.round(value)));
  };
  const setLength = (value: number) => setOptions((o) => ({ ...o, length: clampLength(value) }));
  const toggle = (key: "symbols" | "digits" | "uppercase") =>
    setOptions((o) => ({ ...o, [key]: !o[key] }));

  // Live entropy estimate from the selected pool: bits = length * log2(poolSize).
  const poolSize =
    POOL.lower +
    (options.uppercase ? POOL.upper : 0) +
    (options.digits ? POOL.digits : 0) +
    (options.symbols ? POOL.symbols : 0);
  const bits = Math.round(options.length * Math.log2(poolSize));
  const tier = bits < 60 ? "weak" : bits < 90 ? "fair" : bits < 120 ? "good" : "strong";
  const lengthPct =
    ((options.length - GEN_LENGTH_MIN) / (GEN_LENGTH_MAX - GEN_LENGTH_MIN)) * 100;
  const strengthPct = Math.min(100, Math.round((bits / 128) * 100));

  return (
    <div className="pw-generator" ref={rootRef}>
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
          <div className="pw-gen-head">
            <label htmlFor="pw-gen-length" className="pw-gen-label">
              {t("passwords.generator.length")}
            </label>
            <input
              id="pw-gen-length"
              type="number"
              className="pw-gen-value"
              min={GEN_LENGTH_MIN}
              max={GEN_LENGTH_MAX}
              value={options.length}
              aria-label={t("passwords.generator.length")}
              onChange={(e) => setLength(e.target.valueAsNumber)}
            />
          </div>

          <input
            id="pw-gen-length-range"
            type="range"
            className="pw-gen-range"
            min={GEN_LENGTH_MIN}
            max={GEN_LENGTH_MAX}
            value={options.length}
            aria-label={t("passwords.generator.length")}
            style={{ ["--val" as string]: `${lengthPct}%` }}
            onChange={(e) => setLength(e.target.valueAsNumber)}
          />

          <div className="pw-gen-strength" data-tier={tier}>
            <span className="pw-gen-strength-track">
              <span className="pw-gen-strength-fill" style={{ width: `${strengthPct}%` }} />
            </span>
            <span className="pw-gen-strength-text" aria-label={`≈${bits} bits`}>
              ≈{bits} bits
            </span>
          </div>

          <div className="pw-gen-chips">
            <span
              className="pw-gen-chip pw-gen-chip-locked"
              title={t("passwords.generator.lowercaseNote")}
              aria-label={t("passwords.generator.lowercaseNote")}
            >
              a–z
            </span>
            <button
              id="pw-gen-uppercase"
              type="button"
              className="pw-gen-chip"
              aria-pressed={options.uppercase}
              aria-label={t("passwords.generator.uppercase")}
              title={t("passwords.generator.uppercase")}
              onClick={() => toggle("uppercase")}
            >
              A–Z
            </button>
            <button
              id="pw-gen-digits"
              type="button"
              className="pw-gen-chip"
              aria-pressed={options.digits}
              aria-label={t("passwords.generator.digits")}
              title={t("passwords.generator.digits")}
              onClick={() => toggle("digits")}
            >
              0–9
            </button>
            <button
              id="pw-gen-symbols"
              type="button"
              className="pw-gen-chip"
              aria-pressed={options.symbols}
              aria-label={t("passwords.generator.symbols")}
              title={t("passwords.generator.symbols")}
              onClick={() => toggle("symbols")}
            >
              !@#
            </button>
          </div>

          <Button
            type="button"
            size="sm"
            onClick={() => onGenerate(options)}
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

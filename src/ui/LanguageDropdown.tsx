import { useEffect, useId, useRef, useState } from "react";
import { LANGUAGE_LABELS } from "../shared/i18n";
import type { AppLanguage } from "../shared/settingsStore";

interface LanguageDropdownProps {
  label: string;
  value: AppLanguage;
  onChange: (language: AppLanguage) => void;
}

const LANGUAGE_OPTIONS: AppLanguage[] = ["zh-CN", "en-US"];

export function LanguageDropdown({ label, value, onChange }: LanguageDropdownProps) {
  const [open, setOpen] = useState(false);
  const listboxId = useId();
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  return (
    <div className="language-dropdown" ref={rootRef}>
      <button
        aria-controls={open ? listboxId : undefined}
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={`${label} ${LANGUAGE_LABELS[value]}`}
        className="language-dropdown-trigger"
        type="button"
        onClick={() => setOpen((next) => !next)}
      >
        <span>{LANGUAGE_LABELS[value]}</span>
        <span aria-hidden="true">⌄</span>
      </button>
      {open ? (
        <div
          aria-label={label}
          className="language-dropdown-menu"
          id={listboxId}
          role="listbox"
        >
          {LANGUAGE_OPTIONS.map((language) => (
            <button
              aria-selected={language === value}
              className={language === value ? "is-selected" : ""}
              key={language}
              role="option"
              type="button"
              onClick={() => {
                onChange(language);
                setOpen(false);
              }}
            >
              <span aria-hidden="true">{language === value ? "✓" : ""}</span>
              <span>{LANGUAGE_LABELS[language]}</span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

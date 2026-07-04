import type { AppLanguage, PromptInsertionMode, Settings } from "../shared/settingsStore";
import { getMessages } from "../shared/i18n";
import { LanguageDropdown } from "./LanguageDropdown";

interface SettingsPanelProps {
  settings: Settings;
  onLanguageChange: (language: AppLanguage) => void;
  onPromptInsertionModeChange: (mode: PromptInsertionMode) => void;
  onBack?: () => void;
}

export function SettingsPanel({
  settings,
  onLanguageChange,
  onPromptInsertionModeChange,
  onBack,
}: SettingsPanelProps) {
  const t = getMessages(settings.language);

  return (
    <div className="settings-panel page-stack">
      <header className="page-header settings-page-header">
        <div className="settings-title-row">
          {onBack ? (
            <button
              aria-label={t.settings.backToManager}
              className="button icon-button settings-back-button"
              type="button"
              onClick={onBack}
            >
              ←
            </button>
          ) : null}
          <h1>{t.settings.title}</h1>
        </div>
      </header>

      <section className="settings-card">
        <div className="settings-card-heading">
          <h2>{t.settings.languageTitle}</h2>
        </div>
        <div className="settings-row">
          <span className="settings-row-main">
            <span className="settings-row-title">{t.settings.languageField}</span>
          </span>
          <span className="settings-row-control">
            <LanguageDropdown
              label={t.settings.languageField}
              value={settings.language}
              onChange={onLanguageChange}
            />
          </span>
        </div>
      </section>

      <section className="settings-card">
        <div className="settings-card-heading">
          <h2>{t.settings.clickBehaviorTitle}</h2>
        </div>
        <div className="settings-row">
          <div className="settings-row-main">
            <div className="settings-row-title">{t.settings.clickBehaviorField}</div>
          </div>
          <div className="settings-row-control">
            <div
              className="segmented-control settings-segmented-control"
              aria-label={t.settings.clickBehaviorTitle}
            >
              <button
                className={settings.promptInsertion.mode === "paste_only" ? "is-selected" : ""}
                type="button"
                aria-pressed={settings.promptInsertion.mode === "paste_only"}
                onClick={() => onPromptInsertionModeChange("paste_only")}
              >
                {t.settings.pasteOnly}
              </button>
              <button
                className={
                  settings.promptInsertion.mode === "paste_and_submit" ? "is-selected" : ""
                }
                type="button"
                aria-pressed={settings.promptInsertion.mode === "paste_and_submit"}
                onClick={() => onPromptInsertionModeChange("paste_and_submit")}
              >
                {t.settings.pasteAndSubmit}
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

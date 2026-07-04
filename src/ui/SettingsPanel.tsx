import type { AppLanguage, PromptInsertionMode, Settings } from "../shared/settingsStore";
import { LANGUAGE_LABELS, getMessages } from "../shared/i18n";

interface SettingsPanelProps {
  settings: Settings;
  onLanguageChange: (language: AppLanguage) => void;
  onPromptInsertionModeChange: (mode: PromptInsertionMode) => void;
}

export function SettingsPanel({
  settings,
  onLanguageChange,
  onPromptInsertionModeChange,
}: SettingsPanelProps) {
  const t = getMessages(settings.language);

  return (
    <div className="settings-panel page-stack">
      <header className="page-header settings-page-header">
        <div>
          <h1>{t.settings.title}</h1>
        </div>
      </header>

      <section className="settings-card">
        <div className="settings-card-heading">
          <h2>{t.settings.languageTitle}</h2>
        </div>
        <label className="settings-row">
          <span className="settings-row-main">
            <span className="settings-row-title">{t.settings.languageField}</span>
          </span>
          <span className="settings-row-control">
            <select
              className="field settings-select"
              value={settings.language}
              onChange={(event) => onLanguageChange(event.target.value as AppLanguage)}
            >
              <option value="zh-CN">{LANGUAGE_LABELS["zh-CN"]}</option>
              <option value="en-US">{LANGUAGE_LABELS["en-US"]}</option>
            </select>
          </span>
        </label>
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

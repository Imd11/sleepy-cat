interface SettingsPanelProps {
  onBack: () => void;
}

export function SettingsPanel({ onBack }: SettingsPanelProps) {
  return (
    <div className="settings-panel">
      <h2>Settings</h2>
      <p>Blacklist management coming soon...</p>
      <button onClick={onBack}>← Back</button>
    </div>
  );
}
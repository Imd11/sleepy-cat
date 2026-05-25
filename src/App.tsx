import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PromptItem } from "./shared/promptTypes";
import { PromptPopover } from "./ui/PromptPopover";
import { PromptManager } from "./ui/PromptManager";
import { SettingsPanel } from "./ui/SettingsPanel";
import type { AppMode } from "./app/AppMode";
import "./styles.css";

interface AppProps {
  prompts: PromptItem[];
}

export function App({ prompts }: AppProps) {
  const [mode, setMode] = useState<AppMode>("popover");

  const handleSelect = async (prompt: PromptItem) => {
    try {
      await invoke("paste_prompt", { body: prompt.body });
    } catch (e) {
      console.error("Failed to paste prompt:", e);
    }
  };

  const handleManage = () => {
    setMode("manager");
  };

  const handleSettings = () => {
    setMode("settings");
  };

  const handleBackToPopover = () => {
    setMode("popover");
  };

  if (mode === "manager") {
    return (
      <div className="app-container">
        <PromptManager
          prompts={prompts}
          onCreate={() => {}}
          onUpdate={() => {}}
          onDelete={() => {}}
          onReorder={() => {}}
          onImport={() => {}}
          onExport={() => {}}
        />
        <button className="back-btn" onClick={handleBackToPopover}>← Back</button>
      </div>
    );
  }

  if (mode === "settings") {
    return (
      <div className="app-container">
        <SettingsPanel onBack={handleBackToPopover} />
      </div>
    );
  }

  return (
    <div className="app-container">
      <PromptPopover
        prompts={prompts}
        onSelect={handleSelect}
        onManage={handleManage}
      />
    </div>
  );
}

export default App;
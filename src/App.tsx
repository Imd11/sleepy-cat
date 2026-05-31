import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import type { PromptItem } from "./shared/promptTypes";
import type { Settings } from "./shared/settingsStore";
import { createSettingsStore } from "./shared/settingsStore";
import { createPromptStore } from "./shared/promptStore";
import { createTauriPromptStorage } from "./storage/tauriPromptStorage";
import { createTauriSettingsStorage } from "./storage/tauriSettingsStorage";
import { getAccessibilityStatus, hidePromptPopover, pastePrompt } from "./platform/platformApi";
import { useInputTargetPolling } from "./overlay/useInputTargetPolling";
import { PromptQuickList } from "./ui/PromptQuickList";
import { PromptManager } from "./ui/PromptManager";
import { SettingsPanel } from "./ui/SettingsPanel";
import type { AppMode } from "./app/AppMode";
import "./styles.css";

interface AppProps {
  settings?: Settings;
  onRemoveBlacklist?: (bundleId: string) => void;
}

const DEFAULT_SETTINGS: Settings = {
  version: 1,
  blacklistedApps: [],
  overlayPlacement: { buttonOffset: null },
  floatingButton: { visible: true },
};

function initialWindowLabel() {
  return new URLSearchParams(window.location.search).has("mode")
    ? "prompt-popover"
    : "main";
}

export function App({
  settings = DEFAULT_SETTINGS,
}: AppProps) {
  const [mode, setMode] = useState<AppMode>(() => {
    const initialMode = new URLSearchParams(window.location.search).get("mode");
    if (initialMode === "manager") return "manager";
    if (initialMode === "settings") return "settings";
    if (initialMode === "button-controls") return "button-controls";
    return "popover";
  });
  const [windowLabel, setWindowLabel] = useState(initialWindowLabel);
  const [prompts, setPrompts] = useState<PromptItem[]>([]);
  const [activeSettings, setActiveSettings] = useState<Settings>(settings);
  const storeRef = useRef(createPromptStore(createTauriPromptStorage()));
  const settingsStoreRef = useRef(createSettingsStore(createTauriSettingsStorage()));

  useEffect(() => {
    storeRef.current.list().then(setPrompts);
    settingsStoreRef.current.get().then(setActiveSettings);
    try {
      setWindowLabel(getCurrentWindow().label);
    } catch {
      setWindowLabel(initialWindowLabel());
    }
  }, []);

  const handleSelect = async (prompt: PromptItem) => {
    try {
      const status = await getAccessibilityStatus();
      if (status?.trusted === false) {
        alert(
          "Accessibility permission required. Please grant permission in System Settings > Privacy & Security > Accessibility."
        );
        return;
      }
      await pastePrompt(prompt.body);
      await hidePromptPopover();
    } catch (e) {
      console.error("Failed to paste prompt:", e);
      alert("Failed to paste prompt. Please try again.");
    }
  };

  const handleManage = () => setMode("manager");
  const handleSettings = () => setMode("settings");
  const handleBackToPopover = () => setMode("popover");

  const removeBlacklistedApp = async (bundleId: string) => {
    await settingsStoreRef.current.removeBlacklistedApp(bundleId);
    setActiveSettings(await settingsStoreRef.current.get());
  };

  // eslint-disable-next-line react-hooks/rules-of-hooks
  useInputTargetPolling(
    activeSettings.blacklistedApps.map((app) => app.bundleId),
    activeSettings.overlayPlacement,
    {},
    activeSettings.floatingButton.visible
  );

  // ── Main window: show/hide floating button controls ────────────────────
  if (windowLabel === "main" && mode === "popover") {
    return (
      <MainWindow
        floatingButtonVisible={activeSettings.floatingButton.visible}
        onManage={handleManage}
        onSettings={handleSettings}
        onShowFloatingButton={async () => {
          await settingsStoreRef.current.setFloatingButtonVisible(true);
          setActiveSettings(await settingsStoreRef.current.get());
          const pos =
            await invoke<{ x: number; y: number }>("prompt_button_position_cmd");
          if (pos) {
            await invoke("show_prompt_button", { x: pos.x, y: pos.y });
          } else {
            await invoke("show_prompt_button", { x: 960, y: 700 });
          }
        }}
        onHideFloatingButton={async () => {
          await settingsStoreRef.current.setFloatingButtonVisible(false);
          setActiveSettings(await settingsStoreRef.current.get());
          await invoke("hide_prompt_button");
        }}
      />
    );
  }

  // ── Manager ─────────────────────────────────────────────────────────
  if (mode === "manager") {
    return (
      <div className="app-container">
        <PromptManager
          prompts={prompts}
          onCreate={async (input) => {
            await storeRef.current.create(input);
            setPrompts(await storeRef.current.list());
          }}
          onUpdate={async (id, input) => {
            await storeRef.current.update(id, input);
            setPrompts(await storeRef.current.list());
          }}
          onDelete={async (id) => {
            await storeRef.current.remove(id);
            setPrompts(await storeRef.current.list());
          }}
          onReorder={async (ids) => {
            await storeRef.current.reorder(ids);
            setPrompts(await storeRef.current.list());
          }}
          onImport={async () => {
            try {
              const file = await open({
                filters: [{ name: "JSON", extensions: ["json"] }],
                multiple: false,
              });
              if (file) {
                const content = await readTextFile(file as string);
                await storeRef.current.importJson(content);
                setPrompts(await storeRef.current.list());
              }
            } catch (e) {
              console.error("Import failed:", e);
              alert("Failed to import prompts. Please check the file format.");
            }
          }}
          onExport={async () => {
            try {
              const path = await save({
                filters: [{ name: "JSON", extensions: ["json"] }],
                defaultPath: "prompts.json",
              });
              if (path) {
                const json = await storeRef.current.exportJson();
                await writeTextFile(path, json);
              }
            } catch (e) {
              console.error("Export failed:", e);
              alert("Failed to export prompts. Please try again.");
            }
          }}
        />
        <button className="back-btn" onClick={handleBackToPopover}>
          ← Back
        </button>
      </div>
    );
  }

  // ── Settings ────────────────────────────────────────────────────────
  if (mode === "settings") {
    return (
      <div className="app-container">
        <SettingsPanel
          settings={activeSettings}
          onRemove={removeBlacklistedApp}
        />
        <button className="back-btn" onClick={handleBackToPopover}>
          ← Back
        </button>
      </div>
    );
  }

  // ── Button controls mode ────────────────────────────────────────────
  if (mode === "button-controls") {
    return (
      <div className="app-container button-controls">
        <button
          onClick={async () => {
            await settingsStoreRef.current.setFloatingButtonVisible(false);
            setActiveSettings(await settingsStoreRef.current.get());
            await hidePromptPopover();
          }}
        >
          Hide Button
        </button>
        <button
          onClick={async () => {
            const { openMainWindow } = await import("./platform/platformApi");
            await openMainWindow();
            await hidePromptPopover();
          }}
        >
          Open Prompt Picker
        </button>
      </div>
    );
  }

  // ── Default: popover quick-list ─────────────────────────────────────
  return (
    <div className="app-container">
      <PromptQuickList prompts={prompts} onSelect={handleSelect} />
    </div>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

function MainWindow({
  floatingButtonVisible,
  onManage,
  onSettings,
  onShowFloatingButton,
  onHideFloatingButton,
}: {
  floatingButtonVisible: boolean;
  onManage: () => void;
  onSettings: () => void;
  onShowFloatingButton: () => void;
  onHideFloatingButton: () => void;
}) {
  return (
    <div className="app-container">
      <div className="floating-button-status">
        <h2>Prompt Picker</h2>
        <p>Status: {floatingButtonVisible ? "Visible" : "Hidden"}</p>
        <div className="main-actions">
          {floatingButtonVisible ? (
            <button onClick={onHideFloatingButton}>Hide Floating Button</button>
          ) : (
            <button onClick={onShowFloatingButton}>Show Floating Button</button>
          )}
        </div>
      </div>
      <div className="main-actions">
        <button onClick={onManage}>Manage Prompts</button>
        <button onClick={onSettings}>Settings</button>
      </div>
    </div>
  );
}

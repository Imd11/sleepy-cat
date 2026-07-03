import { useState, useEffect, useRef, useCallback } from "react";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import type { PromptContainer } from "./shared/promptTypes";
import { getPromptContainerBodies } from "./shared/promptTypes";
import type { Settings } from "./shared/settingsStore";
import { createSettingsStore } from "./shared/settingsStore";
import { createPromptStore } from "./shared/promptStore";
import { createTauriPromptStorage } from "./storage/tauriPromptStorage";
import { createTauriSettingsStorage } from "./storage/tauriSettingsStorage";
import {
  hidePromptButton,
  hidePromptPopover,
  openAccessibilitySettings,
  openMainWindow,
  pastePromptAndSubmitToLastTarget,
  pastePromptSequenceAndSubmitToLastTarget,
  quitPromptPicker,
} from "./platform/platformApi";
import type { AutosendOutcome, AutosendSequenceOutcome } from "./platform/platformApi";
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
  overlayPlacement: { buttonOffset: null, buttonPosition: null },
  floatingButton: { visible: true },
};

const waitForWindowHide = () => new Promise((resolve) => window.setTimeout(resolve, 260));

type AutosendStatusKind = "sent" | "failed";
type AutosendStatusAction = "open_accessibility_settings" | "request_accessibility_permission";

async function emitAutosendStatus(
  kind: AutosendStatusKind,
  message: string,
  action?: AutosendStatusAction
) {
  try {
    const payload = action ? { kind, message, action } : { kind, message };
    await emit("prompt-autosend-status", payload);
  } catch (error) {
    console.warn("Failed to emit autosend status:", error);
  }
}

async function emitPromptThrowSend(kind: "single" | "group") {
  try {
    await emit("prompt-throw-send", { kind });
  } catch (error) {
    console.warn("Failed to emit prompt throw animation:", error);
  }
}

async function emitPromptPopoverDismissed() {
  try {
    await emit("prompt-popover-dismissed");
  } catch (error) {
    console.warn("Failed to emit prompt popover dismissal:", error);
  }
}

function statusForAutosendOutcome(outcome: AutosendOutcome): {
  kind: AutosendStatusKind;
  message: string;
  action?: AutosendStatusAction;
} {
  if (outcome.sent) {
    return { kind: "sent", message: "已粘贴并回车" };
  }

  switch (outcome.reason) {
    case "missing_accessibility_permission":
      return {
        kind: "failed",
        message: "点击授权",
        action: "request_accessibility_permission",
      };
    case "no_safe_target":
      return { kind: "failed", message: "已复制，未发送" };
    case "copy_failed":
      return { kind: "failed", message: "未能复制" };
    case "paste_event_failed":
      return { kind: "failed", message: "未能粘贴" };
    case "return_event_failed":
      return { kind: "failed", message: "已粘贴，未发送" };
    case "target_focus_failed":
      return { kind: "failed", message: "请先切到输入页" };
    default:
      return {
        kind: "failed",
        message: outcome.copied ? "未能自动发送" : "未能复制",
      };
  }
}

function statusForAutosendSequenceOutcome(outcome: AutosendSequenceOutcome): {
  kind: AutosendStatusKind;
  message: string;
  action?: AutosendStatusAction;
} {
  if (outcome.sent) {
    return { kind: "sent", message: "已粘贴并回车" };
  }
  if (outcome.reason === "missing_accessibility_permission") {
    return {
      kind: "failed",
      message: "点击授权",
      action: "request_accessibility_permission",
    };
  }
  return {
    kind: "failed",
    message: `第 ${outcome.failed_index ?? 1} 条失败`,
  };
}

interface InputTargetPollingControllerProps {
  settings: Settings;
  onButtonDragEnd: (
    position: { x: number; y: number },
    basePosition: [number, number] | null
  ) => void;
}

function InputTargetPollingController({
  settings,
  onButtonDragEnd,
}: InputTargetPollingControllerProps) {
  useInputTargetPolling(
    settings.blacklistedApps.map((app) => app.bundleId),
    settings.overlayPlacement,
    { onButtonDragEnd },
    settings.floatingButton.visible
  );

  return null;
}

function initialWindowLabel() {
  return new URLSearchParams(window.location.search).has("mode")
    ? "prompt-popover"
    : "main";
}

function currentWindowLabel() {
  try {
    return getCurrentWindow().label;
  } catch {
    return initialWindowLabel();
  }
}

export function App({
  settings = DEFAULT_SETTINGS,
}: AppProps) {
  const [mode, setMode] = useState<AppMode>(() => {
    const initialMode = new URLSearchParams(window.location.search).get("mode");
    if (initialMode === "manager") return "manager";
    if (initialMode === "settings") return "settings";
    if (initialMode === "button-controls") return "button-controls";
    return currentWindowLabel() === "main" ? "manager" : "popover";
  });
  const [windowLabel, setWindowLabel] = useState(currentWindowLabel);
  const [prompts, setPrompts] = useState<PromptContainer[]>([]);
  const [submittingPromptId, setSubmittingPromptId] = useState<string | null>(null);
  const [activeSettings, setActiveSettings] = useState<Settings>(settings);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const storeRef = useRef(createPromptStore(createTauriPromptStorage()));
  const settingsStoreRef = useRef(createSettingsStore(createTauriSettingsStorage()));

  useEffect(() => {
    let active = true;
    storeRef.current.list().then((items) => {
      if (active) setPrompts(items);
    });
    settingsStoreRef.current.get().then((loadedSettings) => {
      if (!active) return;
      setActiveSettings(loadedSettings);
      setSettingsLoaded(true);
    });
    const label = currentWindowLabel();
    setWindowLabel(label);

    return () => {
      active = false;
    };
  }, []);

  const handleSelect = async (prompt: PromptContainer) => {
    if (submittingPromptId) return;
    setSubmittingPromptId(prompt.id);
    try {
      await hidePromptPopover();
      await waitForWindowHide();
      await emitPromptThrowSend(prompt.type === "group" ? "group" : "single");
      const bodies = getPromptContainerBodies(prompt);
      if (bodies.length === 0) {
        await emitAutosendStatus("failed", "未能发送，请重试");
        return;
      }
      const status = prompt.type === "group"
        ? statusForAutosendSequenceOutcome(
          await pastePromptSequenceAndSubmitToLastTarget(bodies, prompt.intervalMs)
        )
        : statusForAutosendOutcome(
          await pastePromptAndSubmitToLastTarget(bodies[0])
        );
      await emitAutosendStatus(status.kind, status.message, status.action);
    } catch (e) {
      console.warn("Prompt autosend failed without blocking the picker:", e);
      await emitAutosendStatus("failed", "未能发送，请重试");
    } finally {
      setSubmittingPromptId(null);
    }
  };

  const handleBackToPopover = () => setMode("popover");

  const handleButtonDragEnd = useCallback(
    async (
      position: { x: number; y: number },
      _basePosition: [number, number] | null
    ) => {
      await settingsStoreRef.current.setOverlayButtonPosition(position);
      setActiveSettings(await settingsStoreRef.current.get());
    },
    []
  );

  const removeBlacklistedApp = async (bundleId: string) => {
    await settingsStoreRef.current.removeBlacklistedApp(bundleId);
    setActiveSettings(await settingsStoreRef.current.get());
  };

  const pollingController =
    windowLabel === "main" && settingsLoaded ? (
      <InputTargetPollingController
        settings={activeSettings}
        onButtonDragEnd={handleButtonDragEnd}
      />
    ) : null;

  // ── Manager ─────────────────────────────────────────────────────────
  if (mode === "manager") {
    return (
      <>
        {pollingController}
        <div className="app-window app-window-main">
          <PromptManager
            prompts={prompts}
            onCreate={async (input) => {
              await storeRef.current.create(input);
              setPrompts(await storeRef.current.list());
            }}
            onCreateGroup={async (input) => {
              await storeRef.current.createGroup(input);
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
          {windowLabel !== "main" ? (
            <div className="page-footer">
              <button className="button button-secondary" onClick={handleBackToPopover}>
                Back
              </button>
            </div>
          ) : null}
        </div>
      </>
    );
  }

  // ── Settings ────────────────────────────────────────────────────────
  if (mode === "settings") {
    return (
      <>
        {pollingController}
        <div className="app-window app-window-main">
          <SettingsPanel
            settings={activeSettings}
            onRemove={removeBlacklistedApp}
          />
          <div className="page-footer">
            <button className="button button-secondary" onClick={handleBackToPopover}>
              Back
            </button>
          </div>
        </div>
      </>
    );
  }

  // ── Button controls mode ────────────────────────────────────────────
  if (mode === "button-controls") {
    return (
      <>
        {pollingController}
        <div className="button-controls">
          <button
            className="button button-primary"
            onClick={async () => {
              await openMainWindow();
              await hidePromptPopover();
              await emitPromptPopoverDismissed();
            }}
          >
            Manage Prompts...
          </button>
          <button
            className="button button-secondary"
            onClick={async () => {
              await settingsStoreRef.current.setFloatingButtonVisible(false);
              setActiveSettings(await settingsStoreRef.current.get());
              await hidePromptButton();
              await hidePromptPopover();
              await emitPromptPopoverDismissed();
            }}
          >
            Hide Calico
          </button>
          <button
            className="button button-secondary"
            onClick={async () => {
              await openAccessibilitySettings();
              await hidePromptPopover();
              await emitPromptPopoverDismissed();
            }}
          >
            Open Accessibility Settings
          </button>
          <button
            className="button button-danger"
            onClick={async () => {
              await quitPromptPicker();
            }}
          >
            Quit Prompt Picker
          </button>
        </div>
      </>
    );
  }

  // ── Default: popover quick-list ─────────────────────────────────────
  return (
    <>
      {pollingController}
      <div className="popover-window">
        <PromptQuickList
          prompts={prompts}
          onSelect={handleSelect}
          submittingPromptId={submittingPromptId}
        />
      </div>
    </>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

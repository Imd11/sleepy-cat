import { useState, useEffect, useRef, useCallback } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import type { PromptContainer } from "./shared/promptTypes";
import { getPromptContainerBodies } from "./shared/promptTypes";
import type { AppLanguage, PromptInsertionMode, Settings } from "./shared/settingsStore";
import { createSettingsStore } from "./shared/settingsStore";
import { getMessages, type Messages } from "./shared/i18n";
import { createPromptStore } from "./shared/promptStore";
import { createTauriPromptStorage } from "./storage/tauriPromptStorage";
import { createTauriSettingsStorage } from "./storage/tauriSettingsStorage";
import {
  hidePromptButton,
  hidePromptPopover,
  openAccessibilitySettings,
  openMainWindow,
  pastePromptToLastTarget,
  pastePromptAndSubmitToLastTarget,
  pastePromptSequenceAndSubmitToLastTarget,
  quitPromptPicker,
  setMenuLanguage,
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
  promptInsertion: { mode: "paste_and_submit" },
  language: "zh-CN",
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

async function emitPromptPopoverDismissed() {
  try {
    await emit("prompt-popover-dismissed");
  } catch (error) {
    console.warn("Failed to emit prompt popover dismissal:", error);
  }
}

function pasteOnlyBody(prompt: PromptContainer, bodies: string[]): string {
  if (prompt.type === "group") return bodies.join("\n\n");
  return bodies[0] ?? "";
}

function statusForAutosendOutcome(outcome: AutosendOutcome, t: Messages): {
  kind: AutosendStatusKind;
  message: string;
  action?: AutosendStatusAction;
} {
  if (outcome.sent) {
    return { kind: "sent", message: t.autosend.sent };
  }

  switch (outcome.reason) {
    case "missing_accessibility_permission":
      return {
        kind: "failed",
        message: t.autosend.clickToAuthorize,
        action: "request_accessibility_permission",
      };
    case "no_safe_target":
      return { kind: "failed", message: t.autosend.copiedNotSent };
    case "copy_failed":
      return { kind: "failed", message: t.autosend.copyFailed };
    case "paste_event_failed":
      return { kind: "failed", message: t.autosend.pasteFailed };
    case "return_event_failed":
      return { kind: "failed", message: t.autosend.pastedNotSent };
    case "target_focus_failed":
      return { kind: "failed", message: t.autosend.targetFocusFailed };
    default:
      return {
        kind: "failed",
        message: outcome.copied ? t.autosend.automaticFailed : t.autosend.copyFailed,
      };
  }
}

function statusForAutosendSequenceOutcome(outcome: AutosendSequenceOutcome, t: Messages): {
  kind: AutosendStatusKind;
  message: string;
  action?: AutosendStatusAction;
} {
  if (outcome.sent) {
    return { kind: "sent", message: t.autosend.sent };
  }
  if (outcome.reason === "missing_accessibility_permission") {
    return {
      kind: "failed",
      message: t.autosend.clickToAuthorize,
      action: "request_accessibility_permission",
    };
  }
  return {
    kind: "failed",
    message: t.autosend.sequenceFailed(outcome.failed_index ?? 1),
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
  const [hoverResetKey, setHoverResetKey] = useState(0);
  const storeRef = useRef(createPromptStore(createTauriPromptStorage()));
  const settingsStoreRef = useRef(createSettingsStore(createTauriSettingsStorage()));
  const promptListRefreshingRef = useRef(false);
  const t = getMessages(activeSettings.language);
  const reloadPrompts = useCallback(async () => {
    setPrompts(await storeRef.current.list());
  }, []);
  const resetPromptHoverPreview = useCallback(() => {
    setHoverResetKey((key) => key + 1);
  }, []);

  useEffect(() => {
    let active = true;
    storeRef.current.list().then((items) => {
      if (active) setPrompts(items);
    });
    settingsStoreRef.current.get().then((loadedSettings) => {
      if (!active) return;
      setActiveSettings(loadedSettings);
      setSettingsLoaded(true);
      setMenuLanguage(loadedSettings.language).catch((error) => {
        console.warn("Failed to update menu language:", error);
      });
    });
    const label = currentWindowLabel();
    setWindowLabel(label);

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    const disposers: Array<() => void> = [];

    listen("open-manager-window", () => {
      if (!active || currentWindowLabel() !== "main") return;
      setMode("manager");
    })
      .then((unlisten) => {
        if (active) {
          disposers.push(unlisten);
          return;
        }
        unlisten();
      })
      .catch((error) => {
        console.warn("Failed to listen for manager window requests:", error);
      });

    listen("open-settings-window", () => {
      if (!active || currentWindowLabel() !== "main") return;
      setMode("settings");
    })
      .then((unlisten) => {
        if (active) {
          disposers.push(unlisten);
          return;
        }
        unlisten();
      })
      .catch((error) => {
        console.warn("Failed to listen for settings window requests:", error);
      });

    return () => {
      active = false;
      disposers.forEach((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    if (windowLabel !== "prompt-popover") return;

    let active = true;
    const disposers: Array<() => void> = [];

    listen<string>("prompt-popover-opened", async (event) => {
      if (!active || event.payload !== "popover") return;
      resetPromptHoverPreview();
      promptListRefreshingRef.current = true;
      try {
        await reloadPrompts();
      } finally {
        promptListRefreshingRef.current = false;
      }
    })
      .then((unlisten) => {
        if (active) {
          disposers.push(unlisten);
          return;
        }
        unlisten();
      })
      .catch((error) => {
        console.warn("Failed to listen for prompt popover refresh:", error);
      });

    listen("prompt-popover-dismissed", () => {
      if (!active || currentWindowLabel() !== "prompt-popover") return;
      resetPromptHoverPreview();
    })
      .then((unlisten) => {
        if (active) {
          disposers.push(unlisten);
          return;
        }
        unlisten();
      })
      .catch((error) => {
        console.warn("Failed to listen for prompt popover dismissal:", error);
      });

    return () => {
      active = false;
      disposers.forEach((dispose) => dispose());
    };
  }, [reloadPrompts, resetPromptHoverPreview, windowLabel]);

  const handleSelect = async (prompt: PromptContainer) => {
    if (submittingPromptId || promptListRefreshingRef.current) return;
    setSubmittingPromptId(prompt.id);
    try {
      await hidePromptPopover();
      await waitForWindowHide();
      const bodies = getPromptContainerBodies(prompt);
      if (bodies.length === 0) {
        await emitAutosendStatus("failed", t.autosend.genericFailed);
        return;
      }
      if (activeSettings.promptInsertion.mode === "paste_only") {
        await pastePromptToLastTarget(pasteOnlyBody(prompt, bodies));
        await emitAutosendStatus("sent", t.autosend.insertedIntoInput);
        return;
      }
      const status = prompt.type === "group"
        ? statusForAutosendSequenceOutcome(
          await pastePromptSequenceAndSubmitToLastTarget(bodies, prompt.intervalMs),
          t
        )
        : statusForAutosendOutcome(
          await pastePromptAndSubmitToLastTarget(bodies[0]),
          t
        );
      await emitAutosendStatus(status.kind, status.message, status.action);
    } catch (e) {
      console.warn("Prompt autosend failed without blocking the picker:", e);
      await emitAutosendStatus("failed", t.autosend.genericFailed);
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

  const updatePromptInsertionMode = async (mode: PromptInsertionMode) => {
    await settingsStoreRef.current.setPromptInsertionMode(mode);
    setActiveSettings(await settingsStoreRef.current.get());
  };

  const updateLanguage = async (language: AppLanguage) => {
    await settingsStoreRef.current.setLanguage(language);
    const nextSettings = await settingsStoreRef.current.get();
    setActiveSettings(nextSettings);
    setMenuLanguage(language).catch((error) => {
      console.warn("Failed to update menu language:", error);
    });
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
            messages={t}
            onOpenSettings={() => setMode("settings")}
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
                alert(t.manager.importFailed);
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
                alert(t.manager.exportFailed);
              }
            }}
          />
          {windowLabel !== "main" ? (
            <div className="page-footer">
              <button className="button button-secondary" onClick={handleBackToPopover}>
                {t.common.back}
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
            onLanguageChange={updateLanguage}
            onPromptInsertionModeChange={updatePromptInsertionMode}
          />
          {windowLabel !== "main" ? (
            <div className="page-footer">
              <button className="button button-secondary" onClick={handleBackToPopover}>
                {t.common.back}
              </button>
            </div>
          ) : null}
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
              {t.buttonControls.managePrompts}
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
            {t.buttonControls.hideCalico}
          </button>
          <button
            className="button button-secondary"
            onClick={async () => {
              await openAccessibilitySettings();
              await hidePromptPopover();
              await emitPromptPopoverDismissed();
            }}
          >
            {t.buttonControls.openAccessibilitySettings}
          </button>
          <button
            className="button button-danger"
            onClick={async () => {
              await quitPromptPicker();
            }}
          >
            {t.buttonControls.quit}
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
          messages={t.quickList}
          groupMeta={t.manager.groupMeta}
          onSelect={handleSelect}
          submittingPromptId={submittingPromptId}
          hoverResetKey={hoverResetKey}
        />
      </div>
    </>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

export type OverlayButtonOffset = {
  x: number;
  y: number;
};

export type OverlayButtonPosition = {
  x: number;
  y: number;
};

export type PromptInsertionMode = "paste_only" | "paste_and_submit";
export type AppLanguage = "zh-CN" | "en-US";

export type PromptLibraryLink = {
  mode: "copy" | "linked";
  path: string | null;
  lastKnownSignature: string | null;
  lastSyncedAt: string | null;
};

export type Settings = {
  version: 1;
  language: AppLanguage;
  blacklistedApps: Array<{ bundleId: string; name: string }>;
  overlayPlacement: {
    buttonOffset: OverlayButtonOffset | null;
    buttonPosition: OverlayButtonPosition | null;
  };
  floatingButton: {
    visible: boolean;
  };
  promptInsertion: {
    mode: PromptInsertionMode;
  };
  permissions: {
    accessibilityPromptRequested: boolean;
  };
  promptLibraryLink: PromptLibraryLink;
};

interface SettingsAdapter {
  read(): Promise<string | null>;
  write(value: string): Promise<void>;
}

export function createSettingsStore(adapter: SettingsAdapter) {
  function defaultPromptLibraryLink(): PromptLibraryLink {
    return {
      mode: "copy",
      path: null,
      lastKnownSignature: null,
      lastSyncedAt: null,
    };
  }

  function defaultSettings(): Settings {
    return {
      version: 1,
      language: "zh-CN",
      blacklistedApps: [],
      overlayPlacement: { buttonOffset: null, buttonPosition: null },
      floatingButton: { visible: true },
      promptInsertion: { mode: "paste_and_submit" },
      permissions: { accessibilityPromptRequested: false },
      promptLibraryLink: defaultPromptLibraryLink()
    };
  }

  function normalizePoint<T extends OverlayButtonOffset | OverlayButtonPosition>(value: unknown): T | null {
    if (!value || typeof value !== "object") return null;
    const candidate = value as Partial<T>;
    if (typeof candidate.x !== "number" || typeof candidate.y !== "number") return null;
    if (!Number.isFinite(candidate.x) || !Number.isFinite(candidate.y)) return null;
    return { x: candidate.x, y: candidate.y } as T;
  }

  function normalizeSettings(value: unknown): Settings {
    if (!value || typeof value !== "object") return defaultSettings();
    const candidate = value as Partial<Settings> & {
      overlayPlacement?: { buttonOffset?: unknown; buttonPosition?: unknown };
      permissions?: { accessibilityPromptRequested?: unknown };
      promptLibraryLink?: Partial<PromptLibraryLink>;
    };
    if (candidate.version !== 1 || !Array.isArray(candidate.blacklistedApps)) {
      return defaultSettings();
    }
    const rawPromptLibraryLink = candidate.promptLibraryLink;
    const promptLibraryPath = typeof rawPromptLibraryLink?.path === "string" && rawPromptLibraryLink.path.trim()
      ? rawPromptLibraryLink.path
      : null;
    const promptLibraryLink: PromptLibraryLink = rawPromptLibraryLink?.mode === "linked" && promptLibraryPath
      ? {
          mode: "linked",
          path: promptLibraryPath,
          lastKnownSignature: typeof rawPromptLibraryLink.lastKnownSignature === "string"
            ? rawPromptLibraryLink.lastKnownSignature
            : null,
          lastSyncedAt: typeof rawPromptLibraryLink.lastSyncedAt === "string"
            ? rawPromptLibraryLink.lastSyncedAt
            : null,
        }
      : defaultPromptLibraryLink();
    return {
      version: 1,
      language: candidate.language === "en-US" ? "en-US" : "zh-CN",
      blacklistedApps: candidate.blacklistedApps
        .filter((app) => app && typeof app.bundleId === "string" && typeof app.name === "string")
        .map((app) => ({ bundleId: app.bundleId, name: app.name })),
      overlayPlacement: {
        buttonOffset: normalizePoint<OverlayButtonOffset>(candidate.overlayPlacement?.buttonOffset),
        buttonPosition: normalizePoint<OverlayButtonPosition>(candidate.overlayPlacement?.buttonPosition)
      },
      floatingButton: {
        visible: candidate.floatingButton?.visible === false ? false : true
      },
      promptInsertion: {
        mode: candidate.promptInsertion?.mode === "paste_only"
          ? "paste_only"
          : "paste_and_submit"
      },
      permissions: {
        accessibilityPromptRequested: candidate.permissions?.accessibilityPromptRequested === true
      },
      promptLibraryLink
    };
  }

  async function load(): Promise<Settings> {
    const data = await adapter.read();
    if (!data) return defaultSettings();
    try {
      return normalizeSettings(JSON.parse(data));
    } catch {
      return defaultSettings();
    }
  }

  async function save(settings: Settings): Promise<void> {
    await adapter.write(JSON.stringify(settings, null, 2));
  }

  return {
    async get(): Promise<Settings> {
      return load();
    },

    async addBlacklistedApp(bundleId: string, name: string): Promise<void> {
      const settings = await load();
      if (settings.blacklistedApps.some(a => a.bundleId === bundleId)) {
        return; // duplicate
      }
      settings.blacklistedApps.push({ bundleId, name });
      await save(settings);
    },

    async removeBlacklistedApp(bundleId: string): Promise<void> {
      const settings = await load();
      settings.blacklistedApps = settings.blacklistedApps.filter(a => a.bundleId !== bundleId);
      await save(settings);
    },

    async isAppBlacklisted(bundleId: string): Promise<boolean> {
      const settings = await load();
      return settings.blacklistedApps.some(a => a.bundleId === bundleId);
    },

    async setOverlayButtonOffset(offset: OverlayButtonOffset | null): Promise<void> {
      const settings = await load();
      settings.overlayPlacement.buttonOffset = offset;
      await save(settings);
    },

    async setOverlayButtonPosition(position: OverlayButtonPosition | null): Promise<void> {
      const settings = await load();
      settings.overlayPlacement.buttonPosition = position;
      settings.overlayPlacement.buttonOffset = null;
      await save(settings);
    },

    async setFloatingButtonVisible(visible: boolean): Promise<void> {
      const settings = await load();
      settings.floatingButton.visible = visible;
      await save(settings);
    },

    async setPromptInsertionMode(mode: PromptInsertionMode): Promise<void> {
      const settings = await load();
      settings.promptInsertion.mode = mode;
      await save(settings);
    },

    async setLanguage(language: AppLanguage): Promise<void> {
      const settings = await load();
      settings.language = language;
      await save(settings);
    },

    async setAccessibilityPromptRequested(requested: boolean): Promise<void> {
      const settings = await load();
      settings.permissions.accessibilityPromptRequested = requested;
      await save(settings);
    },

    async setPromptLibraryLink(link: PromptLibraryLink): Promise<void> {
      const settings = await load();
      settings.promptLibraryLink = link.mode === "linked" && link.path
        ? link
        : defaultPromptLibraryLink();
      await save(settings);
    },

    async clearPromptLibraryLink(): Promise<void> {
      const settings = await load();
      settings.promptLibraryLink = defaultPromptLibraryLink();
      await save(settings);
    }
  };
}

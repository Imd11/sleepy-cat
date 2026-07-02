export type OverlayButtonOffset = {
  x: number;
  y: number;
};

export type OverlayButtonPosition = {
  x: number;
  y: number;
};

export type Settings = {
  version: 1;
  blacklistedApps: Array<{ bundleId: string; name: string }>;
  overlayPlacement: {
    buttonOffset: OverlayButtonOffset | null;
    buttonPosition: OverlayButtonPosition | null;
  };
  floatingButton: {
    visible: boolean;
  };
};

interface SettingsAdapter {
  read(): Promise<string | null>;
  write(value: string): Promise<void>;
}

export function createSettingsStore(adapter: SettingsAdapter) {
  function defaultSettings(): Settings {
    return {
      version: 1,
      blacklistedApps: [],
      overlayPlacement: { buttonOffset: null, buttonPosition: null },
      floatingButton: { visible: true }
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
    };
    if (candidate.version !== 1 || !Array.isArray(candidate.blacklistedApps)) {
      return defaultSettings();
    }
    return {
      version: 1,
      blacklistedApps: candidate.blacklistedApps
        .filter((app) => app && typeof app.bundleId === "string" && typeof app.name === "string")
        .map((app) => ({ bundleId: app.bundleId, name: app.name })),
      overlayPlacement: {
        buttonOffset: normalizePoint<OverlayButtonOffset>(candidate.overlayPlacement?.buttonOffset),
        buttonPosition: normalizePoint<OverlayButtonPosition>(candidate.overlayPlacement?.buttonPosition)
      },
      floatingButton: {
        visible: candidate.floatingButton?.visible === false ? false : true
      }
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
    }
  };
}

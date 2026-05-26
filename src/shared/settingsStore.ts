export type OverlayButtonOffset = {
  x: number;
  y: number;
};

export type Settings = {
  version: 1;
  blacklistedApps: Array<{ bundleId: string; name: string }>;
  overlayPlacement: {
    buttonOffset: OverlayButtonOffset | null;
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
      overlayPlacement: { buttonOffset: null }
    };
  }

  function normalizeOffset(value: unknown): OverlayButtonOffset | null {
    if (!value || typeof value !== "object") return null;
    const candidate = value as Partial<OverlayButtonOffset>;
    if (typeof candidate.x !== "number" || typeof candidate.y !== "number") return null;
    if (!Number.isFinite(candidate.x) || !Number.isFinite(candidate.y)) return null;
    return { x: candidate.x, y: candidate.y };
  }

  async function load(): Promise<Settings> {
    const data = await adapter.read();
    if (!data) return defaultSettings();
    try {
      const parsed = JSON.parse(data) as Settings;
      // Ensure overlayPlacement exists
      if (!parsed.overlayPlacement) {
        parsed.overlayPlacement = { buttonOffset: null };
      }
      // Ensure buttonOffset is normalized
      parsed.overlayPlacement.buttonOffset = normalizeOffset(parsed.overlayPlacement.buttonOffset);
      return parsed;
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
    }
  };
}
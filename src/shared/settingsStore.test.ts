import { describe, expect, it } from "vitest";
import { createSettingsStore } from "./settingsStore";

function createTestSettingsStore(initial?: string | null) {
  let state = initial ?? null;
  return createSettingsStore({
    read: async () => state,
    write: async (value) => { state = value; }
  });
}

describe("settings store", () => {
  it("default blacklist is empty", async () => {
    const store = createTestSettingsStore();
    const settings = await store.get();
    expect(settings.blacklistedApps).toEqual([]);
  });

  it("add bundle id to blacklist", async () => {
    const store = createTestSettingsStore();
    await store.addBlacklistedApp("com.example.app", "Example App");
    const settings = await store.get();
    expect(settings.blacklistedApps).toContainEqual({ bundleId: "com.example.app", name: "Example App" });
  });

  it("remove bundle id from blacklist", async () => {
    const store = createTestSettingsStore('{"version":1,"blacklistedApps":[{"bundleId":"com.example.app","name":"Example App"}]}');
    await store.removeBlacklistedApp("com.example.app");
    const settings = await store.get();
    expect(settings.blacklistedApps.find(a => a.bundleId === "com.example.app")).toBeUndefined();
  });

  it("duplicate adds are ignored", async () => {
    const store = createTestSettingsStore();
    await store.addBlacklistedApp("com.example.app", "Example App");
    await store.addBlacklistedApp("com.example.app", "Example App");
    const settings = await store.get();
    const matching = settings.blacklistedApps.filter(a => a.bundleId === "com.example.app");
    expect(matching).toHaveLength(1);
  });

  it("defaults overlay placement to no saved offset", async () => {
    const store = createTestSettingsStore();
    const settings = await store.get();
    expect(settings.overlayPlacement).toEqual({ buttonOffset: null });
  });

  it("saves overlay button offset", async () => {
    const store = createTestSettingsStore();
    await store.setOverlayButtonOffset({ x: 24, y: -12 });
    const settings = await store.get();
    expect(settings.overlayPlacement.buttonOffset).toEqual({ x: 24, y: -12 });
  });

  it("normalizes old settings without overlay placement", async () => {
    const store = createTestSettingsStore('{"version":1,"blacklistedApps":[]}');
    const settings = await store.get();
    expect(settings.overlayPlacement).toEqual({ buttonOffset: null });
  });

  it("defaults floating button visibility to visible", async () => {
    const store = createTestSettingsStore();
    const settings = await store.get();
    expect(settings.floatingButton.visible).toBe(true);
  });

  it("saves floating button visibility", async () => {
    const store = createTestSettingsStore();
    await store.setFloatingButtonVisible(false);
    expect((await store.get()).floatingButton.visible).toBe(false);
    await store.setFloatingButtonVisible(true);
    expect((await store.get()).floatingButton.visible).toBe(true);
  });

  it("normalizes old settings without floating button visibility", async () => {
    const store = createTestSettingsStore('{"version":1,"blacklistedApps":[],"overlayPlacement":{"buttonOffset":null}}');
    const settings = await store.get();
    expect(settings.floatingButton.visible).toBe(true);
  });
});
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
    expect(settings.promptInsertion.mode).toBe("paste_and_submit");
    expect(settings.language).toBe("zh-CN");
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
    expect(settings.overlayPlacement).toEqual({ buttonOffset: null, buttonPosition: null });
  });

  it("saves overlay button offset", async () => {
    const store = createTestSettingsStore();
    await store.setOverlayButtonOffset({ x: 24, y: -12 });
    const settings = await store.get();
    expect(settings.overlayPlacement.buttonOffset).toEqual({ x: 24, y: -12 });
  });

  it("saves overlay button position and clears legacy offset", async () => {
    const store = createTestSettingsStore();
    await store.setOverlayButtonOffset({ x: 24, y: -12 });
    await store.setOverlayButtonPosition({ x: 420, y: 260 });
    const settings = await store.get();
    expect(settings.overlayPlacement.buttonPosition).toEqual({ x: 420, y: 260 });
    expect(settings.overlayPlacement.buttonOffset).toBeNull();
  });

  it("normalizes old settings without overlay placement", async () => {
    const store = createTestSettingsStore('{"version":1,"blacklistedApps":[]}');
    const settings = await store.get();
    expect(settings.overlayPlacement).toEqual({ buttonOffset: null, buttonPosition: null });
  });

  it("normalizes old settings without absolute button position", async () => {
    const store = createTestSettingsStore('{"version":1,"blacklistedApps":[],"overlayPlacement":{"buttonOffset":{"x":24,"y":-12}}}');
    const settings = await store.get();
    expect(settings.overlayPlacement).toEqual({
      buttonOffset: { x: 24, y: -12 },
      buttonPosition: null
    });
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

  it("saves prompt insertion mode", async () => {
    const store = createTestSettingsStore();
    await store.setPromptInsertionMode("paste_only");
    expect((await store.get()).promptInsertion.mode).toBe("paste_only");
    await store.setPromptInsertionMode("paste_and_submit");
    expect((await store.get()).promptInsertion.mode).toBe("paste_and_submit");
  });

  it("normalizes old settings without prompt insertion mode", async () => {
    const store = createTestSettingsStore(
      '{"version":1,"blacklistedApps":[],"overlayPlacement":{"buttonOffset":null},"floatingButton":{"visible":true}}'
    );
    const settings = await store.get();
    expect(settings.promptInsertion.mode).toBe("paste_and_submit");
  });

  it("saves language", async () => {
    const store = createTestSettingsStore();
    await store.setLanguage("en-US");
    expect((await store.get()).language).toBe("en-US");
    await store.setLanguage("zh-CN");
    expect((await store.get()).language).toBe("zh-CN");
  });

  it("normalizes old settings without language", async () => {
    const store = createTestSettingsStore(
      '{"version":1,"blacklistedApps":[],"overlayPlacement":{"buttonOffset":null},"floatingButton":{"visible":true},"promptInsertion":{"mode":"paste_only"}}'
    );
    const settings = await store.get();
    expect(settings.language).toBe("zh-CN");
  });

  it("normalizes invalid language to Chinese", async () => {
    const store = createTestSettingsStore(
      '{"version":1,"language":"fr-FR","blacklistedApps":[],"overlayPlacement":{"buttonOffset":null},"floatingButton":{"visible":true},"promptInsertion":{"mode":"paste_only"}}'
    );
    const settings = await store.get();
    expect(settings.language).toBe("zh-CN");
  });

  it("defaults permission prompt history to not requested", async () => {
    const store = createTestSettingsStore();
    const settings = await store.get();
    expect(settings.permissions).toEqual({ accessibilityPromptRequested: false });
  });

  it("defaults prompt library linking to copy mode", async () => {
    const store = createTestSettingsStore();
    const settings = await store.get();
    expect(settings.promptLibraryLink).toEqual({
      mode: "copy",
      path: null,
      lastKnownSignature: null,
      lastSyncedAt: null,
    });
  });

  it("normalizes linked prompt library settings", async () => {
    const store = createTestSettingsStore(JSON.stringify({
      version: 1,
      language: "zh-CN",
      blacklistedApps: [],
      overlayPlacement: { buttonOffset: null, buttonPosition: null },
      floatingButton: { visible: true },
      promptInsertion: { mode: "paste_and_submit" },
      permissions: { accessibilityPromptRequested: false },
      promptLibraryLink: {
        mode: "linked",
        path: "/Users/example/Desktop/prompts.json",
        lastKnownSignature: "100:1700000000000",
        lastSyncedAt: "2026-07-06T00:00:00.000Z",
      },
    }));

    const settings = await store.get();
    expect(settings.promptLibraryLink).toEqual({
      mode: "linked",
      path: "/Users/example/Desktop/prompts.json",
      lastKnownSignature: "100:1700000000000",
      lastSyncedAt: "2026-07-06T00:00:00.000Z",
    });
  });

  it("clears prompt library linking", async () => {
    const store = createTestSettingsStore();
    await store.setPromptLibraryLink({
      mode: "linked",
      path: "/Users/example/Desktop/prompts.json",
      lastKnownSignature: "100:1700000000000",
      lastSyncedAt: "2026-07-06T00:00:00.000Z",
    });

    await store.clearPromptLibraryLink();

    const settings = await store.get();
    expect(settings.promptLibraryLink).toEqual({
      mode: "copy",
      path: null,
      lastKnownSignature: null,
      lastSyncedAt: null,
    });
  });

  it("normalizes old settings without permissions", async () => {
    const store = createTestSettingsStore(
      JSON.stringify({
        version: 1,
        language: "zh-CN",
        blacklistedApps: [],
        overlayPlacement: { buttonOffset: null, buttonPosition: null },
        floatingButton: { visible: true },
        promptInsertion: { mode: "paste_and_submit" }
      })
    );

    const settings = await store.get();
    expect(settings.permissions).toEqual({ accessibilityPromptRequested: false });
  });

  it("saves accessibility prompt requested state", async () => {
    const store = createTestSettingsStore();

    await store.setAccessibilityPromptRequested(true);

    const settings = await store.get();
    expect(settings.permissions).toEqual({ accessibilityPromptRequested: true });
  });
});

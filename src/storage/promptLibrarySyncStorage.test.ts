import { describe, expect, it, vi } from "vitest";
import { createPromptLibrarySyncStorage } from "./promptLibrarySyncStorage";
import type { PromptLibraryLink } from "../shared/settingsStore";
import type { StorageAdapter } from "../shared/promptStore";

function memoryStorage(initial: string | null = null): StorageAdapter & {
  state(): string | null;
} {
  let value = initial;
  return {
    read: async () => value,
    write: async (next) => {
      value = next;
    },
    state: () => value,
  };
}

function copyLink(): PromptLibraryLink {
  return {
    mode: "copy",
    path: null,
    lastKnownSignature: null,
    lastSyncedAt: null,
  };
}

function linked(path = "/Users/example/Desktop/prompts.json"): PromptLibraryLink {
  return {
    mode: "linked",
    path,
    lastKnownSignature: "10:1000",
    lastSyncedAt: "2026-07-06T00:00:00.000Z",
  };
}

describe("prompt library sync storage", () => {
  it("reads AppData when prompt library link is in copy mode", async () => {
    const appDataStorage = memoryStorage("appdata");
    const readExternal = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => copyLink(),
      setLink: async () => {},
      readExternal,
      writeExternal: vi.fn(),
      getExternalMetadata: vi.fn(),
    });

    await expect(storage.read()).resolves.toBe("appdata");
    expect(readExternal).not.toHaveBeenCalled();
  });

  it("reads linked external file and refreshes AppData cache when link is valid", async () => {
    let link = linked();
    const externalContent = JSON.stringify({ version: 2, containers: [] });
    const appDataStorage = memoryStorage("appdata");
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => link,
      setLink: async (next) => { link = next; },
      readExternal: async () => ({ content: externalContent, signature: "20:2000" }),
      writeExternal: vi.fn(),
      getExternalMetadata: vi.fn(),
    });

    await expect(storage.read()).resolves.toBe(externalContent);
    expect(appDataStorage.state()).toBe(externalContent);
    expect(link.lastKnownSignature).toBe("20:2000");
    expect(link.lastSyncedAt).toEqual(expect.any(String));
  });

  it("does not overwrite AppData when linked external file is invalid JSON", async () => {
    const appDataStorage = memoryStorage(JSON.stringify({ version: 2, containers: [] }));
    const onSyncError = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: async () => ({ content: "{", signature: "20:2000" }),
      writeExternal: vi.fn(),
      getExternalMetadata: vi.fn(),
      onSyncError,
    });

    const fallback = appDataStorage.state();
    await expect(storage.read()).resolves.toBe(fallback);
    expect(appDataStorage.state()).toBe(fallback);
    expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({
      kind: "read_failed",
      path: "/Users/example/Desktop/prompts.json",
    }));
  });

  it("does not overwrite AppData when linked external file is valid JSON but not a prompt library", async () => {
    const fallback = JSON.stringify({ version: 2, containers: [] });
    const appDataStorage = memoryStorage(fallback);
    const onSyncError = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: async () => ({ content: JSON.stringify({ foo: true }), signature: "20:2000" }),
      writeExternal: vi.fn(),
      getExternalMetadata: vi.fn(),
      onSyncError,
    });

    await expect(storage.read()).resolves.toBe(fallback);
    expect(appDataStorage.state()).toBe(fallback);
    expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({
      kind: "read_failed",
      path: "/Users/example/Desktop/prompts.json",
    }));
  });

  it("falls back to AppData when linked file is missing", async () => {
    const appDataStorage = memoryStorage("appdata");
    const onSyncError = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: async () => { throw new Error("missing"); },
      writeExternal: vi.fn(),
      getExternalMetadata: vi.fn(),
      onSyncError,
    });

    await expect(storage.read()).resolves.toBe("appdata");
    expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({
      kind: "read_failed",
      path: "/Users/example/Desktop/prompts.json",
    }));
  });

  it("writes both AppData and external file in linked mode", async () => {
    let link = linked();
    const appDataStorage = memoryStorage("old");
    const writeExternal = vi.fn().mockResolvedValue({ signature: "30:3000" });
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => link,
      setLink: async (next) => { link = next; },
      readExternal: vi.fn(),
      writeExternal,
      getExternalMetadata: async () => ({ signature: "10:1000" }),
    });

    await storage.write("next");

    expect(appDataStorage.state()).toBe("next");
    expect(writeExternal).toHaveBeenCalledWith("/Users/example/Desktop/prompts.json", "next");
    expect(link.lastKnownSignature).toBe("30:3000");
  });

  it("does not overwrite external file when its signature changed since last sync", async () => {
    const appDataStorage = memoryStorage("old");
    const writeExternal = vi.fn();
    const onSyncError = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: vi.fn(),
      writeExternal,
      getExternalMetadata: async () => ({ signature: "changed" }),
      onSyncError,
    });

    await expect(storage.write("next")).rejects.toThrow("changed outside Sleepy Cat");
    expect(appDataStorage.state()).toBe("old");
    expect(writeExternal).not.toHaveBeenCalled();
    expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({ kind: "conflict" }));
  });

  it("does not write AppData when linked file has a conflict", async () => {
    const appDataStorage = memoryStorage("old");
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: vi.fn(),
      writeExternal: vi.fn(),
      getExternalMetadata: async () => ({ signature: "changed" }),
    });

    await expect(storage.write("next")).rejects.toThrow();
    expect(appDataStorage.state()).toBe("old");
  });

  it("saves AppData and reports sync failure when external write fails after preflight", async () => {
    const appDataStorage = memoryStorage("old");
    const onSyncError = vi.fn();
    const storage = createPromptLibrarySyncStorage({
      appDataStorage,
      getLink: async () => linked(),
      setLink: async () => {},
      readExternal: vi.fn(),
      writeExternal: async () => { throw new Error("denied"); },
      getExternalMetadata: async () => ({ signature: "10:1000" }),
      onSyncError,
    });

    await storage.write("next");

    expect(appDataStorage.state()).toBe("next");
    expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({
      kind: "write_failed",
      path: "/Users/example/Desktop/prompts.json",
    }));
  });
});

import { describe, expect, it } from "vitest";
import { createPromptStore } from "../shared/promptStore";
import { createTauriPromptStorage } from "./tauriPromptStorage";

describe("tauri prompt storage", () => {
  it("creates a storage adapter using tauri fs plugin", async () => {
    const storage = createTauriPromptStorage();
    expect(typeof storage.read).toBe("function");
    expect(typeof storage.write).toBe("function");
  });

  it("can be used with prompt store", async () => {
    const storage = createTauriPromptStorage();
    const store = createPromptStore(storage);
    const prompts = await store.list();
    expect(Array.isArray(prompts)).toBe(true);
  });
});
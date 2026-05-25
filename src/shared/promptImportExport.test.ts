import { describe, expect, it } from "vitest";
import { createPromptStore } from "./promptStore";

function createTestStore(initial?: string | null) {
  let state = initial ?? null;
  return createPromptStore({
    read: async () => state,
    write: async (value) => { state = value; }
  });
}

describe("prompt import export", () => {
  it("export includes version and prompts", async () => {
    const store = createTestStore();
    await store.create({ title: "A", body: "a" });

    const json = await store.exportJson();
    const data = JSON.parse(json);

    expect(data.version).toBe(1);
    expect(Array.isArray(data.prompts)).toBe(true);
  });

  it("import rejects malformed JSON", async () => {
    const store = createTestStore();

    await expect(store.importJson("not json")).rejects.toThrow();
  });

  it("import rejects prompts without title/body", async () => {
    const store = createTestStore();

    await expect(store.importJson(JSON.stringify({
      version: 1,
      prompts: [{ id: "1", title: "", body: "" }]
    }))).rejects.toThrow();
  });

  it("import preserves manual order", async () => {
    const store = createTestStore();
    await store.create({ title: "Original", body: "original" });

    const imported = JSON.stringify({
      version: 1,
      prompts: [
        { id: "imported-1", title: "First", body: "first", order: 0, createdAt: "2026-05-26T00:00:00.000Z", updatedAt: "2026-05-26T00:00:00.000Z" },
        { id: "imported-2", title: "Second", body: "second", order: 1, createdAt: "2026-05-26T00:00:00.000Z", updatedAt: "2026-05-26T00:00:00.000Z" }
      ]
    });

    await store.importJson(imported);

    const list = await store.list();
    expect(list.map(p => p.title)).toEqual(["First", "Second"]);
  });
});
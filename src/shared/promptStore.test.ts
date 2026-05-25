import { describe, expect, it } from "vitest";
import { createPromptStore } from "./promptStore";

function createTestStore() {
  let state: string | null = null;
  return createPromptStore({
    read: async () => state,
    write: async (value) => {
      state = value;
    }
  });
}

describe("prompt store", () => {
  it("creates and lists prompts in manual order", async () => {
    const store = createTestStore();
    await store.create({ title: "B", body: "second" });
    await store.create({ title: "A", body: "first" });

    expect((await store.list()).map((p) => p.title)).toEqual(["B", "A"]);
  });

  it("create assigns id and order", async () => {
    const store = createTestStore();
    const prompt = await store.create({ title: "Test", body: "body" });

    expect(prompt.id).toBeDefined();
    expect(typeof prompt.order).toBe("number");
    expect(prompt.title).toBe("Test");
    expect(prompt.body).toBe("body");
  });

  it("update changes title and body", async () => {
    const store = createTestStore();
    const created = await store.create({ title: "Original", body: "original body" });
    const updated = await store.update(created.id, { title: "Updated", body: "updated body" });

    expect(updated?.title).toBe("Updated");
    expect(updated?.body).toBe("updated body");
  });

  it("delete removes item", async () => {
    const store = createTestStore();
    const created = await store.create({ title: "ToDelete", body: "body" });
    await store.remove(created.id);

    expect(await store.list()).toHaveLength(0);
  });

  it("reorder persists new order", async () => {
    const store = createTestStore();
    const first = await store.create({ title: "First", body: "first" });
    const second = await store.create({ title: "Second", body: "second" });

    await store.reorder([second.id, first.id]);

    const list = await store.list();
    expect(list.map((p) => p.title)).toEqual(["Second", "First"]);
  });

  it("export returns portable JSON", async () => {
    const store = createTestStore();
    await store.create({ title: "A", body: "a" });

    const json = await store.exportJson();
    const data = JSON.parse(json);

    expect(data.version).toBe(1);
    expect(Array.isArray(data.prompts)).toBe(true);
  });

  it("import replaces prompts with valid imported data", async () => {
    const store = createTestStore();
    await store.create({ title: "Original", body: "original" });

    const imported = JSON.stringify({
      version: 1,
      prompts: [
        {
          id: "imported-1",
          title: "Imported",
          body: "imported body",
          order: 0,
          createdAt: "2026-05-26T00:00:00.000Z",
          updatedAt: "2026-05-26T00:00:00.000Z"
        }
      ]
    });

    await store.importJson(imported);

    const list = await store.list();
    expect(list).toHaveLength(1);
    expect(list[0].title).toBe("Imported");
  });
});
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

function createTestStoreWithState(initial: string | null) {
  let state = initial;
  return createPromptStore({
    read: async () => state,
    write: async (value) => {
      state = value;
    }
  });
}

describe("prompt store", () => {
  it("creates and lists prompt containers in manual order", async () => {
    const store = createTestStore();
    await store.create({ title: "B", body: "second" });
    await store.create({ title: "A", body: "first" });

    expect((await store.list()).map((p) => p.title)).toEqual(["B", "A"]);
  });

  it("create assigns id, order, and a single prompt entry", async () => {
    const store = createTestStore();
    const prompt = await store.create({ title: "Test", body: "body" });

    expect(prompt.id).toBeDefined();
    expect(prompt.categoryId).toBe("category-default");
    expect(typeof prompt.order).toBe("number");
    expect(prompt.title).toBe("Test");
    expect(prompt.type).toBe("single");
    expect(prompt.sendBehavior).toBe("inherit");
    expect(prompt.prompts[0].body).toBe("body");
  });

  it("persists explicit send behavior for new containers", async () => {
    const store = createTestStore();
    const prompt = await store.create({
      title: "Test",
      body: "body",
      sendBehavior: "paste_command_enter",
    });

    expect(prompt.sendBehavior).toBe("paste_command_enter");
    expect((await store.list())[0].sendBehavior).toBe("paste_command_enter");
  });

  it("creates grouped prompt containers with ordered entries", async () => {
    const store = createTestStore();

    const group = await store.createGroup({
      title: "Repair flow",
      intervalMs: 450,
      prompts: [
        { body: "First prompt" },
        { body: "Second prompt" },
      ],
    });

    expect(group.type).toBe("group");
    expect(group.intervalMs).toBe(450);
    expect(group.prompts.map((prompt) => prompt.body)).toEqual([
      "First prompt",
      "Second prompt",
    ]);
  });

  it("update changes title and body", async () => {
    const store = createTestStore();
    const created = await store.create({ title: "Original", body: "original body" });
    const updated = await store.update(created.id, { title: "Updated", body: "updated body" });

    expect(updated?.title).toBe("Updated");
    expect(updated?.prompts[0].body).toBe("updated body");
  });

  it("update can replace group prompt entries", async () => {
    const store = createTestStore();
    const created = await store.createGroup({
      title: "Original group",
      prompts: [{ body: "one" }, { body: "two" }],
    });

    const updated = await store.update(created.id, {
      title: "Updated group",
      intervalMs: 900,
      prompts: [{ body: "new one" }, { body: "new two" }, { body: "new three" }],
    });

    expect(updated?.title).toBe("Updated group");
    expect(updated?.intervalMs).toBe(900);
    expect(updated?.prompts.map((prompt) => prompt.body)).toEqual([
      "new one",
      "new two",
      "new three",
    ]);
  });

  it("delete removes item", async () => {
    const store = createTestStore();
    const created = await store.create({ title: "ToDelete", body: "body" });
    await store.remove(created.id);

    expect(await store.list()).toHaveLength(0);
  });

  it("combines non-adjacent singles in requested order and replaces their earliest position", async () => {
    const store = createTestStore();
    const first = await store.create({ title: "First", body: "first" });
    const middle = await store.create({ title: "Middle", body: "middle" });
    const last = await store.create({ title: "Last", body: "last" });

    const group = await store.combineSingles({
      ids: [last.id, first.id],
      title: "Combined",
      deleteOriginals: true,
    });

    expect((await store.list()).map((container) => container.title)).toEqual([
      "Combined",
      "Middle",
    ]);
    expect(group.prompts.map((entry) => [entry.title, entry.body])).toEqual([
      ["Last", "last"],
      ["First", "first"],
    ]);
    expect((await store.list()).some((container) => container.id === middle.id)).toBe(true);
  });

  it("keeps source singles and appends the combined group when requested", async () => {
    const store = createTestStore();
    const first = await store.create({ title: "First", body: "first" });
    const second = await store.create({ title: "Second", body: "second" });

    await store.combineSingles({
      ids: [first.id, second.id],
      title: "Combined",
      deleteOriginals: false,
    });

    expect((await store.list()).map((container) => container.title)).toEqual([
      "First",
      "Second",
      "Combined",
    ]);
  });

  it("splits a group in place and restores preserved source titles", async () => {
    const store = createTestStore();
    await store.create({ title: "Before", body: "before" });
    const group = await store.createGroup({
      title: "Workflow",
      prompts: [
        { title: "Analyze", body: "analyze" },
        { title: "Repair", body: "repair" },
      ],
    });
    await store.create({ title: "After", body: "after" });

    const singles = await store.splitGroup(group.id);

    expect(singles.map((container) => container.title)).toEqual(["Analyze", "Repair"]);
    expect((await store.list()).map((container) => container.title)).toEqual([
      "Before",
      "Analyze",
      "Repair",
      "After",
    ]);
  });

  it("generates stable titles when splitting a legacy group without entry titles", async () => {
    const store = createTestStore();
    const group = await store.createGroup({
      title: "Workflow",
      prompts: [{ body: "one" }, { body: "two" }],
    });

    await store.splitGroup(group.id);

    expect((await store.list()).map((container) => container.title)).toEqual([
      "Workflow 1",
      "Workflow 2",
    ]);
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

    expect(data.version).toBe(3);
    expect(Array.isArray(data.categories)).toBe(true);
    expect(Array.isArray(data.containers)).toBe(true);
    expect(data.activeCategoryId).toBe("category-default");
  });

  it("import replaces containers with valid imported v2 data", async () => {
    const store = createTestStore();
    await store.create({ title: "Original", body: "original" });

    const imported = JSON.stringify({
      version: 2,
      containers: [
        {
          id: "imported-1",
          title: "Imported",
          type: "single",
          prompts: [{ id: "entry-1", body: "imported body", order: 0 }],
          intervalMs: 700,
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
    expect(list[0].sendBehavior).toBe("inherit");
  });

  it("normalizes invalid imported send behavior to inherit", async () => {
    const store = createTestStore();
    await store.importJson(JSON.stringify({
      version: 3,
      categories: [
        {
          id: "category-default",
          name: "Default",
          order: 0,
          createdAt: "2026-05-26T00:00:00.000Z",
          updatedAt: "2026-05-26T00:00:00.000Z"
        }
      ],
      containers: [
        {
          id: "container-1",
          categoryId: "category-default",
          title: "Imported",
          type: "single",
          sendBehavior: "invalid",
          prompts: [{ id: "entry-1", body: "body", order: 0 }],
          intervalMs: 700,
          order: 0,
          createdAt: "2026-05-26T00:00:00.000Z",
          updatedAt: "2026-05-26T00:00:00.000Z"
        }
      ],
      activeCategoryId: "category-default",
    }));

    expect((await store.list())[0].sendBehavior).toBe("inherit");
  });

  it("loads legacy v1 prompts as single containers", async () => {
    const store = createTestStore();
    await store.importJson(JSON.stringify({
      version: 1,
      prompts: [
        {
          id: "legacy-1",
          title: "Legacy",
          body: "legacy body",
          order: 0,
          createdAt: "2026-05-26T00:00:00.000Z",
          updatedAt: "2026-05-26T00:00:00.000Z"
        }
      ]
    }));

    const list = await store.list();
    expect(list[0].type).toBe("single");
    expect(list[0].prompts[0].body).toBe("legacy body");
  });

  it("migrates v2 containers into a default category", async () => {
    const store = createTestStoreWithState(JSON.stringify({
      version: 2,
      containers: [
        {
          id: "container-1",
          title: "Code Review",
          type: "single",
          prompts: [{ id: "entry-1", body: "Review this", order: 0 }],
          intervalMs: 700,
          order: 0,
          createdAt: "2026-05-26T00:00:00.000Z",
          updatedAt: "2026-05-26T00:00:00.000Z"
        }
      ]
    }));

    const categories = await store.listCategories();
    const prompts = await store.list();

    expect(categories).toHaveLength(1);
    expect(categories[0].name).toBe("Default");
    expect(prompts[0].categoryId).toBe(categories[0].id);
  });

  it("creates, renames, and removes empty categories", async () => {
    const store = createTestStore();
    const created = await store.createCategory("Writing");

    expect((await store.listCategories()).map((category) => category.name)).toContain("Writing");

    await store.renameCategory(created.id, "Drafting");
    expect((await store.listCategories()).find((category) => category.id === created.id)?.name)
      .toBe("Drafting");

    await store.removeCategory(created.id);
    expect((await store.listCategories()).some((category) => category.id === created.id)).toBe(false);
  });

  it("does not remove the last category", async () => {
    const store = createTestStore();
    const [category] = await store.listCategories();

    await expect(store.removeCategory(category.id)).rejects.toThrow("Cannot remove last category");
  });

  it("does not remove categories that contain prompts", async () => {
    const store = createTestStore();
    const [category] = await store.listCategories();
    await store.createCategory("Other");
    await store.create({ title: "A", body: "a", categoryId: category.id });

    await expect(store.removeCategory(category.id)).rejects.toThrow("Cannot remove category with prompts");
  });

  it("reorders prompts only inside the selected category", async () => {
    const store = createTestStore();
    const dev = await store.createCategory("开发代码");
    const writing = await store.createCategory("写作");
    const devFirst = await store.create({ title: "Dev First", body: "a", categoryId: dev.id });
    const devSecond = await store.create({ title: "Dev Second", body: "b", categoryId: dev.id });
    const writingOnly = await store.create({ title: "Writing Only", body: "c", categoryId: writing.id });

    await store.reorder([devSecond.id, devFirst.id], dev.id);

    const devTitles = (await store.list())
      .filter((prompt) => prompt.categoryId === dev.id)
      .map((prompt) => prompt.title);
    const writingTitles = (await store.list())
      .filter((prompt) => prompt.categoryId === writing.id)
      .map((prompt) => prompt.title);

    expect(devTitles).toEqual(["Dev Second", "Dev First"]);
    expect(writingTitles).toEqual([writingOnly.title]);
  });
});

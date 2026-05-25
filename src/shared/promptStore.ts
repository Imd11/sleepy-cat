import type { PromptItem } from "./promptTypes";

export interface StorageAdapter {
  read(): Promise<string | null>;
  write(value: string): Promise<void>;
}

type PromptStoreData = {
  version: 1;
  prompts: PromptItem[];
};

export function createPromptStore(adapter: StorageAdapter) {
  async function load(): Promise<PromptItem[]> {
    const data = await adapter.read();
    if (!data) return [];
    try {
      const parsed: PromptStoreData = JSON.parse(data);
      return parsed.prompts ?? [];
    } catch {
      return [];
    }
  }

  async function save(prompts: PromptItem[]): Promise<void> {
    const data: PromptStoreData = { version: 1, prompts };
    await adapter.write(JSON.stringify(data, null, 2));
  }

  function generateId(): string {
    return `prompt-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
  }

  return {
    async list(): Promise<PromptItem[]> {
      return load();
    },

    async create(input: { title: string; body: string }): Promise<PromptItem> {
      const prompts = await load();
      const maxOrder = prompts.reduce((max, p) => Math.max(max, p.order), -1);
      const now = new Date().toISOString();
      const prompt: PromptItem = {
        id: generateId(),
        title: input.title,
        body: input.body,
        order: maxOrder + 1,
        createdAt: now,
        updatedAt: now
      };
      prompts.push(prompt);
      await save(prompts);
      return prompt;
    },

    async update(id: string, input: { title?: string; body?: string }): Promise<PromptItem | null> {
      const prompts = await load();
      const idx = prompts.findIndex((p) => p.id === id);
      if (idx === -1) return null;
      const prompt = prompts[idx];
      const updated: PromptItem = {
        ...prompt,
        title: input.title ?? prompt.title,
        body: input.body ?? prompt.body,
        updatedAt: new Date().toISOString()
      };
      prompts[idx] = updated;
      await save(prompts);
      return updated;
    },

    async remove(id: string): Promise<void> {
      const prompts = await load();
      const filtered = prompts.filter((p) => p.id !== id);
      await save(filtered);
    },

    async reorder(orderedIds: string[]): Promise<void> {
      const prompts = await load();
      const map = new Map(prompts.map((p) => [p.id, p]));
      const reordered: PromptItem[] = [];
      for (const id of orderedIds) {
        const p = map.get(id);
        if (p) reordered.push(p);
      }
      // Add any not in orderedIds at the end
      for (const p of prompts) {
        if (!orderedIds.includes(p.id)) reordered.push(p);
      }
      // Re-assign order values
      reordered.forEach((p, i) => {
        p.order = i;
        p.updatedAt = new Date().toISOString();
      });
      await save(reordered);
    },

    async exportJson(): Promise<string> {
      const prompts = await load();
      const data: PromptStoreData = { version: 1, prompts };
      return JSON.stringify(data, null, 2);
    },

    async importJson(json: string): Promise<void> {
      const parsed = JSON.parse(json) as PromptStoreData;
      if (parsed.version !== 1 || !Array.isArray(parsed.prompts)) {
        throw new Error("Invalid format");
      }
      // Validate each prompt has required fields
      for (const p of parsed.prompts) {
        if (!p.title || !p.body) throw new Error("Invalid prompt data");
      }
      await save(parsed.prompts);
    }
  };
}
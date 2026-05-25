import { readTextFile, writeTextFile, BaseDirectory } from "@tauri-apps/plugin-fs";

export function createTauriPromptStorage() {
  const FILE_NAME = "prompts.json";

  return {
    async read(): Promise<string | null> {
      try {
        const content = await readTextFile(FILE_NAME, { baseDir: BaseDirectory.AppData });
        return content;
      } catch {
        return null;
      }
    },

    async write(value: string): Promise<void> {
      await writeTextFile(FILE_NAME, value, { baseDir: BaseDirectory.AppData });
    }
  };
}
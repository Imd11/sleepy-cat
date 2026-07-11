import { validatePromptLibraryJson, type StorageAdapter } from "../shared/promptStore";
import type { PromptLibraryLink } from "../shared/settingsStore";

export type PromptLibrarySyncErrorKind = "read_failed" | "write_failed" | "conflict";

export type PromptLibrarySyncError = {
  kind: PromptLibrarySyncErrorKind;
  path: string;
  error: unknown;
};

type PromptLibraryFileData = {
  content: string;
  signature: string;
};

type PromptLibraryFileMetadata = {
  signature: string;
};

type PromptLibrarySyncStorageDeps = {
  appDataStorage: StorageAdapter;
  getLink: () => Promise<PromptLibraryLink>;
  setLink: (link: PromptLibraryLink) => Promise<void>;
  readExternal: (path: string) => Promise<PromptLibraryFileData>;
  writeExternal: (path: string, content: string) => Promise<PromptLibraryFileMetadata>;
  getExternalMetadata: (path: string) => Promise<PromptLibraryFileMetadata>;
  onSyncError?: (error: PromptLibrarySyncError) => void;
  now?: () => Date;
};

function linkedPath(link: PromptLibraryLink): string | null {
  return link.mode === "linked" && link.path ? link.path : null;
}

function syncedLink(
  link: PromptLibraryLink,
  signature: string,
  now: () => Date
): PromptLibraryLink {
  return {
    ...link,
    lastKnownSignature: signature,
    lastSyncedAt: now().toISOString(),
  };
}

export function createPromptLibrarySyncStorage({
  appDataStorage,
  getLink,
  setLink,
  readExternal,
  writeExternal,
  getExternalMetadata,
  onSyncError,
  now = () => new Date(),
}: PromptLibrarySyncStorageDeps): StorageAdapter {
  let hasUnsyncedLocalChanges = false;

  async function readLinkedExternal(
    link: PromptLibraryLink,
    path: string
  ): Promise<string> {
    const external = await readExternal(path);
    validatePromptLibraryJson(external.content);
    await appDataStorage.write(external.content);
    await setLink(syncedLink(link, external.signature, now));
    hasUnsyncedLocalChanges = false;
    return external.content;
  }

  async function readLinkedWithFallback(
    link: PromptLibraryLink,
    path: string
  ): Promise<string | null> {
    try {
      return await readLinkedExternal(link, path);
    } catch (error) {
      onSyncError?.({ kind: "read_failed", path, error });
      return appDataStorage.read();
    }
  }

  return {
    async read(): Promise<string | null> {
      const link = await getLink();
      const path = linkedPath(link);
      if (!path) return appDataStorage.read();

      if (hasUnsyncedLocalChanges) {
        return appDataStorage.read();
      }

      return readLinkedWithFallback(link, path);
    },

    async write(value: string): Promise<void> {
      const link = await getLink();
      const path = linkedPath(link);
      if (!path) {
        await appDataStorage.write(value);
        return;
      }

      let metadata: PromptLibraryFileMetadata;
      try {
        metadata = await getExternalMetadata(path);
      } catch (error) {
        await appDataStorage.write(value);
        hasUnsyncedLocalChanges = true;
        onSyncError?.({ kind: "write_failed", path, error });
        return;
      }

      if (link.lastKnownSignature && metadata.signature !== link.lastKnownSignature) {
        const error = new Error("Linked prompt file changed outside Prompt Drawer.");
        onSyncError?.({ kind: "conflict", path, error });
        throw error;
      }

      await appDataStorage.write(value);
      try {
        const written = await writeExternal(path, value);
        hasUnsyncedLocalChanges = false;
        await setLink(syncedLink(link, written.signature, now));
      } catch (error) {
        hasUnsyncedLocalChanges = true;
        onSyncError?.({ kind: "write_failed", path, error });
      }
    },
  };
}

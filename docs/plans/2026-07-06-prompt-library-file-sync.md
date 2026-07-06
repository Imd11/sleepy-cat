# Prompt Library File Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an explicit "Link and sync file" mode so a user-selected external prompt JSON can stay synchronized with the prompt library shown and edited inside Prompt Picker.

**Architecture:** Keep the existing one-time Import behavior as the default and add a separate linked-file mode. The app continues to keep an AppData cache at `BaseDirectory.AppData/prompts.json`, while linked mode records the external file path in settings, reads from that file on startup/refresh, writes App edits back to it, and falls back to AppData with clear status when the external file is missing or invalid.

**Tech Stack:** Tauri 2, Rust commands for controlled external JSON file reads/writes, React 19, TypeScript, Vitest, existing prompt/settings stores.

---

## Current Behavior To Preserve

- The current prompt store reads and writes only `prompts.json` under `BaseDirectory.AppData`; see `src/storage/tauriPromptStorage.ts:3-19`.
- Import is currently a one-time copy: the user chooses a JSON file, App reads that file once, then `storeRef.current.importJson(content)` overwrites AppData; see `src/App.tsx:576-588`.
- Prompt mutations call `createPromptStore(...).create/update/remove/reorder/importJson`, and all saves go through `StorageAdapter.write`; see `src/shared/promptStore.ts:352-608`.
- Existing users must not be migrated into linked mode automatically.

## Target User Experience

The manager page keeps the current Import/Export workflow and adds a clear sync choice:

```text
Import
┌────────────────────────────────────────────┐
│ Import prompt library                       │
│                                            │
│ ○ Import as copy                            │
│   Safe. The app uses its own copy.          │
│                                            │
│ ○ Link and sync file                        │
│   Keep this JSON file and Prompt Picker     │
│   synchronized. App edits update the file.  │
│                                            │
│ [Cancel]                       [Continue]  │
└────────────────────────────────────────────┘
```

Linked mode displays a small status near the manager import/export controls:

```text
Prompt Library: Linked to prompts.json       [Sync now] [Unlink]
```

If the linked file is missing or invalid:

```text
Prompt Library: Link needs attention         [Choose file] [Unlink]
```

The popover should not show sync controls. It should simply reflect the current prompt library after reload.

## Safety Rules

- Default behavior remains one-time copy import.
- Do not auto-scan Desktop, Downloads, iCloud Drive, or arbitrary folders.
- Do not silently overwrite a linked external file if it changed outside the app since the last known sync.
- Always keep AppData as a fallback cache.
- If writing the external file fails, AppData save may remain successful, but the UI must show sync failure instead of pretending everything is synchronized.
- If the linked file contains invalid prompt JSON, do not destroy the last valid AppData prompt library.
- When importing as copy from an already-linked state, clear the link before writing imported data so the old linked file is not overwritten.
- When linking a new file from an already-linked state, import through an AppData-only store first, then link the new file after the normalized JSON is written to that new file.
- For linked writes, check the external file signature before writing AppData. If the external file changed, block the save and keep the user in the editor.
- Prompt edits must close their editor only after the async create/update/delete/reorder operation succeeds. On sync failure, keep the user's draft visible.
- First implementation must not use background polling. Support startup sync and a manual `Sync now` action first; automatic polling can be a later follow-up after dirty-edit tracking is explicit.
- External file commands must reject non-JSON paths, directories, oversized files, and must use atomic write semantics.

---

### Task 1: Add Settings Schema For Linked Prompt Library

**Files:**
- Modify: `src/shared/settingsStore.ts:14-31`
- Modify: `src/shared/settingsStore.test.ts`

**Step 1: Write failing tests**

Add tests that cover:

```ts
it("defaults prompt library linking to copy mode", async () => {
  const store = createSettingsStore(memoryAdapter(null));
  await expect(store.get()).resolves.toMatchObject({
    promptLibraryLink: {
      mode: "copy",
      path: null,
      lastKnownSignature: null,
      lastSyncedAt: null,
    },
  });
});

it("normalizes linked prompt library settings", async () => {
  const store = createSettingsStore(memoryAdapter(JSON.stringify({
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
  })));
  await expect(store.get()).resolves.toMatchObject({
    promptLibraryLink: {
      mode: "linked",
      path: "/Users/example/Desktop/prompts.json",
    },
  });
});
```

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/shared/settingsStore.test.ts
```

Expected: FAIL because `promptLibraryLink` does not exist.

**Step 3: Implement minimal settings support**

Add types:

```ts
export type PromptLibraryLink = {
  mode: "copy" | "linked";
  path: string | null;
  lastKnownSignature: string | null;
  lastSyncedAt: string | null;
};
```

Extend `Settings`:

```ts
promptLibraryLink: PromptLibraryLink;
```

Default:

```ts
promptLibraryLink: {
  mode: "copy",
  path: null,
  lastKnownSignature: null,
  lastSyncedAt: null,
}
```

Normalize only valid linked settings:

```ts
const rawLink = candidate.promptLibraryLink;
const path = typeof rawLink?.path === "string" && rawLink.path.trim()
  ? rawLink.path
  : null;
const promptLibraryLink = rawLink?.mode === "linked" && path
  ? {
      mode: "linked" as const,
      path,
      lastKnownSignature: typeof rawLink.lastKnownSignature === "string"
        ? rawLink.lastKnownSignature
        : null,
      lastSyncedAt: typeof rawLink.lastSyncedAt === "string"
        ? rawLink.lastSyncedAt
        : null,
    }
  : defaultSettings().promptLibraryLink;
```

Add store methods:

```ts
async setPromptLibraryLink(link: PromptLibraryLink): Promise<void>;
async clearPromptLibraryLink(): Promise<void>;
```

**Step 4: Run tests**

Run:

```bash
npm test -- src/shared/settingsStore.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/shared/settingsStore.ts src/shared/settingsStore.test.ts
git commit -m "feat: store prompt library link settings"
```

---

### Task 2: Add Native External Prompt File Commands

**Files:**
- Create: `src-tauri/src/prompt_files.rs`
- Modify: `src-tauri/src/lib.rs:9-23`
- Modify: `src-tauri/src/lib.rs:1067-1094`
- Modify: `src/platform/platformApi.ts:1-123`

**Step 1: Write failing Rust tests**

Create focused tests in `src-tauri/src/prompt_files.rs` for path validation and file signature calculation:

```rust
#[test]
fn rejects_non_json_prompt_library_path() {
    let error = validate_prompt_library_path("/tmp/prompts.txt").unwrap_err();
    assert!(error.contains("JSON"));
}

#[test]
fn allows_json_prompt_library_path() {
    assert!(validate_prompt_library_path("/tmp/prompts.json").is_ok());
}
```

**Step 2: Run Rust tests to verify failure**

Run:

```bash
cd src-tauri && cargo test prompt_files
```

Expected: FAIL until the module exists and is wired.

**Step 3: Implement commands**

Create:

```rust
use std::{fs, path::Path, time::UNIX_EPOCH};

#[derive(Clone, Debug, serde::Serialize)]
pub struct PromptLibraryFile {
    content: String,
    signature: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PromptLibraryFileMetadata {
    signature: String,
}

pub fn validate_prompt_library_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("json") => Ok(()),
        _ => Err("Please choose a JSON prompt library file.".to_string()),
    }
}

fn file_signature(path: &str) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Ok(format!("{}:{}", metadata.len(), modified_ms))
}

const MAX_PROMPT_LIBRARY_BYTES: u64 = 10 * 1024 * 1024;

fn validate_prompt_library_file(path: &str) -> Result<(), String> {
    validate_prompt_library_path(&path)?;
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("Please choose a JSON prompt library file.".to_string());
    }
    if metadata.len() > MAX_PROMPT_LIBRARY_BYTES {
        return Err("Prompt library file is too large.".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn read_prompt_library_file(path: String) -> Result<PromptLibraryFile, String> {
    validate_prompt_library_file(&path)?;
    let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    Ok(PromptLibraryFile { content, signature: file_signature(&path)? })
}

#[tauri::command]
pub fn write_prompt_library_file(
    path: String,
    content: String,
) -> Result<PromptLibraryFileMetadata, String> {
    validate_prompt_library_path(&path)?;
    if content.as_bytes().len() as u64 > MAX_PROMPT_LIBRARY_BYTES {
        return Err("Prompt library file is too large.".to_string());
    }
    let tmp_path = format!("{}.tmp", path);
    fs::write(&tmp_path, content).map_err(|error| error.to_string())?;
    fs::rename(&tmp_path, &path).map_err(|error| error.to_string())?;
    Ok(PromptLibraryFileMetadata { signature: file_signature(&path)? })
}

#[tauri::command]
pub fn prompt_library_file_metadata(path: String) -> Result<PromptLibraryFileMetadata, String> {
    validate_prompt_library_file(&path)?;
    Ok(PromptLibraryFileMetadata { signature: file_signature(&path)? })
}
```

Wire the module in `src-tauri/src/lib.rs`:

```rust
mod prompt_files;
pub use prompt_files::{
    prompt_library_file_metadata, read_prompt_library_file, write_prompt_library_file,
};
```

Add the commands to `tauri::generate_handler!`.

Add platform wrappers:

```ts
export interface PromptLibraryFile {
  content: string;
  signature: string;
}

export interface PromptLibraryFileMetadata {
  signature: string;
}

export async function readPromptLibraryFile(path: string): Promise<PromptLibraryFile> {
  return invoke<PromptLibraryFile>("read_prompt_library_file", { path });
}

export async function writePromptLibraryFile(
  path: string,
  content: string
): Promise<PromptLibraryFileMetadata> {
  return invoke<PromptLibraryFileMetadata>("write_prompt_library_file", { path, content });
}

export async function getPromptLibraryFileMetadata(
  path: string
): Promise<PromptLibraryFileMetadata> {
  return invoke<PromptLibraryFileMetadata>("prompt_library_file_metadata", { path });
}
```

**Step 4: Run tests**

Run:

```bash
cd src-tauri && cargo test prompt_files
npm test -- src/app/tauriCapabilities.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/prompt_files.rs src-tauri/src/lib.rs src/platform/platformApi.ts
git commit -m "feat: add prompt library file commands"
```

---

### Task 3: Add Linked Storage Adapter

**Files:**
- Create: `src/storage/promptLibrarySyncStorage.ts`
- Create: `src/storage/promptLibrarySyncStorage.test.ts`
- Modify: `src/storage/tauriPromptStorage.ts:1-19`

**Step 1: Write failing tests**

Cover these cases:

```ts
it("reads AppData when prompt library link is in copy mode", async () => {});
it("reads linked external file and refreshes AppData cache when link is valid", async () => {});
it("falls back to AppData when linked file is missing", async () => {});
it("writes both AppData and external file in linked mode", async () => {});
it("does not overwrite external file when its signature changed since last sync", async () => {});
it("does not write AppData when linked file has a conflict", async () => {});
it("saves AppData and reports sync failure when external write fails after preflight", async () => {});
```

Use in-memory adapters and mocked file commands.

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/storage/promptLibrarySyncStorage.test.ts
```

Expected: FAIL because the storage adapter does not exist.

**Step 3: Implement storage adapter**

The adapter must implement the existing `StorageAdapter` interface:

```ts
type LinkedPromptStorageDeps = {
  appDataStorage: StorageAdapter;
  getLink: () => Promise<PromptLibraryLink>;
  setLink: (link: PromptLibraryLink) => Promise<void>;
  readExternal: (path: string) => Promise<{ content: string; signature: string }>;
  writeExternal: (path: string, content: string) => Promise<{ signature: string }>;
  getExternalMetadata: (path: string) => Promise<{ signature: string }>;
  onSyncError?: (error: PromptLibrarySyncError) => void;
};
```

Read behavior:

```ts
async read() {
  const link = await getLink();
  if (link.mode !== "linked" || !link.path) return appDataStorage.read();

  try {
    const external = await readExternal(link.path);
    await appDataStorage.write(external.content);
    await setLink({ ...link, lastKnownSignature: external.signature, lastSyncedAt: new Date().toISOString() });
    return external.content;
  } catch (error) {
    onSyncError?.({ kind: "read_failed", path: link.path, error });
    return appDataStorage.read();
  }
}
```

Write behavior:

```ts
async write(value: string) {
  const link = await getLink();
  if (link.mode !== "linked" || !link.path) {
    await appDataStorage.write(value);
    return;
  }

  const metadata = await getExternalMetadata(link.path);
  if (link.lastKnownSignature && metadata.signature !== link.lastKnownSignature) {
    onSyncError?.({ kind: "conflict", path: link.path, error: null });
    throw new Error("Linked prompt file changed outside Prompt Picker.");
  }

  await appDataStorage.write(value);
  try {
    const written = await writeExternal(link.path, value);
    await setLink({ ...link, lastKnownSignature: written.signature, lastSyncedAt: new Date().toISOString() });
  } catch (error) {
    onSyncError?.({ kind: "write_failed", path: link.path, error });
  }
}
```

Keep `createTauriPromptStorage()` as the AppData-only primitive. Compose the sync adapter outside it.

**Step 4: Run tests**

Run:

```bash
npm test -- src/storage/promptLibrarySyncStorage.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/storage/promptLibrarySyncStorage.ts src/storage/promptLibrarySyncStorage.test.ts src/storage/tauriPromptStorage.ts
git commit -m "feat: sync prompts with linked library file"
```

---

### Task 4: Wire Linked Storage Into App Startup

**Files:**
- Modify: `src/App.tsx:200-230`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing tests**

Add App-level tests:

```ts
it("loads prompts from linked file when prompt library link is enabled", async () => {});
it("falls back to AppData prompts when linked file read fails", async () => {});
it("shows a sync error when linked file write fails after editing a prompt", async () => {});
it("keeps the edit form open when a linked-file conflict blocks update", async () => {});
```

Mock:

- `settings.json` with `promptLibraryLink.mode === "linked"`.
- platform API `readPromptLibraryFile`, `writePromptLibraryFile`, `getPromptLibraryFileMetadata`.

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: FAIL because App still constructs `createPromptStore(createTauriPromptStorage())`.

**Step 3: Compose stores in App**

Replace:

```ts
const storeRef = useRef(createPromptStore(createTauriPromptStorage()));
const settingsStoreRef = useRef(createSettingsStore(createTauriSettingsStorage()));
```

With:

```ts
const appDataPromptStoreRef = useRef(createPromptStore(createTauriPromptStorage()));
const settingsStoreRef = useRef(createSettingsStore(createTauriSettingsStorage()));
const promptStorageRef = useRef(createPromptLibrarySyncStorage({
  appDataStorage: createTauriPromptStorage(),
  getLink: async () => (await settingsStoreRef.current.get()).promptLibraryLink,
  setLink: async (promptLibraryLink) => {
    const settings = await settingsStoreRef.current.get();
    await settingsStoreRef.current.setPromptLibraryLink(promptLibraryLink);
    setActiveSettings({ ...settings, promptLibraryLink });
  },
  readExternal: readPromptLibraryFile,
  writeExternal: writePromptLibraryFile,
  getExternalMetadata: getPromptLibraryFileMetadata,
  onSyncError: setPromptLibrarySyncError,
}));
const storeRef = useRef(createPromptStore(promptStorageRef.current));
```

Use a dedicated `promptLibrarySyncError` state so UI can display linked-file issues.

Also update `PromptManager` callback types and submit handlers so edits close only after the async operation succeeds:

```ts
onUpdate: (...) => Promise<void>;
onCreate: (...) => Promise<void>;
onCreateGroup: (...) => Promise<void>;
onDelete: (...) => Promise<void>;
onReorder: (...) => Promise<void>;
```

The edit save flow must be:

```ts
try {
  await onUpdate(id, input);
  setEditingId(null);
  setEditDraft(emptyDraft());
} catch (error) {
  setSyncErrorVisible(true);
}
```

**Step 4: Run tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "feat: load prompts from linked library file"
```

---

### Task 5: Add Import UX Choice

**Files:**
- Modify: `src/App.tsx:576-594`
- Modify: `src/ui/PromptManager.tsx`
- Modify: `src/shared/i18n.ts`
- Modify: `src/styles.css`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing tests**

Cover:

```ts
it("keeps import as copy by default", async () => {});
it("links and syncs an imported file when user chooses link mode", async () => {});
it("shows linked file status and unlink action in manager", async () => {});
```

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: FAIL because there is no import choice or linked status UI.

**Step 3: Implement UI**

Add a small modal or inline confirmation after the user clicks Import and picks a JSON file. The minimal acceptable UX:

```text
Import prompt library
[ ] Link and sync this file after importing
[Cancel] [Import]
```

Behavior:

- If unchecked: clear any existing link first, then import through the AppData-only store. Do not route copy imports through linked storage.
- If checked:
  1. Clear any existing link first so the old linked file cannot be overwritten.
  2. Validate/import the selected content into AppData via the AppData-only prompt store.
  3. Write the normalized current library back to the newly selected file using `appDataPromptStoreRef.current.exportJson()` and `writePromptLibraryFile(...)`.
  4. Save `promptLibraryLink.mode = "linked"` with the selected path and returned signature.
  5. Reload prompt data and show linked status.

Add manager status text:

```ts
const promptLibraryStatus = activeSettings.promptLibraryLink.mode === "linked"
  ? t.manager.promptLibraryLinked(fileName)
  : t.manager.promptLibraryLocal;
```

Add i18n keys in Chinese and English:

- `promptLibraryLocal`
- `promptLibraryLinked(fileName)`
- `promptLibrarySyncFailed`
- `linkAndSyncThisFile`
- `unlinkPromptLibrary`
- `syncPromptLibraryNow`

**Step 4: Run tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/App.tsx src/ui/PromptManager.tsx src/shared/i18n.ts src/styles.css src/app/App.test.tsx
git commit -m "feat: choose linked prompt library imports"
```

---

### Task 6: Add Manual Sync Now And Unlink

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/ui/PromptManager.tsx`
- Modify: `src/shared/i18n.ts`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing tests**

Cover:

```ts
it("sync now reloads valid linked file changes into the manager", async () => {});
it("sync now rejects invalid linked JSON and keeps current prompts", async () => {});
it("unlink switches back to AppData without deleting either file", async () => {});
```

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: FAIL until actions exist.

**Step 3: Implement actions**

Add callbacks:

```ts
async function handleSyncLinkedPromptLibraryNow() {
  await reloadPromptData();
}

async function handleUnlinkPromptLibrary() {
  await settingsStoreRef.current.clearPromptLibraryLink();
  setActiveSettings(await settingsStoreRef.current.get());
  setPromptLibrarySyncError(null);
}
```

Important behavior:

- `Sync now` should use the linked storage read path, which validates and caches external data.
- If external JSON is invalid, AppData remains the active fallback and UI shows the error.
- `Unlink` must not delete the external JSON.

**Step 4: Run tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/App.tsx src/ui/PromptManager.tsx src/shared/i18n.ts src/app/App.test.tsx
git commit -m "feat: manage linked prompt library sync"
```

---

### Task 7: Guard Against Draft Loss And Defer Automatic Polling

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/ui/PromptManager.tsx`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing tests**

Cover:

```ts
it("does not automatically poll linked files in the first implementation", async () => {});
it("does not reload linked data while a create form is open", async () => {});
it("does not reload linked data while an edit form is open", async () => {});
it("does not reload linked data while delete confirmation is open", async () => {});
```

**Step 2: Run tests to verify failure**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: FAIL until dirty-state reporting and polling guard behavior exist.

**Step 3: Implement conservative first-version behavior**

Do not add interval polling in this task. Instead, make the first version explicit:

- Sync from external file on app startup through linked storage read.
- Sync from external file when the user clicks `Sync now`.
- Do not background-refresh while the user is editing.
- Add `onDraftActivityChange(active: boolean)` or an equivalent prop from `PromptManager` to `App`.
- Disable `Sync now` or ask for confirmation when a draft/create/edit/delete-confirm state is active.

The manager should report active drafts:

```ts
const hasDraftActivity =
  createPanelOpen || editingId !== null || deleteConfirmId !== null;

useEffect(() => {
  onDraftActivityChange?.(hasDraftActivity);
}, [hasDraftActivity, onDraftActivityChange]);
```

Automatic polling is a non-goal for this first version. Add it in a later plan only after dirty-state coverage is mature.

**Step 4: Run tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/App.tsx src/ui/PromptManager.tsx src/app/App.test.tsx
git commit -m "feat: guard linked prompt sync during edits"
```

---

### Task 8: Document Prompt Library Storage Modes

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `README.es.md`
- Modify: `README.hi.md`
- Modify: `README.ar.md`
- Optionally modify: `docs/install-macos.md`

**Step 1: Update docs**

Document:

- Default mode is local AppData copy.
- Import as copy does not sync with the source JSON.
- Link and sync file keeps one chosen JSON synchronized.
- AppData remains fallback cache.
- Linked file errors can be resolved by choosing another file, fixing JSON, or unlinking.

Add concise English text:

```md
Prompt Picker supports two prompt library modes:

- Import as copy: imports a JSON file once and stores the active library in the app data folder.
- Link and sync file: keeps one selected JSON file synchronized with the active library. App edits write back to that file, and external file changes can be synced back into the app.
```

Add equivalent localized text in the other README files.

**Step 2: Run Markdown checks**

Run:

```bash
git diff --check -- README.md README.zh-CN.md README.es.md README.hi.md README.ar.md docs/install-macos.md
```

Expected: PASS.

**Step 3: Commit**

```bash
git add README.md README.zh-CN.md README.es.md README.hi.md README.ar.md docs/install-macos.md
git commit -m "docs: explain prompt library sync modes"
```

---

### Task 9: Full Verification

**Files:**
- No code changes expected.

**Step 1: Run targeted tests**

Run:

```bash
npm test -- \
  src/shared/settingsStore.test.ts \
  src/storage/promptLibrarySyncStorage.test.ts \
  src/shared/promptStore.test.ts \
  src/shared/promptImportExport.test.ts \
  src/app/App.test.tsx
```

Expected: PASS.

**Step 2: Run full frontend test suite**

Run:

```bash
npm test
```

Expected: PASS.

**Step 3: Run Rust tests**

Run:

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 4: Run build**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 5: Manual acceptance checklist**

Check manually:

- Import as copy still replaces AppData and does not update the source Desktop JSON.
- Link and sync imports a selected Desktop JSON and shows linked status.
- Editing a prompt in App updates the linked Desktop JSON.
- Editing the linked Desktop JSON and clicking Sync now updates the App UI.
- Editing the linked Desktop JSON while an App edit form is open does not overwrite the in-progress draft.
- Invalid external JSON does not erase AppData prompts.
- Deleting/moving linked file shows attention state and lets user unlink.
- App restart keeps the linked file path and loads it.
- Import as copy from an already-linked state does not modify the old linked file.

**Step 6: Final commit if verification docs or snapshots changed**

Only if verification creates intentional file changes:

```bash
git add <intentional-files>
git commit -m "test: verify prompt library sync"
```

---

## Non-Goals

- Do not auto-scan the user's Desktop or any folder for prompt JSON files.
- Do not make every import linked by default.
- Do not add cloud sync.
- Do not merge multiple prompt JSON files automatically.
- Do not delete external JSON files from the user's machine.
- Do not change prompt JSON schema unless required by existing validation.

## Implementation Notes

- Keep all sync failures explicit and recoverable.
- Prefer preserving user data over aggressive automatic repair.
- If conflict detection becomes too complex, ship the safer version first: block App write-back when external signature changed and ask user to Sync now or Unlink.
- Do not stage unrelated build artifacts currently present in the worktree.

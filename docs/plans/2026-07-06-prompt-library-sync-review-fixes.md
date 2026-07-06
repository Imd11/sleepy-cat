# Prompt Library Sync Review Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the review findings for linked prompt library sync so invalid external prompt files cannot overwrite the app's last valid prompt library, and add regression coverage for the planned sync behavior.

**Architecture:** Reuse the existing prompt import validation path as the canonical prompt-library schema gate before a linked external file is accepted into AppData. Keep the sync storage behavior narrow: valid linked files refresh AppData, invalid or unreadable linked files report a sync error and fall back to AppData without data loss. Add focused app and storage tests around manual sync, write failure, unlinking, and disabled sync during draft/edit state.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Tauri command mocks.

---

### Task 1: Add Prompt Library Schema Validation to Linked Reads

**Files:**
- Modify: `src/shared/promptStore.ts`
- Modify: `src/storage/promptLibrarySyncStorage.ts`
- Test: `src/storage/promptLibrarySyncStorage.test.ts`

**Step 1: Write the failing storage test**

Add a test next to the invalid JSON fallback case:

```ts
it("does not overwrite AppData when linked external file is valid JSON but not a prompt library", async () => {
  const fallback = JSON.stringify({ version: 2, containers: [make a valid container or []] });
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
  expect(onSyncError).toHaveBeenCalledWith(expect.objectContaining({ kind: "read_failed" }));
});
```

**Step 2: Run the targeted storage test and confirm it fails**

Run: `npm test -- src/storage/promptLibrarySyncStorage.test.ts`

Expected before implementation: the new test fails because `{ "foo": true }` is currently accepted as JSON and written to AppData.

**Step 3: Export a prompt-library validation helper**

In `src/shared/promptStore.ts`, expose a small helper that reuses the existing import validation:

```ts
export function validatePromptLibraryJson(json: string): void {
  validateImportedData(json);
}
```

Do not change `parseStore`; it remains the tolerant AppData reader for older/empty/corrupt local state.

**Step 4: Use schema validation before accepting linked file content**

In `src/storage/promptLibrarySyncStorage.ts`, replace the local `assertJsonContent()` helper with `validatePromptLibraryJson()` from `src/shared/promptStore.ts`:

```ts
import { validatePromptLibraryJson, type StorageAdapter } from "../shared/promptStore";
```

Then call `validatePromptLibraryJson(external.content)` before writing to AppData and updating the linked signature.

**Step 5: Run targeted storage tests**

Run: `npm test -- src/storage/promptLibrarySyncStorage.test.ts`

Expected: all storage sync tests pass.

---

### Task 2: Cover App-Level Read Fallback and Manual Sync Safety

**Files:**
- Modify: `src/storage/promptLibrarySyncStorage.ts`
- Modify: `src/App.tsx`
- Modify: `src/app/App.test.tsx`

**Step 1: Add linked read failure fallback test**

Add an app test that starts with linked settings and a valid AppData prompt, mocks `read_prompt_library_file` to throw, renders the manager, and verifies:

- the AppData prompt remains visible
- the sync error text is visible
- the linked file was not allowed to blank the prompt list

**Step 2: Add manual sync valid external change test**

Add an app test that starts with a linked prompt file containing `Linked Initial`, then changes the mocked external content to `Linked Updated`, clicks `立即同步`, and verifies `Linked Updated` appears.

**Step 3: Add manual sync invalid schema fallback test**

Add an app test that starts with a linked prompt file containing a valid prompt, then changes the mocked external content to `JSON.stringify({ foo: true })`, clicks `立即同步`, and verifies:

- the previous valid prompt remains visible
- the sync error text is visible
- no empty prompt library replaces the current UI

**Step 4: Run targeted app tests**

Run: `npm test -- src/app/App.test.tsx`

Expected: new tests fail before Task 1 implementation and pass after Task 1 is complete.

---

### Task 3: Cover Linked Write Failure, Unlink, and Draft Guard

**Files:**
- Modify: `src/app/App.test.tsx`

**Step 1: Add linked write failure status test**

Add an app test that starts linked, mocks a successful metadata preflight, mocks `write_prompt_library_file` to throw, edits a prompt, and saves. Verify:

- the edited prompt remains in AppData/UI
- the sync error text is shown
- this path reports a recoverable external write failure rather than losing the local edit

**Step 2: Preserve AppData after a linked write failure**

In `src/storage/promptLibrarySyncStorage.ts`, track whether a linked write saved AppData but failed to write the external file. While that flag is active, ordinary `read()` calls should return AppData instead of pulling the stale linked file again.

Expose a narrow `syncNow()` method on the sync storage object and call it from `src/App.tsx` only when the user clicks `立即同步`. This keeps automatic UI refreshes from losing local edits while preserving the existing explicit “pull external changes now” behavior.

**Step 3: Add unlink preservation test**

Add an app test that starts linked, renders a linked prompt, clicks `取消同步`, and verifies:

- the prompt remains visible
- settings now store `promptLibraryLink.mode === "copy"`
- `立即同步` and `取消同步` controls are no longer shown

**Step 4: Add draft/edit guard test**

Add an app test that starts linked, clicks `编辑`, and verifies the `立即同步` button is disabled while the edit form is active.

**Step 5: Run targeted app tests**

Run: `npm test -- src/app/App.test.tsx`

Expected: all app tests pass.

---

### Task 4: Clarify Sync Error Status Copy Without Adding New Flow

**Files:**
- Modify: `src/shared/i18n.ts`
- Modify: `src/ui/PromptManager.tsx`
- Test: `src/app/App.test.tsx`

**Step 1: Add localized attention text**

Add manager messages:

```ts
promptLibraryNeedsAttention: (fileName: string) => `提示词库：${fileName} 需要处理`,
promptLibraryNeedsAttention: (fileName: string) => `Prompt library: ${fileName} needs attention`,
```

**Step 2: Show attention text only while a linked sync error exists**

In `PromptManager`, when `linkedPath` exists and `promptLibrarySyncError` exists, show the new attention text in the status `<strong>`. Keep the existing error detail line and the existing `立即同步` / `取消同步` recovery controls.

Do not add a new picker flow or extra buttons in this repair; that would be a separate UX expansion.

**Step 3: Verify with an existing or new App test**

Make one sync-error App test assert the attention text appears. This keeps coverage close to the real screen wiring.

**Step 4: Run focused UI/app tests**

Run: `npm test -- src/app/App.test.tsx`

Expected: tests pass and the normal linked status remains unchanged when there is no error.

---

### Task 5: Full Verification, Commit, and Push

**Files:**
- Inspect: `git diff -- src/shared/promptStore.ts src/storage/promptLibrarySyncStorage.ts src/storage/promptLibrarySyncStorage.test.ts src/app/App.test.tsx src/shared/i18n.ts src/ui/PromptManager.tsx docs/plans/2026-07-06-prompt-library-sync-review-fixes.md`

**Step 1: Run focused verification**

Run:

```bash
npm test -- src/storage/promptLibrarySyncStorage.test.ts src/app/App.test.tsx
```

Expected: all focused tests pass.

**Step 2: Run full frontend verification**

Run:

```bash
npm test
npm run build
```

Expected: both commands exit 0.

**Step 3: Run backend verification**

Run:

```bash
cd src-tauri && cargo test
```

Expected: all Rust tests pass.

**Step 4: Review changed files**

Run:

```bash
git diff --stat
git status --short
```

Expected: task-related source/test/plan files are modified; pre-existing generated/build dirt remains unstaged.

**Step 5: Commit only task-related files**

Run:

```bash
git add src/shared/promptStore.ts src/storage/promptLibrarySyncStorage.ts src/storage/promptLibrarySyncStorage.test.ts src/app/App.test.tsx src/shared/i18n.ts src/ui/PromptManager.tsx docs/plans/2026-07-06-prompt-library-sync-review-fixes.md
git commit -m "fix: validate linked prompt library sync"
```

Expected: commit succeeds and unrelated dirty files are not staged.

**Step 6: Push main**

Run:

```bash
git push origin main
```

Expected: push succeeds.

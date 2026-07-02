# Prompt Containers And Groups Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add prompt containers so the picker can send either one prompt or an ordered group of prompts one by one.

**Architecture:** Keep the current local JSON store and Tauri autosend pipeline, but evolve the frontend model from `PromptItem` to `PromptContainer`. Single containers remain the current behavior; group containers hold ordered prompt entries and use a new backend command that captures the target app once and sends every entry to that same target.

**Tech Stack:** React, TypeScript, Vitest, Tauri 2, Rust, macOS AppleScript/System Events.

---

## Review Addendum: Hidden Risk Controls

This plan is executable only if these gates are kept in scope:

- **Clean-source gate:** Verify the source files required by autosend are committed with the group feature. The group command in `src-tauri/src/lib.rs` depends on macOS support in `src-tauri/src/platform/macos.rs`, platform exports in `src-tauri/src/platform/mod.rs`, overlay session capture in `public/overlay.html`, and window stability in `src-tauri/src/windows.rs`.
- **No partial-push gate:** Before pushing, run `git status --short` and specifically confirm all source/test files touched by the current runtime are either committed or intentionally unrelated build artifacts. A narrow status check over only prompt group files is not enough.
- **Clean-verification gate:** Run `npm test -- --run`, `cd src-tauri && cargo test`, and `npm run tauri build` after all source support files are committed, not only before the final commit.
- **Signing gate:** After packaging, run `npm run sign:macos` so the built app has the stable bundle identifier `local.promptpicker.dev`; this reduces macOS Accessibility permission churn.
- **Runtime boundary gate:** Keep group execution backend-owned. The frontend must not loop over the single-prompt command because `PromptPickSessionState.take()` consumes the target after the first send.

These controls are not extra product features. They are required to prevent a locally-working but remotely-incomplete build.

### Task 1: Add Prompt Container Types

**Files:**
- Modify: `src/shared/promptTypes.ts`
- Test: `src/shared/promptTypes.test.ts`

**Step 1: Write failing type/utility tests**

Add tests for:
- a single container preview uses its first prompt body
- a group container preview joins numbered entries without calling them "steps"
- default group interval is milliseconds-level

**Step 2: Run focused tests**

Run: `npm test -- --run src/shared/promptTypes.test.ts`

Expected: tests fail because the new types/helpers do not exist.

**Step 3: Implement minimal types and helpers**

Add:
- `PromptEntry`
- `PromptContainer`
- `PromptContainerInput`
- `DEFAULT_GROUP_INTERVAL_MS = 700`
- `MIN_GROUP_INTERVAL_MS = 200`
- `MAX_GROUP_INTERVAL_MS = 3000`
- helpers to create previews and labels for single/group containers

Keep `PromptItem` as a compatibility alias or legacy type so old tests and migration code can refer to it clearly.

**Step 4: Re-run focused tests**

Run: `npm test -- --run src/shared/promptTypes.test.ts`

Expected: pass.

**Step 5: Commit**

Run:

```bash
git add src/shared/promptTypes.ts src/shared/promptTypes.test.ts
git commit -m "feat: add prompt container types"
```

### Task 2: Migrate Prompt Store To Containers

**Files:**
- Modify: `src/shared/promptStore.ts`
- Modify: `src/shared/promptStore.test.ts`
- Modify: `src/shared/promptImportExport.test.ts`
- Modify: `src/shared/promptFixtures.ts`

**Step 1: Write failing store tests**

Add tests for:
- importing/listing legacy `version: 1` `{ prompts: PromptItem[] }` produces single containers
- exporting uses `version: 2` and `containers`
- creating a single container preserves current user-facing behavior
- creating a group container stores ordered prompt entries
- updating a group can add/remove/reorder numbered entries
- invalid group data is rejected or normalized without producing empty entries

**Step 2: Run focused tests**

Run: `npm test -- --run src/shared/promptStore.test.ts src/shared/promptImportExport.test.ts`

Expected: fail because store still only supports `PromptItem`.

**Step 3: Implement migration and CRUD**

Update `createPromptStore` so:
- `list()` returns `PromptContainer[]`
- old `version: 1` data is migrated in memory to single containers
- saves write `version: 2`
- `create()` creates a single container for backward-compatible call sites
- `createGroup()` creates a group container
- `update()` accepts single or group fields
- `reorder()` reorders containers
- `importJson()` accepts both old and new formats

Do not add cloud sync, categories, cancellation, or progress state.

**Step 4: Re-run focused tests**

Run: `npm test -- --run src/shared/promptStore.test.ts src/shared/promptImportExport.test.ts`

Expected: pass.

**Step 5: Commit**

Run:

```bash
git add src/shared/promptStore.ts src/shared/promptStore.test.ts src/shared/promptImportExport.test.ts src/shared/promptFixtures.ts
git commit -m "feat: store prompt containers"
```

### Task 3: Update Manager UI For Singles And Groups

**Files:**
- Modify: `src/ui/PromptManager.tsx`
- Modify: `src/ui/PromptManager.test.tsx`
- Modify: `src/styles.css`

**Step 1: Write failing UI tests**

Add tests for:
- the manager opens directly as a prompt library
- user can add a single prompt
- user can add a group prompt
- group editor uses numbered prompts, not "Step"
- group cards show a visual distinction and prompt count
- delete/reorder still works for containers

**Step 2: Run focused tests**

Run: `npm test -- --run src/ui/PromptManager.test.tsx`

Expected: fail because UI only supports one prompt body.

**Step 3: Implement manager UI**

Build a simple management page:
- header: `Manage Prompts`
- two creation modes: `Single` and `Group`
- single mode: title + one body
- group mode: title + interval milliseconds + numbered textareas
- group controls: add prompt, delete prompt, move up/down
- list cards show `Single` or `Group · N prompts`

Avoid workflow language like "steps". Use "Prompt 1", "Prompt 2", etc.

**Step 4: Re-run focused tests**

Run: `npm test -- --run src/ui/PromptManager.test.tsx`

Expected: pass.

**Step 5: Commit**

Run:

```bash
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/styles.css
git commit -m "feat: manage prompt groups"
```

### Task 4: Update Quick Picker List

**Files:**
- Modify: `src/ui/PromptQuickList.tsx`
- Modify: `src/ui/PromptQuickList.test.tsx`
- Modify: `src/styles.css`

**Step 1: Write failing quick-list tests**

Add tests for:
- single containers render like regular prompt options
- group containers show a distinct badge/count
- clicking a group selects the whole container
- disabled state still prevents duplicate clicks

**Step 2: Run focused tests**

Run: `npm test -- --run src/ui/PromptQuickList.test.tsx`

Expected: fail because quick list expects `PromptItem`.

**Step 3: Implement quick-list rendering**

Render containers:
- single: title + body preview
- group: title + `Group · N prompts` + compact numbered preview

No progress bubble, no cancel button, no execution queue UI.

**Step 4: Re-run focused tests**

Run: `npm test -- --run src/ui/PromptQuickList.test.tsx`

Expected: pass.

**Step 5: Commit**

Run:

```bash
git add src/ui/PromptQuickList.tsx src/ui/PromptQuickList.test.tsx src/styles.css
git commit -m "feat: show prompt groups in picker"
```

### Task 5: Add Backend Sequence Autosend

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/macos.rs`
- Modify: `src/platform/platformApi.ts`
- Test: `src-tauri/src/lib.rs`

**Step 1: Write failing Rust tests**

Add tests for:
- sequence autosend uses the session target once
- it sends all bodies to the same bundle id in order
- it sleeps only between entries
- it stops at the first failed entry and reports the index
- it copies without sending when no safe target exists

**Step 2: Run focused Rust tests**

Run: `cd src-tauri && cargo test autosend_sequence`

Expected: fail because no sequence command exists.

**Step 3: Implement sequence command**

Add:
- `AutosendSequenceOutcome`
- command `paste_prompt_sequence_and_submit_to_last_target`
- helper that takes the session target once, then loops over bodies using the same `bundle_id` and click point
- interval clamped to 200-3000ms

Do not use Cmd+Tab. Do not search for a text field. The backend should activate the target app and use the same paste+return mechanism as the successful single prompt path.

**Step 4: Update TypeScript platform API**

Add:
- `AutosendSequenceOutcome`
- `pastePromptSequenceAndSubmitToLastTarget(bodies, intervalMs)`

**Step 5: Re-run focused Rust tests**

Run: `cd src-tauri && cargo test autosend_sequence`

Expected: pass.

**Step 6: Commit**

Run:

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/macos.rs src/platform/platformApi.ts
git commit -m "feat: autosend prompt groups"
```

### Task 6: Wire App Selection Logic

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/app/App.test.tsx`

**Step 1: Write failing app tests**

Add tests for:
- selecting a single container calls the existing single autosend command
- selecting a group container calls the new sequence command once with ordered bodies
- popover hides before either send path
- group failure emits a concise failure status

**Step 2: Run focused tests**

Run: `npm test -- --run src/app/App.test.tsx`

Expected: fail until app uses `PromptContainer` and the sequence command.

**Step 3: Implement app selection**

Update `handleSelect(container)`:
- if `container.type === "single"`, send first body through current single command
- if `container.type === "group"`, send ordered bodies through sequence command
- emit existing sent status when all entries are sent
- emit `第 N 条失败` when the sequence stops early

No per-entry progress UI and no cancellation.

**Step 4: Re-run focused tests**

Run: `npm test -- --run src/app/App.test.tsx`

Expected: pass.

**Step 5: Commit**

Run:

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "feat: send prompt groups from picker"
```

### Task 7: Full Verification, Build, Push

**Files:**
- Verify all changed files

**Step 1: Run full frontend tests**

Run: `npm test -- --run`

Expected: all tests pass.

**Step 2: Run Rust tests**

Run: `cd src-tauri && cargo test`

Expected: all tests pass.

**Step 3: Build app bundle**

Run: `npm run tauri build`

Expected: macOS app and DMG are produced in `src-tauri/target/release/bundle/`.

**Step 4: Check git diff**

Run: `git status --short && git diff --stat`

Expected: all runtime source/test files needed by the prompt group and autosend path are committed or staged for commit. Local build outputs such as `dist/`, `node_modules/`, and `src-tauri/target/` may be dirty after packaging but must not be confused with uncommitted source dependencies.

**Step 5: Push to GitHub main**

Run:

```bash
git status --short
git push origin main
```

Expected: push succeeds.

**Step 6: Report user-facing result**

Explain that users now see a prompt library with singles and groups, and clicking a group sends its prompts one by one to the same target app using the existing paste+return behavior.

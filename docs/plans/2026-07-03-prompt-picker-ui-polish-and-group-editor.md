# Prompt Picker UI Polish And Group Editor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rework Prompt Picker into a more refined Codex/IMD-style prompt management tool, including a polished floating picker, concise preview rules, hover full-preview panels, and a simpler group prompt editor with contextual row controls and drag ordering.

**Architecture:** Keep the existing prompt storage, autosend, Tauri commands, and app state model unchanged. Make the work primarily in React components and CSS, with small shared helper updates only where preview text needs structured output. Tests should cover preview rules and UI behavior without overfitting to visual implementation details.

**Tech Stack:** React, TypeScript, Vite, Vitest, Testing Library, Tauri 2, plain CSS.

---

## Constraints And Scope

- Do not change prompt sending, autosend, accessibility, overlay window, menu bar, or backend command behavior.
- Do not add a new UI framework or drag-and-drop dependency unless native drag events prove insufficient.
- Keep changes surgical: `PromptManager`, `PromptQuickList`, prompt preview helpers, tests, and CSS.
- Preserve existing data model compatibility for single and group prompt containers.
- The final UI should remain usable in the current Tauri window sizes and in the small floating picker window.

## Success Criteria

- Single prompt preview in the floating picker shows exactly one line with ellipsis when long.
- Group prompt preview in the floating picker shows the first two prompt entries, each one line with ellipsis.
- Hovering a floating picker item shows a bounded full-content preview panel for both single and group prompts.
- Floating picker panel has rounded corners and a lighter Codex/IMD-like visual style.
- Main Manage Prompt page looks like a focused prompt manager, not an old SaaS dashboard.
- Group editor removes visible `Remove` and full-width `Add Prompt` controls.
- Group editor row controls `+`, `-`, `↑`, `↓` appear only when hovering/focusing a row.
- Group editor supports drag reordering from the row handle/title area.
- Existing tests pass, and new tests cover preview/helper behavior and group editor interactions.

---

### Task 1: Add Structured Preview Helpers

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/shared/promptTypes.ts`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/shared/promptTypes.test.ts`

**Step 1: Write failing tests for preview lines**

Add tests covering:

```ts
import {
  getPromptContainerPreviewLines,
  type PromptContainer,
} from "./promptTypes";

test("single prompt preview returns one body line", () => {
  const prompt: PromptContainer = {
    id: "single",
    title: "Discussion",
    type: "single",
    prompts: [{ id: "entry-1", body: "Use brainstorming skill, discuss first.", order: 0 }],
    intervalMs: 700,
    createdAt: "2026-07-03T00:00:00.000Z",
    updatedAt: "2026-07-03T00:00:00.000Z",
  };

  expect(getPromptContainerPreviewLines(prompt)).toEqual([
    "Use brainstorming skill, discuss first.",
  ]);
});

test("group prompt preview returns first two ordered prompt lines", () => {
  const prompt: PromptContainer = {
    id: "group",
    title: "Debug",
    type: "group",
    intervalMs: 700,
    createdAt: "2026-07-03T00:00:00.000Z",
    updatedAt: "2026-07-03T00:00:00.000Z",
    prompts: [
      { id: "entry-2", body: "Second prompt", order: 1 },
      { id: "entry-1", body: "First prompt", order: 0 },
      { id: "entry-3", body: "Third prompt", order: 2 },
    ],
  };

  expect(getPromptContainerPreviewLines(prompt)).toEqual([
    "1. First prompt",
    "2. Second prompt",
  ]);
});
```

**Step 2: Run targeted test and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/shared/promptTypes.test.ts
```

Expected: FAIL because `getPromptContainerPreviewLines` does not exist.

**Step 3: Implement helper**

Add a helper that:

- Sorts entries by `order`.
- For `single`, returns only the first body, normalized to one visual preview line.
- For `group`, returns up to the first two non-empty entries prefixed with `1.`, `2.`.
- Keeps existing `getPromptContainerPreview` for compatibility, implemented using the first preview line if appropriate.

Implementation shape:

```ts
export function getPromptContainerPreviewLines(prompt: PromptContainer): string[] {
  const orderedPrompts = [...prompt.prompts]
    .sort((a, b) => a.order - b.order)
    .map((entry) => entry.body.trim().replace(/\s+/g, " "))
    .filter(Boolean);

  if (prompt.type === "group") {
    return orderedPrompts.slice(0, 2).map((body, index) => `${index + 1}. ${body}`);
  }

  return orderedPrompts.slice(0, 1);
}
```

**Step 4: Run targeted test and verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/shared/promptTypes.test.ts
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/shared/promptTypes.ts src/shared/promptTypes.test.ts
git commit -m "test: define prompt preview line behavior"
```

---

### Task 2: Update Floating Picker Preview Rendering

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.test.tsx`

**Step 1: Write failing UI tests for single and group previews**

Add tests that render:

- A single prompt with a long body and assert the body appears once.
- A group prompt with three entries and assert `1. first`, `2. second` appear, but `3. third` does not appear in the list preview.

Example:

```tsx
expect(screen.getByText(/^1\. First group prompt/)).toBeInTheDocument();
expect(screen.getByText(/^2\. Second group prompt/)).toBeInTheDocument();
expect(screen.queryByText(/^3\. Third group prompt/)).not.toBeInTheDocument();
```

**Step 2: Run targeted test and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: FAIL because the current group preview is one block from `getPromptContainerPreview`.

**Step 3: Render preview lines**

Change `PromptQuickList` to import `getPromptContainerPreviewLines`.

Render preview lines like:

```tsx
<span className="prompt-quick-preview-lines">
  {getPromptContainerPreviewLines(prompt).map((line) => (
    <span className="prompt-quick-preview-line" key={line}>
      {line}
    </span>
  ))}
</span>
```

Keep the outer button click behavior unchanged.

**Step 4: Run targeted test and verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptQuickList.tsx src/ui/PromptQuickList.test.tsx
git commit -m "feat: show concise prompt previews"
```

---

### Task 3: Add Hover Full-Preview Panel To Floating Picker

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptQuickList.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Write failing hover preview tests**

Use Testing Library `fireEvent.mouseEnter` or `userEvent.hover` to assert:

- Hovering a single prompt item displays a preview panel with full body text.
- Hovering a group prompt item displays all group entries.
- Moving away hides the panel.

Test for accessible structure:

```tsx
expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
fireEvent.mouseEnter(screen.getByRole("option", { name: /Debug/i }));
expect(screen.getByRole("tooltip")).toHaveTextContent("3. Third group prompt");
fireEvent.mouseLeave(screen.getByRole("option", { name: /Debug/i }));
expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
```

**Step 2: Run targeted test and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: FAIL because there is no hover preview panel.

**Step 3: Implement minimal hover state**

In `PromptQuickList`:

- Track `hoveredPromptId`.
- Show `<aside role="tooltip" className="prompt-hover-preview">` next to or inside the list container.
- For single prompt: render title, meta, full body preserving line breaks.
- For group prompt: render title, meta, all ordered bodies numbered.
- Do not add edit/send controls to this panel.

Implementation sketch:

```tsx
const hoveredPrompt = prompts.find((prompt) => prompt.id === hoveredPromptId) ?? null;

<div className="prompt-quick-list-wrap">
  <div className="prompt-quick-list" role="listbox" aria-label="Prompts">...</div>
  {hoveredPrompt ? <PromptHoverPreview prompt={hoveredPrompt} /> : null}
</div>
```

**Step 4: Add bounded preview CSS**

Add CSS:

```css
.prompt-hover-preview {
  position: absolute;
  right: calc(100% + 10px);
  top: 8px;
  width: min(320px, 72vw);
  max-height: min(420px, 70vh);
  overflow: auto;
  border-radius: 12px;
  border: 1px solid var(--pp-border);
  background: rgba(255, 255, 255, 0.98);
  box-shadow: var(--pp-shadow-popover);
}
```

Use CSS variables from Task 4 after they exist; if this task lands first, define local fallback values.

**Step 5: Run targeted test and verify pass**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptQuickList.tsx src/ui/PromptQuickList.test.tsx src/styles.css
git commit -m "feat: preview full prompt content on hover"
```

---

### Task 4: Establish Refined Visual Tokens And Floating Picker Styling

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`
- Test: existing UI tests only, plus visual review during app run if available.

**Step 1: Add CSS tokens**

Add under `:root`:

```css
:root {
  --pp-bg: #f6f7f8;
  --pp-surface: #ffffff;
  --pp-surface-subtle: #f7f9fb;
  --pp-border: #e3e7ed;
  --pp-border-strong: #d3dae4;
  --pp-text: #111827;
  --pp-muted: #687386;
  --pp-accent: #2563eb;
  --pp-accent-soft: #eef4ff;
  --pp-radius-sm: 8px;
  --pp-radius-md: 10px;
  --pp-radius-lg: 14px;
  --pp-shadow-popover: 0 18px 48px rgba(15, 23, 42, 0.16), 0 2px 8px rgba(15, 23, 42, 0.08);
  --pp-shadow-soft: 0 1px 2px rgba(15, 23, 42, 0.04);
}
```

**Step 2: Restyle floating picker shell**

Update:

- `.popover-window`
- `.popover-window::after`
- `.prompt-quick-list`
- `.prompt-quick-item`
- `.prompt-quick-item-group`
- `.prompt-quick-title`
- `.prompt-quick-meta`
- `.prompt-quick-preview-lines`
- `.prompt-quick-preview-line`

Target:

- rounded outer panel
- low contrast border
- soft shadow
- compact row spacing
- one-line truncation per preview line
- no heavy blue card fill

**Step 3: Run tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptQuickList.test.tsx src/ui/PromptPopover.test.tsx
```

Expected: PASS.

**Step 4: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/styles.css
git commit -m "style: refine floating prompt picker"
```

---

### Task 5: Rework Main Manage Prompt Page Structure

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Write/adjust structure tests**

Ensure tests verify:

- Page title remains prompt management focused.
- Create form exists for single/group.
- Prompt list is visible.
- Import/export actions remain available.
- No test depends on old dashboard card wording if the wording is removed.

**Step 2: Run targeted test before implementation**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: existing tests may PASS or FAIL depending on old text assumptions. Update tests first where needed to describe desired behavior.

**Step 3: Simplify page layout**

In `PromptManager`:

- Keep a single page header: `Prompt Picker` or `Manage Prompts`.
- Keep short subtitle.
- Move Import/Export into a light toolbar.
- Keep `New Prompt Container` as a section, but reduce visual weight.
- Keep `Prompt List` as a section.
- Do not reintroduce floating button status/settings panels.

Structure target:

```tsx
<div className="prompt-manager-shell">
  <header className="prompt-manager-header">...</header>
  <section className="prompt-section prompt-compose-section">...</section>
  <section className="prompt-section prompt-library-section">...</section>
</div>
```

**Step 4: Restyle main page**

Update CSS:

- `.app-window`, `.app-window-main`
- `.prompt-manager`
- `.page-header`
- `.editor-panel`
- `.list-panel`
- `.prompt-item`
- `.field`
- `.button`
- `.segmented-control`

Target:

- centered max width around `820px`
- section containers with light borders and minimal shadows
- smaller text hierarchy
- less saturated large blue surfaces
- inputs with refined border radius and focus ring

**Step 5: Run targeted test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/styles.css
git commit -m "style: simplify prompt manager layout"
```

---

### Task 6: Simplify Group Editor Row Controls

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Write failing tests for row controls**

Add tests:

- Clicking `+` on Prompt 1 inserts a blank prompt after Prompt 1.
- Clicking `-` removes that prompt.
- `-` is disabled or hidden when there is only one prompt.
- Up/down still reorder prompts.

Use accessible labels:

```tsx
await user.click(screen.getByRole("button", { name: /insert prompt after prompt 1/i }));
await user.click(screen.getByRole("button", { name: /remove prompt 2/i }));
await user.click(screen.getByRole("button", { name: /move prompt 2 up/i }));
```

**Step 2: Run targeted test and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: FAIL because insert-after buttons do not exist and current controls differ.

**Step 3: Replace `GroupFields` controls**

In `GroupFields`:

- Remove full-width `Add Prompt`.
- Replace `Remove` text button with icon-like button `-`.
- Add `+` button for insert after current row.
- Keep up/down buttons as `↑` and `↓`.
- Add accessible labels for all four buttons.
- Hide controls visually by default using CSS, but keep them keyboard accessible on row focus.

Component shape:

```tsx
<div className="group-prompt-row">
  <button className="group-prompt-handle" aria-label={`Drag prompt ${index + 1}`}>
    Prompt {index + 1}
  </button>
  <textarea ... />
  <div className="group-prompt-actions" aria-label={`Prompt ${index + 1} actions`}>
    <button aria-label={`Insert prompt after prompt ${index + 1}`}>+</button>
    <button aria-label={`Remove prompt ${index + 1}`}>-</button>
    <button aria-label={`Move prompt ${index + 1} up`}>↑</button>
    <button aria-label={`Move prompt ${index + 1} down`}>↓</button>
  </div>
</div>
```

**Step 4: Update `GroupFields` props**

Replace `onAddPrompt` with:

```ts
onInsertPrompt: (index: number) => void;
```

In create/edit forms:

```ts
onInsertPrompt={(index) => {
  const next = [...draft.prompts];
  next.splice(index + 1, 0, "");
  setDraft({ ...draft, prompts: next });
}}
```

**Step 5: Add hover/focus CSS**

Add:

```css
.group-prompt-actions {
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms ease;
}

.group-prompt-row:hover .group-prompt-actions,
.group-prompt-row:focus-within .group-prompt-actions {
  opacity: 1;
  pointer-events: auto;
}
```

Keep disabled first/last controls visible but muted when row controls are visible.

**Step 6: Run targeted test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 7: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/styles.css
git commit -m "feat: simplify group prompt row controls"
```

---

### Task 7: Add Native Drag Reordering For Group Prompt Rows

**Files:**
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/ui/PromptManager.test.tsx`
- Modify: `/Users/yang/Desktop/GitHub-pre/prompt-picker/src/styles.css`

**Step 1: Write failing drag reorder test**

Use `fireEvent.dragStart`, `fireEvent.dragOver`, `fireEvent.drop`, `fireEvent.dragEnd` on row handles.

Assert text values reorder:

```tsx
const firstHandle = screen.getByRole("button", { name: /drag prompt 1/i });
const thirdRow = screen.getByLabelText(/prompt 3 body/i);

fireEvent.dragStart(firstHandle, { dataTransfer });
fireEvent.dragOver(thirdRow);
fireEvent.drop(thirdRow);

expect(getPromptTextareas()[2]).toHaveValue("original first");
```

**Step 2: Run targeted test and verify failure**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: FAIL because drag behavior does not exist.

**Step 3: Implement minimal drag state**

Inside `GroupFields`:

- Track `draggingIndex`.
- On drag start from handle, set index and set `dataTransfer.effectAllowed = "move"`.
- On drag over row, prevent default.
- On drop row, call `onMovePrompt(draggingIndex, targetIndex)` if indexes differ.
- On drag end, clear drag state.

Do not add external dependencies.

**Step 4: Add drag styling**

Add:

```css
.group-prompt-row.is-dragging {
  opacity: 0.62;
}

.group-prompt-handle {
  cursor: grab;
}

.group-prompt-handle:active {
  cursor: grabbing;
}
```

**Step 5: Run targeted test**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/styles.css
git commit -m "feat: reorder group prompts by dragging"
```

---

### Task 8: Full Verification And Packaging Check

**Files:**
- No source changes expected unless verification finds a bug.

**Step 1: Run all frontend tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm test -- --run
```

Expected: all tests PASS.

**Step 2: Run Rust tests**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri
cargo test
```

Expected: all tests PASS.

**Step 3: Run production build**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run tauri build
```

Expected: build completes and produces:

- `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/macos/Prompt Picker.app`
- `/Users/yang/Desktop/GitHub-pre/prompt-picker/src-tauri/target/release/bundle/dmg/Prompt Picker_1.0.0_aarch64.dmg`

**Step 4: Sign macOS bundle**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
npm run sign:macos
```

Expected: signature verification succeeds.

**Step 5: Inspect source diff**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git status --short
git diff --stat
```

Expected: only intended source/test/style/plan files are changed, plus generated build artifacts if not ignored.

**Step 6: Commit final verification fixes if any**

Only if verification required fixes:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git add <fixed-files>
git commit -m "fix: polish prompt picker ui regressions"
```

**Step 7: Push to GitHub main**

Run:

```bash
cd /Users/yang/Desktop/GitHub-pre/prompt-picker
git push origin main
```

Expected: push succeeds.

---

## User-Visible Final Effect

After this plan is implemented, the user should see:

- Clicking Calico opens a rounded, refined picker panel instead of a sharp old-style list.
- Single prompts show title, light metadata, and one single-line preview.
- Group prompts show title, light metadata, and the first two prompts, each one line.
- Hovering a prompt container shows a bounded full preview panel with complete content.
- The main App page directly feels like a prompt manager, with less clutter and a cleaner Codex-like style.
- Group editing feels like editing an ordered list: row controls appear only when needed, and rows can be dragged to reorder.


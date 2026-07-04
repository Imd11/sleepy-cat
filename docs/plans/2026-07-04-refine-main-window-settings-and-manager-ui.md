# Prompt Picker Main Window UI Refinement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refine the Prompt Picker menu-bar main window settings and prompt management screens into a tighter, more polished desktop UI without changing product behavior.

**Architecture:** Keep the existing React component boundaries and Tauri window behavior intact. Update `SettingsPanel` to use row-based desktop settings markup, lightly restructure `PromptManager` for a denser panel/list presentation, and consolidate the main-window visual system in `src/styles.css` using scoped app-window tokens plus a lower-saturation blue-gray desktop theme. Do not change root-level tokens that the prompt popover also consumes.

**Tech Stack:** React 19, TypeScript, Vite, Vitest, Testing Library, CSS, Tauri 2.

---

## Non-Goals

- Do not change prompt creation, editing, deletion, import/export, reorder, group prompt, autosend, storage, language persistence, or Tauri window behavior.
- Do not change the menu bar/tray implementation.
- Do not change the Calico floating button or prompt popover quick list behavior.
- Do not change global `:root` Prompt Picker tokens unless the change is explicitly intended to affect both the main window and prompt popover. For this task, scope visual token changes to `.app-window`.
- Do not resize the main Tauri window to hide layout problems. The UI must work in the existing `760 x 560` default window and `640 x 460` minimum window.
- Do not add a new design system dependency or icon library.
- Do not add explanatory/help copy back into the settings page.

## Worktree and Staging Guardrails

This repository may already contain unrelated generated artifacts or user work. Before implementation:

```bash
git status --short --branch
```

If unrelated files are dirty, either execute this plan in a clean worktree or use exact-path `git add` commands only. Never stage `dist`, `node_modules`, `src-tauri/target`, release bundles, or unrelated docs as part of this UI task.

## Current Problem Summary

The current settings page is still shaped like an early form:

```text
设置
控制 Calico 如何填入提示词。

语言
选择应用界面使用的语言。
界面语言
[中文 v]
```

The language selector drops to a second line because `SettingsPanel.tsx` renders:

```tsx
<label className="settings-field">
  <span>{t.settings.languageField}</span>
  <select ... />
</label>
```

and `src/styles.css` defines:

```css
.settings-field {
  display: grid;
  max-width: 360px;
  gap: 6px;
}
```

The desired settings page is a compact row-based desktop settings surface:

```text
设置

┌──────────────────────────────────────────────┐
│ 语言                                         │
│──────────────────────────────────────────────│
│ 界面语言                         [ 中文   v ] │
└──────────────────────────────────────────────┘

┌──────────────────────────────────────────────┐
│ 点击行为                                      │
│──────────────────────────────────────────────│
│ 选择提示词时              [只填入] [填入并发送] │
└──────────────────────────────────────────────┘

┌──────────────────────────────────────────────┐
│ 隐藏应用                                      │
│──────────────────────────────────────────────│
│ 暂无隐藏应用                                  │
└──────────────────────────────────────────────┘
```

The prompt manager should keep the current workflow but visually tighten into the same desktop surface language:

```text
管理提示词                              [设置] [导入] [导出]
3 个提示词容器

┌──────────────────────────────────────────────┐
│ 新建提示词                         [单个][群组] │
│ 标题       [______________________________]   │
│ 内容       [______________________________]   │
│                                  [添加提示词] │
└──────────────────────────────────────────────┘

┌──────────────────────────────────────────────┐
│ 提示词                                      │
│──────────────────────────────────────────────│
│ 标题                         [↑] [↓] [编辑] [删除] │
│ 预览内容...                                  │
└──────────────────────────────────────────────┘
```

## Visual Specification

Use these values as the target visual direction. Exact color values may be adjusted by a few points if screenshots show better balance.

```text
Window background: #f2f6f8
Panel background: #fbfdff
Subtle panel background: #f7fafc
Panel border: #dbe5ec
Row divider: #e6edf2
Primary text: #111827
Muted text: #667085
Accent selected surface: #111827 or #fdfefe depending control type
Danger text: #b42318
Radius: 8px
Main padding: 22px 24px
Panel radius: 8px
Row height: 52-58px
Control height: 32px
Main title: 20-22px
Section title: 13-14px
Body/control text: 13px
Button font weight: 600-650
```

---

### Task 1: Add Settings Panel Structure Tests

**Files:**
- Modify: `src/ui/SettingsPanel.test.tsx`
- Reference: `src/ui/SettingsPanel.tsx`

**Step 1: Write failing tests for row-based settings markup**

Add tests near the existing language tests:

```tsx
it("renders language selection as a right-aligned settings row", () => {
  renderPanel();

  const languageSelect = screen.getByLabelText("界面语言");
  const row = languageSelect.closest(".settings-row");
  expect(row).toBeTruthy();
  expect(row?.querySelector(".settings-row-main")).toBeTruthy();
  expect(row?.querySelector(".settings-row-control")).toContainElement(languageSelect);
});

it("does not render instructional settings descriptions", () => {
  renderPanel();

  expect(screen.queryByText("控制 Calico 如何填入提示词。")).toBeNull();
  expect(screen.queryByText("选择应用界面使用的语言。")).toBeNull();
  expect(screen.queryByText("选择点击提示词后，只填入输入框，还是填入并发送。")).toBeNull();
  expect(screen.queryByText("在这些应用中隐藏小猫。")).toBeNull();
});

it("renders prompt click behavior as a compact settings row", () => {
  renderPanel();

  const selectedButton = screen.getByRole("button", { name: "填入并发送" });
  const row = selectedButton.closest(".settings-row");
  expect(row).toBeTruthy();
  expect(row?.querySelector(".settings-row-control")).toContainElement(selectedButton);
});
```

If TypeScript complains about `toContainElement`, either rely on `@testing-library/jest-dom` already configured in `src/test-setup.ts`, or use:

```tsx
expect(row?.querySelector(".settings-row-control")?.contains(languageSelect)).toBe(true);
```

**Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/ui/SettingsPanel.test.tsx
```

Expected: FAIL because `.settings-row`, `.settings-row-main`, and `.settings-row-control` do not exist yet, and the instructional copy still renders.

**Step 3: Commit tests**

Do not commit failing tests alone unless this repository is intentionally using red commits. Prefer to keep the tests unstaged until Task 2 passes.

---

### Task 2: Refactor SettingsPanel Markup

**Files:**
- Modify: `src/ui/SettingsPanel.tsx`
- Test: `src/ui/SettingsPanel.test.tsx`

**Step 1: Replace description-heavy settings sections with row markup**

In `SettingsPanel.tsx`, keep the component props and callbacks unchanged. Replace the current section bodies with a row-oriented structure:

```tsx
return (
  <div className="settings-panel page-stack">
    <header className="page-header settings-page-header">
      <div>
        <h1>{t.settings.title}</h1>
      </div>
    </header>

    <section className="settings-card">
      <div className="settings-card-heading">
        <h2>{t.settings.languageTitle}</h2>
      </div>
      <label className="settings-row">
        <span className="settings-row-main">
          <span className="settings-row-title">{t.settings.languageField}</span>
        </span>
        <span className="settings-row-control">
          <select
            className="field settings-select"
            value={settings.language}
            onChange={(event) => onLanguageChange(event.target.value as AppLanguage)}
          >
            <option value="zh-CN">{LANGUAGE_LABELS["zh-CN"]}</option>
            <option value="en-US">{LANGUAGE_LABELS["en-US"]}</option>
          </select>
        </span>
      </label>
    </section>

    <section className="settings-card">
      <div className="settings-card-heading">
        <h2>{t.settings.clickBehaviorTitle}</h2>
      </div>
      <div className="settings-row">
        <div className="settings-row-main">
          <div className="settings-row-title">{t.settings.clickBehaviorField}</div>
        </div>
        <div className="settings-row-control">
          <div className="segmented-control settings-segmented-control" aria-label={t.settings.clickBehaviorTitle}>
            ...
          </div>
        </div>
      </div>
    </section>

    <section className="settings-card">
      <div className="settings-card-heading">
        <h2>{t.settings.blacklistedAppsTitle}</h2>
      </div>
      {settings.blacklistedApps.length === 0 ? (
        <div className="settings-empty-row">{t.settings.noBlacklistedApps}</div>
      ) : (
        <ul className="blacklist settings-blacklist">
          ...
        </ul>
      )}
    </section>
  </div>
);
```

Add one new i18n key for the row label:

```ts
clickBehaviorField: "选择提示词时",
```

and English:

```ts
clickBehaviorField: "When selecting a prompt",
```

Keep the old description keys in `src/shared/i18n.ts` for compatibility if other code references them, but do not render them in `SettingsPanel`.

**Step 2: Run the focused test**

Run:

```bash
npm test -- src/ui/SettingsPanel.test.tsx
```

Expected: PASS.

**Step 3: Commit**

```bash
git add src/ui/SettingsPanel.tsx src/ui/SettingsPanel.test.tsx src/shared/i18n.ts
git commit -m "refactor: use compact settings rows"
```

---

### Task 3: Implement Settings Page Visual System

**Files:**
- Modify: `src/styles.css`
- Test: `src/ui/SettingsPanel.test.tsx`

**Step 1: Add settings-specific CSS**

Append or replace the existing settings-specific CSS with this scoped system. Keep class names under `.settings-panel` to avoid breaking popover/button-control styles.

```css
.settings-panel {
  width: min(760px, 100%);
  margin: 0 auto;
  gap: 14px;
}

.settings-page-header {
  margin-bottom: 2px;
}

.settings-page-header h1 {
  font-size: 21px;
  font-weight: 680;
  line-height: 1.2;
}

.settings-card {
  overflow: hidden;
  background: var(--pp-surface);
  border: 1px solid var(--pp-border);
  border-radius: var(--pp-radius-sm);
  box-shadow: var(--pp-shadow-soft);
}

.settings-card-heading {
  display: flex;
  min-height: 38px;
  align-items: center;
  padding: 0 14px;
  border-bottom: 1px solid var(--pp-border);
  background: var(--pp-surface-subtle);
}

.settings-card-heading h2 {
  margin: 0;
  color: var(--pp-text);
  font-size: 13px;
  font-weight: 680;
  line-height: 1.25;
}

.settings-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  min-height: 54px;
  align-items: center;
  gap: 16px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--pp-border);
}

.settings-row:last-child {
  border-bottom: 0;
}

.settings-row-main {
  min-width: 0;
}

.settings-row-title {
  display: block;
  color: var(--pp-text);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.35;
}

.settings-row-control {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: flex-end;
}

.settings-select {
  width: 156px;
  min-height: 32px;
  height: 32px;
  padding: 0 30px 0 10px;
}

.settings-segmented-control {
  height: 32px;
}

.settings-segmented-control button {
  min-width: 78px;
  padding: 0 11px;
}

.settings-empty-row {
  min-height: 52px;
  padding: 16px 14px;
  color: var(--pp-muted);
  font-size: 13px;
}

.settings-blacklist {
  margin: 0;
  gap: 0;
}

.settings-blacklist li {
  border: 0;
  border-bottom: 1px solid var(--pp-border);
  border-radius: 0;
  background: transparent;
}

.settings-blacklist li:last-child {
  border-bottom: 0;
}

.settings-blacklist li > div {
  min-width: 0;
}

.settings-blacklist span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

Remove or neutralize the old `.settings-field` rules if they are no longer used. If leaving them in place, ensure they cannot affect the new settings layout.

**Step 2: Tighten main-window-only tokens**

Do not update `:root` for this task. The prompt popover uses root `--pp-*` tokens in `.popover-window` and `.prompt-quick-item`; changing root tokens would silently restyle the popover and violate the non-goal. Scope the new surface values to `.app-window` instead:

```css
.app-window {
  --pp-bg: #f2f6f8;
  --pp-surface: #fbfdff;
  --pp-surface-subtle: #f7fafc;
  --pp-border: #dbe5ec;
  --pp-border-strong: #cfdbe5;
  --pp-muted: #667085;
  --pp-radius-sm: 8px;
  --pp-radius-md: 8px;
  --pp-radius-lg: 10px;
  --pp-shadow-soft: 0 1px 2px rgba(15, 23, 42, 0.035);
}

.app-window-main {
  padding: 22px 24px 30px;
}
```

Do not modify popover transparency rules or root prompt-popover tokens.

**Step 3: Run focused and full frontend tests**

Run:

```bash
npm test -- src/ui/SettingsPanel.test.tsx
npm test
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src/styles.css
git commit -m "style: polish settings panel surface"
```

---

### Task 4: Add PromptManager Copy and Structure Tests

**Files:**
- Modify: `src/ui/PromptManager.test.tsx`
- Reference: `src/ui/PromptManager.tsx`

**Step 1: Write tests that lock in reduced instructional copy**

Add tests:

```tsx
it("does not render instructional manager section descriptions", () => {
  renderManager();

  expect(screen.queryByText("为快速选择器添加一个提示词或一个有顺序的提示词组。")).toBeNull();
  expect(screen.queryByText("选择小猫列表中的显示顺序。")).toBeNull();
});

it("renders create panel heading and type control in the same shell", () => {
  renderManager();

  const singleButton = screen.getByRole("button", { name: "单个" });
  const header = singleButton.closest(".panel-heading-with-actions");
  expect(header).toBeTruthy();
  expect(header?.textContent).toContain("新建提示词容器");
});

it("renders prompt list as a unified row list", () => {
  renderManager();

  const list = screen.getByText("Code Review").closest(".prompt-list");
  expect(list).toBeTruthy();
  expect(list?.querySelectorAll(".prompt-item").length).toBe(2);
});
```

**Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
```

Expected: FAIL because the instructional descriptions still render and `.panel-heading-with-actions` does not exist.

---

### Task 5: Refactor PromptManager Markup Without Changing Behavior

**Files:**
- Modify: `src/ui/PromptManager.tsx`
- Test: `src/ui/PromptManager.test.tsx`

**Step 1: Move type segmented control into create panel heading**

Change the create panel heading from:

```tsx
<div className="section-heading">
  <h2>{messages.manager.newContainerTitle}</h2>
  <p>{messages.manager.newContainerDescription}</p>
</div>
<div className="segmented-control" ...>
```

to:

```tsx
<div className="section-heading panel-heading-with-actions">
  <div>
    <h2>{messages.manager.newContainerTitle}</h2>
  </div>
  <div className="segmented-control" aria-label={messages.manager.promptContainerType}>
    ...
  </div>
</div>
```

Do not render `messages.manager.newContainerDescription`.

**Step 2: Remove prompt list instructional description**

Change:

```tsx
<div className="section-heading">
  <h2>{messages.manager.promptListTitle}</h2>
  <p>{messages.manager.promptListDescription}</p>
</div>
```

to:

```tsx
<div className="section-heading panel-heading-with-actions">
  <div>
    <h2>{messages.manager.promptListTitle}</h2>
  </div>
</div>
```

Keep `messages.manager.count(prompts.length)` in the page header; it is useful status, not instructional copy.

**Step 3: Keep all callbacks and form fields unchanged**

Verify these behaviors still map to the same handlers:

- `onOpenSettings`
- `onImport`
- `onExport`
- `onCreate`
- `onCreateGroup`
- `onUpdate`
- `onDelete`
- `onReorder`

**Step 4: Run focused tests**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx
git commit -m "refactor: tighten prompt manager layout"
```

---

### Task 6: Polish PromptManager CSS to Match Settings Surface

**Files:**
- Modify: `src/styles.css`
- Test: `src/ui/PromptManager.test.tsx`

**Step 1: Add compact shared panel heading style**

```css
.panel-heading-with-actions {
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.panel-heading-with-actions h2 {
  margin: 0;
}
```

**Step 2: Tighten prompt manager width, header, panels, and list rows**

Update existing `.prompt-manager ...` overrides. Target:

```css
.prompt-manager {
  width: min(760px, 100%);
  gap: 14px;
}

.prompt-manager .page-header h1 {
  font-size: 21px;
  line-height: 1.2;
}

.prompt-manager .page-header p {
  font-size: 13px;
}

.prompt-manager .editor-panel,
.prompt-manager .list-panel {
  border-color: var(--pp-border);
  border-radius: var(--pp-radius-sm);
  background: var(--pp-surface);
  box-shadow: var(--pp-shadow-soft);
}

.prompt-manager .editor-panel {
  padding: 14px;
}

.prompt-manager .list-panel {
  padding: 14px;
}

.prompt-manager .prompt-list {
  margin-top: 10px;
  border-color: var(--pp-border);
  border-radius: var(--pp-radius-sm);
  background: var(--pp-surface);
}

.prompt-manager .prompt-item {
  background: transparent;
}

.prompt-manager .prompt-content {
  padding: 11px 13px;
}
```

Keep the existing group prompt row hover behavior unless it conflicts visually.

**Step 3: Make buttons lighter**

Ensure manager buttons do not look like heavy marketing buttons:

```css
.prompt-manager .button {
  min-height: 32px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 620;
}
```

Keep `.button-primary` readable; it can stay dark for the add/save action.

**Step 4: Run focused tests**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
npm test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/styles.css
git commit -m "style: polish prompt manager surface"
```

---

### Task 7: Add Main Window Visual Contract Tests

**Files:**
- Modify: `src/app/App.test.tsx`
- Test: `src/app/App.test.tsx`

**Step 1: Add tests for main-window settings and manager shells**

Add tests near existing manager/settings mode tests. If no direct test exists, use URL mode and mocked settings/prompts similar to current `App.test.tsx` setup.

For settings mode:

```tsx
it("renders settings mode with the desktop settings panel shell", async () => {
  currentWindowLabel = "main";
  window.history.pushState({}, "", "/?mode=settings");

  await act(async () => {
    render(<App />);
  });

  expect(document.querySelector(".app-window-main")).toBeTruthy();
  expect(document.querySelector(".settings-panel")).toBeTruthy();
  expect(document.querySelector(".settings-card")).toBeTruthy();
});
```

For manager mode:

```tsx
it("renders manager mode with the polished prompt manager shell", async () => {
  currentWindowLabel = "main";
  window.history.pushState({}, "", "/?mode=manager");

  await act(async () => {
    render(<App />);
  });

  expect(document.querySelector(".prompt-manager")).toBeTruthy();
  expect(document.querySelector(".panel-heading-with-actions")).toBeTruthy();
});
```

Adjust mocks if `App` attempts to load storage asynchronously. Use the patterns already in `App.test.tsx`.

**Step 2: Run tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS after Tasks 2 and 5.

**Step 3: Commit**

```bash
git add src/app/App.test.tsx
git commit -m "test: cover main window polished shells"
```

---

### Task 8: Manual Responsive Verification at Main Window Sizes

**Files:**
- No code changes expected.
- Verify: `src/styles.css`, `src/ui/SettingsPanel.tsx`, `src/ui/PromptManager.tsx`

**Step 1: Build frontend**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 2: Start a visual verification runtime**

Preferred, because this is a Tauri main-window UI:

```bash
npm run tauri dev
```

If a full Tauri runtime is not needed for the visual pass, Vite can still be used for static mode checks:

Run:

```bash
npm run dev
```

Expected: Vite serves on `http://localhost:1420`.

**Step 3: Inspect settings and manager mode at desktop sizes**

Open these URLs in a browser:

```text
http://localhost:1420/?mode=settings
http://localhost:1420/?mode=manager
```

If browser mode cannot load Tauri APIs cleanly, perform the same checks in the Tauri dev window by opening Settings and Manage Prompts from the menu bar/tray controls.

Check viewport sizes:

```text
760 x 560
640 x 460
```

Expected settings page:

- Language row stays one line at `760 x 560`.
- Language select is right-aligned.
- No instructional descriptions render.
- Click behavior segmented control stays aligned right.
- Hidden apps empty row is visually calm.
- No horizontal overflow.

Expected prompt manager:

- Header controls do not crowd title at `760 x 560`.
- New prompt panel remains usable.
- Prompt list rows scan clearly.
- Buttons do not dominate list text.
- No overlapping text.

**Step 4: Stop dev server**

Use `Ctrl-C` in the dev server terminal.

**Step 5: If visual fixes are needed**

Make only CSS adjustments in `src/styles.css`; do not alter behavior.

Run:

```bash
npm test
npm run build
```

Expected: PASS.

**Step 6: Commit visual adjustments**

```bash
git add src/styles.css
git commit -m "style: tune main window responsive polish"
```

---

### Task 9: Final Full Verification

**Files:**
- No new code expected.

**Step 1: Check dirty worktree carefully**

Run:

```bash
git status --short --branch
```

Expected: Only intentional changes remain. This repository has had unrelated generated artifacts in prior work, so do not stage `node_modules`, `src-tauri/target`, or unrelated docs/plans unless explicitly intended.

**Step 2: Run full frontend tests**

Run:

```bash
npm test
```

Expected: all Vitest tests pass.

**Step 3: Run Rust tests only if no unrelated Cargo edits are present**

Run:

```bash
cargo test
```

from:

```bash
cd src-tauri
```

Expected: all Rust tests pass. If Cargo files are dirty from unrelated work, do not stage those changes as part of this UI task.

**Step 4: Build**

Run:

```bash
npm run build
```

Expected: `tsc && vite build` succeeds.

**Step 5: Final commit if needed**

If Tasks 2, 3, 5, 6, and 7 were not committed separately, make one scoped commit:

```bash
git add src/ui/SettingsPanel.tsx src/ui/SettingsPanel.test.tsx src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/app/App.test.tsx src/shared/i18n.ts src/styles.css
git commit -m "style: refine main window settings and prompt manager"
```

Prefer the smaller task commits above if possible.

---

## Acceptance Criteria

- Settings page no longer renders instructional descriptions.
- Language selector appears on the same row as “界面语言” / “Interface language”.
- Prompt click behavior appears as a right-aligned compact segmented control.
- Hidden apps section uses the same row/list visual language as other settings sections.
- Prompt manager keeps existing behavior and callbacks.
- Prompt manager no longer renders the two instructional section descriptions.
- Main window visual language is consistent across settings and prompt manager.
- UI remains usable at `760 x 560` and `640 x 460`.
- `npm test` passes.
- `npm run build` passes.
- No unrelated dirty worktree files are staged or committed.

## Implementation Notes

- Existing tests already cover most behavior; add structural tests instead of brittle pixel tests.
- JSDOM cannot prove actual layout width. The class/DOM tests prevent the old single-column settings field from returning; manual viewport verification catches the visual layout.
- Keep `src/shared/i18n.ts` description keys unless removing them is proven safe. Not rendering them is enough for this task.
- Be careful with global `.button`, `.field`, `.segmented-control`, and `.list-panel` changes because the prompt popover and button controls share the same stylesheet.
- Prefer scoped overrides under `.settings-panel` and `.prompt-manager`.

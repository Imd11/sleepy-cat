# Reset Hover Preview On Popover Open Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent stale prompt hover detail panels from appearing immediately when Calico opens the prompt list.

**Architecture:** The popover window is reused instead of recreated, so React state can survive `window.hide()` / `window.show()`. Add an explicit hover-preview reset signal from `App` to `PromptQuickList`, and make `PromptQuickList` clear hover state on popover lifecycle changes, prompt selection, and non-hover transitions. Keep hover preview driven only by actual pointer hover, not by stale focus.

**Tech Stack:** React + TypeScript, Tauri event bridge, Vitest + Testing Library, Rust/Tauri window commands.

---

### Task 1: Add Focus/Hover Regression Tests In PromptQuickList

**Files:**
- Modify: `src/ui/PromptQuickList.test.tsx`
- Modify later: `src/ui/PromptQuickList.tsx`

**Step 1: Add failing test for reset signal clearing visible hover preview**

Add a test that reveals a hover preview, rerenders the quick list with a changed reset key, and asserts that the tooltip disappears.

```tsx
it("clears hover preview when the reset key changes", () => {
  vi.useFakeTimers();
  const { rerender } = renderQuickList({ hoverResetKey: 0 });

  revealHoverPreview(screen.getByRole("option", { name: /修复流程/i }));
  expect(screen.getByRole("tooltip")).toBeTruthy();

  const zh = getMessages("zh-CN");
  rerender(
    <PromptQuickList
      prompts={prompts}
      messages={zh.quickList}
      groupMeta={zh.manager.groupMeta}
      onSelect={() => {}}
      hoverResetKey={1}
    />
  );

  expect(screen.queryByRole("tooltip")).toBeNull();
});
```

**Step 2: Add failing test for pending hover timer reset**

This catches the case where the user closes the list before the 1.5s timer fires, then reopens it.

```tsx
it("cancels a pending hover preview when the reset key changes", () => {
  vi.useFakeTimers();
  const { rerender } = renderQuickList({ hoverResetKey: 0 });

  fireEvent.mouseEnter(screen.getByRole("option", { name: /修复流程/i }));

  const zh = getMessages("zh-CN");
  rerender(
    <PromptQuickList
      prompts={prompts}
      messages={zh.quickList}
      groupMeta={zh.manager.groupMeta}
      onSelect={() => {}}
      hoverResetKey={1}
    />
  );

  act(() => {
    vi.advanceTimersByTime(1500);
  });

  expect(screen.queryByRole("tooltip")).toBeNull();
});
```

**Step 3: Add failing test that focus alone does not show hover preview**

This prevents an old focused option from recreating the hover panel when the window is shown again.

```tsx
it("does not show hover preview from focus alone", () => {
  vi.useFakeTimers();
  renderQuickList();

  fireEvent.focus(screen.getByRole("option", { name: /修复流程/i }));
  act(() => {
    vi.advanceTimersByTime(1500);
  });

  expect(screen.queryByRole("tooltip")).toBeNull();
});
```

**Step 4: Add failing test that selecting an option clears hover preview**

```tsx
it("clears hover preview before selecting a prompt", () => {
  vi.useFakeTimers();
  let selected: PromptContainer | null = null;
  renderQuickList({ onSelect: (prompt) => { selected = prompt; } });

  const option = screen.getByRole("option", { name: /修复流程/i });
  revealHoverPreview(option);
  fireEvent.click(option);

  expect(selected).toEqual(prompts[1]);
  expect(screen.queryByRole("tooltip")).toBeNull();
});
```

**Step 5: Run tests and verify they fail**

Run:

```bash
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: FAIL because `hoverResetKey` does not exist, focus still schedules hover, and click does not explicitly clear hover before selecting.

---

### Task 2: Implement Hover Reset In PromptQuickList

**Files:**
- Modify: `src/ui/PromptQuickList.tsx`
- Test: `src/ui/PromptQuickList.test.tsx`

**Step 1: Add optional reset prop**

Extend `PromptQuickListProps`.

```ts
interface PromptQuickListProps {
  prompts: PromptContainer[];
  messages: Messages["quickList"];
  groupMeta: Messages["manager"]["groupMeta"];
  onSelect: (prompt: PromptContainer) => void;
  submittingPromptId?: string | null;
  hoverResetKey?: number;
}
```

Default it in the component signature.

```ts
export function PromptQuickList({
  prompts,
  messages,
  groupMeta,
  onSelect,
  submittingPromptId = null,
  hoverResetKey = 0,
}: PromptQuickListProps) {
```

**Step 2: Clear hover state when reset key changes**

Add a reset effect. The effect must clear both visible state and pending timer.

```ts
useEffect(() => {
  hideHoverPreview();
}, [hoverResetKey]);
```

The existing `hideHoverPreview()` already calls:

```ts
clearHoverPreviewTimer();
hoverPreviewAnchorRef.current = null;
setHoverPreview(null);
```

**Step 3: Remove focus-triggered hover scheduling**

Remove this behavior from prompt options:

```tsx
onFocus={(event) => scheduleHoverPreview(prompt, event.currentTarget)}
```

Keep blur as a cleanup path:

```tsx
onBlur={hideHoverPreview}
```

**Step 4: Clear hover before prompt selection**

Add a small local click handler.

```ts
function selectPrompt(prompt: PromptContainer) {
  hideHoverPreview();
  onSelect(prompt);
}
```

Use it in the item button:

```tsx
onClick={() => selectPrompt(prompt)}
```

**Step 5: Run focused tests**

Run:

```bash
npm test -- --run src/ui/PromptQuickList.test.tsx
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src/ui/PromptQuickList.tsx src/ui/PromptQuickList.test.tsx
git commit -m "fix: reset prompt hover preview state"
```

---

### Task 3: Wire Popover Lifecycle Reset From App

**Files:**
- Modify: `src/App.tsx`
- Test: `src/app/App.test.tsx`

**Step 1: Add failing App-level test for popover open reset**

Mock or observe `PromptQuickList` props so the test can verify that `hoverResetKey` changes after `prompt-popover-opened`.

Suggested test shape:

```tsx
it("resets prompt hover state when the popover opens", async () => {
  currentWindowLabel = "prompt-popover";
  const { readTextFile } = await import("@tauri-apps/plugin-fs");
  (readTextFile as ReturnType<typeof vi.fn>)
    .mockResolvedValueOnce(JSON.stringify({ version: 1, prompts: mockPrompts }))
    .mockResolvedValueOnce(JSON.stringify({ version: 1, blacklistedApps: [] }))
    .mockResolvedValueOnce(JSON.stringify({ version: 1, prompts: mockPrompts }));

  await act(async () => {
    render(<App />);
  });

  const before = getLastPromptQuickListResetKey();

  await act(async () => {
    await eventHandlers.get("prompt-popover-opened")?.({ payload: "popover" });
  });

  expect(getLastPromptQuickListResetKey()).toBeGreaterThan(before);
});
```

Implementation note: this test can use a module mock for `../ui/PromptQuickList` that records the last `hoverResetKey` and renders a minimal placeholder. If the current App tests do not want component mocking, use a lower-level DOM test only if it stays simple.

**Step 2: Add failing App-level test for popover dismissed reset**

The stale state can also survive if the user closes the list without selecting a prompt.

```tsx
it("resets prompt hover state when the popover is dismissed", async () => {
  currentWindowLabel = "prompt-popover";
  await act(async () => {
    render(<App />);
  });

  const before = getLastPromptQuickListResetKey();

  await act(async () => {
    await eventHandlers.get("prompt-popover-dismissed")?.({ payload: undefined });
  });

  expect(getLastPromptQuickListResetKey()).toBeGreaterThan(before);
});
```

Expected: FAIL because `App` currently listens for `prompt-popover-opened` only and does not pass a reset key to `PromptQuickList`.

**Step 3: Add reset state in App**

In `App`, add state near other UI state:

```ts
const [hoverResetKey, setHoverResetKey] = useState(0);
const resetPromptHoverPreview = useCallback(() => {
  setHoverResetKey((key) => key + 1);
}, []);
```

**Step 4: Reset on popover open**

Inside the existing `prompt-popover-opened` listener, reset before reloading prompts so stale UI disappears immediately.

```ts
listen<string>("prompt-popover-opened", async (event) => {
  if (!active || event.payload !== "popover") return;
  resetPromptHoverPreview();
  promptListRefreshingRef.current = true;
  try {
    await reloadPrompts();
  } finally {
    promptListRefreshingRef.current = false;
  }
})
```

**Step 5: Listen for popover dismissed**

Add a second listener in the same effect or a neighboring effect:

```ts
listen("prompt-popover-dismissed", () => {
  if (!active || currentWindowLabel() !== "prompt-popover") return;
  resetPromptHoverPreview();
})
```

This clears state when the list is hidden without selection.

**Step 6: Pass reset key to PromptQuickList**

```tsx
<PromptQuickList
  prompts={prompts}
  messages={t.quickList}
  groupMeta={t.manager.groupMeta}
  onSelect={handleSelect}
  submittingPromptId={submittingPromptId}
  hoverResetKey={hoverResetKey}
/>
```

**Step 7: Run focused App tests**

Run:

```bash
npm test -- --run src/app/App.test.tsx
```

Expected: PASS.

**Step 8: Commit**

```bash
git add src/App.tsx src/app/App.test.tsx
git commit -m "fix: reset hover preview on popover lifecycle"
```

---

### Task 4: Verify End-To-End Behavior

**Files:**
- No code changes expected.

**Step 1: Run focused quick-list tests**

Run:

```bash
npm test -- --run src/ui/PromptQuickList.test.tsx src/app/App.test.tsx
```

Expected: PASS.

**Step 2: Run full frontend tests**

Run:

```bash
npm test -- --run
```

Expected: PASS.

**Step 3: Run frontend build**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 4: Run Rust tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

**Step 5: Manual product check**

Run:

```bash
npm run tauri dev
```

Check:
- Click Calico to open the prompt list.
- Expected: no hover detail panel appears immediately.
- Move the mouse away from all prompt containers and reopen.
- Expected: no hover detail panel appears immediately.
- Hover a prompt container for less than 1.5 seconds.
- Expected: no hover detail panel appears.
- Hover a prompt container for 1.5 seconds.
- Expected: detail panel appears above that prompt container.
- Close the list while the detail panel is visible, then reopen.
- Expected: the detail panel does not reappear until a fresh hover occurs.
- Click a prompt while the detail panel is visible.
- Expected: the detail panel disappears immediately, then the prompt action continues.

**Step 6: Commit verification-only fixes if needed**

Only if verification exposes a bug:

```bash
git add <files>
git commit -m "fix: address hover preview lifecycle verification"
```

---

### Task 5: Final Review And Push

**Files:**
- Review: `src/ui/PromptQuickList.tsx`
- Review: `src/ui/PromptQuickList.test.tsx`
- Review: `src/App.tsx`
- Review: `src/app/App.test.tsx`

**Step 1: Inspect final diff**

Run:

```bash
git diff --stat HEAD~2..HEAD
git diff HEAD~2..HEAD -- src/ui/PromptQuickList.tsx src/ui/PromptQuickList.test.tsx src/App.tsx src/app/App.test.tsx
```

Expected:
- Changes are scoped to hover-preview lifecycle.
- No visual redesign.
- No prompt autosend logic changes.
- No Rust window logic changes unless verification proves necessary.

**Step 2: Confirm working tree**

Run:

```bash
git status --short
```

Expected:
- Only unrelated generated/cache artifacts may remain dirty.
- Source changes for this task are committed.

**Step 3: Push to GitHub**

Run:

```bash
git push origin main
```

Expected: push succeeds.

**Step 4: User-facing acceptance summary**

Report:
- Clicking Calico opens a clean prompt list.
- Old hover detail panels no longer carry over from the previous open.
- Hover detail appears only after a fresh 1.5-second hover on a prompt container.
- Selecting a prompt clears hover UI before running the prompt action.

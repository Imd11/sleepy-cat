# Prompt Manager Sidebar Scroll Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish the prompt manager so the left category area behaves like a clean vertical tab list, the prompt creation action has appropriate visual weight, and only the prompt list scrolls in normal desktop use.

**Architecture:** Keep the existing prompt category data model and manager page structure. Refine `CategoryRail` into a focused navigation component with per-category overflow actions and inline editing, then adjust manager layout CSS so the page frame is stable and `.prompt-list` owns scrolling. Do not redesign the right-side panels beyond reducing the `Add Prompt` button's visual weight.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, CSS, Tauri app shell.

---

## Scope

This plan implements the UX decisions from the latest discussion:

1. `Rename category` and `Delete category` move into each category row's `⋯` menu.
2. `+ New` becomes a simple plus row below the category tabs, not a large dashed button and not a title-bar button.
3. Clicking `+` inserts a new inline category tab before the plus row; the default name is selected so the user can type over it immediately.
4. Rename uses the same inline edit pattern as create.
5. Delete requires confirmation and stays scoped to the selected category row.
6. The large full-width `Add Prompt` button becomes a normal-sized action aligned to the form's lower right.
7. The manager page becomes a fixed work surface where only the prompt list scrolls.
8. The manager scroll lock is scoped to the manager page only; Settings and other `app-window-main` pages keep their existing scroll behavior.

This plan does not:

1. Change prompt/category storage format.
2. Change import/export behavior.
3. Change prompt item row actions except preserving their current behavior.
4. Redesign settings, the main header, or the prompt editor fields.
5. Change the floating picker popover UI.

## Acceptance Criteria

1. Left category rail no longer shows permanent `Rename category` / `Delete category` buttons.
2. Each category row has a per-row overflow menu with Rename and Delete actions.
3. Clicking `+` below the category rows inserts an inline create row with a prefilled unique localized name selected.
4. Enter saves inline create/rename; Escape cancels; IME composition Enter does not submit.
5. Delete shows a confirmation state before calling `onDelete`.
6. Non-empty category delete errors still display through existing `categoryActionError`.
7. `Add Prompt` and `Add Group` are normal-width form actions aligned to the right, not full-width bars.
8. On normal desktop sizes, the manager header/category rail/create panel stay stable while `.prompt-list` is the scroll container.
9. Settings and other non-manager pages using `.app-window-main` are not changed to `overflow: hidden`.
10. Inline create/rename uses localized strings correctly: the input label and the default category name are separate messages.
11. Clicking Escape or Cancel in inline edit never accidentally submits from the following blur event.
12. Existing prompt create/edit/delete/reorder/import/export behavior still passes tests.
13. Verification passes:
    - `npm test -- src/ui/CategoryRail.test.tsx`
    - `npm test -- src/ui/PromptManager.test.tsx`
    - `npm test`
    - `npm run build`

---

## Task 0: Commit This Plan

**Files:**
- Add: `docs/plans/2026-07-05-prompt-manager-sidebar-scroll-polish.md`

**Step 1: Verify the plan file is pending**

Run:

```bash
git status --short docs/plans/2026-07-05-prompt-manager-sidebar-scroll-polish.md
```

Expected:

```text
?? docs/plans/2026-07-05-prompt-manager-sidebar-scroll-polish.md
```

**Step 2: Commit the plan**

Run:

```bash
git add docs/plans/2026-07-05-prompt-manager-sidebar-scroll-polish.md
git commit -m "docs: plan prompt manager sidebar scroll polish"
```

Expected: Commit succeeds.

---

## Task 1: Redesign CategoryRail Interactions With Tests

**Files:**
- Modify: `src/ui/CategoryRail.tsx`
- Test: `src/ui/CategoryRail.test.tsx`
- Modify: `src/shared/i18n.ts`

**Context:**

Current `CategoryRail` renders category rows, then a large `+ New` button, then permanent `Rename category` and `Delete category` buttons. That makes the rail look like a management panel rather than a vertical tab list.

Target structure:

```text
Categories

开发代码       13  ⋯
写作           4  ⋯
+
```

Create state:

```text
开发代码       13  ⋯
写作           4  ⋯
[New category 2]   <- selected text
+
```

Row menu:

```text
开发代码       13  ⋯
                  ┌────────┐
                  │ Rename │
                  │ Delete │
                  └────────┘
```

Delete confirmation:

```text
开发代码       13
Delete this category?
[Cancel] [Delete]
```

**Step 1: Add failing tests for the new category rail shape**

Update `src/ui/CategoryRail.test.tsx`.

Replace the old create test and add coverage for hidden permanent actions:

```tsx
it("renders category tabs with a compact add row and no permanent action buttons", () => {
  renderRail();

  expect(screen.getByRole("button", { name: /开发代码.*13/ })).toBeTruthy();
  expect(screen.getByRole("button", { name: /写作.*4/ })).toBeTruthy();
  expect(screen.getByRole("button", { name: "New category" })).toHaveTextContent("+");
  expect(screen.queryByRole("button", { name: "Rename category" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Delete category" })).toBeNull();
});
```

Add a test for inline create:

```tsx
it("creates a category from an inline tab with preselected default text", () => {
  const onCreate = vi.fn();
  renderRail({ onCreate });

  fireEvent.click(screen.getByRole("button", { name: "New category" }));

  const input = screen.getByRole("textbox", { name: /Category name/ }) as HTMLInputElement;
  expect(input.value).toBe("New category");
  expect(input.selectionStart).toBe(0);
  expect(input.selectionEnd).toBe(input.value.length);

  fireEvent.change(input, { target: { value: "运营" } });
  fireEvent.keyDown(input, { key: "Enter" });

  expect(onCreate).toHaveBeenCalledWith("运营");
});
```

Add a test for unique default names:

```tsx
it("prefills a unique category name when the default already exists", () => {
  renderRail({
    categories: [
      ...categories,
      { id: "cat-new", name: "New category", order: 2, createdAt: "", updatedAt: "" },
    ],
    counts: { "cat-dev": 13, "cat-writing": 4, "cat-new": 0 },
  });

  fireEvent.click(screen.getByRole("button", { name: "New category" }));

  expect(screen.getByRole("textbox", { name: /Category name/ })).toHaveValue("New category 2");
});
```

Add tests for Escape and IME behavior:

```tsx
it("cancels inline category creation with Escape", () => {
  const onCreate = vi.fn();
  renderRail({ onCreate });

  fireEvent.click(screen.getByRole("button", { name: "New category" }));
  const input = screen.getByRole("textbox", { name: /Category name/ });
  fireEvent.keyDown(input, { key: "Escape" });
  fireEvent.blur(input);

  expect(onCreate).not.toHaveBeenCalled();
  expect(screen.queryByRole("textbox", { name: /Category name/ })).toBeNull();
});

it("does not submit inline creation while Chinese IME composition is active", () => {
  const onCreate = vi.fn();
  renderRail({ onCreate });

  fireEvent.click(screen.getByRole("button", { name: "New category" }));
  const input = screen.getByRole("textbox", { name: /Category name/ });

  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "yun" } });
  fireEvent.keyDown(input, { key: "Enter" });

  expect(onCreate).not.toHaveBeenCalled();
});
```

Add tests for row menu rename/delete:

```tsx
it("renames a category from the row overflow menu", () => {
  const onRename = vi.fn();
  renderRail({ onRename });

  fireEvent.click(screen.getByRole("button", { name: /开发代码 的更多操作|More actions for 开发代码/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Rename category" }));

  const input = screen.getByRole("textbox", { name: /Category name/ }) as HTMLInputElement;
  expect(input.value).toBe("开发代码");
  expect(input.selectionStart).toBe(0);
  expect(input.selectionEnd).toBe(input.value.length);

  fireEvent.change(input, { target: { value: "研发" } });
  fireEvent.keyDown(input, { key: "Enter" });

  expect(onRename).toHaveBeenCalledWith("cat-dev", "研发");
});

it("confirms delete from the row overflow menu before calling onDelete", () => {
  const onDelete = vi.fn();
  renderRail({ onDelete });

  fireEvent.click(screen.getByRole("button", { name: /开发代码 的更多操作|More actions for 开发代码/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Delete category" }));

  expect(onDelete).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Delete category" }));

  expect(onDelete).toHaveBeenCalledWith("cat-dev");
});

it("closes the row menu with Escape", () => {
  renderRail();

  fireEvent.click(screen.getByRole("button", { name: /开发代码 的更多操作|More actions for 开发代码/ }));
  expect(screen.getByRole("menu")).toBeTruthy();

  fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

  expect(screen.queryByRole("menu")).toBeNull();
});
```

**Step 2: Run the focused failing tests**

Run:

```bash
npm test -- src/ui/CategoryRail.test.tsx
```

Expected: FAIL because the compact add row, row overflow menu, selected default text, and delete confirmation are not implemented yet.

**Step 3: Implement the minimal CategoryRail behavior**

Modify `src/ui/CategoryRail.tsx`.

Implementation details:

- Replace `EditMode` with:

```ts
type EditMode =
  | { kind: "idle" }
  | { kind: "create"; value: string }
  | { kind: "rename"; categoryId: string; value: string };
```

Keep this shape if already present, but change how it is entered/rendered.

- Extend `CategoryRailMessages` and `src/shared/i18n.ts` before using new labels:

```ts
export type CategoryRailMessages = {
  title: string;
  newCategory: string;
  newCategoryName: string;
  newCategoryDefaultName: string;
  categoryActions: (name: string) => string;
  renameCategory: string;
  deleteCategory: string;
  saveCategory: string;
  cancelCategory: string;
};
```

Use these message values:

```ts
// zh-CN manager
newCategoryDefaultName: "新分类",
categoryActions: (name: string) => `${name} 的更多操作`,

// en-US manager
newCategoryDefaultName: "New category",
categoryActions: (name: string) => `More actions for ${name}`,
```

Keep `newCategoryName` as the input aria label only. Do not use it as the prefilled category name.

- Add local state:

```ts
const [menuCategoryId, setMenuCategoryId] = useState<string | null>(null);
const [deleteConfirmCategoryId, setDeleteConfirmCategoryId] = useState<string | null>(null);
const inputRef = useRef<HTMLInputElement | null>(null);
const suppressNextBlurSubmitRef = useRef(false);
```

- Add helper functions inside the component:

```ts
const uniqueCategoryName = (baseName: string, ignoredCategoryId?: string) => {
  const usedNames = new Set(
    categories
      .filter((category) => category.id !== ignoredCategoryId)
      .map((category) => displayName(category).trim())
  );
  if (!usedNames.has(baseName)) return baseName;
  let suffix = 2;
  while (usedNames.has(`${baseName} ${suffix}`)) {
    suffix += 1;
  }
  return `${baseName} ${suffix}`;
};

const beginCreate = () => {
  suppressNextBlurSubmitRef.current = false;
  setMenuCategoryId(null);
  setDeleteConfirmCategoryId(null);
  setEditMode({
    kind: "create",
    value: uniqueCategoryName(messages.newCategoryDefaultName),
  });
};

const beginRename = (category: PromptCategory) => {
  suppressNextBlurSubmitRef.current = false;
  setMenuCategoryId(null);
  setDeleteConfirmCategoryId(null);
  setEditMode({
    kind: "rename",
    categoryId: category.id,
    value: displayName(category),
  });
};
```

- Focus and select the input when entering create or rename:

```ts
useEffect(() => {
  if (editMode.kind === "idle") return;
  window.requestAnimationFrame(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  });
}, [editMode]);
```

- Add cancel and blur helpers so Escape/Cancel do not accidentally save from the blur event that follows:

```ts
const cancelEdit = () => {
  suppressNextBlurSubmitRef.current = true;
  setEditMode({ kind: "idle" });
};

const handleInputBlur = () => {
  if (suppressNextBlurSubmitRef.current) {
    suppressNextBlurSubmitRef.current = false;
    return;
  }
  submit();
};
```

- Update `submit`:

```ts
const submit = () => {
  if (editMode.kind === "idle") return;
  const value = editMode.value.trim();
  if (!value) {
    setEditMode({ kind: "idle" });
    return;
  }
  if (editMode.kind === "create") {
    void Promise.resolve(onCreate(uniqueCategoryName(value))).then(() => {
      setEditMode({ kind: "idle" });
    });
    return;
  }
  void Promise.resolve(onRename(
    editMode.categoryId,
    uniqueCategoryName(value, editMode.categoryId)
  )).then(() => {
    setEditMode({ kind: "idle" });
  });
};
```

- Update `handleKeyDown`:

```ts
if (event.key === "Escape") {
  event.preventDefault();
  cancelEdit();
  return;
}
if (event.key !== "Enter" || composingRef.current) return;
event.preventDefault();
submit();
```

- Render category rows as:

```tsx
<div className={`category-rail-row ${category.id === activeCategoryId ? "is-active" : ""}`}>
  <button
    className="category-rail-item"
    type="button"
    aria-current={category.id === activeCategoryId ? "true" : undefined}
    onClick={() => onSelect(category.id)}
  >
    <span>{displayName(category)}</span>
    <span>{counts[category.id] ?? 0}</span>
  </button>
  <button
    aria-label={messages.categoryActions(displayName(category))}
    className="category-rail-menu-trigger"
    type="button"
    onClick={(event) => {
      event.stopPropagation();
      setMenuCategoryId(menuCategoryId === category.id ? null : category.id);
      setDeleteConfirmCategoryId(null);
    }}
  >
    {"⋯"}
  </button>
  {menuCategoryId === category.id ? (
    <div
      className="category-rail-menu"
      role="menu"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          setMenuCategoryId(null);
        }
      }}
    >
      <button role="menuitem" type="button" onClick={() => beginRename(category)}>
        {messages.renameCategory}
      </button>
      <button
        role="menuitem"
        type="button"
        className="is-danger"
        onClick={() => {
          setMenuCategoryId(null);
          setDeleteConfirmCategoryId(category.id);
        }}
      >
        {messages.deleteCategory}
      </button>
    </div>
  ) : null}
</div>
```

- Render inline edit rows for create/rename:

```tsx
{editMode.kind !== "idle" ? (
  <div className="category-rail-edit-row">
    <input
      ref={inputRef}
      className="field category-rail-input"
      aria-label={messages.newCategoryName}
      value={editMode.value}
      onBlur={handleInputBlur}
      onChange={(event) => setEditMode({ ...editMode, value: event.target.value })}
      onCompositionStart={() => { composingRef.current = true; }}
      onCompositionEnd={() => { composingRef.current = false; }}
      onKeyDown={handleKeyDown}
    />
  </div>
) : null}
```

- Render compact plus row below categories/edit row:

```tsx
<button
  className="category-rail-add"
  type="button"
  aria-label={messages.newCategoryDefaultName}
  onClick={beginCreate}
>
  +
</button>
```

- Render delete confirmation near the target row:

```tsx
{deleteConfirmCategoryId === category.id ? (
  <div className="category-rail-delete-confirm" role="alert">
    <span>{messages.deleteCategory}?</span>
    <button type="button" className="button button-secondary" onClick={() => setDeleteConfirmCategoryId(null)}>
      {messages.cancelCategory}
    </button>
    <button
      type="button"
      className="button button-ghost-danger"
      onClick={() => {
        onDelete(category.id);
        setDeleteConfirmCategoryId(null);
      }}
    >
      {messages.deleteCategory}
    </button>
  </div>
) : null}
```

**Step 4: Run CategoryRail tests**

Run:

```bash
npm test -- src/ui/CategoryRail.test.tsx
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add src/ui/CategoryRail.tsx src/ui/CategoryRail.test.tsx src/shared/i18n.ts
git commit -m "feat: refine category rail actions"
```

Expected: Commit succeeds.

---

## Task 2: Adjust Category Rail Styling

**Files:**
- Modify: `src/styles.css`
- Test: `src/ui/CategoryRailStyles.test.ts`

**Context:**

The new markup needs to look like a vertical tab list. The rail should not show large management buttons. The plus row should be compact and live under the category tabs.

**Step 1: Add a failing CSS structure test**

Create `src/ui/CategoryRailStyles.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import css from "../styles.css?raw";

function ruleBody(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "m").exec(css);
  return match?.[1] ?? "";
}

describe("category rail styles", () => {
  it("uses compact vertical tab rows with inline menus", () => {
    expect(ruleBody(".category-rail-row")).toContain("position: relative");
    expect(ruleBody(".category-rail-menu")).toContain("position: absolute");
    expect(ruleBody(".category-rail-add")).toContain("min-height: 34px");
    expect(ruleBody(".category-rail-actions")).toBe("");
  });
});
```

**Step 2: Run the failing style test**

Run:

```bash
npm test -- src/ui/CategoryRailStyles.test.ts
```

Expected: FAIL because the new selectors do not exist yet and `.category-rail-actions` still exists.

**Step 3: Implement compact category rail CSS**

Modify `src/styles.css`.

Replace the old `.category-rail-item`, `.category-rail-new`, `.category-rail-editor`, `.category-rail-editor-actions`, and `.category-rail-actions` blocks with:

```css
.category-rail-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.category-rail-row {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 28px;
  align-items: center;
  border-radius: 7px;
}

.category-rail-row.is-active {
  background: #eef4ff;
}

.category-rail-item {
  display: flex;
  width: 100%;
  min-height: 34px;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 8px;
  color: var(--pp-muted);
  background: transparent;
  border: 0;
  border-radius: 7px 0 0 7px;
  cursor: pointer;
  font-size: 12px;
  text-align: left;
}

.category-rail-row.is-active .category-rail-item {
  color: var(--pp-text);
  font-weight: 700;
}

.category-rail-item span:first-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.category-rail-menu-trigger {
  display: inline-flex;
  width: 26px;
  min-height: 30px;
  align-items: center;
  justify-content: center;
  color: var(--pp-muted);
  background: transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
  opacity: 0;
}

.category-rail-row:hover .category-rail-menu-trigger,
.category-rail-row:focus-within .category-rail-menu-trigger,
.category-rail-row.is-active .category-rail-menu-trigger {
  opacity: 1;
}

.category-rail-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 10;
  display: grid;
  min-width: 118px;
  padding: 4px;
  background: var(--pp-surface);
  border: 1px solid var(--pp-border);
  border-radius: var(--pp-radius-md);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.12);
}

.category-rail-menu button {
  min-height: 28px;
  padding: 0 8px;
  color: #344054;
  background: transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 620;
  text-align: left;
}

.category-rail-menu button:hover {
  background: var(--pp-surface-subtle);
}

.category-rail-menu button.is-danger {
  color: var(--pp-danger);
}

.category-rail-edit-row {
  display: flex;
  min-height: 34px;
}

.category-rail-input {
  min-height: 32px;
  padding: 0 8px;
  font-size: 12px;
}

.category-rail-add {
  display: flex;
  width: 100%;
  min-height: 34px;
  align-items: center;
  justify-content: center;
  color: var(--pp-muted);
  background: transparent;
  border: 0;
  border-radius: 7px;
  cursor: pointer;
  font-size: 18px;
  font-weight: 700;
}

.category-rail-add:hover {
  background: var(--pp-surface-subtle);
  color: var(--pp-text);
}

.category-rail-delete-confirm {
  grid-column: 1 / -1;
  display: grid;
  grid-template-columns: 1fr auto auto;
  gap: 6px;
  align-items: center;
  padding: 8px;
  color: var(--pp-danger);
  background: #fff7f7;
  border: 1px solid #f0d1cc;
  border-radius: 7px;
  font-size: 11px;
  font-weight: 620;
}
```

Keep `.category-rail-error` because it is still used for non-empty category delete failures.

**Step 4: Run the style test**

Run:

```bash
npm test -- src/ui/CategoryRailStyles.test.ts
```

Expected: PASS.

**Step 5: Run CategoryRail tests again**

Run:

```bash
npm test -- src/ui/CategoryRail.test.tsx
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/styles.css src/ui/CategoryRailStyles.test.ts
git commit -m "style: polish category rail layout"
```

Expected: Commit succeeds.

---

## Task 3: Make Add Prompt A Normal Right-Aligned Action

**Files:**
- Modify: `src/ui/PromptManager.tsx`
- Modify: `src/styles.css`
- Test: `src/ui/PromptManager.test.tsx`

**Context:**

The current `Add Prompt` button is visually too heavy because it spans the full form width. It should remain the primary create action, but it should look like a normal desktop form action aligned to the right.

Target:

```text
Title
Prompt body...

                                      [Add Prompt]
```

**Step 1: Add a failing PromptManager test for the action row**

Update `src/ui/PromptManager.test.tsx`:

```tsx
it("renders the create action in a right-aligned form action row", () => {
  renderManager();

  const addButton = screen.getByRole("button", { name: "添加提示词" });
  const actionRow = addButton.closest(".editor-submit-row");

  expect(actionRow).toBeTruthy();
  expect(actionRow?.textContent).toContain("添加提示词");
});
```

**Step 2: Run the failing PromptManager test**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
```

Expected: FAIL because `.editor-submit-row` does not exist yet.

**Step 3: Wrap the submit button in a form action row**

Modify `src/ui/PromptManager.tsx`.

Replace:

```tsx
<button
  className="button button-primary editor-submit"
  type="submit"
  onPointerDown={(event) => event.preventDefault()}
  onPointerUp={() => runSubmitOnce(() => handleCreate(draftFromCreateDom()))}
>
  {draft.type === "group" ? messages.manager.addGroup : messages.manager.addPrompt}
</button>
```

with:

```tsx
<div className="editor-submit-row">
  <button
    className="button button-primary editor-submit"
    type="submit"
    onPointerDown={(event) => event.preventDefault()}
    onPointerUp={() => runSubmitOnce(() => handleCreate(draftFromCreateDom()))}
  >
    {draft.type === "group" ? messages.manager.addGroup : messages.manager.addPrompt}
  </button>
</div>
```

Do not add `disabled={!hasValidDraft(draft)}` in this task. Existing tests protect first-click submission while a focused field has newer DOM text than React state, and this visual-only change should not alter submit eligibility.

**Step 4: Add CSS for the right-aligned action**

Modify `src/styles.css`:

```css
.editor-submit-row {
  display: flex;
  justify-content: flex-end;
}

.prompt-manager .editor-submit {
  width: auto;
  min-width: 118px;
  padding-inline: 16px;
}
```

Ensure there is no manager-specific style making `.editor-submit` full width.

**Step 5: Run PromptManager tests**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 6: Commit**

Run:

```bash
git add src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/styles.css
git commit -m "style: right align prompt create action"
```

Expected: Commit succeeds.

---

## Task 4: Make Only Prompt List Scroll In Manager

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Test: `src/ui/PromptManagerLayoutStyles.test.ts`

**Context:**

The manager page currently lets the main page scroll because the shared `.app-window-main` container is `overflow: auto`, while `.prompt-list` is not a scroll container. For the manager page only, the stable work surface should keep the header, category rail, and create form in place while the prompt list owns scrolling.

Do not change base `.app-window-main` to `overflow: hidden`. That class is also used by Settings and other windows. Add a manager-only class in `src/App.tsx` and scope the scroll lock to that class.

**Step 1: Add a failing CSS layout test**

Create `src/ui/PromptManagerLayoutStyles.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import css from "../styles.css?raw";

function ruleBody(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "m").exec(css);
  return match?.[1] ?? "";
}

describe("prompt manager layout styles", () => {
  it("keeps the manager frame stable and makes the prompt list scroll", () => {
    expect(ruleBody(".app-window-main")).not.toContain("overflow: hidden");
    expect(ruleBody(".app-window-main.app-window-manager")).toContain("overflow: hidden");
    expect(ruleBody(".prompt-manager")).toContain("height:");
    expect(ruleBody(".prompt-manager-content")).toContain("min-height: 0");
    expect(ruleBody(".prompt-manager .list-panel")).toContain("min-height: 0");
    expect(ruleBody(".prompt-manager .prompt-list")).toContain("overflow-y: auto");
    expect(ruleBody(".prompt-manager .prompt-list")).toContain("overscroll-behavior: contain");
  });
});
```

**Step 2: Run the failing layout style test**

Run:

```bash
npm test -- src/ui/PromptManagerLayoutStyles.test.ts
```

Expected: FAIL because the current manager list does not own scrolling.

**Step 3: Add a manager-only window class**

Modify the manager branch in `src/App.tsx`.

Replace:

```tsx
<div className="app-window app-window-main">
```

with:

```tsx
<div className="app-window app-window-main app-window-manager">
```

Only do this in the `mode === "manager"` branch. Do not add `app-window-manager` to the Settings branch.

**Step 4: Implement stable manager frame CSS**

Modify the manager-specific section of `src/styles.css`.

Keep the shared `.app-window-main` rule scrollable. Add or change the manager-specific rule to:

```css
.app-window-main.app-window-manager {
  height: 100vh;
  min-height: 0;
  overflow: hidden;
  padding: 22px 24px 30px;
}
```

Change `.prompt-manager` to:

```css
.prompt-manager {
  width: min(760px, 100%);
  height: 100%;
  min-height: 0;
  margin: 0 auto;
  gap: 14px;
}
```

Change `.prompt-manager-body` to:

```css
.prompt-manager-body {
  display: grid;
  grid-template-columns: 172px minmax(0, 1fr);
  min-height: 0;
  flex: 1;
  gap: 14px;
  align-items: start;
}
```

Change `.prompt-manager-content` to:

```css
.prompt-manager-content {
  display: flex;
  min-width: 0;
  min-height: 0;
  height: 100%;
  flex-direction: column;
  gap: 14px;
}
```

Change `.prompt-manager .list-panel` to:

```css
.prompt-manager .list-panel {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  padding: 14px;
}
```

Change `.prompt-manager .prompt-list` to:

```css
.prompt-manager .prompt-list {
  min-height: 0;
  flex: 1;
  gap: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  margin-top: 10px;
  background: var(--pp-surface);
  border: 1px solid var(--pp-border);
  border-radius: var(--pp-radius-sm);
}
```

Keep existing responsive media query behavior for narrow viewports. In the mobile media query, allow the page to scroll normally:

```css
@media (max-width: 720px) {
  .app-window-main.app-window-manager {
    height: auto;
    min-height: 100vh;
    overflow: auto;
  }

  .prompt-manager {
    height: auto;
  }
}
```

**Step 5: Run the layout style test**

Run:

```bash
npm test -- src/ui/PromptManagerLayoutStyles.test.ts
```

Expected: PASS.

**Step 6: Run PromptManager tests**

Run:

```bash
npm test -- src/ui/PromptManager.test.tsx
```

Expected: PASS.

**Step 7: Commit**

Run:

```bash
git add src/App.tsx src/styles.css src/ui/PromptManagerLayoutStyles.test.ts
git commit -m "style: constrain manager scrolling to prompt list"
```

Expected: Commit succeeds.

---

## Task 5: Integration Coverage For App-Level Category Flow

**Files:**
- Test: `src/app/App.test.tsx`
- Modify only if needed: `src/App.tsx`

**Context:**

App-level category flow already exists. Because the category rail markup changes, the app tests should assert that creating a category from the new compact plus row still creates and selects it, and deleting a non-empty category still surfaces an error.

**Step 1: Update app tests that rely on old `+ New` or permanent buttons**

Find current tests:

```bash
rg -n "creates a category from the manager rail|deleting a non-empty category|Rename category|Delete category|\\+ New|New category" src/app/App.test.tsx src/ui/CategoryRail.test.tsx
```

Update old selectors:

- Replace `screen.getByRole("button", { name: /\+ New/ })`
- With `screen.getByRole("button", { name: "New category" })` in English tests, or `screen.getByRole("button", { name: "新分类" })` in Chinese tests.

For delete tests, open the row overflow menu:

```tsx
fireEvent.click(screen.getByRole("button", { name: /开发代码 的更多操作|More actions for 开发代码/ }));
fireEvent.click(screen.getByRole("menuitem", { name: "删除分类" }));
fireEvent.click(screen.getByRole("button", { name: "删除分类" }));
```

Use the actual localized message labels from `getMessages("zh-CN")`.

**Step 2: Run affected app tests**

Run:

```bash
npm test -- src/app/App.test.tsx
```

Expected: PASS. If failures appear because labels are language-specific, update tests to match the current `getMessages("zh-CN")` or `getMessages("en-US")` text rather than hard-coding stale English labels.

**Step 3: Commit if tests changed**

If `src/app/App.test.tsx` changed, run:

```bash
git add src/app/App.test.tsx src/App.tsx
git commit -m "test: cover updated category manager flow"
```

If no files changed, do not create an empty commit.

---

## Task 6: Final Verification

**Files:**
- No source file edits expected unless verification exposes a defect.

**Step 1: Run focused tests**

Run:

```bash
npm test -- src/ui/CategoryRail.test.tsx src/ui/CategoryRailStyles.test.ts src/ui/PromptManager.test.tsx src/ui/PromptManagerLayoutStyles.test.ts src/app/App.test.tsx
```

Expected: PASS.

**Step 2: Run full frontend tests**

Run:

```bash
npm test
```

Expected: PASS.

**Step 3: Run production build**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 4: Review task diff**

Run:

```bash
git status --short
git diff --stat
git diff --name-only -- src/ui/CategoryRail.tsx src/ui/CategoryRail.test.tsx src/ui/CategoryRailStyles.test.ts src/ui/PromptManager.tsx src/ui/PromptManager.test.tsx src/ui/PromptManagerLayoutStyles.test.ts src/styles.css src/shared/i18n.ts src/app/App.test.tsx docs/plans/2026-07-05-prompt-manager-sidebar-scroll-polish.md
```

Expected:

- Only planned files are changed.
- No generated files are staged.
- No unrelated app logic is changed.

**Step 5: Manual smoke checklist before completion**

Open the app in dev or built mode and verify visually:

```bash
npm run dev
```

Manual checks:

- Category rail shows category rows, row counts, per-row `⋯`, and compact `+`.
- `+` inserts a new selected inline category name before the `+`.
- Typing a category name and pressing Enter saves and selects it.
- `⋯ > Rename` edits the clicked category inline.
- `⋯ > Delete` requires confirmation.
- Large permanent `Rename category` and `Delete category` buttons are gone.
- `Add Prompt` is right-aligned and no longer full width.
- Scrolling over the prompt list scrolls only the list.
- Scrolling outside the prompt list does not move the normal desktop manager frame.

If a dev server is already running, use its existing URL rather than starting another server.

**Step 6: Commit any final fixes**

If verification required small fixes:

```bash
git add <changed-files>
git commit -m "fix: stabilize prompt manager polish"
```

Expected: Commit succeeds.

---

## Review Checklist Before Acceptance

Before marking complete, confirm:

1. Category rail is a vertical tab list, not a button stack.
2. Add category appears as a compact plus below category tabs.
3. Inline create and rename select the default/current text.
4. Row `⋯` menu owns Rename/Delete.
5. Delete requires confirmation.
6. Non-empty category deletion remains blocked by store behavior and visible error.
7. Add Prompt visual weight is reduced.
8. Prompt List is the only scrollable manager area on desktop.
9. Narrow/mobile layouts still allow the page to scroll instead of clipping controls.
10. Tests and build pass.

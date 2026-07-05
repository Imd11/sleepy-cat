import { describe, expect, it } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { PromptManager } from "./PromptManager";
import { getMessages } from "../shared/i18n";
import type { PromptCategory, PromptContainer } from "../shared/promptTypes";

describe("prompt manager", () => {
  const mockPrompts: PromptContainer[] = [
    {
      id: "1",
      categoryId: "category-default",
      title: "Code Review",
      type: "single",
      prompts: [{ id: "1-entry", body: "Review this code for bugs.", order: 0 }],
      intervalMs: 700,
      order: 0,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    },
    {
      id: "2",
      categoryId: "category-default",
      title: "Repair Group",
      type: "group",
      prompts: [
        { id: "2-entry-1", body: "Analyze root cause.", order: 0 },
        { id: "2-entry-2", body: "Execute the fix.", order: 1 },
      ],
      intervalMs: 700,
      order: 1,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    }
  ];

  const defaultCategory: PromptCategory = {
    id: "category-default",
    name: "Default",
    order: 0,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z",
  };

  function renderManager(overrides: Partial<Parameters<typeof PromptManager>[0]> = {}) {
    const props = {
      prompts: mockPrompts,
      categories: [defaultCategory],
      activeCategoryId: defaultCategory.id,
      categoryCounts: { [defaultCategory.id]: mockPrompts.length },
      totalPromptCount: mockPrompts.length,
      onCreate: () => {},
      onCreateGroup: () => {},
      onUpdate: () => {},
      onDelete: () => {},
      onReorder: () => {},
      onSelectCategory: () => {},
      onCreateCategory: () => {},
      onRenameCategory: () => {},
      onDeleteCategory: () => {},
      getCategoryDisplayName: (category: PromptCategory) =>
        category.id === "category-default" && category.name === "Default" ? "默认" : category.name,
      onImport: () => {},
      onExport: () => {},
      messages: getMessages("zh-CN"),
      onOpenSettings: () => {},
      ...overrides,
    };
    const result = render(<PromptManager {...props} />);
    return { props, ...result };
  }

  function makePrompt(overrides: Partial<PromptContainer>): PromptContainer {
    return {
      id: "prompt",
      categoryId: "category-default",
      title: "Prompt",
      type: "single",
      prompts: [{ id: "entry", body: "Body", order: 0 }],
      intervalMs: 700,
      order: 0,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z",
      ...overrides,
    };
  }

  function openCreatePanel(name = "+ 添加提示词") {
    fireEvent.click(screen.getByRole("button", { name }));
  }

  it("renders prompt containers with group distinction", () => {
    renderManager();

    expect(screen.getByText("Code Review")).toBeTruthy();
    expect(screen.getByText("Repair Group")).toBeTruthy();
    expect(screen.queryByText("Single · 1 prompt")).toBeNull();
    expect(screen.getByText("群组 · 2 条")).toBeTruthy();
    expect(screen.queryByText(/700ms/)).toBeNull();
  });

  it("does not render instructional manager section descriptions", () => {
    renderManager();

    expect(screen.queryByText("为快速选择器添加一个提示词或一个有顺序的提示词组。")).toBeNull();
    expect(screen.queryByText("选择小猫列表中的显示顺序。")).toBeNull();
  });

  it("keeps the create panel collapsed until adding a prompt", () => {
    renderManager();

    expect(screen.queryByRole("heading", { name: "新建提示词容器" })).toBeNull();

    openCreatePanel();

    const singleButton = screen.getByRole("button", { name: "单个" });
    const header = singleButton.closest(".panel-heading-with-actions");

    expect(header).toBeTruthy();
    expect(header?.textContent).toContain("新建提示词容器");
  });

  it("renders the create action in a right-aligned form action row", () => {
    renderManager();

    openCreatePanel();

    const addButton = screen.getByRole("button", { name: "添加提示词" });
    const actionRow = addButton.closest(".editor-submit-row");

    expect(actionRow).toBeTruthy();
    expect(actionRow?.textContent).toContain("添加提示词");
  });

  it("marks prompt container type segments with pressed state", () => {
    renderManager();

    openCreatePanel();

    expect(screen.getByRole("button", { name: "单个" }).getAttribute("aria-pressed"))
      .toBe("true");
    expect(screen.getByRole("button", { name: "群组" }).getAttribute("aria-pressed"))
      .toBe("false");

    fireEvent.click(screen.getByRole("button", { name: "群组" }));

    expect(screen.getByRole("button", { name: "单个" }).getAttribute("aria-pressed"))
      .toBe("false");
    expect(screen.getByRole("button", { name: "群组" }).getAttribute("aria-pressed"))
      .toBe("true");
  });

  it("renders prompt list as a unified row list", () => {
    renderManager();

    const list = screen.getByText("Code Review").closest(".prompt-list");

    expect(list).toBeTruthy();
    expect(list?.querySelectorAll(".prompt-item").length).toBe(2);
  });

  it("renders a left category rail with the list-first manager layout", () => {
    renderManager({
      categories: [
        { id: "cat-dev", name: "开发代码", order: 0, createdAt: "", updatedAt: "" },
        { id: "cat-writing", name: "写作", order: 1, createdAt: "", updatedAt: "" },
      ],
      activeCategoryId: "cat-dev",
      categoryCounts: { "cat-dev": 2, "cat-writing": 0 },
    });

    expect(screen.getByRole("heading", { name: "分类" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /开发代码.*2/ })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "提示词列表" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "新建提示词容器" })).toBeNull();
    openCreatePanel();
    expect(screen.getByRole("heading", { name: "新建提示词容器" })).toBeTruthy();
  });

  it("clears transient edit and delete state when the active category changes", () => {
    const categories = [
      { id: "cat-dev", name: "开发代码", order: 0, createdAt: "", updatedAt: "" },
      { id: "cat-writing", name: "写作", order: 1, createdAt: "", updatedAt: "" },
    ];
    const { rerender, props } = renderManager({
      categories,
      activeCategoryId: "cat-dev",
      categoryCounts: { "cat-dev": 1, "cat-writing": 1 },
      prompts: [makePrompt({ id: "dev-1", categoryId: "cat-dev", title: "Code Review" })],
    });

    fireEvent.click(screen.getByRole("button", { name: "编辑" }));
    expect(screen.getByDisplayValue("Code Review")).toBeTruthy();

    rerender(
      <PromptManager
        {...props}
        activeCategoryId="cat-writing"
        prompts={[makePrompt({ id: "writing-1", categoryId: "cat-writing", title: "Blog Draft" })]}
      />
    );

    expect(screen.queryByDisplayValue("Code Review")).toBeNull();
  });

  it("creates a single prompt container", async () => {
    let created: { title: string; body: string } | null = null;
    renderManager({ onCreate: (input) => { created = input; } });

    openCreatePanel();
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "New Single" },
    });
    fireEvent.change(screen.getByPlaceholderText("提示词内容..."), {
      target: { value: "Single body" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加提示词" }));

    expect(created).toEqual({ title: "New Single", body: "Single body" });
    await waitFor(() => {
      expect(screen.queryByRole("heading", { name: "新建提示词容器" })).toBeNull();
    });
  });

  it("cancels the inline create form without keeping the draft", () => {
    renderManager();

    openCreatePanel();
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "Discard me" },
    });
    fireEvent.click(screen.getByRole("button", { name: "取消" }));

    expect(screen.queryByRole("heading", { name: "新建提示词容器" })).toBeNull();

    openCreatePanel();

    expect((screen.getByPlaceholderText("标题") as HTMLInputElement).value).toBe("");
  });

  it("shows localized success feedback after creating a single prompt", async () => {
    renderManager({ messages: getMessages("en-US") });

    openCreatePanel("+ Add Prompt");
    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "New Single" },
    });
    fireEvent.change(screen.getByPlaceholderText("Prompt body..."), {
      target: { value: "Single body" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add Prompt" }));

    expect(await screen.findByText("Prompt added")).toBeTruthy();
  });

  it("creates a single prompt on the first click while the body field is focused", () => {
    let created: { title: string; body: string } | null = null;
    renderManager({ onCreate: (input) => { created = input; } });

    openCreatePanel();
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "审阅修复计划" },
    });
    const bodyField = screen.getByPlaceholderText("提示词内容...");
    fireEvent.change(bodyField, {
      target: { value: "你深入分析一下..." },
    });
    bodyField.focus();
    fireEvent.pointerDown(screen.getByRole("button", { name: "添加提示词" }));
    fireEvent.pointerUp(screen.getByRole("button", { name: "添加提示词" }));
    fireEvent.click(screen.getByRole("button", { name: "添加提示词" }));

    expect(created).toEqual({ title: "审阅修复计划", body: "你深入分析一下..." });
  });

  it("does not create duplicate prompts from pointer and click events in the same gesture", () => {
    let createCount = 0;
    renderManager({ onCreate: () => { createCount += 1; } });

    openCreatePanel();
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "No duplicate" },
    });
    fireEvent.change(screen.getByPlaceholderText("提示词内容..."), {
      target: { value: "Only once" },
    });
    const addButton = screen.getByRole("button", { name: "添加提示词" });
    fireEvent.pointerDown(addButton);
    fireEvent.pointerUp(addButton);
    fireEvent.click(addButton);

    expect(createCount).toBe(1);
  });

  it("creates a group with numbered prompts", () => {
    let createdGroup: {
      title: string;
      prompts: Array<{ body: string }>;
      intervalMs: number;
    } | null = null;
    renderManager({ onCreateGroup: (input) => { createdGroup = input; } });

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "Codex Flow" },
    });
    const promptFields = screen.getAllByRole("textbox").filter((field) => {
      return field.getAttribute("placeholder") !== "标题";
    });
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    expect(screen.getByText("提示词 1")).toBeTruthy();
    expect(screen.queryByText(/Step/i)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "添加群组" }));

    expect(createdGroup).toEqual({
      title: "Codex Flow",
      prompts: [
        { body: "First grouped prompt", order: 0 },
        { body: "Second grouped prompt", order: 1 },
      ],
      intervalMs: 700,
    });
  });

  it("shows group interval in seconds while creating a group", () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));

    const intervalInput = screen.getByLabelText("提示词间隔") as HTMLInputElement;
    expect(intervalInput.value).toBe("0.7");
    expect(screen.getByText("s")).toBeTruthy();
    expect(screen.queryByText("ms")).toBeNull();
  });

  it("converts group interval seconds to milliseconds when creating a group", () => {
    let createdIntervalMs: number | null = null;
    renderManager({ onCreateGroup: (input) => { createdIntervalMs = input.intervalMs; } });

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "Timed Group" },
    });
    fireEvent.change(screen.getAllByLabelText(/提示词 \d+ 内容/i)[0], {
      target: { value: "First" },
    });
    fireEvent.change(screen.getByLabelText("提示词间隔"), {
      target: { value: "1.5" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加群组" }));

    expect(createdIntervalMs).toBe(1500);
  });

  it("shows existing group interval in seconds while editing", () => {
    renderManager();

    fireEvent.click(screen.getAllByRole("button", { name: "编辑" })[1]);

    expect((screen.getByLabelText("提示词间隔") as HTMLInputElement).value).toBe("0.7");
  });

  it("shows success feedback after creating a group", async () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "群组提示词" },
    });
    fireEvent.change(screen.getAllByLabelText(/提示词 \d+ 内容/i)[0], {
      target: { value: "第一条" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加群组" }));

    expect(await screen.findByText("已添加提示词组")).toBeTruthy();
  });

  it("inserts and removes group prompts from row controls", () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    expect(screen.getAllByLabelText(/提示词 \d+ 内容/i)).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "在提示词 1 后插入" }));
    expect(screen.getAllByLabelText(/提示词 \d+ 内容/i)).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: "移除提示词 2" }));
    expect(screen.getAllByLabelText(/提示词 \d+ 内容/i)).toHaveLength(2);
  });

  it("reorders group prompts from row controls", () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    const promptFields = screen.getAllByLabelText(/提示词 \d+ 内容/i);
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    fireEvent.click(screen.getByRole("button", { name: "上移提示词 2" }));

    const reorderedFields = screen.getAllByLabelText(/提示词 \d+ 内容/i);
    expect((reorderedFields[0] as HTMLTextAreaElement).value).toBe("Second grouped prompt");
    expect((reorderedFields[1] as HTMLTextAreaElement).value).toBe("First grouped prompt");
  });

  it("does not remove the last group prompt row", () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    fireEvent.click(screen.getByRole("button", { name: "移除提示词 2" }));

    expect((screen.getByRole("button", { name: "移除提示词 1" }) as HTMLButtonElement).disabled)
      .toBe(true);
    expect(screen.getAllByLabelText(/提示词 \d+ 内容/i)).toHaveLength(1);
  });

  it("reorders group prompts by dragging the row handle", () => {
    renderManager();

    openCreatePanel();
    fireEvent.click(screen.getByRole("button", { name: "群组" }));
    const promptFields = screen.getAllByLabelText(/提示词 \d+ 内容/i);
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    const dataTransfer = {
      effectAllowed: "",
      dropEffect: "",
      setData: () => {},
      getData: () => "",
    };

    fireEvent.dragStart(screen.getByRole("button", { name: "拖拽提示词 1" }), {
      dataTransfer,
    });
    fireEvent.dragOver(screen.getByLabelText("提示词 2 内容"), { dataTransfer });
    fireEvent.drop(screen.getByLabelText("提示词 2 内容"), { dataTransfer });

    const reorderedFields = screen.getAllByLabelText(/提示词 \d+ 内容/i);
    expect((reorderedFields[0] as HTMLTextAreaElement).value).toBe("Second grouped prompt");
    expect((reorderedFields[1] as HTMLTextAreaElement).value).toBe("First grouped prompt");
  });

  it("asks for confirmation before delete", () => {
    renderManager();

    const deleteBtn = screen.getAllByText("删除")[0];
    fireEvent.click(deleteBtn);

    expect(screen.getByText("删除这个提示词？")).toBeTruthy();
  });

  it("deletes after confirmation", () => {
    let deleteId: string | null = null;
    renderManager({ onDelete: (id: string) => { deleteId = id; } });

    fireEvent.click(screen.getAllByText("删除")[0]);
    fireEvent.click(screen.getByText("确认"));

    expect(deleteId).toBe("1");
  });

  it("calls reorder with new order when moving down", () => {
    let reorderIds: string[] | null = null;
    renderManager({ onReorder: (ids: string[]) => { reorderIds = ids; } });

    const moveDownBtn = screen.getAllByText("↓")[0];
    fireEvent.click(moveDownBtn);

    expect(reorderIds).toEqual(["2", "1"]);
  });

  it("exposes import and export actions", () => {
    renderManager();

    expect(screen.getByText("设置")).toBeTruthy();
    expect(screen.getByText("导入")).toBeTruthy();
    expect(screen.getByText("导出")).toBeTruthy();
  });

  it("opens settings from the manager header", () => {
    let opened = false;
    renderManager({ onOpenSettings: () => { opened = true; } });

    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    expect(opened).toBe(true);
  });
});

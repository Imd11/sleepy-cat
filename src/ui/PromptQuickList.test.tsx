import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { getMessages } from "../shared/i18n";
import type { PromptContainer } from "../shared/promptTypes";
import { PromptQuickList } from "./PromptQuickList";

const prompts: PromptContainer[] = [
  {
    id: "1",
    title: "讨论方案",
    type: "single",
    prompts: [
      {
        id: "1-entry",
        body: "使用 brainstorming skill，先和我讨论方案，不要修改代码。",
        order: 0,
      },
    ],
    intervalMs: 700,
    order: 0,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z"
  },
  {
    id: "2",
    title: "修复流程",
    type: "group",
    prompts: [
      { id: "2-entry-1", body: "分析根本原因。", order: 0 },
      { id: "2-entry-2", body: "执行修复。", order: 1 },
      { id: "2-entry-3", body: "完成验证。", order: 2 },
    ],
    intervalMs: 700,
    order: 1,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z"
  }
];

function makeRect(
  left: number,
  top: number,
  width: number,
  height: number
): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  } as DOMRect;
}

function revealHoverPreview(
  option: HTMLElement
) {
  fireEvent.mouseEnter(option);
  act(() => {
    vi.advanceTimersByTime(1500);
  });
}

function renderQuickList(
  props: Partial<ComponentProps<typeof PromptQuickList>> = {}
) {
  const zh = getMessages("zh-CN");
  return render(
    <PromptQuickList
      prompts={prompts}
      messages={zh.quickList}
      groupMeta={zh.manager.groupMeta}
      onSelect={() => {}}
      {...props}
    />
  );
}

describe("PromptQuickList", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders compact prompt container options", () => {
    renderQuickList();

    expect(screen.getByText("讨论方案")).toBeTruthy();
    expect(screen.getByText(/brainstorming skill/)).toBeTruthy();
    expect(screen.queryByText("Single · 1 prompt")).toBeNull();
  });

  it("renders group containers with two one-line preview entries", () => {
    renderQuickList();

    expect(screen.getByText("修复流程")).toBeTruthy();
    expect(screen.getByText("群组 · 3 条 · 700ms")).toBeTruthy();
    const groupOption = screen.getByRole("option", { name: /修复流程/i });
    expect(groupOption.textContent).toContain("1. 分析根本原因。");
    expect(groupOption.textContent).toContain("2. 执行修复。");
    expect(groupOption.textContent).not.toContain("3. 完成验证。");
    expect(screen.getByText("1. 分析根本原因。")).toBeTruthy();
    expect(screen.getByText("2. 执行修复。")).toBeTruthy();
    expect(screen.queryByText("3. 完成验证。")).toBeNull();
  });

  it("renders hover preview as a floating tooltip outside the listbox", () => {
    vi.useFakeTimers();
    renderQuickList();

    const listbox = screen.getByRole("listbox", { name: "提示词" });
    revealHoverPreview(screen.getByRole("option", { name: /修复流程/i }));

    const tooltip = screen.getByRole("tooltip");
    expect(listbox.contains(tooltip)).toBe(false);
    expect(tooltip.className).toContain("prompt-hover-preview");
    expect(tooltip.className).toContain("prompt-hover-preview-floating");
  });

  it("anchors hover preview to the hovered prompt container", () => {
    vi.useFakeTimers();
    renderQuickList();

    const shell = document.querySelector(".prompt-quick-shell") as HTMLElement;
    vi.spyOn(shell, "getBoundingClientRect").mockReturnValue(
      makeRect(20, 30, 360, 340)
    );
    Object.defineProperty(shell, "clientWidth", { configurable: true, value: 360 });
    Object.defineProperty(shell, "clientHeight", { configurable: true, value: 340 });
    const option = screen.getByRole("option", { name: /修复流程/i });
    vi.spyOn(option, "getBoundingClientRect").mockReturnValue(
      makeRect(40, 180, 300, 90)
    );

    fireEvent.mouseEnter(option, { clientX: 310, clientY: 250 });
    fireEvent.mouseMove(option, { clientX: 90, clientY: 330 });
    act(() => {
      vi.advanceTimersByTime(1500);
    });

    const tooltip = screen.getByRole("tooltip");
    expect(tooltip.className).toContain("is-above");
    expect(tooltip.style.left).toBe("20px");
    expect(tooltip.style.top).toBe("142px");
    expect(tooltip.style.width).toBe("300px");
  });

  it("delays hover preview for 1.5 seconds", () => {
    vi.useFakeTimers();
    renderQuickList();

    fireEvent.mouseEnter(screen.getByRole("option", { name: /讨论方案/i }));

    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(screen.queryByRole("tooltip")).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.getByRole("tooltip")).toBeTruthy();
  });

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

  it("does not show hover preview from focus alone", () => {
    vi.useFakeTimers();
    renderQuickList();

    fireEvent.focus(screen.getByRole("option", { name: /修复流程/i }));
    act(() => {
      vi.advanceTimersByTime(1500);
    });

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("shows full single prompt content on hover", () => {
    vi.useFakeTimers();
    renderQuickList();

    revealHoverPreview(screen.getByRole("option", { name: /讨论方案/i }));

    const tooltip = screen.getByRole("tooltip");
    expect(tooltip.textContent).toContain(
      "使用 brainstorming skill，先和我讨论方案，不要修改代码。"
    );
    expect(tooltip.querySelector(".prompt-hover-preview-header")).toBeNull();
    expect(tooltip.querySelector("strong")).toBeNull();
  });

  it("shows full group prompt content on hover", () => {
    vi.useFakeTimers();
    renderQuickList();

    expect(screen.queryByRole("tooltip")).toBeNull();

    const option = screen.getByRole("option", { name: /修复流程/i });
    revealHoverPreview(option);

    const tooltip = screen.getByRole("tooltip");
    expect(tooltip.textContent).toContain("1. 分析根本原因。");
    expect(tooltip.textContent).toContain("2. 执行修复。");
    expect(tooltip.textContent).toContain("3. 完成验证。");
    expect(tooltip.textContent).not.toContain("修复流程");
    expect(tooltip.textContent).not.toContain("群组 · 3 条 · 700ms");

    fireEvent.mouseLeave(option);

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("does not render management actions", () => {
    renderQuickList();

    expect(screen.queryByText("管理提示词")).toBeNull();
    expect(screen.queryByText("添加提示词")).toBeNull();
    expect(screen.queryByText("导入")).toBeNull();
    expect(screen.queryByText("导出")).toBeNull();
  });

  it("selects the whole container on click", () => {
    let selected: PromptContainer | null = null;
    renderQuickList({ onSelect: (prompt) => { selected = prompt; } });

    fireEvent.click(screen.getByText("修复流程"));

    expect(selected).toEqual(prompts[1]);
  });

  it("keeps prompt options selectable after hover preview appears", () => {
    vi.useFakeTimers();
    let selected: PromptContainer | null = null;
    renderQuickList({ onSelect: (prompt) => { selected = prompt; } });

    const option = screen.getByRole("option", { name: /修复流程/i });
    revealHoverPreview(option);
    fireEvent.click(option);

    expect(selected).toEqual(prompts[1]);
  });

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

  it("disables the prompt currently being submitted", () => {
    renderQuickList({ submittingPromptId: "1" });

    expect(
      (screen.getByRole("option", { name: /讨论方案/i }) as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });
});

import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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

describe("PromptQuickList", () => {
  it("renders compact prompt container options", () => {
    render(<PromptQuickList prompts={prompts} onSelect={() => {}} />);

    expect(screen.getByText("讨论方案")).toBeTruthy();
    expect(screen.getByText(/brainstorming skill/)).toBeTruthy();
    expect(screen.queryByText("Single · 1 prompt")).toBeNull();
  });

  it("renders group containers with two one-line preview entries", () => {
    render(<PromptQuickList prompts={prompts} onSelect={() => {}} />);

    expect(screen.getByText("修复流程")).toBeTruthy();
    expect(screen.getByText("Group · 3 prompts · 700ms")).toBeTruthy();
    expect(screen.getByText("1. 分析根本原因。")).toBeTruthy();
    expect(screen.getByText("2. 执行修复。")).toBeTruthy();
    expect(screen.queryByText("3. 完成验证。")).toBeNull();
  });

  it("shows full single prompt content on hover", () => {
    render(<PromptQuickList prompts={prompts} onSelect={() => {}} />);

    fireEvent.mouseEnter(screen.getByRole("option", { name: /讨论方案/i }));

    expect(screen.getByRole("tooltip").textContent).toContain(
      "使用 brainstorming skill，先和我讨论方案，不要修改代码。"
    );
  });

  it("shows full group prompt content on hover", () => {
    render(<PromptQuickList prompts={prompts} onSelect={() => {}} />);

    expect(screen.queryByRole("tooltip")).toBeNull();

    fireEvent.mouseEnter(screen.getByRole("option", { name: /修复流程/i }));

    expect(screen.getByRole("tooltip").textContent).toContain("1. 分析根本原因。");
    expect(screen.getByRole("tooltip").textContent).toContain("2. 执行修复。");
    expect(screen.getByRole("tooltip").textContent).toContain("3. 完成验证。");

    fireEvent.mouseLeave(screen.getByRole("option", { name: /修复流程/i }));

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("does not render management actions", () => {
    render(<PromptQuickList prompts={prompts} onSelect={() => {}} />);

    expect(screen.queryByText("Manage Prompts")).toBeNull();
    expect(screen.queryByText("Add Prompt")).toBeNull();
    expect(screen.queryByText("Import")).toBeNull();
    expect(screen.queryByText("Export")).toBeNull();
  });

  it("selects the whole container on click", () => {
    let selected: PromptContainer | null = null;
    render(<PromptQuickList prompts={prompts} onSelect={(prompt) => { selected = prompt; }} />);

    fireEvent.click(screen.getByText("修复流程"));

    expect(selected).toEqual(prompts[1]);
  });

  it("keeps prompt options selectable after hover preview appears", () => {
    let selected: PromptContainer | null = null;
    render(<PromptQuickList prompts={prompts} onSelect={(prompt) => { selected = prompt; }} />);

    const option = screen.getByRole("option", { name: /修复流程/i });
    fireEvent.mouseEnter(option);
    fireEvent.click(option);

    expect(selected).toEqual(prompts[1]);
  });

  it("disables the prompt currently being submitted", () => {
    render(
      <PromptQuickList
        prompts={prompts}
        onSelect={() => {}}
        submittingPromptId="1"
      />
    );

    expect(
      (screen.getByRole("option", { name: /讨论方案/i }) as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });
});

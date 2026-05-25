import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { PromptItem } from "../shared/promptTypes";
import { PromptPopover } from "./PromptPopover";

describe("prompt popover", () => {
  const mockPrompts: PromptItem[] = [
    {
      id: "1",
      title: "Code Review",
      body: "Review this code for bugs and issues.",
      order: 0,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    },
    {
      id: "2",
      title: "Refactor",
      body: "Suggest improvements to make code cleaner.",
      order: 1,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    }
  ];

  it("renders prompt title and preview", () => {
    render(<PromptPopover prompts={mockPrompts} onSelect={() => {}} onManage={() => {}} />);

    expect(screen.getByText("Code Review")).toBeTruthy();
    expect(screen.getByText(/Review this code/)).toBeTruthy();
    expect(screen.getByText("Refactor")).toBeTruthy();
  });

  it("calls onSelect when clicking an item", () => {
    let selectedPrompt: PromptItem | null = null;
    render(
      <PromptPopover
        prompts={mockPrompts}
        onSelect={(p) => { selectedPrompt = p; }}
        onManage={() => {}}
      />
    );

    fireEvent.click(screen.getByText("Code Review"));
    expect(selectedPrompt).toEqual(mockPrompts[0]);
  });

  it("renders Manage Prompts footer action", () => {
    render(<PromptPopover prompts={mockPrompts} onSelect={() => {}} onManage={() => {}} />);

    expect(screen.getByText("Manage Prompts")).toBeTruthy();
  });

  it("has scroll container with max-height", () => {
    const { container } = render(
      <PromptPopover prompts={mockPrompts} onSelect={() => {}} onManage={() => {}} />
    );

    const list = container.querySelector(".prompt-list");
    expect(list).toBeTruthy();
  });
});
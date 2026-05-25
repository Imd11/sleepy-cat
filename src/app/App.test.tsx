import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { App } from "../App";
import type { PromptItem } from "../shared/promptTypes";

// Mock Tauri
vi.mock("@tauri-apps/api/core", () => ({
  invoke: () => Promise.resolve()
}));

const mockPrompts: PromptItem[] = [
  {
    id: "1",
    title: "Test Prompt",
    body: "Test body",
    order: 0,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z"
  }
];

describe("app", () => {
  it("shows prompt list in popover mode by default", () => {
    render(<App prompts={mockPrompts} />);
    expect(screen.getByText("Test Prompt")).toBeTruthy();
  });

  it("shows prompt manager when selecting Manage Prompts", () => {
    render(<App prompts={mockPrompts} />);
    fireEvent.click(screen.getByText("Manage Prompts"));
    expect(screen.getByText("Manage Prompts")).toBeTruthy();
  });
});
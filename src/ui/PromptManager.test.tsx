import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PromptManager } from "./PromptManager";
import type { PromptItem } from "../shared/promptTypes";

describe("prompt manager", () => {
  const mockPrompts: PromptItem[] = [
    {
      id: "1",
      title: "Code Review",
      body: "Review this code for bugs.",
      order: 0,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    },
    {
      id: "2",
      title: "Refactor",
      body: "Suggest improvements.",
      order: 1,
      createdAt: "2026-05-26T00:00:00.000Z",
      updatedAt: "2026-05-26T00:00:00.000Z"
    }
  ];

  it("renders prompt list", () => {
    const onCreate = () => {};
    const onUpdate = () => {};
    const onDelete = () => {};
    const onReorder = () => {};
    const onImport = () => {};
    const onExport = () => {};

    render(
      <PromptManager
        prompts={mockPrompts}
        onCreate={onCreate}
        onUpdate={onUpdate}
        onDelete={onDelete}
        onReorder={onReorder}
        onImport={onImport}
        onExport={onExport}
      />
    );

    expect(screen.getByText("Code Review")).toBeTruthy();
    expect(screen.getByText("Refactor")).toBeTruthy();
  });

  it("asks for confirmation before delete", () => {
    const onDelete = () => {};

    render(
      <PromptManager
        prompts={mockPrompts}
        onCreate={() => {}}
        onUpdate={() => {}}
        onDelete={onDelete}
        onReorder={() => {}}
        onImport={() => {}}
        onExport={() => {}}
      />
    );

    const deleteBtn = screen.getAllByText("Delete")[0];
    fireEvent.click(deleteBtn);

    expect(screen.getByText("Delete this prompt?")).toBeTruthy();
  });

  it("deletes after confirmation", () => {
    let deleteId: string | null = null;
    const onDelete = (id: string) => { deleteId = id; };

    render(
      <PromptManager
        prompts={mockPrompts}
        onCreate={() => {}}
        onUpdate={() => {}}
        onDelete={onDelete}
        onReorder={() => {}}
        onImport={() => {}}
        onExport={() => {}}
      />
    );

    fireEvent.click(screen.getAllByText("Delete")[0]);
    fireEvent.click(screen.getByText("Confirm"));

    expect(deleteId).toBe("1");
  });

  it("calls reorder with new order when moving down", () => {
    let reorderIds: string[] | null = null;
    const onReorder = (ids: string[]) => { reorderIds = ids; };

    render(
      <PromptManager
        prompts={mockPrompts}
        onCreate={() => {}}
        onUpdate={() => {}}
        onDelete={() => {}}
        onReorder={onReorder}
        onImport={() => {}}
        onExport={() => {}}
      />
    );

    // Move first item (Code Review) down
    const moveDownBtn = screen.getAllByText("↓")[0];
    fireEvent.click(moveDownBtn);

    expect(reorderIds).toEqual(["2", "1"]);
  });

  it("exposes import and export actions", () => {
    render(
      <PromptManager
        prompts={mockPrompts}
        onCreate={() => {}}
        onUpdate={() => {}}
        onDelete={() => {}}
        onReorder={() => {}}
        onImport={() => {}}
        onExport={() => {}}
      />
    );

    expect(screen.getByText("Import")).toBeTruthy();
    expect(screen.getByText("Export")).toBeTruthy();
  });
});
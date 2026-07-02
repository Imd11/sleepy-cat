import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PromptManager } from "./PromptManager";
import type { PromptContainer } from "../shared/promptTypes";

describe("prompt manager", () => {
  const mockPrompts: PromptContainer[] = [
    {
      id: "1",
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

  function renderManager(overrides: Partial<Parameters<typeof PromptManager>[0]> = {}) {
    const props = {
      prompts: mockPrompts,
      onCreate: () => {},
      onCreateGroup: () => {},
      onUpdate: () => {},
      onDelete: () => {},
      onReorder: () => {},
      onImport: () => {},
      onExport: () => {},
      ...overrides,
    };
    render(<PromptManager {...props} />);
    return props;
  }

  it("renders prompt containers with group distinction", () => {
    renderManager();

    expect(screen.getByText("Code Review")).toBeTruthy();
    expect(screen.getByText("Repair Group")).toBeTruthy();
    expect(screen.getByText("Single · 1 prompt")).toBeTruthy();
    expect(screen.getByText("Group · 2 prompts · 700ms")).toBeTruthy();
  });

  it("creates a single prompt container", () => {
    let created: { title: string; body: string } | null = null;
    renderManager({ onCreate: (input) => { created = input; } });

    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "New Single" },
    });
    fireEvent.change(screen.getByPlaceholderText("Prompt body..."), {
      target: { value: "Single body" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add Prompt" }));

    expect(created).toEqual({ title: "New Single", body: "Single body" });
  });

  it("creates a group with numbered prompts", () => {
    let createdGroup: {
      title: string;
      prompts: Array<{ body: string }>;
      intervalMs: number;
    } | null = null;
    renderManager({ onCreateGroup: (input) => { createdGroup = input; } });

    fireEvent.click(screen.getByRole("button", { name: "Group" }));
    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "Codex Flow" },
    });
    const promptFields = screen.getAllByRole("textbox").filter((field) => {
      return field.getAttribute("placeholder") !== "Title";
    });
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    expect(screen.getByText("Prompt 1")).toBeTruthy();
    expect(screen.queryByText(/Step/i)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add Group" }));

    expect(createdGroup).toEqual({
      title: "Codex Flow",
      prompts: [
        { body: "First grouped prompt", order: 0 },
        { body: "Second grouped prompt", order: 1 },
      ],
      intervalMs: 700,
    });
  });

  it("inserts and removes group prompts from row controls", () => {
    renderManager();

    fireEvent.click(screen.getByRole("button", { name: "Group" }));
    expect(screen.getAllByLabelText(/Prompt \d+ body/i)).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "Insert prompt after Prompt 1" }));
    expect(screen.getAllByLabelText(/Prompt \d+ body/i)).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: "Remove Prompt 2" }));
    expect(screen.getAllByLabelText(/Prompt \d+ body/i)).toHaveLength(2);
  });

  it("reorders group prompts from row controls", () => {
    renderManager();

    fireEvent.click(screen.getByRole("button", { name: "Group" }));
    const promptFields = screen.getAllByLabelText(/Prompt \d+ body/i);
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    fireEvent.click(screen.getByRole("button", { name: "Move Prompt 2 up" }));

    const reorderedFields = screen.getAllByLabelText(/Prompt \d+ body/i);
    expect((reorderedFields[0] as HTMLTextAreaElement).value).toBe("Second grouped prompt");
    expect((reorderedFields[1] as HTMLTextAreaElement).value).toBe("First grouped prompt");
  });

  it("does not remove the last group prompt row", () => {
    renderManager();

    fireEvent.click(screen.getByRole("button", { name: "Group" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove Prompt 2" }));

    expect((screen.getByRole("button", { name: "Remove Prompt 1" }) as HTMLButtonElement).disabled)
      .toBe(true);
    expect(screen.getAllByLabelText(/Prompt \d+ body/i)).toHaveLength(1);
  });

  it("reorders group prompts by dragging the row handle", () => {
    renderManager();

    fireEvent.click(screen.getByRole("button", { name: "Group" }));
    const promptFields = screen.getAllByLabelText(/Prompt \d+ body/i);
    fireEvent.change(promptFields[0], { target: { value: "First grouped prompt" } });
    fireEvent.change(promptFields[1], { target: { value: "Second grouped prompt" } });

    const dataTransfer = {
      effectAllowed: "",
      dropEffect: "",
      setData: () => {},
      getData: () => "",
    };

    fireEvent.dragStart(screen.getByRole("button", { name: "Drag Prompt 1" }), {
      dataTransfer,
    });
    fireEvent.dragOver(screen.getByLabelText("Prompt 2 body"), { dataTransfer });
    fireEvent.drop(screen.getByLabelText("Prompt 2 body"), { dataTransfer });

    const reorderedFields = screen.getAllByLabelText(/Prompt \d+ body/i);
    expect((reorderedFields[0] as HTMLTextAreaElement).value).toBe("Second grouped prompt");
    expect((reorderedFields[1] as HTMLTextAreaElement).value).toBe("First grouped prompt");
  });

  it("asks for confirmation before delete", () => {
    renderManager();

    const deleteBtn = screen.getAllByText("Delete")[0];
    fireEvent.click(deleteBtn);

    expect(screen.getByText("Delete this prompt?")).toBeTruthy();
  });

  it("deletes after confirmation", () => {
    let deleteId: string | null = null;
    renderManager({ onDelete: (id: string) => { deleteId = id; } });

    fireEvent.click(screen.getAllByText("Delete")[0]);
    fireEvent.click(screen.getByText("Confirm"));

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

    expect(screen.getByText("Import")).toBeTruthy();
    expect(screen.getByText("Export")).toBeTruthy();
  });
});

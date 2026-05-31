import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { App } from "../App";
import type { PromptItem } from "../shared/promptTypes";

// Mock Tauri core invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

// Mock getCurrentWindow
let currentWindowLabel = "prompt-popover";
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: currentWindowLabel }),
}));

// Mock fs plugin
vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
  BaseDirectory: { AppData: "AppData" },
}));

// Mock dialog plugin
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

const mockPrompts: PromptItem[] = [
  {
    id: "1",
    title: "Test Prompt",
    body: "Test body",
    order: 0,
    createdAt: "2026-05-26T00:00:00.000Z",
    updatedAt: "2026-05-26T00:00:00.000Z",
  },
];

describe("app", () => {
  beforeEach(() => {
    currentWindowLabel = "prompt-popover";
  });

  it("shows prompt list in popover mode by default", async () => {
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await waitFor(() => {
      expect(screen.getByText("Test Prompt")).toBeTruthy();
    });
  });

  it("shows only prompt choices in popover mode", async () => {
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await waitFor(() => {
      expect(screen.getByText("Test Prompt")).toBeTruthy();
    });

    expect(screen.queryByText("Manage Prompts")).toBeNull();
    expect(screen.queryByText("Settings")).toBeNull();
    expect(screen.queryByText("Import")).toBeNull();
    expect(screen.queryByText("Export")).toBeNull();
  });

  it("opens prompt manager from the main app window", async () => {
    currentWindowLabel = "main";
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Manage Prompts" }));
    });

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Manage Prompts" })
      ).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "Add Prompt" }));

    expect(screen.getByPlaceholderText("Title")).toBeTruthy();
  });

  it("shows a newly saved prompt in the main window prompt list", async () => {
    currentWindowLabel = "main";
    const files = new Map<string, string>();
    const { readTextFile, writeTextFile } = await import(
      "@tauri-apps/plugin-fs"
    );
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        const value = files.get(path);
        if (!value) throw new Error("missing file");
        return value;
      }
    );
    (writeTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string, value: string) => {
        files.set(path, value);
      }
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(screen.getByRole("button", { name: "Manage Prompts" }));
    fireEvent.click(screen.getByRole("button", { name: "Add Prompt" }));
    // Editor is always visible; fill title and body
    fireEvent.change(screen.getByPlaceholderText("Title"), {
      target: { value: "My Prompt" },
    });
    fireEvent.change(screen.getByPlaceholderText("Prompt body..."), {
      target: { value: "My saved prompt body" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add Prompt" }));

    await waitFor(() => {
      expect(screen.getByText("My Prompt")).toBeTruthy();
    });
  });

  it("shows floating button status and can hide or show it from the main window", async () => {
    currentWindowLabel = "main";
    const files = new Map<string, string>();
    const { readTextFile, writeTextFile } = await import(
      "@tauri-apps/plugin-fs"
    );
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        const value = files.get(path);
        if (!value) throw new Error("missing file");
        return value;
      }
    );
    (writeTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string, value: string) => {
        files.set(path, value);
      }
    );

    await act(async () => {
      render(<App />);
    });

    expect(await screen.findByText("Prompt Picker")).toBeTruthy();
    expect(screen.getByText("Status: Visible")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Hide Floating Button" }));
    await waitFor(() => {
      expect(screen.getByText("Status: Hidden")).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "Show Floating Button" }));
    await waitFor(() => {
      expect(screen.getByText("Status: Visible")).toBeTruthy();
    });
  });

  it("renders button controls mode without prompt management UI", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (path.includes("prompts"))
          return JSON.stringify({ version: 1, prompts: mockPrompts });
        if (path.includes("settings"))
          return JSON.stringify({
            version: 1,
            blacklistedApps: [],
            overlayPlacement: { buttonOffset: null },
            floatingButton: { visible: true },
          });
        throw new Error("unexpected path: " + path);
      }
    );

    await act(async () => {
      render(<App />);
    });

    expect(
      await screen.findByRole("button", { name: "Hide Button" })
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Open Prompt Picker" })
    ).toBeTruthy();
    expect(screen.queryByText("Manage Prompts")).toBeNull();
    expect(screen.queryByText("Import")).toBeNull();
    expect(screen.queryByText("Export")).toBeNull();
  });
});

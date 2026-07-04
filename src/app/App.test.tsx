import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { App } from "../App";
import type { PromptItem } from "../shared/promptTypes";

const inputTargetPollingMock = vi.hoisted(() => vi.fn());
const emitMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
const listenMock = vi.hoisted(() => vi.fn());
const eventHandlers = vi.hoisted(
  () => new Map<string, (event: { payload: unknown }) => unknown>()
);

vi.mock("../overlay/useInputTargetPolling", () => ({
  useInputTargetPolling: inputTargetPollingMock,
}));

// Mock Tauri core invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: emitMock,
  listen: listenMock,
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
    window.history.pushState({}, "", "/");
    inputTargetPollingMock.mockClear();
    emitMock.mockReset();
    emitMock.mockResolvedValue(undefined);
    eventHandlers.clear();
    listenMock.mockReset();
    listenMock.mockImplementation(
      async (event: string, handler: (event: { payload: unknown }) => unknown) => {
        eventHandlers.set(event, handler);
        return () => eventHandlers.delete(event);
      }
    );
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function renderPromptPopover() {
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (path.includes("prompts")) {
          return JSON.stringify({ version: 1, prompts: mockPrompts });
        }
        if (path.includes("settings")) {
          return JSON.stringify({
            version: 1,
            language: "zh-CN",
            blacklistedApps: [],
            overlayPlacement: { buttonOffset: null, buttonPosition: null },
            floatingButton: { visible: true },
            promptInsertion: { mode: "paste_and_submit" },
          });
        }
        throw new Error("unexpected path: " + path);
      }
    );

    await act(async () => {
      render(<App />);
    });
    await screen.findByText("Test Prompt");
  }

  function revealPromptPopoverTooltip() {
    vi.useFakeTimers();
    fireEvent.mouseMove(screen.getByRole("option", { name: /Test Prompt/i }));
    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(screen.getByRole("tooltip")).toBeTruthy();
  }

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

  it("uses a transparent page shell only for prompt-list popovers", async () => {
    await renderPromptPopover();

    expect(document.querySelector(".popover-root")).toBeTruthy();
    expect(document.documentElement.classList.contains("popover-transparent-page")).toBe(true);
    expect(document.body.classList.contains("popover-transparent-page")).toBe(true);
  });

  it("does not use the transparent prompt-list shell for button controls", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");

    await act(async () => {
      render(<App />);
    });

    expect(document.querySelector(".button-controls")).toBeTruthy();
    expect(document.querySelector(".popover-root")).toBeNull();
    expect(document.documentElement.classList.contains("popover-transparent-page")).toBe(false);
    expect(document.body.classList.contains("popover-transparent-page")).toBe(false);
  });

  it("refreshes prompt data when a reused popover is opened", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    const initialPrompts = JSON.stringify({ version: 1, prompts: mockPrompts });
    const refreshedPrompts = JSON.stringify({
      version: 1,
      prompts: [
        {
          ...mockPrompts[0],
          id: "2",
          title: "Fresh Prompt",
          body: "Fresh body",
        },
      ],
    });
    let promptData = initialPrompts;
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (path.includes("prompts")) return promptData;
        throw new Error("missing file");
      }
    );

    await act(async () => {
      render(<App />);
    });

    expect(await screen.findByText("Test Prompt")).toBeTruthy();
    await waitFor(() => {
      expect(listenMock).toHaveBeenCalledWith(
        "prompt-popover-opened",
        expect.any(Function)
      );
    });

    promptData = refreshedPrompts;
    const handler = eventHandlers.get("prompt-popover-opened");
    expect(handler).toBeTruthy();
    await act(async () => {
      await handler?.({ payload: "popover" });
    });

    expect(await screen.findByText("Fresh Prompt")).toBeTruthy();
    expect(screen.queryByText("Test Prompt")).toBeNull();
  });

  it("clears visible prompt hover preview when a reused popover is opened", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    await renderPromptPopover();
    revealPromptPopoverTooltip();

    const handler = eventHandlers.get("prompt-popover-opened");
    expect(handler).toBeTruthy();
    await act(async () => {
      await handler?.({ payload: "popover" });
    });

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("clears visible prompt hover preview when the popover is dismissed", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    await renderPromptPopover();
    revealPromptPopoverTooltip();

    await act(async () => {
      await eventHandlers.get("prompt-popover-dismissed")?.({ payload: undefined });
    });

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("does not select stale prompt rows while a reused popover is refreshing", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue(undefined);
    const initialPrompts = JSON.stringify({ version: 1, prompts: mockPrompts });
    const refreshedPrompts = JSON.stringify({
      version: 1,
      prompts: [
        {
          ...mockPrompts[0],
          id: "2",
          title: "Fresh Prompt",
          body: "Fresh body",
        },
      ],
    });
    let resolveRefresh: (value: string) => void = () => undefined;
    const refreshRead = new Promise<string>((resolve) => {
      resolveRefresh = resolve;
    });
    let promptReadCount = 0;
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (!path.includes("prompts")) throw new Error("missing file");
        promptReadCount += 1;
        return promptReadCount === 1 ? initialPrompts : refreshRead;
      }
    );

    await act(async () => {
      render(<App />);
    });

    expect(await screen.findByText("Test Prompt")).toBeTruthy();
    const handler = eventHandlers.get("prompt-popover-opened");
    let refreshResult: unknown;
    act(() => {
      refreshResult = handler?.({ payload: "popover" });
    });

    fireEvent.click(screen.getByText("Test Prompt"));
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 320));
    });
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
      "paste_prompt_and_submit_to_last_target",
      expect.anything()
    );

    await act(async () => {
      resolveRefresh(refreshedPrompts);
      await refreshResult;
    });

    expect(await screen.findByText("Fresh Prompt")).toBeTruthy();
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

    expect(screen.queryByText("管理提示词")).toBeNull();
    expect(screen.queryByText("设置")).toBeNull();
    expect(screen.queryByText("导入")).toBeNull();
    expect(screen.queryByText("导出")).toBeNull();
  });

  it("does not start input target polling in prompt popover windows", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await screen.findByText("Test Prompt");
    expect(inputTargetPollingMock).not.toHaveBeenCalled();
  });

  it("does not start input target polling in button controls windows", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(
      async (path: string) => {
        if (path.includes("prompts")) {
          return JSON.stringify({ version: 1, prompts: mockPrompts });
        }
        if (path.includes("settings")) {
          return JSON.stringify({
            version: 1,
            blacklistedApps: [],
            overlayPlacement: { buttonOffset: null },
            floatingButton: { visible: true },
          });
        }
        throw new Error("unexpected path: " + path);
      }
    );

    await act(async () => {
      render(<App />);
    });

    await screen.findByRole("button", { name: "隐藏 Calico" });
    expect(inputTargetPollingMock).not.toHaveBeenCalled();
  });

  it("starts input target polling in the main window", async () => {
    currentWindowLabel = "main";
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await screen.findByRole("heading", { name: "管理提示词" });
    expect(inputTargetPollingMock).toHaveBeenCalled();
  });

  it("switches the main window to settings when the menu bar requests settings", async () => {
    currentWindowLabel = "main";
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>)
      .mockResolvedValueOnce(JSON.stringify({ version: 1, prompts: mockPrompts }))
      .mockResolvedValueOnce(JSON.stringify({
        version: 1,
        blacklistedApps: [],
        overlayPlacement: { buttonOffset: null, buttonPosition: null },
        floatingButton: { visible: true },
        promptInsertion: { mode: "paste_and_submit" },
      }));

    await act(async () => {
      render(<App />);
    });

    await screen.findByRole("heading", { name: "管理提示词" });
    await act(async () => {
      eventHandlers.get("open-settings-window")?.({ payload: null });
    });

    expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "填入并发送" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
  });

  it("autosends selected prompt into the backend last input target", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_and_submit_to_last_target",
        { body: "Test body" }
      );
    });
  });

  it("pastes without pressing return when prompt insertion mode is paste only", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>)
      .mockResolvedValueOnce(JSON.stringify({ version: 1, prompts: mockPrompts }))
      .mockResolvedValueOnce(JSON.stringify({
        version: 1,
        blacklistedApps: [],
        overlayPlacement: { buttonOffset: null, buttonPosition: null },
        floatingButton: { visible: true },
        promptInsertion: { mode: "paste_only" },
      }));

    await act(async () => {
      render(<App />);
    });

    await screen.findByText("Test Prompt");
    await waitFor(() => {
      expect(readTextFile).toHaveBeenCalledWith(
        "settings.json",
        expect.objectContaining({ baseDir: "AppData" })
      );
    });
    fireEvent.click(screen.getByText("Test Prompt"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("paste_prompt_to_last_target", {
        body: "Test body",
      });
    });
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
      "paste_prompt_and_submit_to_last_target",
      expect.anything()
    );
    expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
      kind: "sent",
      message: "已填入输入框",
    });
  });

  it("hides the prompt list before autosending a selected single prompt without throw animation", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const callOrder: string[] = [];
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      callOrder.push(`invoke:${command}`);
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    emitMock.mockImplementation(async (event: string) => {
      callOrder.push(`emit:${event}`);
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_and_submit_to_last_target",
        { body: "Test body" }
      );
    });
    expect(callOrder.indexOf("invoke:hide_prompt_popover")).toBeLessThan(
      callOrder.indexOf("invoke:paste_prompt_and_submit_to_last_target")
    );
    expect(emitMock).not.toHaveBeenCalledWith(
      "prompt-throw-send",
      expect.anything()
    );
  });

  it("autosends grouped prompts through the sequence backend command", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_sequence_and_submit_to_last_target") {
        return {
          copied: true,
          sent: true,
          sent_count: 2,
          failed_index: null,
          error: null,
          reason: null,
        };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({
        version: 2,
        containers: [
          {
            id: "group-1",
            title: "Repair Group",
            type: "group",
            prompts: [
              { id: "entry-1", body: "First prompt", order: 0 },
              { id: "entry-2", body: "Second prompt", order: 1 },
            ],
            intervalMs: 700,
            order: 0,
            createdAt: "2026-07-03T00:00:00.000Z",
            updatedAt: "2026-07-03T00:00:00.000Z",
          },
        ],
      })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Repair Group"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_sequence_and_submit_to_last_target",
        {
          bodies: ["First prompt", "Second prompt"],
          interval_ms: 700,
        }
      );
    });
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
      "paste_prompt_and_submit_to_last_target",
      expect.anything()
    );
  });

  it("autosends grouped prompts without emitting a paper-plane throw event", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_sequence_and_submit_to_last_target") {
        return {
          copied: true,
          sent: true,
          sent_count: 2,
          failed_index: null,
          error: null,
          reason: null,
        };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({
        version: 2,
        containers: [
          {
            id: "group-1",
            title: "Repair Group",
            type: "group",
            prompts: [
              { id: "entry-1", body: "First prompt", order: 0 },
              { id: "entry-2", body: "Second prompt", order: 1 },
            ],
            intervalMs: 700,
            order: 0,
            createdAt: "2026-07-03T00:00:00.000Z",
            updatedAt: "2026-07-03T00:00:00.000Z",
          },
        ],
      })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Repair Group"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_sequence_and_submit_to_last_target",
        {
          bodies: ["First prompt", "Second prompt"],
          interval_ms: 700,
        }
      );
    });
    expect(emitMock).not.toHaveBeenCalledWith(
      "prompt-throw-send",
      expect.anything()
    );
  });

  it("emits a sent status when autosend reports keyboard success", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
        kind: "sent",
        message: "已发送",
      });
    });
    expect(emitMock).not.toHaveBeenCalledWith(
      "prompt-throw-send",
      expect.anything()
    );
  });

  it("emits an actionable permission status when autosend lacks accessibility permission", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return {
          copied: true,
          sent: false,
          error: "Accessibility permission required for autosend.",
          reason: "missing_accessibility_permission",
        };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
        kind: "failed",
        message: "点击授权",
        action: "request_accessibility_permission",
      });
    });
    expect(emitMock).not.toHaveBeenCalledWith(
      "prompt-throw-send",
      expect.anything()
    );
    expect(emitMock).not.toHaveBeenCalledWith(
      "prompt-autosend-status",
      expect.objectContaining({ message: "已复制，可手动 Cmd+V" })
    );
  });

  it("emits a distinct status when autosend pastes but cannot press return", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return {
          copied: true,
          sent: false,
          error: "Native return event failed",
          reason: "return_event_failed",
        };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(emitMock).toHaveBeenCalledWith("prompt-autosend-status", {
        kind: "failed",
        message: "已填入输入框，未发送",
      });
    });
  });

  it("hides the prompt popover before autosending the selected prompt", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      const calls = vi.mocked(invoke).mock.calls.map((call) => call[0]);
      expect(calls).toContain("hide_prompt_popover");
      expect(calls).toContain("paste_prompt_and_submit_to_last_target");
      expect(calls.indexOf("hide_prompt_popover")).toBeLessThan(
        calls.indexOf("paste_prompt_and_submit_to_last_target")
      );
    });
  });

  it("waits long enough for the popover to hide before autosend", async () => {
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockClear();
      vi.mocked(invoke).mockImplementation(async (command: string) => {
        if (command === "paste_prompt_and_submit_to_last_target") {
          return { copied: true, sent: true, error: null };
        }
        return undefined;
      });
      const { readTextFile } = await import("@tauri-apps/plugin-fs");
      (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        JSON.stringify({ version: 1, prompts: mockPrompts })
      );

      await act(async () => {
        render(<App />);
      });

      fireEvent.click(await screen.findByText("Test Prompt"));

      await waitFor(() => {
        expect(vi.mocked(invoke)).toHaveBeenCalledWith("hide_prompt_popover");
      });
      await waitFor(() => {
        expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 260);
      });
      await waitFor(() => {
        expect(vi.mocked(invoke)).toHaveBeenCalledWith(
          "paste_prompt_and_submit_to_last_target",
          { body: "Test body" }
        );
      });
    } finally {
      setTimeoutSpy.mockRestore();
    }
  });

  it("does not run a frontend accessibility preflight before autosend", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("hide_prompt_popover");
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_and_submit_to_last_target",
        { body: "Test body" }
      );
    });

    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("accessibility_status_cmd");
  });

  it("hides the prompt popover and logs autosend failures without a blocking dialog", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const calls: string[] = [];
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "hide_prompt_popover") {
        calls.push("hide");
        return undefined;
      }
      if (command === "paste_prompt_and_submit_to_last_target") {
        calls.push("autosend");
        throw new Error("Accessibility permission required for autosend.");
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(calls).toEqual(["hide", "autosend"]);
      expect(warn).toHaveBeenCalledWith(
        "Prompt autosend failed without blocking the picker:",
        expect.any(Error)
      );
    });
    warn.mockRestore();
  });

  it("does not move the floating button when selecting a prompt from the popover", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        return { copied: true, sent: true, error: null };
      }
      return undefined;
    });
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=popover");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "paste_prompt_and_submit_to_last_target",
        { body: "Test body" }
      );
    });

    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
      "show_prompt_button",
      expect.anything()
    );
    expect(inputTargetPollingMock).not.toHaveBeenCalled();
  });

  it("does not fall back to blind paste when no input target is recorded", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        throw new Error("Click into a text field first, then choose a prompt.");
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(warn).toHaveBeenCalledWith(
        "Prompt autosend failed without blocking the picker:",
        expect.any(Error)
      );
    });
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("paste_prompt", {
      body: "Test body",
    });
    warn.mockRestore();
  });

  it("does not show a blocking permission dialog when backend autosend reports accessibility failure", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "paste_prompt_and_submit_to_last_target") {
        throw new Error(
          "Accessibility permission required for autosend. Enable Prompt Picker in System Settings > Privacy & Security > Accessibility, then try again."
        );
      }
      return undefined;
    });
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByText("Test Prompt"));

    await waitFor(() => {
      expect(warn).toHaveBeenCalledWith(
        "Prompt autosend failed without blocking the picker:",
        expect.any(Error)
      );
    });
    warn.mockRestore();
  });

  it("opens the main app window directly on prompt management", async () => {
    currentWindowLabel = "main";
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "管理提示词" })
      ).toBeTruthy();
    });

    expect(screen.queryByText("Floating Button")).toBeNull();
    expect(screen.queryByText("Hide Floating Button")).toBeNull();
    expect(screen.queryByRole("heading", { name: "设置" })).toBeNull();
    expect(screen.getByPlaceholderText("标题")).toBeTruthy();
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

    // Editor is always visible; fill title and body
    fireEvent.change(screen.getByPlaceholderText("标题"), {
      target: { value: "My Prompt" },
    });
    fireEvent.change(screen.getByPlaceholderText("提示词内容..."), {
      target: { value: "My saved prompt body" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加提示词" }));

    await waitFor(() => {
      expect(screen.getByText("My Prompt")).toBeTruthy();
    });
  });

  it("does not show floating button controls on the main prompt management page", async () => {
    currentWindowLabel = "main";
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({ version: 1, prompts: mockPrompts })
    );

    await act(async () => {
      render(<App />);
    });

    expect(await screen.findByRole("heading", { name: "管理提示词" })).toBeTruthy();
    expect(screen.queryByText("Status: Visible")).toBeNull();
    expect(screen.queryByText("Autosend: Ready")).toBeNull();
    expect(screen.queryByRole("button", { name: "Hide Floating Button" })).toBeNull();
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
      await screen.findByRole("button", { name: "隐藏 Calico" })
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "管理提示词..." })
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "打开辅助功能设置" })
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "退出 Prompt Picker" })
    ).toBeTruthy();
    expect(screen.queryByText("导入")).toBeNull();
    expect(screen.queryByText("导出")).toBeNull();
  });

  it("button controls hide persists state and hides the floating button", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const files = new Map<string, string>();
    const { readTextFile, writeTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) return JSON.stringify({ version: 1, blacklistedApps: [], overlayPlacement: { buttonOffset: null }, floatingButton: { visible: true } });
      throw new Error("missing file");
    });
    (writeTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string, value: string) => {
      files.set(path, value);
    });

    await act(async () => {
      render(<App />);
    });

    await screen.findByRole("button", { name: "隐藏 Calico" });
    fireEvent.click(screen.getByRole("button", { name: "隐藏 Calico" }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("hide_prompt_button");
    });
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("hide_prompt_popover");
    const allCalls = (vi.mocked(invoke) as ReturnType<typeof vi.fn>).mock.calls.map((c) => c[0]);
    expect(allCalls).toContain("hide_prompt_button");
    expect(allCalls).toContain("hide_prompt_popover");
    expect(allCalls).not.toContain("open_main_window");
  });

  it("emits prompt-popover-dismissed when hiding Calico from button controls", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) {
        return JSON.stringify({
          version: 1,
          blacklistedApps: [],
          overlayPlacement: { buttonOffset: null },
          floatingButton: { visible: true },
        });
      }
      throw new Error("unexpected path: " + path);
    });

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByRole("button", { name: "隐藏 Calico" }));

    await waitFor(() => {
      expect(emitMock).toHaveBeenCalledWith("prompt-popover-dismissed");
    });
  });

  it("manage prompts from button controls calls open_main_window", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile, writeTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) return JSON.stringify({ version: 1, blacklistedApps: [], overlayPlacement: { buttonOffset: null }, floatingButton: { visible: true } });
      throw new Error("missing file");
    });
    (writeTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (_path: string, _value: string) => {});

    await act(async () => { render(<App />); });

    await screen.findByRole("button", { name: "管理提示词..." });
    fireEvent.click(screen.getByRole("button", { name: "管理提示词..." }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("open_main_window");
    });
  });

  it("emits prompt-popover-dismissed when button controls open the manager without sending", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) {
        return JSON.stringify({
          version: 1,
          blacklistedApps: [],
          overlayPlacement: { buttonOffset: null },
          floatingButton: { visible: true },
        });
      }
      throw new Error("unexpected path: " + path);
    });

    await act(async () => {
      render(<App />);
    });

    fireEvent.click(await screen.findByRole("button", { name: "管理提示词..." }));

    await waitFor(() => {
      expect(emitMock).toHaveBeenCalledWith("prompt-popover-dismissed");
    });
  });

  it("button controls can open Accessibility settings", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) return JSON.stringify({ version: 1, blacklistedApps: [], overlayPlacement: { buttonOffset: null }, floatingButton: { visible: true } });
      throw new Error("missing file");
    });

    await act(async () => { render(<App />); });

    await screen.findByRole("button", { name: "打开辅助功能设置" });
    fireEvent.click(screen.getByRole("button", { name: "打开辅助功能设置" }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("open_accessibility_settings");
    });
    expect(emitMock).toHaveBeenCalledWith("prompt-popover-dismissed");
  });

  it("button controls can quit Prompt Picker", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) return JSON.stringify({ version: 1, blacklistedApps: [], overlayPlacement: { buttonOffset: null }, floatingButton: { visible: true } });
      throw new Error("missing file");
    });

    await act(async () => { render(<App />); });

    await screen.findByRole("button", { name: "退出 Prompt Picker" });
    fireEvent.click(screen.getByRole("button", { name: "退出 Prompt Picker" }));

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("quit_prompt_picker");
    });
  });

  it("button controls mode does not render manager actions", async () => {
    currentWindowLabel = "prompt-popover";
    window.history.pushState({}, "", "/?mode=button-controls");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    (readTextFile as ReturnType<typeof vi.fn>).mockImplementation(async (path: string) => {
      if (path.includes("prompts")) return JSON.stringify({ version: 1, prompts: mockPrompts });
      if (path.includes("settings")) return JSON.stringify({ version: 1, blacklistedApps: [], overlayPlacement: { buttonOffset: null }, floatingButton: { visible: true } });
      throw new Error("missing file");
    });

    await act(async () => { render(<App />); });

    expect(screen.queryByRole("button", { name: "隐藏 Calico" })).not.toBeNull();
    expect(screen.queryByRole("button", { name: "管理提示词..." })).not.toBeNull();
    expect(screen.queryByRole("button", { name: "打开辅助功能设置" })).not.toBeNull();
    expect(screen.queryByRole("button", { name: "退出 Prompt Picker" })).not.toBeNull();
    expect(screen.queryByText("设置")).toBeNull();
  });
});

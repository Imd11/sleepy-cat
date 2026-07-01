import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useInputTargetPolling } from "./useInputTargetPolling";
import * as platformApi from "../platform/platformApi";

const eventMock = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  listen: vi.fn(
    async (event: string, handler: (event: { payload: unknown }) => void) => {
      eventMock.listeners.set(event, handler);
      return vi.fn();
    }
  ),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: eventMock.listen,
}));

vi.mock("../platform/platformApi", () => ({
  getFrontmostApp: vi.fn(),
  getCurrentInputTarget: vi.fn(),
  showPromptButton: vi.fn().mockResolvedValue(undefined),
  hidePromptButton: vi.fn().mockResolvedValue(undefined),
  showPromptPopover: vi.fn().mockResolvedValue(undefined),
  hidePromptPopover: vi.fn().mockResolvedValue(undefined),
}));

const getFrontmostApp = platformApi.getFrontmostApp as ReturnType<typeof vi.fn>;
const getCurrentInputTarget = platformApi.getCurrentInputTarget as ReturnType<typeof vi.fn>;
const showPromptButton = platformApi.showPromptButton as ReturnType<typeof vi.fn>;
const hidePromptButton = platformApi.hidePromptButton as ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.clearAllMocks();
  eventMock.listeners.clear();
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

describe("useInputTargetPolling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows attached button when target exists", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalledWith(960, 700);
    });
  });

  it("keeps a fallback button visible when no target frame is available", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue(null);

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // Must show at fallback AND not hide
    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalledWith(960, 700);
      expect(hidePromptButton).not.toHaveBeenCalled();
    });
  });

  it("keeps a fallback button visible when frontmost app has no target", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue(null);

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    await waitFor(() => {
      expect(hidePromptButton).not.toHaveBeenCalled();
    });
  });

  it("keeps the floating button visible even when the frontmost app is blacklisted", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Codex", bundle_id: "com.codex.app" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Codex", bundle_id: "com.codex.app" },
    });

    void renderHook(() =>
      useInputTargetPolling(["com.codex.app"], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalledWith(960, 700);
    });
  });

  it("hides the floating button only when user visibility setting is false", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    const { rerender } = renderHook(
      ({ visible }) => useInputTargetPolling([], { buttonOffset: null }, {}, visible),
      { initialProps: { visible: true } }
    );

    await act(async () => { vi.advanceTimersByTime(1500); });
    expect(showPromptButton).toHaveBeenCalledWith(960, 700);

    rerender({ visible: false });

    await act(async () => { vi.advanceTimersByTime(1500); });
    expect(hidePromptButton).toHaveBeenCalled();
  });

  it("keeps the fallback button visible while Prompt Picker itself is frontmost", async () => {
    getFrontmostApp
      .mockResolvedValueOnce({ name: "Finder", bundle_id: "com.apple.finder" })
      .mockResolvedValue({ name: "Prompt Picker", bundle_id: "local.promptpicker.dev" });
    getCurrentInputTarget.mockResolvedValueOnce({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => { vi.advanceTimersByTime(1500); });
    expect(showPromptButton).toHaveBeenCalledWith(960, 700);

    await act(async () => { vi.advanceTimersByTime(2000); });
    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalled();
    });
  });

  it("applies saved overlay offset to attached button position", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: { x: 10, y: -5 } }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalledWith(970, 695);
    });
  });

  it("reports dragged button position relative to the current target base", async () => {
    const onButtonDragEnd = vi.fn();
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling(
        [],
        { buttonOffset: null },
        { onButtonDragEnd },
        true
      )
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    eventMock.listeners.get("prompt-button-drag-ended")?.({
      payload: { x: 1000, y: 680 },
    });

    expect(onButtonDragEnd).toHaveBeenCalledWith(
      { x: 1000, y: 680 },
      [960, 700]
    );
  });

  it("keeps the dragged fallback position instead of snapping back", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue(null);

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    eventMock.listeners.get("prompt-button-drag-ended")?.({
      payload: { x: 420, y: 260 },
    });

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenLastCalledWith(420, 260);
    });
  });

  it("keeps the last button position during overlay self-interaction", async () => {
    getFrontmostApp
      .mockResolvedValueOnce({ name: "Finder", bundle_id: "com.apple.finder" })
      .mockResolvedValueOnce({ name: "Prompt Picker", bundle_id: "local.promptpicker.dev" })
      .mockResolvedValue({ name: "Prompt Picker", bundle_id: "local.promptpicker.dev" });
    getCurrentInputTarget
      .mockResolvedValueOnce({
        frame: { x: 100, y: 200, width: 300, height: 40 },
        window_frame: { x: 100, y: 200, width: 300, height: 40 },
        button_position: [960, 700],
        app: { name: "Finder", bundle_id: "com.apple.finder" },
      })
      .mockResolvedValue(null);

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenLastCalledWith(960, 700);
    });
  });

  it("keeps polling after rerendering with an equivalent blacklist", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    const { rerender } = renderHook(
      ({ blacklist }) => useInputTargetPolling(blacklist, { buttonOffset: null }, {}, true),
      { initialProps: { blacklist: ["com.apple.finder"] } }
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    rerender({ blacklist: ["com.apple.finder"] });

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    expect(showPromptButton).toHaveBeenCalled(); // at least once, polling continues
  });
});

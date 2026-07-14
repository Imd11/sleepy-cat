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

  it("keeps the floating button at a stable position when target exists", async () => {
    getFrontmostApp.mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [320, 280],
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

  it("keeps the fallback button visible while Sleepy Cat itself is frontmost", async () => {
    getFrontmostApp
      .mockResolvedValueOnce({ name: "Finder", bundle_id: "com.apple.finder" })
      .mockResolvedValue({ name: "Sleepy Cat", bundle_id: "local.promptpicker.dev" });
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

  it("uses saved absolute button position instead of target-driven placement", async () => {
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
        { buttonOffset: { x: 10, y: -5 }, buttonPosition: { x: 420, y: 260 } },
        {},
        true
      )
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalledWith(420, 260);
    });
  });

  it("cancels the previous polling loop when saved position changes", async () => {
    getFrontmostApp.mockResolvedValue({
      name: "Sleepy Cat",
      bundle_id: "local.promptpicker.dev",
    });
    getCurrentInputTarget.mockResolvedValue(null);

    const { rerender } = renderHook(
      ({ position }: { position: { x: number; y: number } | null }) =>
        useInputTargetPolling(
          [],
          { buttonOffset: null, buttonPosition: position },
          {},
          true
        ),
      { initialProps: { position: null as { x: number; y: number } | null } }
    );

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenLastCalledWith(960, 700);
    });

    vi.clearAllMocks();
    rerender({ position: { x: 1765, y: 419 } });

    await act(async () => {
      vi.advanceTimersByTime(4000);
    });

    await waitFor(() => {
      expect(showPromptButton).toHaveBeenCalled();
      expect(showPromptButton).not.toHaveBeenCalledWith(960, 700);
      expect(showPromptButton).toHaveBeenLastCalledWith(1765, 419);
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
      .mockResolvedValueOnce({ name: "Sleepy Cat", bundle_id: "local.promptpicker.dev" })
      .mockResolvedValue({ name: "Prompt Drawer", bundle_id: "local.promptpicker.dev" });
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

  it("pauses input target polling while autosend is active", async () => {
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

    await waitFor(() => {
      expect(getFrontmostApp).toHaveBeenCalled();
    });
    vi.clearAllMocks();

    eventMock.listeners.get("prompt-autosend-activity")?.({
      payload: { active: true },
    });

    await act(async () => {
      vi.advanceTimersByTime(2500);
    });

    expect(getFrontmostApp).not.toHaveBeenCalled();
    expect(getCurrentInputTarget).not.toHaveBeenCalled();
    expect(showPromptButton).not.toHaveBeenCalled();
  });

  it("does not apply an in-flight polling result after autosend starts", async () => {
    let resolveFrontmost: (app: { name: string; bundle_id: string }) => void = () => {};
    getFrontmostApp.mockReturnValue(new Promise((resolve) => {
      resolveFrontmost = resolve;
    }));
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await waitFor(() => {
      expect(getFrontmostApp).toHaveBeenCalled();
    });
    eventMock.listeners.get("prompt-autosend-activity")?.({
      payload: { active: true },
    });

    await act(async () => {
      resolveFrontmost({ name: "Finder", bundle_id: "com.apple.finder" });
    });

    expect(getCurrentInputTarget).not.toHaveBeenCalled();
    expect(showPromptButton).not.toHaveBeenCalled();
  });

  it("resumes polling after an in-flight poll is paused by autosend", async () => {
    let resolveFrontmost: (app: { name: string; bundle_id: string }) => void = () => {};
    getFrontmostApp
      .mockReturnValueOnce(new Promise((resolve) => {
        resolveFrontmost = resolve;
      }))
      .mockResolvedValue({ name: "Finder", bundle_id: "com.apple.finder" });
    getCurrentInputTarget.mockResolvedValue({
      frame: { x: 100, y: 200, width: 300, height: 40 },
      window_frame: { x: 100, y: 200, width: 300, height: 40 },
      button_position: [960, 700],
      app: { name: "Finder", bundle_id: "com.apple.finder" },
    });

    void renderHook(() =>
      useInputTargetPolling([], { buttonOffset: null }, {}, true)
    );

    await waitFor(() => {
      expect(getFrontmostApp).toHaveBeenCalledTimes(1);
    });
    eventMock.listeners.get("prompt-autosend-activity")?.({
      payload: { active: true },
    });

    await act(async () => {
      resolveFrontmost({ name: "Finder", bundle_id: "com.apple.finder" });
    });

    expect(getCurrentInputTarget).not.toHaveBeenCalled();
    vi.clearAllMocks();

    eventMock.listeners.get("prompt-autosend-activity")?.({
      payload: { active: false },
    });
    await act(async () => {
      vi.advanceTimersByTime(600);
    });

    await waitFor(() => {
      expect(getFrontmostApp).toHaveBeenCalled();
    });
  });
});

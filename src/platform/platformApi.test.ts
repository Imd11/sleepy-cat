import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import {
  acknowledgePromptPopoverMode,
  pastePromptAndSubmitToLastTarget,
  pastePromptSequenceAndSubmitToLastTarget,
  setPromptButtonVisibility,
} from "./platformApi";

describe("platform API", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("routes persistent pet visibility through one native command", async () => {
    invokeMock.mockResolvedValue({
      visible: false,
      applied: true,
      persisted: true,
      error: null,
    });

    await expect(setPromptButtonVisibility(false)).resolves.toMatchObject({
      visible: false,
      applied: true,
      persisted: true,
    });
    expect(invokeMock).toHaveBeenCalledWith("set_prompt_button_visibility", {
      visible: false,
    });
  });

  it("acknowledges the committed popover mode", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await acknowledgePromptPopoverMode(7, "button-controls");

    expect(invokeMock).toHaveBeenCalledWith("acknowledge_prompt_popover_mode", {
      requestId: 7,
      mode: "button-controls",
    });
  });

  it("passes prompt send behavior to the native autosend command", async () => {
    invokeMock.mockResolvedValueOnce({ copied: true, sent: true, error: null, reason: null });

    await pastePromptAndSubmitToLastTarget("body", "enter", "inherit");

    expect(invokeMock).toHaveBeenCalledWith("paste_prompt_and_submit_to_last_target", {
      body: "body",
      submitKey: "enter",
      sendBehavior: "inherit",
    });
  });

  it("passes sequence arguments using Tauri's camelCase command keys", async () => {
    invokeMock.mockResolvedValueOnce({
      copied: true,
      sent: true,
      sent_count: 2,
      failed_index: null,
      error: null,
      reason: null,
    });

    await pastePromptSequenceAndSubmitToLastTarget(
      ["first", "second"],
      750,
      "command_enter",
      "paste_command_enter"
    );

    expect(invokeMock).toHaveBeenCalledWith("paste_prompt_sequence_and_submit_to_last_target", {
      bodies: ["first", "second"],
      intervalMs: 750,
      submitKey: "command_enter",
      sendBehavior: "paste_command_enter",
    });
  });
});

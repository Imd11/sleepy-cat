import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

describe("overlay button html", () => {
  it("enables global Tauri for the vanilla overlay html", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));

    expect(config.app?.withGlobalTauri).toBe(true);
  });

  it("opens Tauri button controls on right click instead of an inline menu", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("window.__TAURI__");
    expect(html).toContain("contextmenu");
    expect(html).toContain("Tauri invoke API is unavailable");
    expect(html).toContain("Tauri command failed");
    expect(html).toContain("show_prompt_popover_from_button");
    expect(html).toContain("show_prompt_button_controls_from_button");
    expect(html).toContain("prompt-button-drag-started");
    expect(html).toContain("prompt-button-drag-ended");
    expect(html).toContain("event.button === 2");
    expect(html).toContain("event.ctrlKey");
    expect(html).toContain("start.positionReady = false");
    expect(html).not.toContain('id="menu"');
    expect(html).not.toContain("hide_prompt_button");
  });

  it("renders the floating entry as an animated Calico character", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("calico-entry");
    expect(html).toContain("calico-idle.apng");
    expect(html).toContain("calico-react-poke.apng");
    expect(html).toContain("calico-react-drag.apng");
    expect(html).toContain('aria-label="Open Prompt Picker"');
    expect(html).not.toContain("<span>Prompts</span>");
  });

  it("keeps existing drag and click commands for the Calico entry", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("prompt_button_position_cmd");
    expect(html).toContain("move_prompt_button_to");
    expect(html).toContain("show_prompt_popover_from_button");
    expect(html).toContain("prompt-button-drag-started");
    expect(html).toContain("prompt-button-drag-ended");
    expect(html).toContain("setPointerCapture");
    expect(html).toContain("releasePointerCapture");
  });

  it("records the prompt pick session target before opening the prompt list", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("begin_prompt_pick_session");
    expect(html.indexOf("begin_prompt_pick_session")).toBeLessThan(
      html.indexOf("show_prompt_popover_from_button")
    );
  });

  it("listens for prompt autosend status and renders a Calico status bubble", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("prompt-autosend-status");
    expect(html).toContain("calico-status-bubble");
    expect(html).toContain("showStatusBubble");
    expect(html).toContain("hideStatusBubble");
    expect(html).toContain("statusBubble.textContent");
  });

  it("does not use manual paste as the default failure copy", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).not.toContain("可手动 Cmd+V");
  });

  it("requests Accessibility permission from actionable autosend status bubbles", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("request_accessibility_permission");
    expect(html).toContain("request_accessibility_permission_cmd");
    expect(html).toContain("open_accessibility_settings");
    expect(html).toContain("statusBubble.dataset.action");
    expect(html).toContain("is-action");
    expect(html).toContain("Open Accessibility Permission Help");
    expect(html).not.toContain("payload.kind || 'copied'");
  });
});

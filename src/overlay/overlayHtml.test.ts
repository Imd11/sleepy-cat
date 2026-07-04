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
    expect(html).toContain("toggle_prompt_popover_from_button");
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
    expect(html).toContain("calico-sprite");
    expect(html).toContain("calico-idle.apng");
    expect(html).toContain("calico-react-drag.apng");
    expect(html).toContain('data-motion-state="idle"');
    expect(html).not.toContain("calico-projectile");
    expect(html).not.toContain("promptProjectile");
    expect(html).toContain('aria-label="Open Prompt Picker"');
    expect(html).not.toContain("calico-rig");
    expect(html).not.toContain("calico-body");
    expect(html).not.toContain("calico-head");
    expect(html).not.toContain('class="calico-svg"');
    expect(html).not.toContain("<span>Prompts</span>");
  });

  it("keeps existing drag and click commands for the Calico entry", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("prompt_button_position_cmd");
    expect(html).toContain("move_prompt_button_to");
    expect(html).toContain("toggle_prompt_popover_from_button");
    expect(html).toContain("prompt-button-drag-started");
    expect(html).toContain("prompt-button-drag-ended");
    expect(html).toContain("setPointerCapture");
    expect(html).toContain("releasePointerCapture");
  });

  it("hides the prompt popover when Calico dragging starts", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("hidePromptPopoverForDrag");
    expect(html).toContain("await invoke('hide_prompt_popover')");
    expect(html.indexOf("hidePromptPopoverForDrag().catch")).toBeLessThan(
      html.indexOf("emit('prompt-button-drag-started')")
    );
  });

  it("opens the prompt list without awaiting target session capture", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("let promptPickSessionId = 0;");
    expect(html).toContain("const sessionId = ++promptPickSessionId;");
    expect(html).toContain("const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });");
    expect(html).toContain("if (toggleResult?.opened)");
    expect(html).toContain("const sessionPromise = invoke('begin_prompt_pick_session', { sessionId });");
    expect(html).toContain("void sessionPromise.catch(() => null);");
    expect(html).toContain("resetCalicoMotion();");
    expect(html).not.toContain("await invoke('begin_prompt_pick_session')");
    expect(html).not.toContain("await sessionPromise.catch");
    expect(html.indexOf("toggle_prompt_popover_from_button")).toBeLessThan(
      html.indexOf("begin_prompt_pick_session")
    );
  });

  it("requires deliberate pointer movement before treating a click as drag", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("const DRAG_START_DISTANCE_PX = 10;");
    expect(html).toContain("distance(start, current) < DRAG_START_DISTANCE_PX");
    expect(html).not.toContain("distance(start, current) < 4");
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

  it("opens prompts without switching Calico into a throw-ready pose", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("toggle_prompt_popover_from_button");
    expect(html).not.toContain("setMotionState('ready'");
    expect(html).not.toContain('[data-motion-state="ready"]');
    expect(html).not.toContain("setSpriteSource(sprites.ready)");
    expect(html).not.toContain("calico-ready-windup");
    expect(html).not.toContain("calico-ready-projectile");
  });

  it("does not listen for paper-plane throw events", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).not.toContain("prompt-throw-send");
    expect(html).not.toContain("playCalicoThrow");
    expect(html).not.toContain("show_paper_plane_flight_from_button");
    expect(html).not.toContain("setMotionState('throwing'");
    expect(html).not.toContain("setMotionState('recovering'");
    expect(html).not.toContain("calico-throw-snap");
    expect(html).not.toContain("calico-throw-projectile-release");
    expect(html).not.toContain("THROW_RELEASE_MS");
  });

  it("resets Calico when the popover is dismissed without sending", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("prompt-popover-dismissed");
    expect(html).toContain("resetCalicoMotion");
    expect(html).toContain("motionResetTimer");
  });

  it("grants only minimal IPC access to the paper-plane flight window", () => {
    const capability = JSON.parse(
      readFileSync("src-tauri/capabilities/paper-flight.json", "utf8")
    );

    expect(capability.windows).toEqual(["paper-plane-flight"]);
    expect(capability.permissions).toEqual(["core:default"]);
  });
});

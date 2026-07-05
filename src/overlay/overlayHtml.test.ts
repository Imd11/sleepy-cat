import { existsSync, readFileSync } from "fs";
import { describe, expect, it } from "vitest";

function readOverlayHtml(): string {
  return readFileSync("public/overlay.html", "utf8");
}

describe("overlay button html", () => {
  it("enables global Tauri for the vanilla overlay html", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));

    expect(config.app?.withGlobalTauri).toBe(true);
  });

  it("opens Tauri button controls on right click instead of an inline menu", () => {
    const html = readOverlayHtml();

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
    const html = readOverlayHtml();

    expect(html).toContain("calico-entry");
    expect(html).toContain("calico-sprite");
    expect(html).toContain("calico-idle-follow.svg");
    expect(html).toContain("calico-react-drag.apng");
    expect(html).toContain('data-motion-state="idle-follow"');
    expect(html).not.toContain("calico-projectile");
    expect(html).not.toContain("promptProjectile");
    expect(html).toContain('aria-label="Open Prompt Picker"');
    expect(html).not.toContain("calico-rig");
    expect(html).not.toContain("calico-body");
    expect(html).not.toContain("calico-head");
    expect(html).not.toContain('class="calico-svg"');
    expect(html).not.toContain("<span>Prompts</span>");
  });

  it("loads the Calico motion runtime and manifest", () => {
    const html = readOverlayHtml();

    expect(html).toContain("/calico/motion-runtime.js");
    expect(html).toContain("createCalicoMotionRuntime");
    expect(html).toContain("/calico/idle-director.js");
    expect(html).toContain("createCalicoIdleDirector");
    expect(html).toContain("initializeCalicoMotion");
    expect(html).toContain("fetch('/calico/manifest.json')");
    expect(html).toContain("calico-motion");
  });

  it("loads and starts the Calico idle director after the motion runtime is ready", () => {
    const html = readOverlayHtml();

    expect(html).toContain("/calico/idle-director.js");
    expect(html).toContain("createCalicoIdleDirector");
    expect(html).toContain("let calicoIdleDirector = null;");
    expect(html).toContain("calicoIdleDirector = createCalicoIdleDirector");
    expect(html).toContain("applyMotion: applyCalicoMotion");
    expect(html).toContain("resetMotion: resetCalicoMotion");
    expect(html).toContain("getCurrentState: () => btn.dataset.motionState || manifest.defaultState");
    expect(html).toContain("isUserActive: () => Boolean(start || dragging || contextMenuOpened)");
    expect(html).toContain("calicoIdleDirector.start();");
    expect(html.indexOf("calicoMotion = createCalicoMotionRuntime")).toBeLessThan(
      html.indexOf("calicoIdleDirector = createCalicoIdleDirector")
    );
  });

  it("pauses idle motions around deliberate pointer and semantic actions", () => {
    const html = readOverlayHtml();

    expect(html).toContain("pauseIdleForExternalMotion(event.payload);");
    expect(html).toContain("pauseIdleForPointerInteraction(5_000);");
    expect(html).toContain("pauseIdleForPointerInteraction(6_000);");
    expect(html).toContain("calicoIdleDirector?.resetIdleClock();");
    expect(html).toContain("calicoIdleDirector?.pause(4_000);");
    expect(html).toContain("calicoIdleDirector.resetToBaseline();");
  });

  it("opens the prompt list without changing Calico motion state on click", () => {
    const html = readOverlayHtml();
    const clickBlock = html.slice(
      html.indexOf("const permission = await invoke('prompt_interaction_permission_status');"),
      html.indexOf("start = null;", html.indexOf("const permission = await invoke('prompt_interaction_permission_status');"))
    );

    expect(clickBlock).toContain("prompt_interaction_permission_status");
    expect(clickBlock).toContain("handleMissingPromptInteractionPermission(permission)");
    expect(clickBlock).toContain("toggle_prompt_popover_from_button");
    expect(clickBlock).not.toContain("applyCalicoMotion");
  });

  it("keeps existing drag and click commands for the Calico entry", () => {
    const html = readOverlayHtml();

    expect(html).toContain("prompt_button_position_cmd");
    expect(html).toContain("move_prompt_button_to");
    expect(html).toContain("toggle_prompt_popover_from_button");
    expect(html).toContain("prompt-button-drag-started");
    expect(html).toContain("prompt-button-drag-ended");
    expect(html).toContain("setPointerCapture");
    expect(html).toContain("releasePointerCapture");
  });

  it("keeps Calico click hover and drag bound to the centered entry hit area", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("--calico-hit-area-size: 132px");
    expect(html).toContain("--calico-sprite-size: 126px");
    expect(html).toContain("width: var(--calico-hit-area-size);");
    expect(html).toContain("height: var(--calico-hit-area-size);");
    expect(html).toContain("width: var(--calico-sprite-size);");
    expect(html).toContain("height: var(--calico-sprite-size);");
    expect(html).toContain("btn.addEventListener('pointerdown'");
    expect(html).toContain("btn.addEventListener('pointerup'");
    expect(html).toContain("btn.addEventListener('pointermove'");
    expect(html).toContain("btn.addEventListener('pointerenter'");
    expect(html).toContain("handleCalicoPointerEnter");
    expect(html).toContain("calicoIdleDirector?.handleAttention();");
    expect(html).not.toContain("body.addEventListener('pointerenter'");
    expect(html).not.toContain("window.addEventListener('pointerenter'");
  });

  it("starts and pauses the Calico idle director from overlay events", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("let calicoIdleDirector = null;");
    expect(html).toContain("calicoIdleDirector = createCalicoIdleDirector");
    expect(html).toContain("calicoIdleDirector.start();");
    expect(html).toContain("calicoIdleDirector?.pause");
    expect(html).toContain("calicoIdleDirector?.resetIdleClock");
    expect(html).toContain("calicoIdleDirector.resetToBaseline();");
    expect(html).toContain("applyCalicoMotion(event.payload)");
  });

  it("keeps click-to-open neutral and separate from hover attention", () => {
    const html = readFileSync("public/overlay.html", "utf8");
    const clickBlock = html.slice(
      html.indexOf("const permission = await invoke('prompt_interaction_permission_status');"),
      html.indexOf("start = null;", html.indexOf("const permission = await invoke('prompt_interaction_permission_status');"))
    );

    expect(clickBlock).not.toContain("handleAttention");
    expect(clickBlock).not.toContain("hover-attention");
    expect(clickBlock).not.toContain("applyCalicoMotion");
  });

  it("hides the prompt popover when Calico dragging starts", () => {
    const html = readOverlayHtml();

    expect(html).toContain("hidePromptPopoverForDrag");
    expect(html).toContain("await invoke('hide_prompt_popover')");
    expect(html.indexOf("hidePromptPopoverForDrag().catch")).toBeLessThan(
      html.indexOf("emit('prompt-button-drag-started')")
    );
  });

  it("opens the prompt list without awaiting target session capture", () => {
    const html = readOverlayHtml();

    expect(html).toContain("let promptPickSessionId = 0;");
    expect(html).toContain("const sessionId = ++promptPickSessionId;");
    expect(html).toContain("const permission = await invoke('prompt_interaction_permission_status');");
    expect(html).toContain("if (permission?.required && !permission.trusted)");
    expect(html).toContain("await handleMissingPromptInteractionPermission(permission);");
    expect(html).toContain("const toggleResult = await invoke('toggle_prompt_popover_from_button', { sessionId });");
    expect(html).toContain("if (toggleResult?.opened)");
    expect(html).toContain("const sessionPromise = invoke('begin_prompt_pick_session', { sessionId });");
    expect(html).toContain("void sessionPromise.catch(() => null);");
    expect(html).toContain("resetCalicoMotion();");
    expect(html).not.toContain("await invoke('begin_prompt_pick_session')");
    expect(html).not.toContain("await sessionPromise.catch");
    expect(html.indexOf("prompt_interaction_permission_status")).toBeLessThan(
      html.indexOf("toggle_prompt_popover_from_button")
    );
    expect(html.indexOf("toggle_prompt_popover_from_button")).toBeLessThan(
      html.indexOf("begin_prompt_pick_session")
    );
  });

  it("requires deliberate pointer movement before treating a click as drag", () => {
    const html = readOverlayHtml();

    expect(html).toContain("const DRAG_START_DISTANCE_PX = 10;");
    expect(html).toContain("distance(start, current) < DRAG_START_DISTANCE_PX");
    expect(html).toContain("applyCalicoMotion({ state: 'react-drag'");
    expect(html).not.toContain("distance(start, current) < 4");
  });

  it("listens for prompt autosend status and renders a Calico status bubble", () => {
    const html = readOverlayHtml();

    expect(html).toContain("prompt-autosend-status");
    expect(html).toContain("calico-status-bubble");
    expect(html).toContain("showStatusBubble");
    expect(html).toContain("hideStatusBubble");
    expect(html).toContain("statusBubble.textContent");
  });

  it("does not use manual paste as the default failure copy", () => {
    const html = readOverlayHtml();

    expect(html).not.toContain("可手动 Cmd+V");
  });

  it("checks Accessibility permission from the Calico click before opening prompts", () => {
    const html = readOverlayHtml();

    expect(html).toContain("prompt_interaction_permission_status");
    expect(html).toContain("request_prompt_interaction_permission");
    expect(html).toContain("open_accessibility_settings");
    expect(html).toContain("permissionMessages");
    expect(html).toContain("nativePromptFallback");
    expect(html).toContain("lastAccessibilitySettingsOpenAt");
    expect(html).toContain("invokeOrThrow('open_accessibility_settings')");
    expect(html).not.toContain("request_accessibility_permission_cmd");
    expect(html).not.toContain("statusBubble.dataset.action");
    expect(html).not.toContain("is-action");
    expect(html).not.toContain("Open Accessibility Permission Help");
    expect(html).not.toContain("payload.kind || 'copied'");
  });

  it("only debounces Accessibility settings after the settings open command succeeds", () => {
    const html = readOverlayHtml();
    const settingsBlock = html.slice(
      html.indexOf("const now = Date.now();"),
      html.indexOf("} catch (error)", html.indexOf("const now = Date.now();"))
    );

    expect(settingsBlock.indexOf("await invokeOrThrow('open_accessibility_settings');"))
      .toBeLessThan(settingsBlock.indexOf("lastAccessibilitySettingsOpenAt = now;"));
  });

  it("opens prompts without switching Calico into a throw-ready pose", () => {
    const html = readOverlayHtml();

    expect(html).toContain("toggle_prompt_popover_from_button");
    expect(html).not.toContain("setMotionState('ready'");
    expect(html).not.toContain('[data-motion-state="ready"]');
    expect(html).not.toContain("setSpriteSource(sprites.ready)");
    expect(html).not.toContain("calico-ready-windup");
    expect(html).not.toContain("calico-ready-projectile");
  });

  it("does not listen for paper-plane throw events", () => {
    const html = readOverlayHtml();

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
    const html = readOverlayHtml();

    expect(html).toContain("prompt-popover-dismissed");
    expect(html).toContain("resetCalicoMotion");
    expect(html).toContain("motionResetTimer");
  });

  it("does not ship the removed paper-plane flight window", () => {
    expect(existsSync("public/paper-flight.html")).toBe(false);
    expect(existsSync("public/calico/paper-plane.svg")).toBe(false);
    expect(existsSync("src-tauri/capabilities/paper-flight.json")).toBe(false);
  });
});

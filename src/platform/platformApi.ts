import { invoke } from "@tauri-apps/api/core";

export interface FrontmostApp {
  name: string;
  bundle_id: string;
}

export interface AccessibilityStatus {
  trusted: boolean;
}

export interface PromptButtonPosition {
  x: number;
  y: number;
}

export type AutosendFailureReason =
  | "copy_failed"
  | "missing_accessibility_permission"
  | "no_safe_target"
  | "paste_event_failed"
  | "return_event_failed"
  | "target_focus_failed";

export interface AutosendOutcome {
  copied: boolean;
  sent: boolean;
  error: string | null;
  reason: AutosendFailureReason | null;
}

export interface AutosendSequenceOutcome extends AutosendOutcome {
  sent_count: number;
  failed_index: number | null;
}

export async function getAccessibilityStatus(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("accessibility_status_cmd");
}

export async function requestAccessibilityPermission(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("request_accessibility_permission_cmd");
}

export async function openAccessibilitySettings(): Promise<void> {
  return invoke("open_accessibility_settings");
}

export async function getFrontmostApp(): Promise<FrontmostApp | null> {
  return invoke<FrontmostApp | null>("frontmost_app_cmd");
}

export async function pastePrompt(body: string): Promise<void> {
  return invoke("paste_prompt", { body });
}

export async function pastePromptToLastTarget(body: string): Promise<void> {
  return invoke("paste_prompt_to_last_target", { body });
}

export async function pastePromptAndSubmitToLastTarget(
  body: string
): Promise<AutosendOutcome> {
  return invoke<AutosendOutcome>("paste_prompt_and_submit_to_last_target", { body });
}

export async function pastePromptSequenceAndSubmitToLastTarget(
  bodies: string[],
  intervalMs: number
): Promise<AutosendSequenceOutcome> {
  return invoke<AutosendSequenceOutcome>(
    "paste_prompt_sequence_and_submit_to_last_target",
    { bodies, interval_ms: intervalMs }
  );
}

export async function getCurrentInputTarget(): Promise<unknown> {
  return invoke("current_input_target");
}

export async function showPromptButton(x: number, y: number): Promise<void> {
  return invoke("show_prompt_button", { x, y });
}

export async function hidePromptButton(): Promise<void> {
  return invoke("hide_prompt_button");
}

export async function showPromptPopover(x: number, y: number): Promise<void> {
  return invoke("show_prompt_popover", { x, y });
}

export async function hidePromptPopover(): Promise<void> {
  return invoke("hide_prompt_popover");
}

export async function getPromptButtonPosition(): Promise<PromptButtonPosition | null> {
  return invoke<PromptButtonPosition | null>("prompt_button_position_cmd");
}

export async function movePromptButtonTo(x: number, y: number): Promise<void> {
  return invoke("move_prompt_button_to", { x, y });
}

export async function showPromptPopoverFromButton(): Promise<void> {
  return invoke("show_prompt_popover_from_button");
}

export async function pastePromptToApp(body: string, bundle_id: string): Promise<void> {
  return invoke("paste_prompt_to_app", { body, bundle_id });
}
export async function openMainWindow(): Promise<void> {
  return invoke("open_main_window");
}

export async function quitPromptPicker(): Promise<void> {
  return invoke("quit_prompt_picker");
}

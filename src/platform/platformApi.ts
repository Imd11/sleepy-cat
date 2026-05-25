import { invoke } from "@tauri-apps/api/core";

export interface FrontmostApp {
  name: string;
  bundle_id: string;
}

export interface AccessibilityStatus {
  trusted: boolean;
}

export async function getAccessibilityStatus(): Promise<AccessibilityStatus> {
  return invoke<AccessibilityStatus>("accessibility_status_cmd");
}

export async function getFrontmostApp(): Promise<FrontmostApp | null> {
  return invoke<FrontmostApp | null>("frontmost_app_cmd");
}

export async function pastePrompt(body: string): Promise<void> {
  return invoke("paste_prompt", { body });
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
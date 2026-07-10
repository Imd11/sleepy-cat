import { existsSync, readFileSync } from "fs";
import { describe, expect, it } from "vitest";

describe("Tauri capabilities", () => {
  it("grants permissions to every Prompt Picker webview window", () => {
    const capability = JSON.parse(
      readFileSync("src-tauri/capabilities/default.json", "utf8")
    ) as { windows?: string[] };

    expect(capability.windows).toEqual(
      expect.arrayContaining([
        "main",
        "prompt-button",
        "prompt-button-input",
        "prompt-popover"
      ])
    );
    expect(capability.windows).not.toContain("paper-plane-flight");
  });

  it("does not grant permissions to the removed paper-plane flight window", () => {
    expect(existsSync("src-tauri/capabilities/paper-flight.json")).toBe(false);
  });
});

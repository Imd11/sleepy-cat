import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

describe("Tauri capabilities", () => {
  it("grants permissions to every Prompt Picker webview window", () => {
    const capability = JSON.parse(
      readFileSync("src-tauri/capabilities/default.json", "utf8")
    ) as { windows?: string[] };

    expect(capability.windows).toEqual(
      expect.arrayContaining(["main", "prompt-button", "prompt-popover"])
    );
  });
});

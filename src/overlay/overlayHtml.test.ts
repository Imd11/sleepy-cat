import { readFileSync } from "fs";
import { describe, expect, it } from "vitest";

describe("overlay button html", () => {
  it("opens Tauri button controls on right click instead of an inline menu", () => {
    const html = readFileSync("public/overlay.html", "utf8");

    expect(html).toContain("contextmenu");
    expect(html).toContain("show_prompt_button_controls_from_button");
    expect(html).toContain("event.button === 2");
    expect(html).toContain("event.ctrlKey");
    expect(html).not.toContain('id="menu"');
    expect(html).not.toContain("hide_prompt_button");
  });
});

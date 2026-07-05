import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src/styles.css"), "utf8");

function ruleBodies(selector: string): string[] {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return Array.from(styles.matchAll(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "gm"))).map(
    (match) => match[1] ?? ""
  );
}

function hasDeclaration(selector: string, declaration: string): boolean {
  return ruleBodies(selector).some((body) => body.includes(declaration));
}

describe("prompt manager layout styles", () => {
  it("keeps the manager frame stable and makes the prompt list scroll", () => {
    expect(hasDeclaration(".app-window-main", "overflow: hidden")).toBe(false);
    expect(hasDeclaration(".app-window-main.app-window-manager", "overflow: hidden")).toBe(true);
    expect(hasDeclaration(".prompt-manager", "height: 100%")).toBe(true);
    expect(hasDeclaration(".prompt-manager-content", "min-height: 0")).toBe(true);
    expect(hasDeclaration(".prompt-manager .list-panel", "min-height: 0")).toBe(true);
    expect(hasDeclaration(".prompt-manager .prompt-list", "overflow-y: auto")).toBe(true);
    expect(hasDeclaration(".prompt-manager .prompt-list", "overscroll-behavior: contain")).toBe(true);
  });
});

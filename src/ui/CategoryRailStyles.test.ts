import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src/styles.css"), "utf8");

function ruleBody(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = styles.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "m"));
  return match?.[1] ?? "";
}

describe("category rail styles", () => {
  it("uses compact vertical tab rows with inline menus", () => {
    expect(ruleBody(".category-rail-row")).toContain("position: relative");
    expect(ruleBody(".category-rail-menu")).toContain("position: absolute");
    expect(ruleBody(".category-rail-add")).toContain("min-height: 34px");
    expect(ruleBody(".category-rail-actions")).toBe("");
  });
});

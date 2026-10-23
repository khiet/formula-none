import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { listRepoFiles, ROOT, readRepoFile } from "./repo";

// Every asset is CC0 (ADR 0007), and the licence of everything vendored is
// recorded in the assets directory, so the repo can go public as promised.
describe("asset licences", () => {
  const licences = readRepoFile("assets/LICENSES.md");
  const vendored = listRepoFiles("assets").filter((path) => path.split("/").includes("kenney"));

  it("vendor at least the kart", () => {
    expect(vendored.length).toBeGreaterThan(0);
  });

  it.each(vendored)("record %s as CC0 beside the kit's own licence file", (path) => {
    const relative = path.replace(/^assets\//, "");
    const kitDir = relative.slice(0, relative.indexOf("kenney") + "kenney".length);
    expect(existsSync(join(ROOT, "assets", kitDir, "License.txt"))).toBe(true);
    if (relative.endsWith("License.txt")) return;
    expect(licences).toContain(`\`${relative}\``);
    expect(licences).toContain("CC0 1.0");
  });

  it("say the exports and the fixtures are CC0 too", () => {
    expect(licences).toContain("`karts/kart.glb`");
    expect(licences).toContain("`tracks/*/*.glb`");
  });
});

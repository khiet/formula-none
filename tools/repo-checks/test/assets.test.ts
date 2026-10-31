import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { listRepoFiles, ROOT, readRepoFile } from "./repo";

// Every asset is CC0 (ADR 0007), and the licence of everything vendored is
// recorded in the assets directory, so the repo can go public as promised.
describe("asset licences", () => {
  const licences = readRepoFile("assets/LICENSES.md");
  const vendored = listRepoFiles("assets").filter((path) => path.split("/").includes("kenney"));

  it("vendor at least the track scenery", () => {
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
    expect(licences).toMatch(
      /^\| `karts\/kart\.glb` \| this repo, built from primitives .*\| CC0 1\.0 \|$/m,
    );
    expect(licences).toContain("`tracks/*/*.glb`");
  });

  // The item slot's sprites are art too, drawn in the web app's source (ADR 0024).
  it("record the item sprites as CC0 original work", () => {
    expect(licences).toContain("`apps/web/src/item-sprites.ts`");
    expect(existsSync(join(ROOT, "apps/web/src/item-sprites.ts"))).toBe(true);
  });

  // ADR 0025 asks for a CC0 music loop, so its licence row is checked on its own.
  it("record the synthesised sounds and the music loop as CC0", () => {
    expect(licences).toContain("`audio/*.wav`");
    expect(existsSync(join(ROOT, "assets/audio/music-loop.wav"))).toBe(true);
  });

  // The `audio/*.wav` row claims every WAV is synthesised; a hand-placed file
  // with no recorded source would make it false.
  const wavs = listRepoFiles("assets/audio").filter((path) => path.endsWith(".wav"));
  it.each(wavs)("synthesise %s in audio/build.py", (path) => {
    const name = path.slice("assets/audio/".length, -".wav".length);
    expect(readRepoFile("assets/audio/build.py")).toContain(`write("${name}"`);
  });

  // ADR 0066: the images the repo draws outside assets/ are CC0 too.
  it("record the images under docs and the web app's public directory as CC0", () => {
    expect(licences).toMatch(
      /^\| `docs\/\*\*\/\*\.png`.*`apps\/web\/public\/\*`.*\| CC0 1\.0 \|$/m,
    );
  });

  it("say Purrjump was private when the marshal was adapted from it", () => {
    expect(licences).toMatch(/Purrjump \(private at the time of writing\)/);
  });
});

// ADR 0066: the code is MIT, as the root LICENSE says, in every package manifest.
describe("package licences", () => {
  it.each([
    "package.json",
    "apps/web/package.json",
    "tools/kart-poster/package.json",
    "tools/repo-checks/package.json",
  ])("%s declares MIT", (path) => {
    expect(JSON.parse(readRepoFile(path)).license).toBe("MIT");
  });
});

import { existsSync, readFileSync } from "node:fs";
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

// ADR 0069: the music loop runs a minute or more, since the 8 s loop
// repeated too often to race to. Chrome decodes only plain PCM, so the
// header is held to it too.
describe("music loop", () => {
  it("is a minute or more of 16-bit mono PCM", () => {
    const wav = readFileSync(join(ROOT, "assets/audio/music-loop.wav"));
    // The canonical 44-byte header Python's wave module writes: fmt fields
    // from byte 20, the data chunk's size at byte 40.
    const format = wav.readUInt16LE(20);
    const channels = wav.readUInt16LE(22);
    const rate = wav.readUInt32LE(24);
    const bits = wav.readUInt16LE(34);
    const dataBytes = wav.readUInt32LE(40);
    expect({ format, channels, bits }).toEqual({ format: 1, channels: 1, bits: 16 });
    expect(dataBytes / (rate * channels * (bits / 8))).toBeGreaterThanOrEqual(60);
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

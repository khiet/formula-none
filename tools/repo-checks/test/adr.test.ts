import { describe, expect, it } from "vitest";
import { listRepoDir, readRepoFile } from "./repo";

// One ADR per decision: the PRD's Implementation Decisions in the PRD's
// order, then each decision taken since. The file name is the contract; the
// number is its position.
const DECISION_SLUGS = [
  "stack",
  "sim-crate-and-seam",
  "wasm-bridge",
  "protocol-and-codegen",
  "server",
  "repo-layout",
  "art",
  "audio",
  "networking-model",
  "rates",
  "transport",
  "rooms",
  "versioning",
  "gameplay-tuning-module",
  "hosting",
  "ci-and-deploy",
  "cost-ceiling",
  "metrics",
  "server-input-queue",
  "camera-tuning",
  "kart-contact",
  "track-limits",
  "hud-language",
  "item-sprites",
  "audio-engine",
  "kart-readout",
  "handling-seam",
  "cues-from-readings",
  "standings-on-demand",
  "feedback-wired-by-the-view",
  "speed-cluster",
  "race-ends-when-the-humans-finish",
  "grand-prix-poster",
  "chibi-low-poly-art",
  "livery-palette",
  "drivers-race-bare-headed",
  "instrument-panel-in-chibi-low-poly",
  "sponsors-on-the-scenery-only",
  "room-scene-on-the-grand-prix-poster",
  "five-light-start",
  "track-obstacles",
  "runaway-wheel-bounces",
  "bots-set-off-for-the-wheel",
  "runaway-wheel-size",
  "marshal-waves-hazard-flags",
  "spline-progress-searched-near-the-last-reading",
  "suzuqa-patch",
  "snapshots-carry-the-bump",
  "server-probes-each-connection",
  "trackside-props",
  "poster-karts-wear-the-livery",
  "poster-kart-generated-from-kart-glb",
  "open-rooms-kept-on-a-volume",
  "every-course-races-three-laps",
  "tyres-raise-top-speed",
  "coins-replace-the-tyre-pickup",
  "homing-projectile-is-a-camera-quad-drone",
  "boost-is-a-champagne-bottle",
  "kept-rooms-survive-a-track-rename",
  "goldstone-joins-the-courses",
  "the-creator-picks-the-course",
  "the-room-scene-shows-the-course-card",
  "a-kart-behind-a-wall-respawns",
  "monarch-joins-the-courses",
  "a-prop-beside-a-hill-stands-on-a-terrace",
  "open-source-under-mit-and-cc0",
  "a-full-kart-still-takes-coins",
  "a-kart-holding-an-item-still-takes-item-boxes",
  "the-music-loop-runs-a-minute",
];

const expectedFiles = DECISION_SLUGS.map(
  (slug, index) => `${String(index + 1).padStart(4, "0")}-${slug}.md`,
);

describe("ADRs", () => {
  const adrFiles = listRepoDir("docs/adr").filter((name) => /^\d{4}-.*\.md$/.test(name));

  it("has exactly one numbered ADR per PRD decision", () => {
    expect(adrFiles).toEqual(expectedFiles);
  });

  it.each(expectedFiles)("%s records a status, context, decision and consequences", (file) => {
    const adr = readRepoFile(`docs/adr/${file}`);
    const number = file.slice(0, 4);
    expect(adr).toMatch(new RegExp(`^# ADR ${number}: .+`, "m"));
    expect(adr).toMatch(/^## Status\n+Accepted/m);
    expect(adr).toMatch(/^## Context/m);
    expect(adr).toMatch(/^## Decision/m);
    expect(adr).toMatch(/^## Consequences/m);
  });

  // The applied environment is costed in the hosting record and stays under
  // the ceiling that ADR 0017 sets.
  it("the hosting ADR records a monthly cost estimate under USD 25", () => {
    const hosting = readRepoFile("docs/adr/0015-hosting.md");
    const total = /^Total: about USD (\d+(?:\.\d+)?) a month/m.exec(hosting);
    expect(total).not.toBeNull();
    expect(Number(total?.[1])).toBeLessThan(25);
  });
});

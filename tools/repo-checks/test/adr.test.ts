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

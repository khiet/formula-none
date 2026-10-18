import { describe, expect, it } from "vitest";
import { readRepoFile } from "./repo";

// The vocabulary the PRD says agents must share. Each term is a
// definition heading in docs/GLOSSARY.md.
const REQUIRED_TERMS = [
  "kart",
  "room",
  "creator",
  "tick",
  "snapshot",
  "checkpoint",
  "spline progress",
  "hop",
  "drift",
  "mini-turbo",
  "spin-out",
  "livery",
  "reconciliation",
  "Scene",
  "View",
  "Instrument panel",
];

describe("GLOSSARY", () => {
  const glossary = readRepoFile("docs/GLOSSARY.md");
  const headings = glossary
    .split("\n")
    .filter((line) => line.startsWith("### "))
    .map((line) => line.slice(4).trim());

  it.each(REQUIRED_TERMS)("defines %s", (term) => {
    expect(headings).toContain(term);
  });
});

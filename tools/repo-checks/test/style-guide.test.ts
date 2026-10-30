import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ROOT, readRepoFile } from "./repo";

// docs/style-guide.html hand-copies the palette and the liveries so it opens
// from a static server with no build step. These checks keep the copy equal to
// the code, so the guide never shows a colour the game does not paint.
const guide = readRepoFile("docs/style-guide.html");

/** Every colour palette.ts exports, keyed as the guide names it: `CARBON`, `GREYS.dark`, `KART_PAINT.tyre`. */
function paletteColours(): Map<string, number[]> {
  const source = readRepoFile("apps/web/src/palette.ts");
  const numbers = new Map<string, number>();
  for (const [, name = "", value] of source.matchAll(/^export const (\w+) = (0x[0-9a-f]+);/gm)) {
    numbers.set(name, Number(value));
  }
  const resolve = (term: string) => (term.startsWith("0x") ? Number(term) : numbers.get(term));
  const colours = new Map<string, number[]>();
  for (const [name, value] of numbers) colours.set(name, [value]);
  for (const [, name = "", lit = "", mid = "", shaded = ""] of source.matchAll(
    /^export const (\w+): Tones = \{ lit: (\w+), mid: (\w+), shaded: (\w+) \};/gm,
  )) {
    colours.set(
      name,
      [lit, mid, shaded].map((term) => resolve(term) ?? Number.NaN),
    );
  }
  // A record of colours, such as GREYS and KART_PAINT, keyed as `RECORD.key`.
  for (const [, record = "", body = ""] of source.matchAll(
    /^export const (\w+) = \{\n([\s\S]*?)^\} as const;/gm,
  )) {
    for (const [, key, term = ""] of body.matchAll(/^\s+(\w+): (\w+),/gm)) {
      colours.set(`${record}.${key}`, [resolve(term) ?? Number.NaN]);
    }
  }
  return colours;
}

/** The guide's tokened palette entries: `{ token: "CARBON", tones: [...] }` or `hex: 0x...`. */
function guideColours(): Map<string, number[]> {
  const colours = new Map<string, number[]>();
  for (const [, token = "", one, tones] of guide.matchAll(
    /token: "([\w.]+)", (?:hex: (0x[0-9a-f]+)|tones: \[([^\]]+)\])/g,
  )) {
    colours.set(
      token,
      (one ?? tones ?? "").split(",").map((term) => Number(term.trim())),
    );
  }
  return colours;
}

interface LiveryCopy {
  name: string;
  key: number;
  primary: number;
  secondary: number;
  accent: number;
  skin: string;
  hair: string;
  hairColour: number;
  facialHair: string;
  earrings: boolean;
}

/** A livery match whose groups run name, key, primary, secondary, accent, skin, hair, hair colour, facial hair, earrings. */
function toLivery([
  ,
  name = "",
  key,
  primary,
  secondary,
  accent,
  skin = "",
  hair = "",
  hairColour,
  facialHair = "",
  earrings,
]: RegExpExecArray): LiveryCopy {
  return {
    name,
    key: Number(key),
    primary: Number(primary),
    secondary: Number(secondary),
    accent: Number(accent),
    skin,
    hair,
    hairColour: Number(hairColour),
    facialHair,
    earrings: earrings === "true",
  };
}

function codeLiveries(): LiveryCopy[] {
  const source = readRepoFile("apps/web/src/livery.ts");
  return [
    ...source.matchAll(
      /name: "(\w+)",\s+key: (0x\w+),\s+primary: (0x\w+),\s+secondary: (0x\w+),\s+accent: (0x\w+),\s+driver: \{\s+name: "[^"]+",\s+skin: SKIN\.(\w+),\s+hair: "([\w-]+)",\s+hairColour: (0x\w+),\s+facialHair: "([\w-]+)",\s+earrings: (true|false),/g,
    ),
  ].map(toLivery);
}

function guideLiveries(): LiveryCopy[] {
  return [
    ...guide.matchAll(
      /\{ name: "(\w+)", after: "[^"]+", key: (0x\w+), p: (0x\w+), s: (0x\w+), a: (0x\w+), d: \{ skin: SKIN\.(\w+), hair: "([\w-]+)", hc: (0x\w+), fh: "([\w-]+)", ear: (true|false) \} \}/g,
    ),
  ].map(toLivery);
}

const skinTones = (source: string) => source.match(/const SKIN = (\{[^}]+\})/)?.[1];

describe("the style guide", () => {
  it("shows every palette.ts colour with the code's values", () => {
    const code = paletteColours();
    expect(code.size).toBeGreaterThan(20);
    expect(Object.fromEntries(guideColours())).toEqual(Object.fromEntries(code));
  });

  it("copies the eight liveries and their drivers from livery.ts", () => {
    const code = codeLiveries();
    expect(code).toHaveLength(8);
    expect(guideLiveries()).toEqual(code);
    expect(skinTones(guide)).toBe(skinTones(readRepoFile("apps/web/src/livery.ts")));
  });

  // The guide cites ADRs by relative link from docs/.
  it("links only ADRs that exist", () => {
    const links = [...guide.matchAll(/href="(adr\/[^"]+)"/g)].map(([, path = ""]) => path);
    expect(links.length).toBeGreaterThan(0);
    for (const path of links) expect(existsSync(join(ROOT, "docs", path)), path).toBe(true);
  });
});

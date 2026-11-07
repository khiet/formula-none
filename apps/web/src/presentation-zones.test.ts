import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  OPEN_SKY,
  type PresentationZone,
  presentationBlend,
  presentationZonesOf,
} from "./presentation-zones";
import { centrelineOf, courseLength } from "./track";

const LAP_M = 1000;

/** A dim, echoing stretch: in from 100 m, wholly in from 120 m to 180 m, out by 200 m. */
const DIM: PresentationZone = {
  from: 100,
  fullFrom: 120,
  fullTo: 180,
  to: 200,
  light: 0.5,
  echo: 1,
  lamps: null,
  ambience: { harbour: 0 },
  crowd: 0,
};

const blendAt = (zones: readonly PresentationZone[], s: number, lap = LAP_M) =>
  presentationBlend(zones, s, lap);

describe("a presentation zone's blend", () => {
  it("leaves the open sky outside the zone", () => {
    expect(blendAt([DIM], 50)).toEqual(OPEN_SKY);
    expect(blendAt([DIM], 250)).toEqual(OPEN_SKY);
  });

  it("is the open sky at the edge it starts from and the edge it ends at", () => {
    expect(blendAt([DIM], 100)).toEqual(OPEN_SKY);
    expect(blendAt([DIM], 200)).toEqual(OPEN_SKY);
  });

  it("is the zone's whole look from the edge it is fully in to the edge it starts out", () => {
    for (const s of [120, 150, 180]) {
      const blend = blendAt([DIM], s);
      expect(blend.light).toBeCloseTo(0.5);
      expect(blend.echo).toBeCloseTo(1);
    }
  });

  it("eases in and out between the edges, never jumping", () => {
    const ramp = [100, 105, 110, 115, 120].map((s) => blendAt([DIM], s).light);
    for (let i = 1; i < ramp.length; i++) {
      expect(ramp[i]).toBeLessThan(ramp[i - 1] ?? Number.NaN);
    }
    expect(blendAt([DIM], 110).light).toBeCloseTo(0.75);
    expect(blendAt([DIM], 190).echo).toBeCloseTo(0.5);
  });

  it("runs across the start line for a zone that wraps it", () => {
    const overTheLine: PresentationZone = { ...DIM, from: 960, fullFrom: 980, fullTo: 20, to: 40 };
    expect(blendAt([overTheLine], 0).light).toBeCloseTo(0.5);
    expect(blendAt([overTheLine], 999).light).toBeCloseTo(0.5);
    expect(blendAt([overTheLine], 970).light).toBeCloseTo(0.75);
    expect(blendAt([overTheLine], 30).light).toBeCloseTo(0.75);
    expect(blendAt([overTheLine], 500)).toEqual(OPEN_SKY);
  });

  it("reads a progress a lap on, or short of the line, as the same place", () => {
    expect(blendAt([DIM], 150 + LAP_M)).toEqual(blendAt([DIM], 150));
    expect(blendAt([DIM], 150 - LAP_M)).toEqual(blendAt([DIM], 150));
  });

  it("multiplies the light where two zones overlap and keeps the louder of the rest", () => {
    const bright: PresentationZone = {
      ...DIM,
      from: 170,
      fullFrom: 190,
      fullTo: 300,
      to: 320,
      light: 1.2,
      echo: 0,
      ambience: { harbour: 1 },
      crowd: 0.6,
    };
    const blend = blendAt([DIM, bright], 190);
    expect(blend.light).toBeCloseTo(0.75 * 1.2);
    expect(blend.echo).toBeCloseTo(0.5);
    expect(blend.ambience.harbour).toBeCloseTo(1);
    expect(blend.crowd).toBeCloseTo(0.6);
  });

  it("glows brightest under each lamp and never goes dark between them", () => {
    const lamps: PresentationZone = { ...DIM, lamps: { firstAt: 126, spacing: 12 } };
    const under = blendAt([lamps], 138).lampGlow;
    const between = blendAt([lamps], 144).lampGlow;
    expect(under).toBeCloseTo(1);
    expect(between).toBeGreaterThan(0.4);
    expect(between).toBeLessThan(under);
    expect(blendAt([lamps], 50).lampGlow).toBe(0);
  });

  it("swells on from lamp to lamp over the start line, whatever the lap's length", () => {
    const overTheLine: PresentationZone = {
      ...DIM,
      from: 960,
      fullFrom: 980,
      fullTo: 20,
      to: 40,
      lamps: { firstAt: 990, spacing: 12 },
    };
    const before = presentationBlend([overTheLine], 999.99, LAP_M).lampGlow;
    const after = presentationBlend([overTheLine], 0, LAP_M).lampGlow;
    expect(after).toBeCloseTo(before, 2);
  });
});

describe("Bell Deer's bridge, seen twice", () => {
  const zones = presentationZonesOf("bell-deer");
  const lap = courseLength(centrelineOf(sidecar("bell-deer")));
  const at = (s: number) => presentationBlend(zones, s, lap);
  // Where the back straight passes over the underpass, metres along each
  // leg, the embankment's ends on the back straight and the runaway wheel's
  // crossing, from the layout.
  const UNDERPASS_S = 573.8;
  const DECK_S = 1020.7;
  const EMBANKMENT_FROM_S = 965.7;
  const EMBANKMENT_TO_S = 1075.7;
  const WHEEL_CROSSING_S = 601.8;

  it("closes in, dark and echoing, under the deck on the underpass", () => {
    const under = at(UNDERPASS_S);
    expect(under.light).toBeLessThan(0.7);
    expect(under.echo).toBeCloseTo(1);
    expect(under.lampGlow).toBe(0);
  });

  it("opens out bright and quiet on the deck, at the same place in plan", () => {
    const deck = at(DECK_S);
    expect(deck.light).toBeGreaterThan(1);
    expect(deck.echo).toBe(0);
  });

  it("is the open sky before the climb, after it and at the runaway wheel's crossing", () => {
    expect(at(EMBANKMENT_FROM_S)).toEqual(OPEN_SKY);
    expect(at(EMBANKMENT_TO_S)).toEqual(OPEN_SKY);
    expect(at(WHEEL_CROSSING_S)).toEqual(OPEN_SKY);
    expect(at(10)).toEqual(OPEN_SKY);
  });

  it("changes the light by no more than a step a metre on either leg", () => {
    for (const [from, to] of [
      [UNDERPASS_S - 40, UNDERPASS_S + 40],
      [EMBANKMENT_FROM_S, EMBANKMENT_TO_S],
    ] as const) {
      for (let s = from; s < to; s++) {
        expect(Math.abs(at(s + 1).light - at(s).light)).toBeLessThan(0.05);
      }
    }
  });
});

describe("Monarch's tunnel to the harbour", () => {
  const zones = presentationZonesOf("monarch");
  const lap = courseLength(centrelineOf(sidecar("monarch")));
  const at = (s: number) => presentationBlend(zones, s, lap);
  // The layout's segments and the tunnel's roof, in metres from the line.
  const PORTIER_S = 620;
  const ROOF_FROM_S = 678.6;
  const ROOF_TO_S = 826.07;
  const HARBOUR_FROM_S = 884.79;
  const HARBOUR_TO_S = 953.79;
  const TABAC_EXIT_S = 1000;

  it("is the open sky up to the tunnel and on the start straight", () => {
    expect(at(PORTIER_S)).toEqual(OPEN_SKY);
    expect(at(10)).toEqual(OPEN_SKY);
  });

  it("is dark, echoing and lit by its lamps under the roof", () => {
    const inside = at((ROOF_FROM_S + ROOF_TO_S) / 2);
    expect(inside.light).toBeLessThan(0.6);
    expect(inside.echo).toBeCloseTo(1);
    expect(inside.lampGlow).toBeGreaterThan(0.4);
    expect(inside.ambience.harbour).toBe(0);
  });

  it("keeps enough light inside to read the karts and the drones", () => {
    const darkest = Math.min(...Array.from({ length: 200 }, (_, i) => at(ROOF_FROM_S + i).light));
    expect(darkest).toBeGreaterThanOrEqual(0.4);
  });

  it("comes out bright on the harbour, with the water and the stand's crowd", () => {
    const harbour = at((HARBOUR_FROM_S + HARBOUR_TO_S) / 2);
    expect(harbour.light).toBeGreaterThan(1);
    expect(harbour.echo).toBe(0);
    expect(harbour.ambience.harbour).toBeCloseTo(1);
    expect(harbour.crowd).toBeGreaterThan(0);
    expect(at(TABAC_EXIT_S)).toEqual(OPEN_SKY);
  });

  it("changes the light by no more than a step a metre, so the exit is no flash", () => {
    for (let s = PORTIER_S; s < TABAC_EXIT_S; s++) {
      expect(Math.abs(at(s + 1).light - at(s).light)).toBeLessThan(0.05);
    }
  });
});

describe("a course without a zone", () => {
  it("sets none", () => {
    expect(presentationZonesOf("oval")).toEqual([]);
  });
});

function sidecar(name: string): string {
  return readFileSync(
    new URL(`../../../assets/tracks/${name}/${name}.track.json`, import.meta.url),
    "utf8",
  );
}

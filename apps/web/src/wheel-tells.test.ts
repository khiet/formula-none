import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadBridge, type Wheel } from "./bridge";
import { SAND_DUST } from "./dust";
import { IDLE_INPUT } from "./input";
import type { Vec3 } from "./kart-frame";
import { ROCKET_FLAME, WHEEL_SMOKE } from "./palette";
import type { ParticleBirth, Particles } from "./particles";
import { createWheelTellEmitter, wheelBack } from "./wheel-tells";

// The real wasm build and Brands Patch's sidecar, so the passes are the
// sim's own paths; `pnpm build:bridge` must have run first.
const wasm = readFileSync(new URL("./generated/bridge/bridge_bg.wasm", import.meta.url));
const BRANDS_PATCH = readFileSync(
  new URL("../../../assets/tracks/brands-patch/brands-patch.track.json", import.meta.url),
  "utf8",
);
const DT = 1 / 60;
const RADIUS_M = 1.2;

type Rolling = Extract<Wheel, { kind: "rolling" }>;

/** A pool that keeps a copy of every birth, for the test to read. */
function recorder(): Particles & { births: ParticleBirth[] } {
  const births: ParticleBirth[] = [];
  return {
    births,
    capacity: 0,
    emit: (birth) =>
      births.push({ ...birth, position: [...birth.position], velocity: [...birth.velocity] }),
    update() {},
    forEach() {},
  };
}

/** The first pass over Brands Patch's crossing in the race from `seed`, a rolling wheel a tick. */
async function firstPass(seed: bigint): Promise<Rolling[]> {
  const bridge = await loadBridge(wasm);
  const race = bridge.createRace(seed, 1, 0, BRANDS_PATCH);
  const pass: Rolling[] = [];
  for (;;) {
    race.step([IDLE_INPUT], DT);
    const [wheel] = race.wheels();
    if (wheel?.kind === "rolling") pass.push(wheel);
    else if (pass.length > 0) return pass;
  }
}

/** The ticks of `pass` its leg starts over on: where it rebounds off a wall. */
function wallHits(pass: readonly Rolling[]): number[] {
  return pass.flatMap((wheel, tick) =>
    tick > 0 && wheel.legFraction < (pass[tick - 1]?.legFraction ?? 0) ? [tick] : [],
  );
}

/** The ticks of `pass` it lands from its drawn bounce half way through a leg. */
function bounceLandings(pass: readonly Rolling[]): number[] {
  return pass.flatMap((wheel, tick) => {
    const before = pass[tick - 1]?.legFraction;
    return before !== undefined && before < 0.5 && wheel.legFraction >= 0.5 ? [tick] : [];
  });
}

/** Level unit direction from `from` to `to`. */
function heading(from: Rolling, to: Rolling): Vec3 {
  const dx = to.position[0] - from.position[0];
  const dz = to.position[2] - from.position[2];
  const length = Math.hypot(dx, dz);
  return [dx / length, 0, dz / length];
}

const dot = (a: Readonly<Vec3>, b: Readonly<Vec3>) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** Run the emitter over `pass`, a tick a frame, and say which tick each birth came on. */
function emitOver(pass: readonly Rolling[]) {
  const dust = recorder();
  const smoke = recorder();
  const sparks = recorder();
  const emitter = createWheelTellEmitter({ dust, smoke, sparks }, RADIUS_M, () => 0.5);
  const at = (pool: { births: ParticleBirth[] }) => {
    const ticks: number[] = [];
    let seen = 0;
    return {
      ticks,
      after(tick: number) {
        for (; seen < pool.births.length; seen++) ticks.push(tick);
      },
    };
  };
  const dustTicks = at(dust);
  const sparkTicks = at(sparks);
  emitter.emit([{ kind: "warning" }], DT);
  pass.forEach((wheel, tick) => {
    emitter.emit([wheel], DT);
    dustTicks.after(tick);
    sparkTicks.after(tick);
  });
  emitter.emit([{ kind: "quiet" }], DT);
  return { dust, smoke, sparks, dustTicks: dustTicks.ticks, sparkTicks: sparkTicks.ticks };
}

/** Each distinct tick in `ticks`, with how often it appears. */
function bursts(ticks: readonly number[]): Map<number, number> {
  const counts = new Map<number, number>();
  for (const tick of ticks) counts.set(tick, (counts.get(tick) ?? 0) + 1);
  return counts;
}

describe("the runaway wheel's tells", () => {
  it("point the flames away from the way the wheel rolls, flipping as it rebounds off a wall", async () => {
    const pass = await firstPass(7n);
    const [hit] = wallHits(pass);
    if (hit === undefined) throw new Error("no wall hit");
    const back: Vec3 = [0, 0, 0];
    const before = pass.slice(hit - 6, hit - 1);
    const after = pass.slice(hit + 1, hit + 6);
    const backs: Vec3[] = [];
    for (const legs of [before, after]) {
      const [first, last] = [legs[0], legs[legs.length - 1]];
      if (first === undefined || last === undefined) throw new Error("short leg");
      const rolls = heading(first, last);
      wheelBack(last.yaw, back);
      expect(dot(back, rolls)).toBeCloseTo(-1, 3);
      expect(Math.hypot(...back)).toBeCloseTo(1, 6);
      backs.push([...back]);
    }
    // The rebound sends it back across the track: the flames swing round with it.
    const [backBefore, backAfter] = backs;
    if (backBefore === undefined || backAfter === undefined) throw new Error("no backs");
    expect(dot(backBefore, backAfter)).toBeLessThan(0);
  });

  it("throw sparks at each wall hit and dust at each landing and wall hit, on the ticks the path says", async () => {
    const pass = await firstPass(7n);
    const hits = wallHits(pass);
    const landings = bounceLandings(pass);
    // Two rebounds, and a bounce down half way across each of the three legs.
    expect(hits).toHaveLength(2);
    expect(landings).toHaveLength(3);

    const { dust, sparks, dustTicks, sparkTicks } = emitOver(pass);
    expect(bursts(sparkTicks)).toEqual(new Map(hits.map((tick) => [tick, 8])));
    expect(bursts(dustTicks)).toEqual(
      new Map([...hits, ...landings].sort((a, b) => a - b).map((tick) => [tick, 5])),
    );
    for (const spark of sparks.births) {
      expect(spark.colour).toBe(ROCKET_FLAME);
      expect(spark.lifetimeSeconds).toBeCloseTo(0.3, 6);
    }
    for (const puff of dust.births) {
      expect(puff.colour).toBe(SAND_DUST);
      expect(puff.lifetimeSeconds).toBeCloseTo(0.45, 6);
    }
  });

  it("throw every burst when the frames come slower than the ticks", async () => {
    const pass = await firstPass(7n);
    // A frame every third tick, as a 20 Hz tab would draw it.
    const { dustTicks, sparkTicks } = emitOver(pass.filter((_, tick) => tick % 3 === 0));
    expect(sparkTicks).toHaveLength(2 * 8);
    expect(dustTicks).toHaveLength(5 * 5);
  });

  it("strike the sparks off the wall the tyre meets, flying back over the track", async () => {
    const pass = await firstPass(7n);
    const { sparks } = emitOver(pass);
    for (const hit of wallHits(pass)) {
      const [inbound, outbound] = [pass[hit - 1], pass[hit]];
      if (inbound === undefined || outbound === undefined) throw new Error("no hit");
      const into = heading(pass[hit - 5] ?? inbound, inbound);
      const out = heading(outbound, pass[hit + 5] ?? outbound);
      // The wall's normal toward the track: the turn the rebound makes.
      const away: Vec3 = [out[0] - into[0], 0, out[2] - into[2]];
      const thrown = sparks.births.filter(
        (spark) =>
          Math.hypot(
            spark.position[0] - outbound.position[0],
            spark.position[2] - outbound.position[2],
          ) <
          2 * RADIUS_M,
      );
      expect(thrown.length).toBeGreaterThanOrEqual(8);
      for (const spark of thrown) {
        expect(dot(spark.velocity, away)).toBeGreaterThan(0);
        // Struck at the tyre's edge, a radius out from the centre toward the wall.
        const toSpark: Vec3 = [
          spark.position[0] - outbound.position[0],
          0,
          spark.position[2] - outbound.position[2],
        ];
        expect(dot(toSpark, away)).toBeLessThan(0);
      }
    }
  });

  it("trail grey smoke behind the wheel while it rolls, and nothing when quiet", async () => {
    const pass = await firstPass(7n);
    const { smoke } = emitOver(pass);
    expect(smoke.births.length).toBeGreaterThan(pass.length / 10);
    for (const puff of smoke.births) expect(puff.colour).toBe(WHEEL_SMOKE);
    // Each puff is born behind the wheel it trails: its back, from the centre.
    const back: Vec3 = [0, 0, 0];
    for (const wheel of pass.filter((_, tick) => tick % 30 === 15)) {
      wheelBack(wheel.yaw, back);
      const behind = smoke.births.some((puff) => {
        const to: Vec3 = [
          puff.position[0] - wheel.position[0],
          0,
          puff.position[2] - wheel.position[2],
        ];
        return Math.hypot(...to) < 2 * RADIUS_M && dot(to, back) > 0.5 * RADIUS_M;
      });
      expect(behind).toBe(true);
    }

    const quiet = recorder();
    const emitter = createWheelTellEmitter(
      { dust: quiet, smoke: quiet, sparks: quiet },
      RADIUS_M,
      () => 0.5,
    );
    for (let tick = 0; tick < 60; tick++) {
      emitter.emit([{ kind: "quiet" }, { kind: "warning" }], DT);
    }
    expect(quiet.births).toEqual([]);
  });
});

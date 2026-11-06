import { describe, expect, it } from "vitest";
import type { StopTier } from "./bridge";
import { CREW_PER_BOX } from "./crew-roster";
import {
  CATCH_S,
  type CrewWork,
  crewWork,
  FIRST_GUNNER,
  FRONT_JACK,
  jackPlace,
  LOLLIPOP,
  lollipopSign,
  REAR_JACK,
  type Stance,
  stance,
  stopClock,
  WORK_ENDS_S,
} from "./crew-stance";
import { beatsOf } from "./stop-beats";

const CLEAN_LONGEST = 3;
/** The sim's kart, half of it each way from the mark's middle. */
const HALF_WIDTH = 0.6;
const HALF_LENGTH = 0.9;
/** A crew member's body is this wide. */
const BODY_HALF_WIDTH = 0.2;
const JACK_HALF_WIDTH = 0.17;
const GUNNERS = [0, 1, 2, 3].map((corner) => FIRST_GUNNER + corner);

function work(stationaryS: number, tier: StopTier = "clean"): CrewWork {
  return { tier, stationaryS, actFromS: CLEAN_LONGEST };
}

const at = (member: number, w: CrewWork | null, t: number): Stance => stance(member, w, t);
/** Whether a stance is beside one of the kart's wheels, within reach of its hub. */
const atAWheel = ({ x, z }: Stance) =>
  Math.abs(Math.abs(x) - 1.1) < 0.15 && Math.abs(Math.abs(z) - 0.52) < 0.15;

describe("the crew's work and clock", () => {
  it("works the kart's stop from the catch, with its stop clock negative through the guide", () => {
    expect(crewWork(null, CLEAN_LONGEST)).toBeNull();
    const stop = { tier: "slow" as const, stationaryS: 3.6, guideLeftS: 0.3, stationarySoFarS: 0 };
    expect(crewWork(stop, CLEAN_LONGEST)).toEqual({ tier: "slow", stationaryS: 3.6, actFromS: 3 });
    expect(stopClock(stop)).toBeCloseTo(-0.3);
    expect(stopClock({ ...stop, guideLeftS: 0, stationarySoFarS: 1.2 })).toBeCloseTo(1.2);
  });
});

describe("the crew's stance", () => {
  it("waits in a row on the building's side, facing the lane, with no stop", () => {
    for (let member = 0; member < CREW_PER_BOX; member += 1) {
      const waiting = at(member, null, 0);
      expect(waiting.x).toBeGreaterThan(HALF_WIDTH + 1);
      expect(waiting.pose).toBe("stand");
      expect(waiting.stride).toBeNull();
    }
    const spots = Array.from({ length: CREW_PER_BOX }, (_, member) => at(member, null, 0).z);
    expect(new Set(spots.map((z) => z.toFixed(2))).size).toBe(CREW_PER_BOX);
  });

  it("is still in the row on the catch and running out just after it", () => {
    const stop = work(2.65);
    for (let member = 0; member < CREW_PER_BOX; member += 1) {
      expect(at(member, stop, CATCH_S).x).toBeCloseTo(at(member, null, 0).x);
      const leaving = at(member, stop, CATCH_S + 0.1);
      expect(leaving.x).toBeLessThan(at(member, null, 0).x);
    }
    expect(at(FIRST_GUNNER, stop, CATCH_S + 0.1).stride).not.toBeNull();
  });

  it("has a gunner at every wheel, a jack off each end and the holder off the nose once on the mark", () => {
    const stop = work(2.65);
    const t = beatsOf(2.65).gunsOff[0] + 0.3;
    const gunners = GUNNERS.map((member) => at(member, stop, t));
    for (const gunner of gunners) {
      expect(atAWheel(gunner)).toBe(true);
      expect(gunner.pose).toBe("crouch");
    }
    const corners = new Set(gunners.map(({ x, z }) => `${Math.sign(x)},${Math.sign(z)}`));
    expect(corners.size).toBe(4);
    expect(at(FRONT_JACK, stop, t).z).toBeGreaterThan(HALF_LENGTH + 1);
    expect(at(REAR_JACK, stop, t).z).toBeLessThan(-HALF_LENGTH - 1);
    expect(jackPlace("front", stop, t).z).toBeGreaterThan(HALF_LENGTH);
    expect(jackPlace("rear", stop, t).z).toBeLessThan(-HALF_LENGTH);
    const holder = at(LOLLIPOP, stop, t);
    expect(holder.z).toBeGreaterThan(HALF_LENGTH + 1);
    expect(holder.pose).toBe("holdPole");
    expect(lollipopSign(stop, t)).toEqual({ spun: 0, lifted: 0 });
  });

  it("works each beat at the wheels: nuts off, wheel off, wheel on, nuts on, hand up", () => {
    const stop = work(2.65);
    const beats = beatsOf(2.65);
    const gunner = (t: number) => at(FIRST_GUNNER + 1, stop, t);
    const settled = 0.2;
    expect(gunner(beats.gunsOff[0] + settled)).toMatchObject({ pose: "crouch", judder: 1 });
    expect(gunner(beats.wheelsOff[0] + settled)).toMatchObject({ pose: "pull", judder: 0 });
    expect(gunner(beats.wheelsOn[0] + settled)).toMatchObject({ pose: "push", judder: 0 });
    expect(gunner(beats.gunsOn[0] + settled)).toMatchObject({ pose: "crouch", judder: 1 });
    expect(gunner(beats.done[0] + settled)).toMatchObject({ pose: "handUp", blend: 1 });
    expect(at(FRONT_JACK, stop, beats.jackUp[0] + 0.1).pose).toBe("jackPush");
  });

  it("plays a record stop's beats faster", () => {
    const record = work(2);
    const beats = beatsOf(2);
    expect(at(FIRST_GUNNER + 1, record, beats.wheelsOff[0] + 0.2).pose).toBe("pull");
    expect(at(FIRST_GUNNER + 1, work(2.65), beats.wheelsOff[0] + 0.2).pose).toBe("crouch");
  });

  it("spins the sign to GO and has the holder stepped out of the kart's path by the tick of green", () => {
    for (const stationaryS of [2, 2.65, 3.8, 6, 10.5]) {
      const stop = work(stationaryS);
      const green = stationaryS;
      expect(lollipopSign(stop, green - 0.7).spun).toBe(0);
      expect(lollipopSign(stop, green)).toEqual({ spun: 1, lifted: 0 });
      expect(lollipopSign(stop, green + 0.3).lifted).toBe(1);
      const holder = at(LOLLIPOP, stop, green);
      expect(holder.pose).toBe("raisePole");
      expect(holder.x - BODY_HALF_WIDTH).toBeGreaterThan(HALF_WIDTH);
      // Before the kart's first metre, the holder, the front jack and its member are clear of its path.
      for (const t of [green + 0.05, green + 0.3, green + 1]) {
        expect(at(LOLLIPOP, stop, t).x - BODY_HALF_WIDTH).toBeGreaterThan(HALF_WIDTH);
        expect(Math.abs(at(FRONT_JACK, stop, t).x) - BODY_HALF_WIDTH).toBeGreaterThan(HALF_WIDTH);
        expect(Math.abs(jackPlace("front", stop, t).x) - JACK_HALF_WIDTH).toBeGreaterThan(
          HALF_WIDTH,
        );
      }
    }
  });

  it("is looking down the lane a second before green, then at the driver again", () => {
    const stop = work(2.65);
    expect(at(LOLLIPOP, stop, 2.65 - 0.7).headYaw).toBeGreaterThan(0.5);
    expect(Math.abs(at(LOLLIPOP, stop, 2.65 - 1.5).headYaw)).toBeLessThan(0.01);
  });

  it("stands the gunners off the kart on green, and walks the crew back to its row after the hold", () => {
    const stop = work(2.65);
    for (const member of GUNNERS) {
      const off = at(member, stop, 2.65 + 1);
      expect(Math.abs(off.x)).toBeGreaterThan(1.5);
      expect(off.pose).toBe("stand");
    }
    for (let member = 0; member < CREW_PER_BOX; member += 1) {
      const back = at(member, stop, 2.65 + WORK_ENDS_S);
      const row = at(member, null, 0);
      expect(back.x).toBeCloseTo(row.x);
      expect(back.z).toBeCloseTo(row.z);
    }
    const parked = jackPlace("front", null, 0);
    const back = jackPlace("front", stop, 2.65 + WORK_ENDS_S);
    expect(back.x).toBeCloseTo(parked.x);
    expect(back.z).toBeCloseTo(parked.z);
  });

  it("keeps the rear lane-side gunner on a stuck nut through a slow stop's act, the rest with their hands up", () => {
    const stop = work(3.8, "slow");
    const stuck = at(FIRST_GUNNER + 2, stop, 3.2);
    expect(stuck).toMatchObject({ pose: "crouch", judder: 1 });
    expect(atAWheel(stuck)).toBe(true);
    for (const corner of [0, 1, 3]) {
      expect(at(FIRST_GUNNER + corner, stop, 3.2).pose).toBe("handUp");
    }
    expect(at(FIRST_GUNNER + 2, stop, 3.5).pose).toBe("handUp");
  });

  it("swarms the stuck wheel through a botched stop's act, the building side's gunners round the tail", () => {
    const stop = work(6, "botched");
    const swarm = GUNNERS.map((member) => at(member, stop, 5));
    for (const gunner of swarm) {
      expect(gunner.x).toBeLessThan(-HALF_WIDTH);
      expect(gunner.z).toBeLessThan(0.6);
    }
    expect(new Set(swarm.map(({ x, z }) => `${x.toFixed(2)},${z.toFixed(2)}`)).size).toBe(4);
    // Crossing, the building side's gunners pass behind the kart, never over it.
    for (let t = 3.25; t < 5; t += 0.05) {
      for (const corner of [1, 3]) {
        const { x, z } = at(FIRST_GUNNER + corner, stop, t);
        const overTheKart = Math.abs(x) < HALF_WIDTH + BODY_HALF_WIDTH && Math.abs(z) < HALF_LENGTH;
        expect(overTheKart).toBe(false);
      }
    }
    for (const member of GUNNERS) expect(at(member, stop, 5.7).pose).toBe("handUp");
  });

  it("sends the crew off when a disaster drops the jack, and redoes the front wheels once it is back up", () => {
    const stop = work(10.5, "disaster");
    expect(at(FRONT_JACK, stop, 4).pose).toBe("dismay");
    for (const member of GUNNERS) {
      const off = at(member, stop, 4);
      expect(atAWheel(off)).toBe(false);
      expect(off.pose).toBe("stand");
    }
    expect(at(FRONT_JACK, stop, 7.8).z).toBeCloseTo(at(FRONT_JACK, stop, 1).z, 1);
    const [redoFrom] = [8];
    expect(at(FIRST_GUNNER, stop, redoFrom + 0.2)).toMatchObject({ pose: "crouch", judder: 1 });
    expect(at(FIRST_GUNNER, stop, redoFrom + 0.6).pose).toBe("pull");
    expect(at(FIRST_GUNNER + 1, stop, redoFrom + 0.8).pose).toBe("push");
    expect(at(FIRST_GUNNER + 2, stop, redoFrom + 0.6).pose).toBe("handUp");
    expect(atAWheel(at(FIRST_GUNNER, stop, redoFrom + 0.6))).toBe(true);
  });
});

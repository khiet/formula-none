import { Group, type Object3D, Quaternion, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import {
  AIRSHIP_CIRCLE_M,
  AIRSHIP_CIRCUIT_MS,
  HELICOPTER_CIRCLE_M,
  HELICOPTER_CIRCUIT_MS,
  HELICOPTER_DIP_DEG,
  HELICOPTER_MAIN_ROTOR_TURNS_PER_S,
  HELICOPTER_TAIL_ROTOR_TURNS_PER_S,
  JETS_PASS_M,
  JETS_PASS_MS,
  JETS_PERIOD_MS,
  sceneryMotion,
  WHEEL_TURN_MS,
} from "./scenery-motion";

const GONDOLAS = 16;
const RADIUS_M = 12;
const UP = new Vector3(0, 1, 0);
const AXLE = new Vector3(0, 0, 1);

/**
 * A track with the Circuit Wheel as the loader hands it over: the rim's
 * node turned to its heading, with a gondola child at each rim corner,
 * named as the export names them with the dots dropped.
 */
function trackWithWheel(heading: number): { track: Object3D; rim: Object3D; gondolas: Object3D[] } {
  const track = new Group();
  const rim = new Group();
  rim.name = "Propcircuit-wheel1rim";
  rim.position.set(80, 14, -60);
  rim.quaternion.setFromAxisAngle(UP, heading);
  track.add(rim);
  const gondolas = Array.from({ length: GONDOLAS }, (_, index) => {
    const gondola = new Group();
    gondola.name = `Propcircuit-wheel1gondola${index + 1}`;
    const corner = (Math.PI * 2 * index) / GONDOLAS;
    gondola.position.set(RADIUS_M * Math.cos(corner), RADIUS_M * Math.sin(corner), 0);
    rim.add(gondola);
    return gondola;
  });
  return { track, rim, gondolas };
}

/** The angle, in turns, a node has turned about its own `about`, a rim's axle unless named, from `rest`. */
function turnsFrom(rest: Quaternion, node: Object3D, about: Vector3 = AXLE): number {
  const spin = rest.clone().invert().multiply(node.quaternion);
  const axis = new Vector3(spin.x, spin.y, spin.z);
  const angle = 2 * Math.atan2(axis.length(), spin.w);
  const sign = axis.lengthSq() === 0 ? 1 : Math.sign(axis.normalize().dot(about));
  return ((((sign * angle) / (Math.PI * 2)) % 1) + 1) % 1;
}

describe("the Circuit Wheel's motion", () => {
  // A heading past a right angle, where an Euler read of the node would flip.
  const heading = 2.5;

  it("turns the rim a full turn every 90 s from where the export left it", () => {
    const { track, rim } = trackWithWheel(heading);
    const rest = rim.quaternion.clone();
    const motion = sceneryMotion(track);
    motion.update(0);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0, 6);
    motion.update(WHEEL_TURN_MS / 4);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.25, 6);
    motion.update(WHEEL_TURN_MS / 2);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.5, 6);
    motion.update(WHEEL_TURN_MS * 3);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0, 6);
    motion.update(WHEEL_TURN_MS * 3.1);
    expect(turnsFrom(rest, rim)).toBeCloseTo(0.1, 6);
    // It turns about its axle, so the axle itself keeps the export's heading.
    const axle = AXLE.clone().applyQuaternion(rim.quaternion);
    expect(axle.distanceTo(AXLE.clone().applyQuaternion(rest))).toBeCloseTo(0, 6);
  });

  it("keeps every gondola level while the rim carries it round", () => {
    const { track, rim, gondolas } = trackWithWheel(heading);
    const level = rim.quaternion.clone();
    const motion = sceneryMotion(track);
    const top = gondolas[4];
    if (top === undefined) throw new Error("sixteen gondolas");
    rim.updateMatrixWorld(true);
    const before = top.getWorldPosition(new Vector3());
    for (const nowMs of [0, 1234, WHEEL_TURN_MS / 3, WHEEL_TURN_MS / 2, WHEEL_TURN_MS * 0.9]) {
      motion.update(nowMs);
      rim.updateMatrixWorld(true);
      for (const gondola of gondolas) {
        const world = gondola.getWorldQuaternion(new Quaternion());
        expect(Math.abs(world.dot(level)), `gondola ${gondola.name} at ${nowMs} ms`).toBeCloseTo(
          1,
          6,
        );
      }
    }
    // Half a turn on, the gondola that hung at the top hangs at the bottom.
    motion.update(WHEEL_TURN_MS / 2);
    rim.updateMatrixWorld(true);
    const after = top.getWorldPosition(new Vector3());
    expect(after.y).toBeCloseTo(rim.position.y - (before.y - rim.position.y), 6);
    expect(after.distanceTo(rim.position)).toBeCloseTo(RADIUS_M, 6);
  });

  it("leaves a track without a wheel alone", () => {
    const track = new Group();
    const kerb = new Group();
    kerb.name = "Kerb.kerb-red";
    track.add(kerb);
    const before = kerb.quaternion.clone();
    sceneryMotion(track).update(12_345);
    expect(kerb.quaternion.equals(before)).toBe(true);
  });
});

/** The hull's rest in the export: 44.95 m over the grass at the track's (0, -40). */
const NOSE_AT_REST = new Vector3(0, 44.95, 40);
/** The circle's centre, the track's (-60, -40): 60 m along -x from the nose, to its left. */
const CENTRE = new Vector3(-60, 44.95, 40);
/** The nose's heading at rest, the track's +y, which the Y-up export lays along -z. */
const NOSE_HEADING = new Vector3(0, 0, -1);

/**
 * A track with the airship as the loader hands it over: the hull's node at
 * its nose on the circle, turned to the circle's tangent, its name a child.
 */
function trackWithAirship(): { track: Object3D; hull: Object3D; name: Object3D } {
  const track = new Group();
  const hull = new Group();
  hull.name = "Propairship1hull";
  hull.position.copy(NOSE_AT_REST);
  hull.quaternion.setFromUnitVectors(new Vector3(1, 0, 0), NOSE_HEADING);
  const name = new Group();
  name.name = "Propairship1flankleft";
  name.position.set(-12, 0, -3.92);
  hull.add(name);
  track.add(hull);
  return { track, hull, name };
}

/** Where the hull's nose points in the track's frame. */
function noseOf(hull: Object3D): Vector3 {
  return new Vector3(1, 0, 0).applyQuaternion(hull.quaternion);
}

describe("the airship's motion", () => {
  it("flies the hull once round its circle every 4 min from where the export left it", () => {
    const { track, hull } = trackWithAirship();
    const motion = sceneryMotion(track);
    motion.update(0);
    expect(hull.position.distanceTo(NOSE_AT_REST)).toBeCloseTo(0, 6);
    expect(noseOf(hull).distanceTo(NOSE_HEADING)).toBeCloseTo(0, 6);
    // A quarter of the way round it has turned left through a right angle: from the
    // circle's +x to its +y in the track's frame, nose along the track's -x.
    motion.update(AIRSHIP_CIRCUIT_MS / 4);
    expect(hull.position.distanceTo(new Vector3(-60, 44.95, 40 - AIRSHIP_CIRCLE_M))).toBeCloseTo(
      0,
      5,
    );
    expect(noseOf(hull).distanceTo(new Vector3(-1, 0, 0))).toBeCloseTo(0, 6);
    motion.update(AIRSHIP_CIRCUIT_MS / 2);
    expect(hull.position.distanceTo(new Vector3(-120, 44.95, 40))).toBeCloseTo(0, 5);
    motion.update(AIRSHIP_CIRCUIT_MS);
    expect(hull.position.distanceTo(NOSE_AT_REST)).toBeCloseTo(0, 5);
    expect(noseOf(hull).distanceTo(NOSE_HEADING)).toBeCloseTo(0, 6);
    motion.update(AIRSHIP_CIRCUIT_MS * 2.1);
    const tenth = (Math.PI * 2) / 10;
    expect(
      hull.position.distanceTo(
        new Vector3(-60 + 60 * Math.cos(tenth), 44.95, 40 - 60 * Math.sin(tenth)),
      ),
    ).toBeCloseTo(0, 5);
    expect(AIRSHIP_CIRCUIT_MS).toBe(4 * 60 * 1000);
  });

  it("keeps the nose on the tangent and the hull level at every sample", () => {
    const { track, hull } = trackWithAirship();
    const motion = sceneryMotion(track);
    const before = new Vector3();
    const after = new Vector3();
    for (const nowMs of [
      0,
      1234,
      20_000,
      AIRSHIP_CIRCUIT_MS / 3,
      100_000,
      AIRSHIP_CIRCUIT_MS * 0.9,
      777_777,
    ]) {
      motion.update(nowMs);
      expect(hull.position.distanceTo(CENTRE), `${nowMs} ms`).toBeCloseTo(AIRSHIP_CIRCLE_M, 5);
      expect(hull.position.y, `${nowMs} ms`).toBeCloseTo(NOSE_AT_REST.y, 6);
      const nose = noseOf(hull);
      // The tangent: the way it is about to move, read off its own path.
      motion.update(nowMs - 1);
      before.copy(hull.position);
      motion.update(nowMs + 1);
      after.copy(hull.position);
      const travel = after.sub(before).normalize();
      expect(nose.dot(travel), `${nowMs} ms`).toBeCloseTo(1, 6);
      // Level: its own up stays the world's.
      expect(UP.clone().applyQuaternion(hull.quaternion).dot(UP), `${nowMs} ms`).toBeCloseTo(1, 6);
      // The centre stays on its left, so it circles the same way round all the way.
      const port = new Vector3(0, 0, -1).applyQuaternion(hull.quaternion);
      expect(CENTRE.clone().sub(hull.position).normalize().dot(port), `${nowMs} ms`).toBeCloseTo(
        1,
        6,
      );
    }
  });

  it("carries the name with the hull without touching it", () => {
    const { track, hull, name } = trackWithAirship();
    const local = name.position.clone();
    const motion = sceneryMotion(track);
    motion.update(AIRSHIP_CIRCUIT_MS / 3);
    expect(name.position.equals(local)).toBe(true);
    hull.updateMatrixWorld(true);
    const world = name.getWorldPosition(new Vector3());
    expect(world.distanceTo(hull.position)).toBeCloseTo(local.length(), 5);
  });
});

/** The body's rest in the export: the nose 39.95 m over the grass at the track's (270, 120). */
const HELICOPTER_NOSE_AT_REST = new Vector3(270, 39.95, -120);
/** The circle's centre, the track's (170, 120): 100 m along -x from the nose, to its left. */
const HELICOPTER_CENTRE = new Vector3(170, 39.95, -120);
/** The main rotor's turning axis in its node's frame: the builder's up. */
const ROTOR_MAST = new Vector3(0, 1, 0);

/**
 * A track with the helicopter as the loader hands it over: the body's node
 * at its nose on the circle, turned to the circle's tangent, both rotors
 * and the name hung under it.
 */
function trackWithHelicopter(): {
  track: Object3D;
  body: Object3D;
  main: Object3D;
  tail: Object3D;
  name: Object3D;
} {
  const track = new Group();
  const body = new Group();
  body.name = "Prophelicopter1body";
  body.position.copy(HELICOPTER_NOSE_AT_REST);
  body.quaternion.setFromUnitVectors(new Vector3(1, 0, 0), NOSE_HEADING);
  const main = new Group();
  main.name = "Prophelicopter1main-rotor";
  main.position.set(-3, 2.9, 0);
  const tail = new Group();
  tail.name = "Prophelicopter1tail-rotor";
  tail.position.set(-12.4, 2, -0.25);
  const name = new Group();
  name.name = "Prophelicopter1flankleft";
  name.position.set(-3, -0.4, -1.12);
  body.add(main, tail, name);
  track.add(body);
  return { track, body, main, tail, name };
}

describe("the helicopter's motion", () => {
  const dip = (HELICOPTER_DIP_DEG * Math.PI) / 180;

  it("flies the body once round its 100 m circle every 2 min from where the export left it", () => {
    const { track, body } = trackWithHelicopter();
    const motion = sceneryMotion(track);
    motion.update(0);
    expect(body.position.distanceTo(HELICOPTER_NOSE_AT_REST)).toBeCloseTo(0, 6);
    motion.update(HELICOPTER_CIRCUIT_MS / 4);
    expect(
      body.position.distanceTo(new Vector3(170, 39.95, -120 - HELICOPTER_CIRCLE_M)),
    ).toBeCloseTo(0, 5);
    motion.update(HELICOPTER_CIRCUIT_MS / 2);
    expect(body.position.distanceTo(new Vector3(70, 39.95, -120))).toBeCloseTo(0, 5);
    motion.update(HELICOPTER_CIRCUIT_MS);
    expect(body.position.distanceTo(HELICOPTER_NOSE_AT_REST)).toBeCloseTo(0, 5);
    expect(HELICOPTER_CIRCUIT_MS).toBe(2 * 60 * 1000);
    expect(HELICOPTER_CIRCLE_M).toBe(100);
  });

  it("heads along the tangent with the nose dipped 5 deg and no roll at every sample", () => {
    const { track, body } = trackWithHelicopter();
    const motion = sceneryMotion(track);
    const before = new Vector3();
    const after = new Vector3();
    for (const nowMs of [0, 1234, 20_000, HELICOPTER_CIRCUIT_MS / 3, 100_000, 777_777]) {
      motion.update(nowMs);
      expect(body.position.distanceTo(HELICOPTER_CENTRE), `${nowMs} ms`).toBeCloseTo(
        HELICOPTER_CIRCLE_M,
        5,
      );
      expect(body.position.y, `${nowMs} ms`).toBeCloseTo(HELICOPTER_NOSE_AT_REST.y, 6);
      const nose = noseOf(body);
      motion.update(nowMs - 1);
      before.copy(body.position);
      motion.update(nowMs + 1);
      after.copy(body.position);
      const travel = after.sub(before).normalize();
      // Dipped: the nose points down by the dip, and its plan points along the travel.
      expect(nose.y, `${nowMs} ms`).toBeCloseTo(-Math.sin(dip), 6);
      expect(nose.clone().setY(0).normalize().dot(travel), `${nowMs} ms`).toBeCloseTo(1, 6);
      // No roll: its left stays level and points at the centre.
      const port = new Vector3(0, 0, -1).applyQuaternion(body.quaternion);
      expect(port.y, `${nowMs} ms`).toBeCloseTo(0, 6);
      expect(
        HELICOPTER_CENTRE.clone().sub(body.position).normalize().dot(port),
        `${nowMs} ms`,
      ).toBeCloseTo(1, 6);
    }
    expect(HELICOPTER_DIP_DEG).toBe(5);
  });

  it("turns the main rotor 1.5 times a second about its mast and the tail rotor 4.5 about its axle", () => {
    const { track, main, tail } = trackWithHelicopter();
    const mainRest = main.quaternion.clone();
    const tailRest = tail.quaternion.clone();
    const motion = sceneryMotion(track);
    for (const nowMs of [0, 100, 200, 1000, 61_100]) {
      motion.update(nowMs);
      const seconds = nowMs / 1000;
      expect(turnsFrom(mainRest, main, ROTOR_MAST), `main at ${nowMs} ms`).toBeCloseTo(
        (seconds * HELICOPTER_MAIN_ROTOR_TURNS_PER_S) % 1,
        6,
      );
      expect(turnsFrom(tailRest, tail), `tail at ${nowMs} ms`).toBeCloseTo(
        (seconds * HELICOPTER_TAIL_ROTOR_TURNS_PER_S) % 1,
        6,
      );
      // Each turns about its own axis, which keeps where the export left it.
      const mast = ROTOR_MAST.clone().applyQuaternion(main.quaternion);
      expect(mast.distanceTo(ROTOR_MAST.clone().applyQuaternion(mainRest))).toBeCloseTo(0, 6);
      const axle = AXLE.clone().applyQuaternion(tail.quaternion);
      expect(axle.distanceTo(AXLE.clone().applyQuaternion(tailRest))).toBeCloseTo(0, 6);
    }
    expect(HELICOPTER_MAIN_ROTOR_TURNS_PER_S).toBe(1.5);
    expect(HELICOPTER_TAIL_ROTOR_TURNS_PER_S).toBe(4.5);
  });

  it("carries the rotors and the name with the body without moving them on it", () => {
    const { track, main, tail, name } = trackWithHelicopter();
    const local = [main, tail, name].map((node) => node.position.clone());
    const motion = sceneryMotion(track);
    motion.update(HELICOPTER_CIRCUIT_MS / 3);
    expect([main, tail, name].map((node) => node.position)).toEqual(local);
    expect(name.quaternion.equals(new Quaternion())).toBe(true);
  });
});

/**
 * The formation's rest in the export: the leader's nose 59.95 m over the grass
 * over the hangars, the track's (191.43, -57.19), which the Y-up export lays at
 * (x, up, -y).
 */
const JETS_AT_REST = new Vector3(191.43, 59.95, 57.19);
/** The race's direction down the Hangar Straight, 257.5 deg from the track's +x, in the export's frame. */
const HANGAR_STRAIGHT_RACE = new Vector3(
  Math.cos((257.5 * Math.PI) / 180),
  0,
  -Math.sin((257.5 * Math.PI) / 180),
);

/** A track with the display jets as the loader hands them over: the formation's node turned against the race. */
function trackWithJets(): { track: Object3D; formation: Object3D } {
  const track = new Group();
  const formation = new Group();
  formation.name = "Propjets1formation";
  formation.position.copy(JETS_AT_REST);
  formation.quaternion.setFromUnitVectors(
    new Vector3(1, 0, 0),
    HANGAR_STRAIGHT_RACE.clone().negate(),
  );
  track.add(formation);
  return { track, formation };
}

describe("the display jets' flypast", () => {
  /** Where the leader's nose should be `metres` along the pass from its middle. */
  const along = (rest: Quaternion, metres: number) =>
    new Vector3(1, 0, 0).applyQuaternion(rest).multiplyScalar(metres).add(JETS_AT_REST);

  it("flies the 600 m pass in 6 s, centred over the hangars, every 30 s", () => {
    const { track, formation } = trackWithJets();
    const rest = formation.quaternion.clone();
    const motion = sceneryMotion(track);
    for (const period of [0, 1, 37]) {
      const start = period * JETS_PERIOD_MS;
      for (const [nowMs, metres] of [
        [start, -300],
        [start + 1500, -150],
        [start + 3000, 0],
        [start + 4500, 150],
        [start + 5900, 290],
      ] as const) {
        motion.update(nowMs);
        expect(formation.visible, `${nowMs} ms`).toBe(true);
        expect(formation.position.distanceTo(along(rest, metres)), `${nowMs} ms`).toBeCloseTo(0, 4);
      }
    }
    expect(JETS_PASS_M).toBe(600);
    expect(JETS_PASS_MS).toBe(6_000);
    expect(JETS_PERIOD_MS).toBe(30_000);
  });

  it("hides the formation between passes and shows it again at the next", () => {
    const { track, formation } = trackWithJets();
    const motion = sceneryMotion(track);
    for (const nowMs of [JETS_PASS_MS, 15_000, JETS_PERIOD_MS - 1, 2 * JETS_PERIOD_MS + 7_000]) {
      motion.update(nowMs);
      expect(formation.visible, `${nowMs} ms`).toBe(false);
    }
    motion.update(JETS_PERIOD_MS + 10);
    expect(formation.visible).toBe(true);
  });

  it("flies nose first against the race and never turns", () => {
    const { track, formation } = trackWithJets();
    const rest = formation.quaternion.clone();
    const motion = sceneryMotion(track);
    motion.update(1000);
    const before = formation.position.clone();
    motion.update(1100);
    const travel = formation.position.clone().sub(before);
    // 100 m/s: 10 m in a tenth of a second.
    expect(travel.length()).toBeCloseTo(10, 4);
    travel.normalize();
    expect(travel.dot(HANGAR_STRAIGHT_RACE)).toBeCloseTo(-1, 6);
    expect(travel.dot(noseOf(formation))).toBeCloseTo(1, 6);
    expect(formation.quaternion.equals(rest)).toBe(true);
  });
});

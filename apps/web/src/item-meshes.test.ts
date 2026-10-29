import {
  Box3,
  Color,
  type Material,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  type Object3D,
  Vector3,
} from "three";
import { describe, expect, it } from "vitest";
import type { Wheel } from "./bridge";
import {
  hazardConeMesh,
  hazardFlag,
  homingGullMesh,
  itemBoxMesh,
  marshalBeacon,
  marshalMesh,
  runawayWheelBounce,
  runawayWheelFlames,
  runawayWheelMesh,
} from "./item-meshes";
import {
  BEACON_LIT,
  BEACON_OFF,
  HAZARD_FLAG_STRIPE,
  ROCKET,
  ROCKET_FLAME,
  ROCKET_STRAP,
  SIGNAL_YELLOW,
  WHEEL_FLAME,
  WHEEL_TYRE,
} from "./palette";

/** A runaway wheel's radius, any will do. */
const RADIUS_M = 0.45;

function materialsOf(object: Object3D): Material[] {
  const found: Material[] = [];
  object.traverse((child) => {
    if (child instanceof Mesh) found.push(...[child.material].flat());
  });
  return found;
}

function coloursOf(object: Object3D): number[] {
  return materialsOf(object).flatMap((material) =>
    material instanceof MeshLambertMaterial ? [material.color.getHex()] : [],
  );
}

/** The colours `object` draws whatever the sun does: its unlit materials' and their vertices'. */
function glowsOf(object: Object3D): number[] {
  const found: number[] = [];
  const colour = new Color();
  object.traverse((child) => {
    if (!(child instanceof Mesh) || !(child.material instanceof MeshBasicMaterial)) return;
    if (!child.material.vertexColors) {
      found.push(child.material.color.getHex());
      return;
    }
    const colours = child.geometry.getAttribute("color");
    for (let vertex = 0; vertex < colours.count; vertex++) {
      found.push(colour.fromBufferAttribute(colours, vertex).getHex());
    }
  });
  return found;
}

describe("the item meshes", () => {
  it.each([
    ["item box", itemBoxMesh],
    ["hazard cone", hazardConeMesh],
    ["homing gull", homingGullMesh],
    ["hazard flag", () => hazardFlag().mesh],
    ["marshal", () => marshalMesh().figure],
  ])("draw the %s lit by the sun and flat-shaded, facet by facet", (_, mesh) => {
    const materials = materialsOf(mesh());
    expect(materials.length).toBeGreaterThan(0);
    for (const material of materials) {
      expect("isMeshBasicMaterial" in material).toBe(false);
      expect(material).toHaveProperty("flatShading", true);
    }
  });

  it("paint the item box in the item slot's golds", () => {
    expect(coloursOf(itemBoxMesh())).toEqual(expect.arrayContaining([0xf8e39a, 0xe0b54a]));
  });

  it("paint the cone hazard orange with a white band and a grey slab", () => {
    expect(coloursOf(hazardConeMesh())).toEqual(
      expect.arrayContaining([0xff5522, 0xffffff, 0x9aa0a8]),
    );
  });

  it("paint the gull white with a grey wing, an orange beak and a red eye", () => {
    expect(coloursOf(homingGullMesh())).toEqual(
      expect.arrayContaining([0xffffff, 0xb9bec5, 0xff5522, 0xd00a12]),
    );
  });

  it("draw the runaway wheel's tyre and spokes lit, and its rim and hub glowing red hot", () => {
    const wheel = runawayWheelMesh(RADIUS_M);
    expect(coloursOf(wheel)).toEqual(expect.arrayContaining([WHEEL_TYRE, ROCKET_STRAP]));
    // Flame yellow at the hub, the rocket's red at the rim's edge.
    expect(new Set(glowsOf(wheel))).toEqual(new Set([ROCKET_FLAME, ROCKET.mid]));
  });

  it("burn three glowing tongues off the wheel's back, orange round a yellow core", () => {
    const flames = runawayWheelFlames(RADIUS_M);
    expect(flames.children).toHaveLength(3);
    expect(coloursOf(flames)).toEqual([]);
    expect(new Set(glowsOf(flames))).toEqual(new Set([WHEEL_FLAME, ROCKET_FLAME]));
    // Off the back of the tyre, toward +z, reaching past it.
    const bounds = new Box3().setFromObject(flames);
    expect(bounds.min.z).toBeGreaterThan(0);
    expect(bounds.max.z).toBeGreaterThan(RADIUS_M);
  });

  it("draw the runaway wheel as big as it is asked for, its axle across its roll", () => {
    const size = new Box3().setFromObject(runawayWheelMesh(RADIUS_M)).getSize(new Vector3());
    // The spokes stand a hair proud of the tyre's faces.
    expect(size.y).toBeCloseTo(2 * RADIUS_M, 1);
    expect(size.z).toBeCloseTo(2 * RADIUS_M, 1);
    expect(size.x).toBeLessThan(size.z);
  });

  it("bounce the runaway wheel twice a leg, down at each wall", () => {
    for (const down of [0, 1 / 2, 1]) {
      expect(runawayWheelBounce(down)).toBeCloseTo(0, 6);
    }
    for (const up of [1 / 4, 3 / 4]) {
      expect(runawayWheelBounce(up)).toBeGreaterThan(0.3);
    }
  });

  it("fly a hazard flag 0.9 by 0.62 m from the top of a 1 m pole, striped black over yellow", () => {
    const flag = hazardFlag();
    expect(coloursOf(flag.mesh)).toEqual(
      expect.arrayContaining([SIGNAL_YELLOW.mid, HAZARD_FLAG_STRIPE, 0xcfd2d6]),
    );
    flag.fly(0);
    const bounds = new Box3().setFromObject(flag.mesh);
    expect(bounds.min.y).toBeCloseTo(0, 6);
    expect(bounds.max.y).toBeCloseTo(1, 6);
    expect(bounds.max.x).toBeCloseTo(0.9, 6);
    expect(bounds.max.y - clothOf(flag.mesh).min.y).toBeCloseTo(0.62, 6);
  });

  it("ripple the hazard flag's cloth as it flies", () => {
    const flag = hazardFlag();
    flag.fly(0);
    const first = clothOf(flag.mesh);
    flag.fly(0.1);
    expect(clothOf(flag.mesh).equals(first)).toBe(false);
  });
});

/** The bounds of what a hazard flag shows of its cloth. */
function clothOf(flag: Object3D): Box3 {
  const cloth = flag.getObjectByName("cloth");
  if (cloth === undefined) throw new Error("no cloth");
  return new Box3().setFromObject(cloth);
}

const QUIET: Wheel = { kind: "quiet" };
const WARNING: Wheel = { kind: "warning", position: [0, 0, 0], yaw: 0 };
const ROLLING: Wheel = {
  kind: "rolling",
  position: [0, 0, 0],
  yaw: 0,
  fraction: 0.5,
  legFraction: 0.5,
  crossingYaw: 0,
};

/** A second of the View's clock sampled every frame at 60 Hz. */
const A_SECOND = Array.from({ length: 60 }, (_, frame) => frame / 60);

/** What the marshal shows posed for `wheel` at `clockSeconds`. */
function posed(wheel: Wheel, clockSeconds = 0) {
  const marshal = marshalMesh();
  marshal.pose(wheel.kind, clockSeconds);
  marshal.figure.updateMatrixWorld(true);
  const visibleIn = (part: Object3D) => {
    let visible = true;
    part.traverseAncestors((ancestor) => {
      visible &&= ancestor.visible;
    });
    return visible && part.visible;
  };
  const shown = (name: string) => {
    const part = marshal.figure.getObjectByName(name);
    if (part === undefined) throw new Error(`no ${name}`);
    return visibleIn(part);
  };
  const head = marshal.figure.getObjectByName("head")?.getWorldPosition(new Vector3()).y ?? NaN;
  const flags = ["flag.left", "flag.right"].map((name) => {
    const flag = marshal.figure.getObjectByName(name);
    if (flag === undefined) throw new Error(`no ${name}`);
    const cloth = flag.getObjectByName("cloth");
    if (cloth === undefined) throw new Error(`no cloth on ${name}`);
    return { flying: visibleIn(cloth), tilt: flag.rotation.z };
  });
  return { shown, head, flags };
}

describe("the marshal", () => {
  it("sits calm on his stool with his tea while the wheel is quiet, his flags furled", () => {
    const quiet = posed(QUIET);
    expect(quiet.shown("stool")).toBe(true);
    expect(quiet.shown("cup")).toBe(true);
    expect(quiet.shown("face.calm")).toBe(true);
    expect(quiet.shown("face.panic")).toBe(false);
    expect(quiet.flags.map(({ flying }) => flying)).toEqual([false, false]);
    expect(quiet.head).toBeLessThan(posed(WARNING).head - 0.1);
  });

  it.each([
    ["the flag is up", WARNING],
    ["the wheel rolls", ROLLING],
  ])("jumps up in a panic waving both flags while %s", (_, wheel) => {
    const up = posed(wheel);
    expect(up.shown("stool")).toBe(false);
    expect(up.shown("cup")).toBe(false);
    expect(up.shown("face.calm")).toBe(false);
    expect(up.shown("face.panic")).toBe(true);
    expect(up.flags.map(({ flying }) => flying)).toEqual([true, true]);
  });

  it("stands still through the warning, about 1.75 m tall", () => {
    const heads = A_SECOND.map((clock) => posed(WARNING, clock).head);
    expect(Math.max(...heads) - Math.min(...heads)).toBeCloseTo(0, 6);
    const marshal = marshalMesh();
    marshal.pose("warning", 0);
    const head = marshal.figure.getObjectByName("head");
    if (head === undefined) throw new Error("no head");
    expect(new Box3().setFromObject(head).max.y).toBeCloseTo(1.75, 1);
  });

  it("hops up to 0.18 m while the wheel rolls", () => {
    const standing = posed(WARNING).head;
    const hops = A_SECOND.map((clock) => posed(ROLLING, clock).head - standing);
    expect(Math.min(...hops)).toBeGreaterThanOrEqual(-1e-9);
    expect(Math.max(...hops)).toBeCloseTo(0.18, 2);
  });

  it("waves the flags out of phase through 34 degrees either side of 18 degrees out", () => {
    const degrees = (radians: number) => (radians * 180) / Math.PI;
    // Each flag's tilt away from his head, out toward its own side.
    const left = A_SECOND.map((clock) => degrees(posed(WARNING, clock).flags[0]?.tilt ?? NaN));
    const right = A_SECOND.map((clock) => -degrees(posed(WARNING, clock).flags[1]?.tilt ?? NaN));
    for (const tilts of [left, right]) {
      expect(Math.min(...tilts)).toBeCloseTo(18 - 34, 0);
      expect(Math.max(...tilts)).toBeCloseTo(18 + 34, 0);
    }
    // Out of phase: one is out when the other is in.
    left.forEach((tilt, frame) => {
      expect(tilt + (right[frame] ?? NaN)).toBeCloseTo(36, 6);
    });
  });
});

/** The lamp under a beacon's glass. */
function domeOf(beacon: Object3D): MeshLambertMaterial {
  const dome = beacon.getObjectByName("dome");
  if (!(dome instanceof Mesh) || !(dome.material instanceof MeshLambertMaterial)) {
    throw new Error("no dome");
  }
  return dome.material;
}

describe("the marshal post's beacon", () => {
  it("is dark while the wheel is quiet", () => {
    const beacon = marshalBeacon();
    beacon.show("quiet", 0);
    expect(domeOf(beacon.mesh).color.getHex()).toBe(BEACON_OFF);
    expect(domeOf(beacon.mesh).emissive.getHex()).toBe(0);
    expect(beacon.mesh.getObjectByName("glow")?.visible).toBe(false);
    expect(beacon.mesh.getObjectByName("beam")?.visible).toBe(false);
  });

  it.each([
    ["the flag is up", WARNING],
    ["the wheel rolls", ROLLING],
  ])("is lit and sweeps its beam while %s", (_, wheel) => {
    const beacon = marshalBeacon();
    const sweeps = A_SECOND.map((clock) => {
      beacon.show(wheel.kind, clock);
      expect(domeOf(beacon.mesh).color.getHex()).toBe(BEACON_LIT);
      expect(domeOf(beacon.mesh).emissive.getHex()).toBe(BEACON_LIT);
      expect(beacon.mesh.getObjectByName("glow")?.visible).toBe(true);
      const beam = beacon.mesh.getObjectByName("beam");
      expect(beam?.visible).toBe(true);
      return beam?.rotation.y ?? NaN;
    });
    expect(new Set(sweeps).size).toBe(sweeps.length);
  });

  it("goes dark again once the pass is over", () => {
    const beacon = marshalBeacon();
    beacon.show("rolling", 1);
    beacon.show("quiet", 2);
    expect(domeOf(beacon.mesh).color.getHex()).toBe(BEACON_OFF);
    expect(beacon.mesh.getObjectByName("beam")?.visible).toBe(false);
  });
});

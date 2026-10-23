import { Group, Mesh, MeshBasicMaterial, PlaneGeometry } from "three";
import { describe, expect, it } from "vitest";
import { trackProbe } from "./track-probe";

/** Half the kart's height: the road is never further above a standing kart's centre. */
const REACH_M = 0.3;

/** A stand-in for the track: one flat 20 m square of road at y = 0, in a scene like the glTF's. */
function flatRoad(): Group {
  const track = new Group();
  track.add(new Mesh(new PlaneGeometry(20, 20).rotateX(-Math.PI / 2), new MeshBasicMaterial()));
  // The renderer refreshes the world matrices each frame; the probe reads them as left.
  track.updateMatrixWorld(true);
  return track;
}

function expectClose(actual: readonly number[], expected: readonly number[], digits = 4) {
  expect(actual).toHaveLength(expected.length);
  actual.forEach((value, index) => {
    expect(value, `component ${index}`).toBeCloseTo(expected[index] ?? Number.NaN, digits);
  });
}

describe("the track probe", () => {
  it("finds the surface straight under a point and the surface's normal", () => {
    const hit = trackProbe(flatRoad(), REACH_M).under(3, 0.41, -7);
    expectClose(hit?.point ?? [], [3, 0, -7]);
    expectClose(hit?.normal ?? [], [0, 1, 0]);
  });

  it("reports nothing with no surface below", () => {
    expect(trackProbe(flatRoad(), REACH_M).under(30, 0.3, 0)).toBeUndefined();
  });

  it("still finds the road under a kart pressed a little into it", () => {
    const hit = trackProbe(flatRoad(), REACH_M).under(0, -0.2, 0);
    expectClose(hit?.point ?? [], [0, 0, 0]);
  });

  it("passes over an overhang between its start and the kart for the road below", () => {
    const track = flatRoad();
    // A roof 0.8 m up, within the metre the ray starts above the kart's centre.
    const roof = new Mesh(new PlaneGeometry(4, 4).rotateX(-Math.PI / 2), new MeshBasicMaterial());
    roof.position.y = 0.8;
    track.add(roof);
    track.updateMatrixWorld(true);
    const hit = trackProbe(track, REACH_M).under(0, 0.3, 0);
    expectClose(hit?.point ?? [], [0, 0, 0]);
  });

  it("gives the normal in the world's frame when the road mesh is tilted", () => {
    const track = new Group();
    const road = new Mesh(new PlaneGeometry(20, 20).rotateX(-Math.PI / 2), new MeshBasicMaterial());
    // A 0.2 rad bank about Z: the surface climbs to the right and its normal leans left.
    road.rotation.z = 0.2;
    track.add(road);
    track.updateMatrixWorld(true);
    const hit = trackProbe(track, REACH_M).under(-1, 0.3, 0);
    expectClose(hit?.point ?? [], [-1, -Math.tan(0.2), 0]);
    expectClose(hit?.normal ?? [], [-Math.sin(0.2), Math.cos(0.2), 0]);
  });
});

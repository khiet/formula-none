// The pit boxes (ADR 0081, ADR 0082), built in code like the items on the
// road (ADR 0037): at each slot's pit box its painted mark in the slot's
// key colour, a light on a gantry that reads red through the stop and green
// as the kart is let go, and a crew of six chibi figures in the slot's
// overalls, instanced across the boxes. Everything here reads the kart
// readouts: the sim rolls the stop and nothing drawn enters it. The crews
// are drawn only while the camera is near the lane, so a race round the
// rest of the course never draws forty-eight figures nobody can see.

import {
  BoxGeometry,
  Color,
  Group,
  InstancedMesh,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  Object3D,
  SphereGeometry,
  Vector3,
} from "three";
import type { KartReadout, KartSize } from "./bridge";
import { livery } from "./livery";
import { CARBON, GO_GREEN, LAMP_OFF, POSTER_RED, VALUE_WHITE } from "./palette";
import type { PitBox } from "./track";

/** The crews are drawn while the camera is within this of a pit box. */
export const PIT_DRAW_RANGE_M = 160;
/** The light holds green this long after the kart is let go. */
export const GREEN_HOLD_S = 3;
export const CREW_PER_BOX = 6;

/** The mark: a painted outline a kart's size and a bit, on the asphalt. */
const MARK_CLEARANCE_M = 0.5;
const MARK_LINE_M = 0.2;
const MARK_LIFT_M = 0.02;
/** The gantry light: a post at the mark's front right, its lamps up where the driver sees them. */
const GANTRY_POST_M = 2.2;
const GANTRY_POST_THICKNESS_M = 0.08;
const LAMP_M = 0.26;
const LAMP_BOX_M = 0.34;
/** A crew member: a chibi block of overalls under a white helmet. */
const CREW_BODY_WIDTH_M = 0.42;
const CREW_BODY_HEIGHT_M = 0.78;
const CREW_BODY_DEPTH_M = 0.3;
const CREW_HELMET_RADIUS_M = 0.21;
/** Crouched at a wheel the figure is this share of its standing height. */
const CREW_CROUCH_SHARE = 0.72;
/** Idle, the crew waits in a row this far to the right of the mark, this far apart. */
const CREW_ROW_RIGHT_M = 2.6;
const CREW_ROW_PITCH_M = 0.7;
/** At work the gunners kneel this far out from each wheel and the jacks stand this far off each end. */
const GUNNER_OUT_M = 0.5;
const JACK_OFF_END_M = 0.9;
/** The gunners judder at the wheel this fast through this much. */
const GUN_JUDDER_RAD_PER_S = 28;
const GUN_JUDDER_M = 0.03;
/** The idle crew bobs this slowly through this much, out of step with its neighbour. */
const IDLE_BOB_RAD_PER_S = 2.4;
const IDLE_BOB_M = 0.02;

export interface PitBoxMeshes {
  readonly mesh: Group;
  /**
   * Light each box and pose its crew from its slot's kart in `karts`,
   * `frameSeconds` on: the light red while the kart is in its stop, green
   * for `GREEN_HOLD_S` after it is let go and off otherwise; the crew at the
   * wheels while the kart stands on its mark and waiting in its row
   * otherwise. Crews are drawn only within `PIT_DRAW_RANGE_M` of `camera`.
   */
  show(
    karts: readonly KartReadout[],
    camera: readonly [number, number, number],
    frameSeconds: number,
  ): void;
}

/** Which lamp a box's light shows. */
export type PitLight = "off" | "red" | "green";

/**
 * The light a box shows for its kart: red through the stop, green for
 * `GREEN_HOLD_S` after `letGoAt`, the View's clock as the kart was let go,
 * and off otherwise.
 */
export function pitLight(
  kart: KartReadout | undefined,
  letGoAt: number | null,
  clockSeconds: number,
): PitLight {
  if (kart?.stop != null) return "red";
  if (letGoAt !== null && clockSeconds - letGoAt < GREEN_HOLD_S) return "green";
  return "off";
}

function lambert(colour: number): MeshLambertMaterial {
  return new MeshLambertMaterial({ color: colour, flatShading: true });
}

/**
 * The pit boxes at `boxes`, one per grid slot in slot order, each sized to
 * `kartSize`: a static mark and gantry per box, and two instanced meshes,
 * the bodies and the helmets, carrying every box's crew.
 */
export function pitBoxMeshes(boxes: readonly PitBox[], kartSize: KartSize): PitBoxMeshes {
  const mesh = new Group();
  mesh.name = "pitBoxes";
  const markLength = kartSize.length + 2 * MARK_CLEARANCE_M;
  const markWidth = kartSize.width + 2 * MARK_CLEARANCE_M;
  const lampMaterials = {
    off: new MeshBasicMaterial({ color: LAMP_OFF.mid }),
    red: new MeshBasicMaterial({ color: POSTER_RED.lit }),
    green: new MeshBasicMaterial({ color: GO_GREEN.lit }),
  };
  const crewCount = boxes.length * CREW_PER_BOX;
  const bodies = new InstancedMesh(
    new BoxGeometry(CREW_BODY_WIDTH_M, CREW_BODY_HEIGHT_M, CREW_BODY_DEPTH_M),
    lambert(VALUE_WHITE),
    crewCount,
  );
  bodies.name = "crewBodies";
  const helmets = new InstancedMesh(
    new SphereGeometry(CREW_HELMET_RADIUS_M, 8, 6),
    lambert(VALUE_WHITE),
    crewCount,
  );
  helmets.name = "crewHelmets";
  mesh.add(bodies, helmets);

  const placed = boxes.map((box, slot) => {
    const frame = new Group();
    frame.name = `pitBox${slot}`;
    frame.position.set(...box.position);
    frame.rotation.y = box.yaw;
    const key = lambert(livery(slot).key);
    // The mark: four bars round the kart's footprint, flat on the road.
    const across = new BoxGeometry(markWidth, MARK_LINE_M, MARK_LINE_M);
    const along = new BoxGeometry(MARK_LINE_M, MARK_LINE_M, markLength);
    for (const end of [-1, 1]) {
      const bar = new Mesh(across, key);
      bar.position.set(0, MARK_LIFT_M, (end * markLength) / 2);
      bar.name = "mark";
      frame.add(bar);
      const side = new Mesh(along, key);
      side.position.set((end * markWidth) / 2, MARK_LIFT_M, 0);
      side.name = "mark";
      frame.add(side);
    }
    // The gantry at the mark's front right corner, its lamps facing the kart.
    const gantry = new Group();
    gantry.name = "gantry";
    gantry.position.set(markWidth / 2 + MARK_LINE_M, 0, -markLength / 2);
    const post = new Mesh(
      new BoxGeometry(GANTRY_POST_THICKNESS_M, GANTRY_POST_M, GANTRY_POST_THICKNESS_M),
      lambert(CARBON.mid),
    );
    post.position.y = GANTRY_POST_M / 2;
    const housing = new Mesh(
      new BoxGeometry(LAMP_BOX_M, LAMP_BOX_M * 2, LAMP_BOX_M),
      lambert(CARBON.shaded),
    );
    housing.position.y = GANTRY_POST_M - LAMP_BOX_M;
    const lampGeometry = new BoxGeometry(LAMP_M, LAMP_M, 0.04);
    const redLamp = new Mesh(lampGeometry, lampMaterials.off);
    redLamp.name = "lamp.red";
    redLamp.position.set(0, GANTRY_POST_M - LAMP_BOX_M / 2, LAMP_BOX_M / 2 + 0.01);
    const greenLamp = new Mesh(lampGeometry, lampMaterials.off);
    greenLamp.name = "lamp.green";
    greenLamp.position.set(0, GANTRY_POST_M - LAMP_BOX_M * 1.5, LAMP_BOX_M / 2 + 0.01);
    gantry.add(post, housing, redLamp, greenLamp);
    frame.add(gantry);
    mesh.add(frame);
    const colour = new Color(livery(slot).key);
    for (let member = 0; member < CREW_PER_BOX; member += 1) {
      bodies.setColorAt(slot * CREW_PER_BOX + member, colour);
    }
    return { frame, redLamp, greenLamp, letGoAt: null as number | null, wasStopping: false };
  });
  if (bodies.instanceColor !== null) bodies.instanceColor.needsUpdate = true;

  const matrix = new Matrix4();
  const hidden = new Matrix4().makeScale(0, 0, 0);
  const world = new Vector3();
  const cameraAt = new Vector3();
  const dummy = new Object3D();
  let clockSeconds = 0;
  /**
   * The crew member's place in the box's frame, metres across and along
   * the mark, and whether they are crouched.
   */
  const stance = (
    member: number,
    atWork: boolean,
  ): { x: number; z: number; crouched: boolean; judder: boolean } => {
    if (!atWork) {
      return {
        x: CREW_ROW_RIGHT_M,
        z: (member - (CREW_PER_BOX - 1) / 2) * CREW_ROW_PITCH_M,
        crouched: false,
        judder: false,
      };
    }
    if (member < 4) {
      // The gunners, one at each wheel: left and right, front and rear.
      const side = member % 2 === 0 ? -1 : 1;
      const end = member < 2 ? -1 : 1;
      return {
        x: side * (kartSize.width / 2 + GUNNER_OUT_M),
        z: (end * kartSize.length) / 2,
        crouched: true,
        judder: true,
      };
    }
    // The jacks, one off each end.
    const end = member === 4 ? -1 : 1;
    return {
      x: 0,
      z: end * (kartSize.length / 2 + JACK_OFF_END_M),
      crouched: false,
      judder: false,
    };
  };

  return {
    mesh,
    show(karts, camera, frameSeconds) {
      clockSeconds += frameSeconds;
      cameraAt.set(...camera);
      placed.forEach((box, slot) => {
        const kart = karts[slot];
        const stopping = kart?.stop != null;
        if (box.wasStopping && !stopping) box.letGoAt = clockSeconds;
        box.wasStopping = stopping;
        const light = pitLight(kart, box.letGoAt, clockSeconds);
        box.redLamp.material = light === "red" ? lampMaterials.red : lampMaterials.off;
        box.greenLamp.material = light === "green" ? lampMaterials.green : lampMaterials.off;
        box.frame.getWorldPosition(world);
        const near = world.distanceTo(cameraAt) <= PIT_DRAW_RANGE_M;
        const atWork = kart?.stop?.guideLeftS === 0;
        box.frame.updateWorldMatrix(true, false);
        for (let member = 0; member < CREW_PER_BOX; member += 1) {
          const index = slot * CREW_PER_BOX + member;
          if (!near) {
            bodies.setMatrixAt(index, hidden);
            helmets.setMatrixAt(index, hidden);
            continue;
          }
          const { x, z, crouched, judder } = stance(member, atWork);
          const height = crouched ? CREW_CROUCH_SHARE : 1;
          const phase =
            clockSeconds * (judder ? GUN_JUDDER_RAD_PER_S : IDLE_BOB_RAD_PER_S) + member;
          const sway = judder ? Math.sin(phase) * GUN_JUDDER_M : 0;
          const bob = judder ? 0 : Math.sin(phase) * IDLE_BOB_M;
          const bodyHeight = CREW_BODY_HEIGHT_M * height;
          dummy.position.set(x + sway, bodyHeight / 2 + bob, z);
          dummy.scale.set(1, height, 1);
          dummy.rotation.set(0, 0, 0);
          dummy.updateMatrix();
          matrix.multiplyMatrices(box.frame.matrixWorld, dummy.matrix);
          bodies.setMatrixAt(index, matrix);
          dummy.position.set(x + sway, bodyHeight + CREW_HELMET_RADIUS_M * 0.9 + bob, z);
          dummy.scale.set(1, 1, 1);
          dummy.updateMatrix();
          matrix.multiplyMatrices(box.frame.matrixWorld, dummy.matrix);
          helmets.setMatrixAt(index, matrix);
        }
      });
      bodies.instanceMatrix.needsUpdate = true;
      helmets.instanceMatrix.needsUpdate = true;
    },
  };
}

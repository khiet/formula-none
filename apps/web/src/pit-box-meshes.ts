// The pit boxes (ADR 0081, ADR 0082), built in code like the items on the
// road (ADR 0037): at each slot's pit box its painted mark in the slot's
// key colour, a light on a gantry that reads red through the stop and green
// as the kart is let go, a boom over the box that the air lines hang from,
// and the lollipop crew of seven in the slot's overalls: a jack at each
// end, a gunner at every wheel and a sign holder off the nose. Every crew
// member wears the driver's head from kart.glb, a face of their own from
// the roster under a headset. Everything here reads the kart readouts: the
// sim rolls the stop and nothing drawn enters it; the crew's places and
// poses are `stance` of the stop clock. The crews are drawn only while the
// camera is near the lane, so a race round the rest of the course never
// draws fifty-six figures nobody can see.

import {
  BoxGeometry,
  BufferAttribute,
  BufferGeometry,
  CanvasTexture,
  CircleGeometry,
  Color,
  CylinderGeometry,
  Euler,
  Group,
  InstancedMesh,
  LineBasicMaterial,
  LineSegments,
  type Material,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  MeshStandardMaterial,
  type Object3D,
  SphereGeometry,
  SRGBColorSpace,
  Vector3,
} from "three";
import type { KartReadout, KartSize } from "./bridge";
import { CREW_PER_BOX, crewRoster } from "./crew-roster";
import {
  CREW_ROLES,
  type CrewWork,
  crewWork,
  FIRST_GUNNER,
  jackPlace,
  jointsOf,
  LOLLIPOP,
  lollipopSign,
  stance,
  stopClock,
  WORK_ENDS_S,
} from "./crew-stance";
import { FACE_PARTS, partsOf } from "./driver-parts";
import { INSTRUMENT_FONT_FAMILY, loadInstrumentFont } from "./instrument-panel";
import { DRIVER_MATERIALS } from "./kart-model";
import { livery } from "./livery";
import { CARBON, GO_GREEN, GREYS, INK, LAMP_OFF, POSTER_RED, VALUE_WHITE, WHITE } from "./palette";
import { GREEN_HOLD_S, KART_LIFT_M, kartOnJacks } from "./stop-beats";
import type { PitBox } from "./track";

export { CREW_PER_BOX, GREEN_HOLD_S };

/** The crews are drawn while the camera is within this of a pit box. */
export const PIT_DRAW_RANGE_M = 160;

/** The mark: a painted outline round a kart's footprint with this much clearance, in lines this wide, this far off the asphalt. */
const MARK_CLEARANCE_M = 0.5;
const MARK_LINE_M = 0.2;
const MARK_LIFT_M = 0.02;
/** The gantry light: a post at the mark's front corner on the building's side, its lamps up where the driver sees them. */
const GANTRY_POST_M = 2.2;
const GANTRY_POST_THICKNESS_M = 0.08;
const LAMP_M = 0.26;
const LAMP_BOX_M = 0.34;
/** The boom: across the box from the building, above the chase camera's frame once the kart is on its mark. */
export const BOOM_HEIGHT_M = 4.8;
const BOOM_FROM_M = -2.05;
const BOOM_TO_M = 5.15;
const REEL_X_M = 1.2;
const REEL_DROP_M = 0.17;
/** An air line from its reel to its gun: this many points, sagging this much at its middle. */
const HOSE_POINTS = 7;
const HOSE_SAG_M = 0.8;

/** A crew member: a chibi body under the driver's head at this share of its size. */
const CREW_HEAD_SHARE = 0.26 / 0.3;
const THIGH_M = 0.24;
const SHIN_M = 0.22;
const BOOT_M = 0.04;
const TORSO_M = 0.42;
const UPPER_ARM_M = 0.24;
const FOREARM_M = 0.22;
/** The gunners' hands judder at the nut this fast through this much. */
const JUDDER_RAD_PER_S = 28;
const JUDDER_M = 0.03;
/** The crew in its row bobs this slowly through this much, out of step with its neighbour. */
const IDLE_BOB_RAD_PER_S = 2.4;
const IDLE_BOB_M = 0.015;

/** The colours the crew's kit is drawn in. */
const GLOVE = WHITE.mid;
const BOOT = CARBON.shaded;
const STEEL = GREYS.dark;
const JACK_RED = POSTER_RED.mid;

/** What the View reads of the stop's tuning, and the head every crew member wears. */
export interface PitBoxOptions {
  /** The longest a clean stop lasts: past it the crew acts the roll out. */
  readonly cleanStopSeconds: number;
  /**
   * kart.glb's `head` node, with every face part under it: each crew
   * member's head is its geometry, instanced, with only their face's parts.
   */
  readonly head: Object3D;
}

export interface PitBoxMeshes {
  readonly mesh: Group;
  /**
   * Light each pit box and pose its crew from its slot's kart in `karts`,
   * `frameSeconds` on: the light red while the kart is in its stop, green
   * for `GREEN_HOLD_S` after it is let go and off otherwise; the crew at
   * the stance of the stop clock, which runs on past green from the
   * stop's stationary time until the crew is back in its row. Crews are
   * drawn only within `PIT_DRAW_RANGE_M` of `camera`.
   */
  show(
    karts: readonly KartReadout[],
    camera: readonly [number, number, number],
    frameSeconds: number,
  ): void;
}

/** Which lamp a pit box's light shows. */
export type PitLight = "off" | "red" | "green";

/**
 * The light a pit box shows: red while its kart is `stopping`, green for
 * `GREEN_HOLD_S` after `letGoAt`, the View's clock as the kart was let go,
 * and off otherwise.
 */
export function pitLight(
  stopping: boolean,
  letGoAt: number | null,
  clockSeconds: number,
): PitLight {
  if (stopping) return "red";
  if (letGoAt !== null && clockSeconds - letGoAt < GREEN_HOLD_S) return "green";
  return "off";
}

function lambert(colour: number): MeshLambertMaterial {
  return new MeshLambertMaterial({ color: colour, flatShading: true });
}

/** One kind of part, instanced across every crew or every box, coloured per instance. */
interface Batch {
  readonly mesh: InstancedMesh;
  /** Instances per figure or per box. */
  readonly per: number;
}

function batch(
  name: string,
  geometry: BufferGeometry,
  material: Material,
  owners: number,
  per: number,
): Batch {
  const mesh = new InstancedMesh(geometry, material, owners * per);
  mesh.name = name;
  // Never culled by its bounds, which a frame drawn out of range would leave empty.
  mesh.frustumCulled = false;
  return { mesh, per };
}

/** A geometry of the head and the face part it belongs to, or null for the head itself. */
interface HeadPiece {
  readonly part: string | null;
  readonly batch: Batch;
  /** The piece's place in the head's frame. */
  readonly local: Matrix4;
  /** Whether each instance takes the face's skin or hair colour, or keeps the material's. */
  readonly paint: "skin" | "hair" | null;
}

/** Every mesh under `head` as a piece of it, instanced for `figures` crew members. */
function headPieces(head: Object3D, figures: number): HeadPiece[] {
  head.updateWorldMatrix(true, true);
  const into = head.matrixWorld.clone().invert();
  const pieces: HeadPiece[] = [];
  head.traverse((object) => {
    if (!(object instanceof Mesh) || Array.isArray(object.material)) return;
    let part: string | null = null;
    for (let at: Object3D | null = object; at !== null && at !== head; at = at.parent) {
      if (FACE_PARTS.has(at.name)) part = at.name;
    }
    const source = object.material as Material;
    const paint =
      source.name === DRIVER_MATERIALS.skin
        ? "skin"
        : source.name === DRIVER_MATERIALS.hairColour
          ? "hair"
          : null;
    const material = source.clone();
    if (paint !== null && material instanceof MeshStandardMaterial) material.color.setHex(0xffffff);
    pieces.push({
      part,
      batch: batch(
        `crew.${part ?? "head"}.${pieces.length}`,
        object.geometry,
        material,
        figures,
        1,
      ),
      local: into.clone().multiply(object.matrixWorld),
      paint,
    });
  });
  return pieces;
}

/** A sign face: its word in the Instrument panel's pixel face, once the face is loaded. */
function signFace(word: string, background: number, ink: number): Material {
  const material = new MeshLambertMaterial({ color: background });
  if (typeof document === "undefined") return material;
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 256;
  const context = canvas.getContext("2d");
  if (context === null) return material;
  const texture = new CanvasTexture(canvas);
  texture.colorSpace = SRGBColorSpace;
  const draw = () => {
    context.fillStyle = `#${background.toString(16).padStart(6, "0")}`;
    context.fillRect(0, 0, 256, 256);
    context.fillStyle = `#${ink.toString(16).padStart(6, "0")}`;
    context.font = `${word.length > 3 ? 84 : 120}px "${INSTRUMENT_FONT_FAMILY}", monospace`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText(word, 128, 136);
    texture.needsUpdate = true;
  };
  draw();
  void loadInstrumentFont().then(draw);
  material.color.setHex(0xffffff);
  material.map = texture;
  return material;
}

/**
 * The pit boxes at `pitBoxes`, one per grid slot in slot order, each sized
 * to `kartSize`: a static mark, gantry and boom per pit box, and the crews
 * and their kit as instanced meshes, one per kind of part, across every
 * box. The air lines are one set of line segments across every box.
 */
export function pitBoxMeshes(
  pitBoxes: readonly PitBox[],
  kartSize: KartSize,
  options: PitBoxOptions,
): PitBoxMeshes {
  const mesh = new Group();
  mesh.name = "pitBoxes";
  const markLength = kartSize.length + 2 * MARK_CLEARANCE_M;
  const markWidth = kartSize.width + 2 * MARK_CLEARANCE_M;
  const lampMaterials = {
    off: new MeshBasicMaterial({ color: LAMP_OFF.mid }),
    red: new MeshBasicMaterial({ color: POSTER_RED.lit }),
    green: new MeshBasicMaterial({ color: GO_GREEN.lit }),
  };
  const figures = pitBoxes.length * CREW_PER_BOX;
  const boxes = pitBoxes.length;
  const unitBox = new BoxGeometry(1, 1, 1);
  const unitBall = new SphereGeometry(1, 8, 6);
  const unitDrum = new CylinderGeometry(1, 1, 1, 8);
  const tinted = () => lambert(VALUE_WHITE);
  // The body: overalls in the slot's key colour, and the trim, gloves and boots.
  const suit = batch("crewSuit", unitBox, tinted(), figures, 9);
  const joints = batch("crewShoulders", unitBall, tinted(), figures, 2);
  const gloves = batch("crewGloves", unitBall, lambert(GLOVE), figures, 2);
  const trim = batch("crewTrim", unitBox, tinted(), figures, 4);
  const heads = headPieces(options.head, figures);
  // The kit: four guns, two jacks and a lollipop per box.
  const kitBoxes = batch("pitKitBoxes", unitBox, tinted(), boxes, 4 * 2 + 2 * 5 + 1);
  const kitDrums = batch("pitKitDrums", unitDrum, tinted(), boxes, 4 * 2 + 2 * 2 + 1);
  const signDisc = new CircleGeometry(0.32, 16);
  const stopFace = batch(
    "signStop",
    signDisc,
    signFace("STOP", POSTER_RED.mid, VALUE_WHITE),
    boxes,
    1,
  );
  const goFace = batch("signGo", signDisc, signFace("GO", GO_GREEN.mid, INK), boxes, 1);
  const batches = [suit, joints, gloves, trim, kitBoxes, kitDrums, stopFace, goFace];
  for (const { mesh: instanced } of [...batches, ...heads.map((piece) => piece.batch)]) {
    mesh.add(instanced);
  }
  const hoseCount = boxes * 4;
  const hosePositions = new Float32Array(hoseCount * (HOSE_POINTS - 1) * 2 * 3);
  const hoseGeometry = new BufferGeometry();
  hoseGeometry.setAttribute("position", new BufferAttribute(hosePositions, 3));
  const airLines = new LineSegments(hoseGeometry, new LineBasicMaterial({ color: CARBON.lit }));
  airLines.name = "airLines";
  airLines.frustumCulled = false;
  mesh.add(airLines);

  const colour = new Color();
  const set = (target: Batch, owner: number, index: number, hex: number) =>
    target.mesh.setColorAt(owner * target.per + index, colour.setHex(hex));

  const placed = pitBoxes.map((pitBox, slot) => {
    const frame = new Group();
    frame.name = `pitBox${slot}`;
    frame.position.set(...pitBox.position);
    frame.rotation.y = pitBox.yaw;
    frame.updateMatrix();
    const key = livery(slot).key;
    const keyMaterial = lambert(key);
    // The mark: four bars round the kart's footprint, flat on the road.
    const across = new BoxGeometry(markWidth, MARK_LINE_M, MARK_LINE_M);
    const along = new BoxGeometry(MARK_LINE_M, MARK_LINE_M, markLength);
    for (const end of [-1, 1]) {
      const bar = new Mesh(across, keyMaterial);
      bar.position.set(0, MARK_LIFT_M, (end * markLength) / 2);
      bar.name = "mark";
      frame.add(bar);
      const side = new Mesh(along, keyMaterial);
      side.position.set((end * markWidth) / 2, MARK_LIFT_M, 0);
      side.name = "mark";
      frame.add(side);
    }
    // The gantry at the mark's front corner on the building's side, clear of
    // the fast lane, its lamps facing the kart.
    const gantry = new Group();
    gantry.name = "gantry";
    gantry.position.set(pitBox.buildingSide * (markWidth / 2 + MARK_LINE_M), 0, -markLength / 2);
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
    // The boom from the building's face across the box, a reel over each side of the kart.
    const boom = new Group();
    boom.name = "boom";
    const carbon = lambert(CARBON.mid);
    const beam = new Mesh(new BoxGeometry(BOOM_TO_M - BOOM_FROM_M, 0.1, 0.1), carbon);
    beam.position.set((pitBox.buildingSide * (BOOM_TO_M + BOOM_FROM_M)) / 2, BOOM_HEIGHT_M, 0);
    const end = new Mesh(new BoxGeometry(0.1, 0.1, 1.6), carbon);
    end.position.set(pitBox.buildingSide * (BOOM_FROM_M + 0.15), BOOM_HEIGHT_M, 0);
    boom.add(beam, end);
    for (const side of [-1, 1]) {
      const reel = new Mesh(new CylinderGeometry(0.14, 0.14, 0.12, 10), lambert(CARBON.shaded));
      reel.rotation.z = Math.PI / 2;
      reel.position.set(side * REEL_X_M, BOOM_HEIGHT_M - REEL_DROP_M, 0);
      boom.add(reel);
    }
    frame.add(boom);
    mesh.add(frame);
    // Each crew member's colours: the overalls in the slot's key, the
    // trim white over carbon, and the roster's face.
    const roster = crewRoster(slot);
    roster.forEach((face, member) => {
      const figure = slot * CREW_PER_BOX + member;
      for (let index = 0; index < suit.per; index += 1) set(suit, figure, index, key);
      for (let index = 0; index < joints.per; index += 1) set(joints, figure, index, key);
      set(trim, figure, 0, VALUE_WHITE);
      for (let index = 1; index < trim.per; index += 1)
        set(trim, figure, index, index === 1 ? CARBON.mid : BOOT);
      for (const piece of heads) {
        if (piece.paint === null) continue;
        set(piece.batch, figure, 0, piece.paint === "skin" ? face.skin : face.hairColour);
      }
    });
    // The kit's colours, in the order `show` places its parts.
    const boxColours = [
      ...Array.from({ length: 4 }, () => [key, STEEL]).flat(),
      ...Array.from({ length: 2 }, () => [CARBON.mid, JACK_RED, JACK_RED, STEEL, JACK_RED]).flat(),
      STEEL,
    ];
    boxColours.forEach((hex, index) => {
      set(kitBoxes, slot, index, hex);
    });
    const drumColours = [
      ...Array.from({ length: 4 }, () => [CARBON.mid, STEEL]).flat(),
      ...Array.from({ length: 4 }, () => STEEL),
      CARBON.mid,
    ];
    drumColours.forEach((hex, index) => {
      set(kitDrums, slot, index, hex);
    });
    return {
      frame,
      buildingSide: pitBox.buildingSide,
      worn: roster.map((face) => partsOf(face, "headset")),
      heights: roster.map((face) => face.height),
      redLamp,
      greenLamp,
      letGoAt: null as number | null,
      wasStopping: false,
      /** The stop the crew is working, kept past green until the crew is back in its row. */
      work: null as CrewWork | null,
      /** The stop clock when the kart was let go: the stop's stationary time. */
      greenAt: 0,
    };
  });
  for (const { mesh: instanced } of [...batches, ...heads.map((piece) => piece.batch)]) {
    if (instanced.instanceColor !== null) instanced.instanceColor.needsUpdate = true;
  }

  const hidden = new Matrix4().makeScale(0, 0, 0);
  const world = new Vector3();
  const cameraAt = new Vector3();
  const euler = new Euler();
  const scratch = new Matrix4();
  const root = new Matrix4();
  const hips = new Matrix4();
  const torso = new Matrix4();
  const head = new Matrix4();
  const limb = new Matrix4();
  const lower = new Matrix4();
  const gun = new Matrix4();
  const kit = new Matrix4();
  const point = new Vector3();
  const reel = new Vector3();
  const faceTurn = new Matrix4().makeRotationY(Math.PI);
  const headScale = new Matrix4().makeScale(CREW_HEAD_SHARE, CREW_HEAD_SHARE, CREW_HEAD_SHARE);
  let clockSeconds = 0;

  /** `out` = `parent` × translate × rotate. */
  const joint = (
    out: Matrix4,
    parent: Matrix4,
    x: number,
    y: number,
    z: number,
    rx = 0,
    ry = 0,
    rz = 0,
  ) =>
    out
      .makeRotationFromEuler(euler.set(rx, ry, rz))
      .setPosition(x, y, z)
      .premultiply(parent);
  /** Place instance `index` of `owner` in `target`: a unit part scaled to `size` at `offset` in `parent`'s frame. */
  const part = (
    target: Batch,
    owner: number,
    index: number,
    parent: Matrix4,
    offset: readonly [number, number, number],
    size: readonly [number, number, number],
  ) => {
    scratch
      .makeScale(...size)
      .setPosition(...offset)
      .premultiply(parent);
    target.mesh.setMatrixAt(owner * target.per + index, scratch);
  };
  const hide = (target: Batch, owner: number) => {
    for (let index = 0; index < target.per; index += 1) {
      target.mesh.setMatrixAt(owner * target.per + index, hidden);
    }
  };
  /** The pit box's frame from its own terms: `x` toward the building, `z` toward the nose, yaw from the nose. */
  const place = (
    out: Matrix4,
    frame: Matrix4,
    buildingSide: 1 | -1,
    x: number,
    z: number,
    yaw: number,
  ) =>
    joint(
      out,
      frame,
      buildingSide * x,
      0,
      -z,
      0,
      Math.atan2(buildingSide * Math.sin(yaw), -Math.cos(yaw)),
    );

  return {
    mesh,
    show(karts, camera, frameSeconds) {
      clockSeconds += frameSeconds;
      cameraAt.set(...camera);
      placed.forEach((pitBox, slot) => {
        const stop = karts[slot]?.stop ?? null;
        const stopping = stop !== null;
        if (pitBox.wasStopping && !stopping) {
          pitBox.letGoAt = clockSeconds;
          pitBox.greenAt = pitBox.work?.stationaryS ?? 0;
        }
        pitBox.wasStopping = stopping;
        const light = pitLight(stopping, pitBox.letGoAt, clockSeconds);
        pitBox.redLamp.material = light === "red" ? lampMaterials.red : lampMaterials.off;
        pitBox.greenLamp.material = light === "green" ? lampMaterials.green : lampMaterials.off;
        // The stop clock: the kart's own through its stop, run on from green after it.
        let secondsOnMark = 0;
        if (stop !== null) {
          pitBox.work = crewWork(stop, options.cleanStopSeconds);
          secondsOnMark = stopClock(stop);
        } else if (pitBox.work !== null && pitBox.letGoAt !== null) {
          secondsOnMark = pitBox.greenAt + (clockSeconds - pitBox.letGoAt);
          if (secondsOnMark - pitBox.greenAt > WORK_ENDS_S) pitBox.work = null;
        } else {
          pitBox.work = null;
        }
        const work = pitBox.work;
        const lift = kartOnJacks(stop, options.cleanStopSeconds);
        pitBox.frame.getWorldPosition(world);
        const near = world.distanceTo(cameraAt) <= PIT_DRAW_RANGE_M;
        const frame = pitBox.frame.matrix;
        const side = pitBox.buildingSide;
        const hoseBase = slot * 4 * (HOSE_POINTS - 1) * 2 * 3;
        if (!near) {
          for (let member = 0; member < CREW_PER_BOX; member += 1) {
            const figure = slot * CREW_PER_BOX + member;
            for (const target of [suit, joints, gloves, trim]) hide(target, figure);
            for (const piece of heads) hide(piece.batch, figure);
          }
          for (const target of [kitBoxes, kitDrums, stopFace, goFace]) hide(target, slot);
          hosePositions.fill(0, hoseBase, hoseBase + 4 * (HOSE_POINTS - 1) * 2 * 3);
          return;
        }
        for (let member = 0; member < CREW_PER_BOX; member += 1) {
          const figure = slot * CREW_PER_BOX + member;
          const now = stance(member, work, secondsOnMark);
          const pose = jointsOf(now.from, now.pose, now.blend);
          let { hipL, hipR, kneeL, kneeR, shoulderL, shoulderR } = pose;
          let bob = 0;
          if (now.stride !== null) {
            const swing = Math.sin(now.stride);
            hipL += 0.7 * swing;
            hipR -= 0.7 * swing;
            kneeL -= 0.5 * Math.max(0, -swing);
            kneeR -= 0.5 * Math.max(0, swing);
            shoulderL -= 0.5 * swing;
            shoulderR += 0.5 * swing;
            bob = Math.abs(Math.cos(now.stride)) * 0.03;
          } else if (work === null) {
            bob = Math.sin(clockSeconds * IDLE_BOB_RAD_PER_S + member) * IDLE_BOB_M;
          }
          const legHeight = (hip: number, knee: number) =>
            THIGH_M * Math.cos(hip) + SHIN_M * Math.cos(hip + knee) + BOOT_M;
          const hipY = Math.max(legHeight(hipL, kneeL), legHeight(hipR, kneeR), 0.22) + bob;
          const shake = Math.sin(clockSeconds * JUDDER_RAD_PER_S + member) * JUDDER_M * now.judder;
          const height = pitBox.heights[member] ?? 1;
          place(root, frame, side, now.x, now.z, now.yaw);
          root.multiply(scratch.makeScale(height, height, height));
          joint(hips, root, shake, hipY, 0);
          joint(torso, hips, 0, 0, 0, pose.lean);
          part(suit, figure, 0, torso, [0, TORSO_M / 2, 0], [0.4, TORSO_M, 0.26]);
          part(trim, figure, 0, torso, [0, TORSO_M * 0.62, 0.135], [0.28, 0.08, 0.02]);
          part(trim, figure, 1, torso, [0, 0.03, 0], [0.42, 0.05, 0.28]);
          joint(
            head,
            torso,
            0,
            TORSO_M + 0.01,
            0,
            pose.headPitch + now.headPitch - pose.lean * 0.6,
            pose.headYaw + now.headYaw,
          );
          head.multiply(headScale).multiply(faceTurn);
          const worn = pitBox.worn[member];
          for (const piece of heads) {
            if (piece.part !== null && worn?.has(piece.part) !== true) {
              piece.batch.mesh.setMatrixAt(figure, hidden);
              continue;
            }
            scratch.multiplyMatrices(head, piece.local);
            piece.batch.mesh.setMatrixAt(figure, scratch);
          }
          // Arms and legs, left then right.
          for (const [index, sign] of [
            [0, -1],
            [1, 1],
          ] as const) {
            const shoulder = sign < 0 ? shoulderL : shoulderR;
            const roll = sign < 0 ? pose.rollL : pose.rollR;
            const elbow = sign < 0 ? pose.elbowL : pose.elbowR;
            joint(limb, torso, sign * 0.25, TORSO_M - 0.05, 0, -shoulder, 0, roll);
            part(joints, figure, index, limb, [0, 0, 0], [0.07, 0.07, 0.07]);
            part(suit, figure, 1 + index, limb, [0, -UPPER_ARM_M / 2, 0], [0.1, UPPER_ARM_M, 0.1]);
            joint(lower, limb, 0, -UPPER_ARM_M, 0, -elbow);
            part(suit, figure, 3 + index, lower, [0, -FOREARM_M / 2, 0], [0.09, FOREARM_M, 0.09]);
            part(gloves, figure, index, lower, [0, -FOREARM_M - 0.03, 0], [0.06, 0.06, 0.06]);
            const hip = sign < 0 ? hipL : hipR;
            const knee = sign < 0 ? kneeL : kneeR;
            joint(limb, hips, sign * 0.11, 0, 0, -hip);
            part(suit, figure, 5 + index, limb, [0, -THIGH_M / 2, 0], [0.14, THIGH_M, 0.14]);
            joint(lower, limb, 0, -THIGH_M, 0, -knee);
            part(suit, figure, 7 + index, lower, [0, -SHIN_M / 2, 0], [0.12, SHIN_M, 0.12]);
            part(
              trim,
              figure,
              2 + index,
              lower,
              [0, -SHIN_M - BOOT_M / 2 + 0.02, 0.03],
              [0.14, BOOT_M * 2, 0.22],
            );
          }
          const gunner = member - FIRST_GUNNER;
          if (gunner >= 0 && gunner < 4) {
            // The gun in both hands in front of the chest, on its air line from the boom.
            joint(gun, torso, 0.1, 0.1, 0.42);
            joint(kit, gun, 0, 0, 0.1, Math.PI / 2);
            part(kitDrums, slot, gunner * 2, kit, [0, 0, 0], [0.05, 0.3, 0.05]);
            joint(kit, gun, 0, 0, 0.31, Math.PI / 2);
            part(kitDrums, slot, gunner * 2 + 1, kit, [0, 0, 0], [0.035, 0.12, 0.035]);
            part(kitBoxes, slot, gunner * 2, gun, [0, -0.08, 0.04], [0.04, 0.12, 0.05]);
            part(kitBoxes, slot, gunner * 2 + 1, gun, [0, -0.13, -0.08], [0.02, 0.02, 0.2]);
            point.set(0, -0.13, -0.18).applyMatrix4(gun);
            const reelSide = gunner % 2 === 0 ? -1 : 1;
            reel
              .set(side * reelSide * REEL_X_M, BOOM_HEIGHT_M - REEL_DROP_M, 0)
              .applyMatrix4(frame);
            let at = hoseBase + gunner * (HOSE_POINTS - 1) * 2 * 3;
            for (let k = 0; k < HOSE_POINTS - 1; k += 1) {
              for (const u of [k / (HOSE_POINTS - 1), (k + 1) / (HOSE_POINTS - 1)]) {
                hosePositions[at] = reel.x + (point.x - reel.x) * u;
                hosePositions[at + 1] =
                  reel.y + (point.y - reel.y) * u - HOSE_SAG_M * 4 * u * (1 - u);
                hosePositions[at + 2] = reel.z + (point.z - reel.z) * u;
                at += 3;
              }
            }
          }
          if (member === LOLLIPOP) {
            // The sign held up off the nose, STOP to the driver until it spins to GO.
            const sign = lollipopSign(work, secondsOnMark);
            point.set(0.32, 0.25 + sign.lifted * 0.7, 0.3 - sign.lifted * 0.2).applyMatrix4(root);
            const facing = Math.PI + sign.spun * Math.PI;
            const turned = Math.atan2(side * Math.sin(facing), -Math.cos(facing));
            joint(
              kit,
              scratch.identity(),
              point.x,
              point.y,
              point.z,
              0,
              (pitBoxes[slot]?.yaw ?? 0) + turned,
            );
            kit.multiply(scratch.makeRotationX(-sign.lifted * 0.4));
            part(kitBoxes, slot, 18, kit, [0, 0.65, 0], [0.04, 1.3, 0.04]);
            joint(gun, kit, 0, 1.5, 0, Math.PI / 2);
            part(kitDrums, slot, 12, gun, [0, 0, 0], [0.34, 0.03, 0.34]);
            joint(gun, kit, 0, 1.5, 0.017);
            stopFace.mesh.setMatrixAt(slot, gun);
            joint(gun, kit, 0, 1.5, -0.017, 0, Math.PI);
            goFace.mesh.setMatrixAt(slot, gun);
          }
        }
        // The jacks: rolled under each end, the pad up with the kart, the lever pumped.
        (["front", "rear"] as const).forEach((end, index) => {
          const jack = jackPlace(end, work, secondsOnMark);
          const up = end === "front" ? lift.front : lift.rear;
          const lever =
            stop === null
              ? 0.6
              : up > 0 && up < 1
                ? 0.6 - Math.abs(Math.sin(up * Math.PI * 2)) * 0.5
                : up >= 1
                  ? 0.1
                  : 0.8;
          place(kit, frame, side, jack.x, jack.z, jack.yaw);
          const boxAt = 8 + index * 5;
          part(kitBoxes, slot, boxAt, kit, [0, 0.06, 0], [0.34, 0.08, 0.5]);
          part(kitBoxes, slot, boxAt + 1, kit, [0, 0.14, 0.1], [0.1, 0.12, 0.3]);
          part(kitBoxes, slot, boxAt + 2, kit, [0, 0.2 + up * KART_LIFT_M, 0.3], [0.2, 0.04, 0.16]);
          for (const wheel of [0, 1]) {
            joint(gun, kit, (wheel * 2 - 1) * 0.2, 0.06, -0.18, 0, 0, Math.PI / 2);
            part(kitDrums, slot, 8 + index * 2 + wheel, gun, [0, 0, 0], [0.06, 0.05, 0.06]);
          }
          joint(gun, kit, 0, 0.16, -0.12, lever);
          part(kitBoxes, slot, boxAt + 3, gun, [0, 0, -0.5], [0.05, 0.05, 1]);
          part(kitBoxes, slot, boxAt + 4, gun, [0, 0, -1], [0.3, 0.05, 0.05]);
        });
      });
      for (const { mesh: instanced } of [...batches, ...heads.map((piece) => piece.batch)]) {
        instanced.instanceMatrix.needsUpdate = true;
      }
      hoseGeometry.getAttribute("position").needsUpdate = true;
    },
  };
}

/** The crew's jobs, in member order, for the style guide and the tests to name. */
export { CREW_ROLES };

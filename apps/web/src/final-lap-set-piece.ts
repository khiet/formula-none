// What the View draws of the final-lap set piece (ADR 0100): a FINAL LAP
// board over each face of the gantry's chequer, and fireworks over the
// course's landmark. Both are found in the loaded track by name, so a course
// without a gantry or a landmark, the oval, draws neither.

import {
  Box3,
  BoxGeometry,
  CanvasTexture,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  NearestFilter,
  type Object3D,
  PlaneGeometry,
  SRGBColorSpace,
  Vector3,
} from "three";
import { INSTRUMENT_FONT_FAMILY } from "./instrument-panel";
import { hexCss } from "./livery";
import { CARBON, COIN_GOLD, GO_GREEN, POSTER_RED, SHIFT_BLUE, VALUE_WHITE } from "./palette";
import { createParticles } from "./particles";
import { particleMesh } from "./tell-meshes";
import type { TrackName } from "./track";

const FINAL_LAP_TEXT = "FINAL LAP";

/**
 * Each course's landmark the fireworks go up over, by the nodes it is
 * exported as (the loader drops the dots from the names): Bell Deer's
 * Circuit Wheel, Goldstone's hangars and Monarch's harbour.
 */
const LANDMARKS: Readonly<Record<TrackName, RegExp | null>> = {
  "bell-deer": /^Prop\.?circuit-wheel/,
  goldstone: /^Prop\.?hangars/,
  monarch: /^Prop\.?harbour/,
  oval: null,
};

/** The gantry's chequer tiles, the board's footprint on each face. */
const GANTRY_CHECKER = /^Gantry\.?checker-(dark|light)$/;
/** Metres the board stands proud of the tiles, so it never shares their plane. */
const BOARD_LIFT_M = 0.03;
/** The board's canvas, in pixels up its height; its width keeps the board's shape. */
const BOARD_CANVAS_HEIGHT_PX = 64;
const BOARD_CANVAS_MAX_WIDTH_PX = 2048;
/** The red bands top and bottom, and the lettering, as shares of the board's height. */
const BOARD_BAND_SHARE = 0.12;
const BOARD_TEXT_SHARE = 0.62;

/** Metres over the landmark's top the shells burst between. */
const BURST_ABOVE_M = 12;
const BURST_SPREAD_UP_M = 14;
/** Seconds between shells; a viewer asking for less motion sees a third as many. */
const SHELL_GAP_S = 0.5;
const SHELL_GAP_REDUCED_S = 1.5;
/** Stars in a shell, how fast they leave its burst and how long they burn. */
const SHELL_STARS = 48;
const STAR_SPEED_M_PER_S = 20;
const STAR_LIFE_S = 1.8;
const STAR_SIZE_M = 2;
/** Shells in the air at once at most; the pool holds that many shells' stars. */
const SHELLS_ALIVE = 5;
const FIREWORK_GRAVITY = 5;
const FIREWORK_DRAG_PER_S = 1.2;
/** Seconds of flight a star's streak spans. */
const FIREWORK_STRETCH_S = 0.12;
/** Each shell's colour and the one it cools to, from the palette's brightest. */
const SHELL_COLOURS: readonly (readonly [number, number])[] = [
  [POSTER_RED.lit, POSTER_RED.shaded],
  [COIN_GOLD, POSTER_RED.mid],
  [SHIFT_BLUE.lit, SHIFT_BLUE.shaded],
  [GO_GREEN.lit, GO_GREEN.shaded],
  [VALUE_WHITE, COIN_GOLD],
];

export interface FinalLapSetPiece {
  /** The meshes to add to the world: the boards and the fireworks. */
  readonly objects: readonly Object3D[];
  /**
   * Show the boards and send shells up while `on`, `frameSeconds` on; the
   * stars already in the air burn out once it is off.
   */
  update(on: boolean, frameSeconds: number): void;
}

/** The set piece for the loaded `track` of course `name`. */
export function finalLapSetPiece(
  track: Object3D,
  name: TrackName,
  reducedMotion: boolean,
): FinalLapSetPiece {
  track.updateMatrixWorld(true);
  const boards = gantryBoards(track);
  const site = landmarkSite(track, LANDMARKS[name]);
  const stars = createParticles({
    capacity: SHELLS_ALIVE * SHELL_STARS,
    gravity: FIREWORK_GRAVITY,
    drag: FIREWORK_DRAG_PER_S,
    stretchSeconds: FIREWORK_STRETCH_S,
  });
  // Unlit and solid: against a daylight sky an additive star washes out to white.
  const starMesh = particleMesh(stars, new BoxGeometry(1, 1, 1), new MeshBasicMaterial(), {
    scaleOf: (size, life) => size * Math.sqrt(life),
    streak: true,
  });
  const gap = reducedMotion ? SHELL_GAP_REDUCED_S : SHELL_GAP_S;
  let untilShell = 0;
  let shell = 0;
  return {
    objects: [...boards.meshes, starMesh.mesh],
    update(on, frameSeconds) {
      boards.show(on);
      if (on && site !== null) {
        // One shell's gap at most to catch up, so a tab back from the
        // background sends one shell, not a volley the pool cannot hold.
        untilShell = Math.max(untilShell - frameSeconds, -gap);
        while (untilShell <= 0) {
          burst(
            stars,
            site,
            SHELL_COLOURS[shell % SHELL_COLOURS.length] ?? [VALUE_WHITE, VALUE_WHITE],
          );
          shell += 1;
          untilShell += gap;
        }
      } else {
        // The first shell goes up the moment the final lap starts.
        untilShell = 0;
      }
      stars.update(frameSeconds);
      starMesh.update();
    },
  };
}

/** Where shells burst: over the landmark's footprint, above its top. */
interface Site {
  readonly min: Vector3;
  readonly max: Vector3;
}

function landmarkSite(track: Object3D, pattern: RegExp | null): Site | null {
  if (pattern === null) return null;
  const bounds = new Box3();
  track.traverse((node) => {
    if (pattern.test(node.name)) bounds.expandByObject(node);
  });
  return bounds.isEmpty() ? null : { min: bounds.min, max: bounds.max };
}

/** One shell's stars, out every way at once from a burst somewhere over `site`. */
function burst(
  stars: ReturnType<typeof createParticles>,
  site: Site,
  [colour, cooledColour]: readonly [number, number],
): void {
  const x = site.min.x + Math.random() * (site.max.x - site.min.x);
  const z = site.min.z + Math.random() * (site.max.z - site.min.z);
  const y = site.max.y + BURST_ABOVE_M + Math.random() * BURST_SPREAD_UP_M;
  for (let star = 0; star < SHELL_STARS; star++) {
    // Even over the sphere: a uniform height and a uniform turn about it.
    const up = 2 * Math.random() - 1;
    const round = Math.sqrt(1 - up * up);
    const turn = Math.random() * Math.PI * 2;
    const speed = STAR_SPEED_M_PER_S * (0.8 + 0.2 * Math.random());
    stars.emit({
      position: [x, y, z],
      velocity: [round * Math.cos(turn) * speed, up * speed, round * Math.sin(turn) * speed],
      colour,
      cooledColour,
      lifetimeSeconds: STAR_LIFE_S,
      size: STAR_SIZE_M,
    });
  }
}

interface GantryBoards {
  readonly meshes: readonly Mesh[];
  show(on: boolean): void;
}

/**
 * A board over the chequer on each face of the gantry's beam. The beam
 * spans the asphalt square to the track, so the tiles' widest spread on the
 * ground is across it; the board covers the tiles from end to end and top
 * to bottom, just proud of each face, reading the right way from its side.
 */
function gantryBoards(track: Object3D): GantryBoards {
  const points: Vector3[] = [];
  track.traverse((node) => {
    if (!GANTRY_CHECKER.test(node.name) && !GANTRY_CHECKER.test(node.parent?.name ?? "")) return;
    if (!(node instanceof Mesh)) return;
    const position = node.geometry.getAttribute("position");
    for (let index = 0; index < position.count; index++) {
      points.push(
        new Vector3().fromBufferAttribute(position, index).applyMatrix4(node.matrixWorld),
      );
    }
  });
  if (points.length === 0) return { meshes: [], show: () => {} };

  const centre = points
    .reduce((sum, point) => sum.add(point), new Vector3())
    .divideScalar(points.length);
  // The ground's principal axis of the tiles: across the track.
  let xx = 0;
  let xz = 0;
  let zz = 0;
  for (const point of points) {
    const dx = point.x - centre.x;
    const dz = point.z - centre.z;
    xx += dx * dx;
    xz += dx * dz;
    zz += dz * dz;
  }
  const angle = 0.5 * Math.atan2(2 * xz, xx - zz);
  const across = new Vector3(Math.cos(angle), 0, Math.sin(angle));
  const along = new Vector3(-across.z, 0, across.x);
  const up = new Vector3(0, 1, 0);
  const spread = (axis: Vector3) => {
    let low = Number.POSITIVE_INFINITY;
    let high = Number.NEGATIVE_INFINITY;
    for (const point of points) {
      const at = point.clone().sub(centre).dot(axis);
      low = Math.min(low, at);
      high = Math.max(high, at);
    }
    return [low, high] as const;
  };
  const [leftmost, rightmost] = spread(across);
  const [behind, ahead] = spread(along);
  const [bottom, top] = spread(up);
  const width = rightmost - leftmost;
  const height = top - bottom;

  const material = new MeshBasicMaterial({ toneMapped: false });
  let drawn = false;
  const meshes = [
    { normal: along.clone(), depth: ahead + BOARD_LIFT_M },
    { normal: along.clone().negate(), depth: -behind + BOARD_LIFT_M },
  ].map(({ normal, depth }) => {
    const mesh = new Mesh(new PlaneGeometry(width, height), material);
    // The plane's x reads left to right to a viewer it faces.
    const right = new Vector3().crossVectors(up, normal);
    mesh.quaternion.setFromRotationMatrix(new Matrix4().makeBasis(right, up, normal));
    mesh.position
      .copy(centre)
      .addScaledVector(across, (leftmost + rightmost) / 2)
      .addScaledVector(normal, depth)
      .addScaledVector(up, (bottom + top) / 2);
    mesh.visible = false;
    return mesh;
  });
  return {
    meshes,
    show(on) {
      if (on && !drawn) {
        // Drawn on first showing, by when the panel's pixel face has loaded.
        material.map = boardTexture(width / height);
        material.needsUpdate = true;
        drawn = true;
      }
      for (const mesh of meshes) mesh.visible = on;
    },
  };
}

/** FINAL LAP in the panel's pixel face, white on carbon between red bands, for a board `aspect` wide to its height. */
function boardTexture(aspect: number): CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.height = BOARD_CANVAS_HEIGHT_PX;
  canvas.width = Math.min(Math.round(BOARD_CANVAS_HEIGHT_PX * aspect), BOARD_CANVAS_MAX_WIDTH_PX);
  const paint = canvas.getContext("2d");
  if (paint !== null) {
    const { width, height } = canvas;
    const band = Math.round(height * BOARD_BAND_SHARE);
    paint.fillStyle = hexCss(CARBON.mid);
    paint.fillRect(0, 0, width, height);
    paint.fillStyle = hexCss(POSTER_RED.mid);
    paint.fillRect(0, 0, width, band);
    paint.fillRect(0, height - band, width, band);
    paint.fillStyle = hexCss(VALUE_WHITE);
    paint.font = `${Math.round(height * BOARD_TEXT_SHARE)}px "${INSTRUMENT_FONT_FAMILY}", monospace`;
    paint.textAlign = "center";
    paint.textBaseline = "middle";
    // Twice over a wide board, so the words sit ahead of either lane.
    const copies = aspect > 12 ? 2 : 1;
    for (let copy = 0; copy < copies; copy++) {
      paint.fillText(FINAL_LAP_TEXT, (width * (copy + 0.5)) / copies, height / 2);
    }
  }
  const texture = new CanvasTexture(canvas);
  texture.colorSpace = SRGBColorSpace;
  texture.magFilter = NearestFilter;
  return texture;
}

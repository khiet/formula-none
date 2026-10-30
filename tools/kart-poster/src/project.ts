// An orthographic side view of triangles in the kart's frame: the camera on
// the kart's right (+X) looking along -X, pitched down so the nose (-Z) runs
// to the right of the picture and every top face shows as a strip. Each
// front face is shaded by its normal against one fixed light, quantised to
// three tones; back faces are culled. Coplanar faces of one node, material
// and tone merge into polygons, and the polygons are put in painter order
// from what overlaps what on the picture plane.

import type { Triangle, Vec3 } from "./glb.ts";

export type Tone = "lit" | "mid" | "shaded";

/** A point on the picture plane, in the kart's metres: x to the right, y up. */
export type Point = readonly [number, number];

export interface View {
  /** How far the camera looks down, in radians; 0 is level with the ground. */
  readonly pitch: number;
  /** The direction towards the light, in the kart's frame; need not be unit length. */
  readonly light: Vec3;
}

/** A front-facing triangle on the picture plane, with its depth towards the camera at each corner. */
export interface Facet {
  readonly node: string;
  readonly material: string;
  readonly tone: Tone;
  readonly points: readonly [Point, Point, Point];
  readonly depths: readonly [number, number, number];
}

/** One flat face of one node, material and tone: its outline loops on the picture plane. */
export interface Plane {
  readonly node: string;
  readonly material: string;
  readonly tone: Tone;
  readonly loops: readonly (readonly Point[])[];
  readonly facets: readonly Facet[];
  /** Mean depth towards the camera: the tie-break that draws the far before the near. */
  readonly depth: number;
}

/** A face is lit when its normal leans this far towards the light, shaded when as far away. */
const TONE_STEP = 0.3;

const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const dot = (a: Vec3, b: Vec3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const unit = (a: Vec3): Vec3 => {
  const length = Math.hypot(...a) || 1;
  return [a[0] / length, a[1] / length, a[2] / length];
};

/** The picture-plane point and the depth towards the camera of a point in the kart's frame. */
export function projectPoint(
  point: Vec3,
  pitch: number,
): { readonly at: Point; readonly depth: number } {
  const [x, y, z] = point;
  return {
    at: [-z, y * Math.cos(pitch) - x * Math.sin(pitch)],
    depth: x * Math.cos(pitch) + y * Math.sin(pitch),
  };
}

const signedArea = (points: readonly Point[]): number =>
  points.reduce((sum, [x, y], index) => {
    const [nx, ny] = points[(index + 1) % points.length] ?? [x, y];
    return sum + (x * ny - nx * y);
  }, 0) / 2;

/** Project every triangle, dropping back faces and faces edge-on to the camera, and tone each by the light. */
export function project(triangles: readonly Triangle[], view: View): Facet[] {
  const light = unit(view.light);
  const facets: Facet[] = [];
  for (const triangle of triangles) {
    const [a, b, c] = triangle.corners;
    const normal = unit(cross(sub(b, a), sub(c, a)));
    const projected = triangle.corners.map((corner) => projectPoint(corner, view.pitch));
    const points = projected.map(({ at }) => at) as [Point, Point, Point];
    // Front faces wind counter-clockwise on a y-up picture plane; a zero area is edge-on.
    if (signedArea(points) <= 1e-9) continue;
    const facing = dot(normal, light);
    facets.push({
      node: triangle.node,
      material: triangle.material,
      tone: facing > TONE_STEP ? "lit" : facing < -TONE_STEP ? "shaded" : "mid",
      points,
      depths: projected.map(({ depth }) => depth) as [number, number, number],
    });
  }
  return facets;
}

const key = (point: Point): string => `${point[0].toFixed(5)},${point[1].toFixed(5)}`;

/**
 * Merge each facet into the flat face it belongs to: facets of one node,
 * material and tone that lie in one plane. Every edge two of them share is
 * interior; the rest, chained end to end, outline the face.
 */
export function planes(facets: readonly Facet[]): Plane[] {
  const groups = new Map<string, Facet[]>();
  for (const facet of facets) {
    const [[x0, y0], [x1, y1], [x2, y2]] = facet.points;
    const [d0, d1, d2] = facet.depths;
    // The plane of the depth over the picture: depth = gx * x + gy * y + g0.
    const det = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    const gx = ((d1 - d0) * (y2 - y0) - (d2 - d0) * (y1 - y0)) / det;
    const gy = ((x1 - x0) * (d2 - d0) - (x2 - x0) * (d1 - d0)) / det;
    const g0 = d0 - gx * x0 - gy * y0;
    const id = `${facet.node}|${facet.material}|${facet.tone}|${gx.toFixed(4)}|${gy.toFixed(4)}|${g0.toFixed(4)}`;
    groups.set(id, [...(groups.get(id) ?? []), facet]);
  }
  return [...groups.values()].map((members) => {
    const first = members[0];
    if (first === undefined) throw new Error("an empty plane");
    const edges = new Map<string, { from: Point; to: Point }>();
    for (const facet of members) {
      facet.points.forEach((from, index) => {
        const to = facet.points[(index + 1) % 3] ?? from;
        const reverse = `${key(to)}>${key(from)}`;
        if (edges.has(reverse)) {
          edges.delete(reverse);
        } else {
          edges.set(`${key(from)}>${key(to)}`, { from, to });
        }
      });
    }
    const next = new Map<string, { from: Point; to: Point }[]>();
    for (const edge of edges.values()) {
      next.set(key(edge.from), [...(next.get(key(edge.from)) ?? []), edge]);
    }
    const id = (edge: { from: Point; to: Point }) => `${key(edge.from)}>${key(edge.to)}`;
    const loops: Point[][] = [];
    for (const start of [...edges.values()]) {
      if (!edges.has(id(start))) continue;
      const loop: Point[] = [];
      let edge = start;
      // Every edge taken is spent, so the walk ends where it began or at a dead end.
      for (;;) {
        loop.push(edge.from);
        edges.delete(id(edge));
        const onward = next.get(key(edge.to))?.find((candidate) => edges.has(id(candidate)));
        if (onward === undefined) break;
        edge = onward;
      }
      if (loop.length >= 3) loops.push(loop);
    }
    const depth =
      members.reduce((sum, facet) => sum + facet.depths.reduce((a, b) => a + b, 0) / 3, 0) /
      members.length;
    return {
      node: first.node,
      material: first.material,
      tone: first.tone,
      loops,
      facets: members,
      depth,
    };
  });
}

/** The polygon of `subject` clipped to the inside of the counter-clockwise triangle `clip`. */
function clip(subject: readonly Point[], clip: readonly [Point, Point, Point]): Point[] {
  let output: Point[] = [...subject];
  for (let index = 0; index < 3 && output.length > 0; index += 1) {
    const a = clip[index] ?? [0, 0];
    const b = clip[(index + 1) % 3] ?? a;
    const inside = ([x, y]: Point) => (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) >= 0;
    const input = output;
    output = [];
    input.forEach((current, i) => {
      const previous = input[(i + input.length - 1) % input.length] ?? current;
      const cut = (): Point => {
        const dx = current[0] - previous[0];
        const dy = current[1] - previous[1];
        const t =
          ((b[0] - a[0]) * (a[1] - previous[1]) - (b[1] - a[1]) * (a[0] - previous[0])) /
          ((b[0] - a[0]) * dy - (b[1] - a[1]) * dx);
        return [previous[0] + t * dx, previous[1] + t * dy];
      };
      if (inside(current)) {
        if (!inside(previous)) output.push(cut());
        output.push(current);
      } else if (inside(previous)) {
        output.push(cut());
      }
    });
  }
  return output;
}

/** The depth of `facet`'s plane at a point on the picture, found from its three corners. */
function depthAt(facet: Facet, [x, y]: Point): number {
  const [[x0, y0], [x1, y1], [x2, y2]] = facet.points;
  const [d0, d1, d2] = facet.depths;
  const det = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
  const u = ((x - x0) * (y2 - y0) - (x2 - x0) * (y - y0)) / det;
  const v = ((x1 - x0) * (y - y0) - (x - x0) * (y1 - y0)) / det;
  return d0 + u * (d1 - d0) + v * (d2 - d0);
}

const bounds = (facet: Facet) => ({
  minX: Math.min(...facet.points.map((p) => p[0])),
  maxX: Math.max(...facet.points.map((p) => p[0])),
  minY: Math.min(...facet.points.map((p) => p[1])),
  maxY: Math.max(...facet.points.map((p) => p[1])),
});

type Box = ReturnType<typeof bounds>;

const apart = (a: Box, b: Box) =>
  a.maxX <= b.minX || b.maxX <= a.minX || a.maxY <= b.minY || b.maxY <= a.minY;

const contains = ([a, b, c]: readonly [Point, Point, Point], [x, y]: Point): boolean =>
  (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) >= -1e-9 &&
  (c[0] - b[0]) * (y - b[1]) - (c[1] - b[1]) * (x - b[0]) >= -1e-9 &&
  (a[0] - c[0]) * (y - c[1]) - (a[1] - c[1]) * (x - c[0]) >= -1e-9;

/**
 * Drop every facet nothing of can show: one whose every sample point lies
 * under a nearer facet of a node always drawn with it, `fixed` or its own.
 * A facet under a part that may be left off stays, since it shows when the
 * part is. The samples are the centroid and points most of the way to each
 * corner and each edge's middle, so a facet is dropped only when covered
 * well inside its outline.
 */
export function visible(facets: readonly Facet[], fixed: (node: string) => boolean): Facet[] {
  const boxes = facets.map(bounds);
  const samples = (facet: Facet): Point[] => {
    const [a, b, c] = facet.points;
    const centre: Point = [(a[0] + b[0] + c[0]) / 3, (a[1] + b[1] + c[1]) / 3];
    const towards = ([x, y]: Point): Point => [
      centre[0] + 0.9 * (x - centre[0]),
      centre[1] + 0.9 * (y - centre[1]),
    ];
    const mid = (p: Point, q: Point): Point => [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2];
    return [centre, ...[a, b, c].map(towards), ...[mid(a, b), mid(b, c), mid(c, a)].map(towards)];
  };
  return facets.filter((facet, index) => {
    const box = boxes[index];
    if (box === undefined) return true;
    const covered = samples(facet).every((sample) =>
      facets.some((other, otherIndex) => {
        const otherBox = boxes[otherIndex];
        if (otherIndex === index || otherBox === undefined || apart(box, otherBox)) return false;
        if (!(fixed(other.node) || other.node === facet.node)) return false;
        return (
          contains(other.points, sample) && depthAt(other, sample) > depthAt(facet, sample) + 1e-7
        );
      }),
    );
    return !covered;
  });
}

const faceOf = (plane: Plane) => `${plane.node}|${plane.material}|${plane.tone}`;

/**
 * What must be drawn before what: for every two planes drawn `together` that
 * overlap on the picture, the one behind at the overlap goes first, weighted
 * by the overlap's area. Two planes that pass through each other overlap
 * both ways; the larger overlap wins and the smaller is hidden, so the
 * picture is right where it shows most. Returned as, for each plane's
 * index, the indices of the planes it is behind and the area at stake.
 */
export function dependencies(
  planesIn: readonly Plane[],
  together: (a: string, b: string) => boolean = () => true,
): Map<number, Map<number, number>> {
  const boxes = planesIn.map((plane) => plane.facets.map(bounds));
  const weights = new Map<number, Map<number, number>>();
  const addWeight = (behind: number, front: number, area: number) => {
    const row = weights.get(behind) ?? new Map<number, number>();
    row.set(front, (row.get(front) ?? 0) + area);
    weights.set(behind, row);
  };
  planesIn.forEach((a, ia) => {
    planesIn.forEach((b, ib) => {
      if (ib <= ia || !together(a.node, b.node)) return;
      a.facets.forEach((fa, fia) => {
        const ba = boxes[ia]?.[fia];
        b.facets.forEach((fb, fib) => {
          const bb = boxes[ib]?.[fib];
          if (ba === undefined || bb === undefined || apart(ba, bb)) return;
          const overlap = clip(fa.points, fb.points);
          const area = Math.abs(signedArea(overlap));
          if (area < 1e-10) return;
          const n = overlap.length;
          const centre: Point = [
            overlap.reduce((sum, p) => sum + p[0], 0) / n,
            overlap.reduce((sum, p) => sum + p[1], 0) / n,
          ];
          const gap = depthAt(fa, centre) - depthAt(fb, centre);
          if (Math.abs(gap) < 1e-7) return;
          if (gap > 0) addWeight(ib, ia, area);
          else addWeight(ia, ib, area);
        });
      });
    });
  });
  const kept = new Map<number, Map<number, number>>();
  for (const [behind, row] of weights) {
    for (const [front, area] of row) {
      const back = weights.get(front)?.get(behind) ?? 0;
      if (area > back || (area === back && behind < front)) {
        kept.set(behind, (kept.get(behind) ?? new Map()).set(front, area));
      }
    }
  }
  return kept;
}

/**
 * Overlap, in square metres of picture, below which one face is drawn
 * whole over a sliver of another that should have covered it: about six
 * square units on the poster, a speck at the edge of a part.
 */
const SPECK = 1e-4;

/**
 * Painter order from `dependencies`, a face at a time, where a face is the
 * planes of one node, material and tone: a face is behind another when more
 * of it lies behind than in front, and the faces are drawn furthest first.
 * Faces behind each other round a cycle are a part passing through another
 * or wrapping round it. The cycle's lightest step gives way when it is a
 * speck; otherwise the face behind splits into the planes behind the other
 * and the rest, and that face draws in two, which is as few paths as the
 * overlap allows. Among faces free to draw, the node just drawn goes on,
 * so a node's paths run together wherever the order allows.
 */
export function order(
  input: readonly Plane[],
  together: (a: string, b: string) => boolean = () => true,
): Plane[] {
  const planesIn = [...input];
  const weights = dependencies(planesIn, together);
  const faceOfPlane = planesIn.map(faceOf);
  let splits = 0;
  for (;;) {
    const members = new Map<string, number[]>();
    faceOfPlane.forEach((face, index) => {
      members.set(face, [...(members.get(face) ?? []), index]);
    });
    const between = new Map<string, Map<string, number>>();
    for (const [behind, row] of weights) {
      for (const [front, area] of row) {
        const a = faceOfPlane[behind] ?? "";
        const b = faceOfPlane[front] ?? "";
        if (a === b) continue;
        const fromA = between.get(a) ?? new Map<string, number>();
        fromA.set(b, (fromA.get(b) ?? 0) + area);
        between.set(a, fromA);
      }
    }
    const names = [...members.keys()];
    const id = new Map(names.map((name, index) => [name, index]));
    const edgesOut = new Map<number, Set<number>>();
    const edgeWeight = new Map<number, Map<number, number>>();
    for (const [a, row] of between) {
      for (const [b, area] of row) {
        const back = between.get(b)?.get(a) ?? 0;
        if (area > back || (area === back && a < b)) {
          const ia = id.get(a) ?? 0;
          const ib = id.get(b) ?? 0;
          edgesOut.set(ia, (edgesOut.get(ia) ?? new Set()).add(ib));
          edgeWeight.set(ia, (edgeWeight.get(ia) ?? new Map()).set(ib, area));
        }
      }
    }
    const depthOf = (face: number) => {
      const own = members.get(names[face] ?? "") ?? [];
      return own.reduce((sum, index) => sum + (planesIn[index]?.depth ?? 0), 0) / (own.length || 1);
    };
    const remaining = new Set(names.map((_, index) => index));
    const drawn: number[] = [];
    let split = false;
    while (remaining.size > 0 && !split) {
      const free = [...remaining].filter((face) =>
        [...remaining].every((other) => !edgesOut.get(other)?.has(face)),
      );
      if (free.length > 0) {
        // Stay with the node just drawn while it has a free face, so a node's paths run together.
        const lastNode =
          planesIn[members.get(names[drawn[drawn.length - 1] ?? -1] ?? "")?.[0] ?? -1]?.node;
        const nodeOf = (face: number) => planesIn[members.get(names[face] ?? "")?.[0] ?? -1]?.node;
        const continuing = free.filter((face) => nodeOf(face) === lastNode);
        const pick = (continuing.length > 0 ? continuing : free).reduce((best, face) =>
          depthOf(face) < depthOf(best) ? face : best,
        );
        drawn.push(pick);
        remaining.delete(pick);
        continue;
      }
      const cycle = findCycle(remaining, edgesOut);
      const steps = cycle
        .map((behind, index) => {
          const front = cycle[(index + 1) % cycle.length] ?? behind;
          return { behind, front, area: edgeWeight.get(behind)?.get(front) ?? 0 };
        })
        .sort((x, y) => x.area - y.area);
      const lightest = steps[0];
      if (lightest === undefined) throw new Error("stuck without a cycle");
      if (lightest.area <= SPECK) {
        edgesOut.get(lightest.behind)?.delete(lightest.front);
        continue;
      }
      for (const step of steps) {
        const behindFace = names[step.behind] ?? "";
        const frontFace = names[step.front] ?? "";
        const inFront = new Set(members.get(frontFace));
        const behindOnes = (members.get(behindFace) ?? []).filter((plane) => {
          let behind = 0;
          for (const [front, area] of weights.get(plane) ?? [])
            if (inFront.has(front)) behind += area;
          let front = 0;
          for (const other of inFront) front += weights.get(other)?.get(plane) ?? 0;
          return behind > front;
        });
        const all = members.get(behindFace) ?? [];
        if (behindOnes.length === 0 || behindOnes.length === all.length) continue;
        splits += 1;
        for (const plane of behindOnes) faceOfPlane[plane] = `${behindFace}#${splits}`;
        split = true;
        break;
      }
      // Faces wholly behind each other round a cycle: the lightest step gives way.
      if (!split) edgesOut.get(lightest.behind)?.delete(lightest.front);
    }
    if (split) continue;
    return drawn.flatMap((face) =>
      (members.get(names[face] ?? "") ?? []).map((index) => planesIn[index] as Plane),
    );
  }
}

/** A cycle among `remaining` planes along `edgesOut`, as the indices around it; the graph is known to hold one. */
function findCycle(
  remaining: ReadonlySet<number>,
  edgesOut: ReadonlyMap<number, ReadonlySet<number>>,
): number[] {
  const state = new Map<number, "open" | "done">();
  const path: number[] = [];
  const visit = (index: number): number[] | undefined => {
    state.set(index, "open");
    path.push(index);
    for (const next of edgesOut.get(index) ?? []) {
      if (!remaining.has(next)) continue;
      const seen = state.get(next);
      if (seen === "open") return path.slice(path.indexOf(next));
      if (seen === undefined) {
        const found = visit(next);
        if (found !== undefined) return found;
      }
    }
    path.pop();
    state.set(index, "done");
    return undefined;
  };
  for (const index of remaining) {
    if (state.has(index)) continue;
    const found = visit(index);
    if (found !== undefined) return found;
  }
  throw new Error("no cycle to break");
}

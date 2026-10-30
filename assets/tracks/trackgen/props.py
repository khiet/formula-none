"""Trackside props: scenery a layout declares in its decor and the builder
places, never collided with and never in the sidecar (ADR 0050). The
woods: a patch of trees the builder scatters, each tree the style guide's
Woods card in three dimensions, flat facets in a lit and a shade tone on a
trunk, standing flat at the grass's level. The Circuit Wheel: the card's
Ferris wheel outside the first curve, its rim and gondolas nodes of their
own, pivoted where the View turns them. The arch: a giant tyre spanning
Dunlop, lettered with its sponsor on both faces. The pit building: a row
of garages inside the start straight, a door and a flag in each livery's
key colour, which the grid faces through the countdown."""

from __future__ import annotations

import math
import random
from collections.abc import Callable, Iterator, Sequence
from dataclasses import dataclass

from . import mesh
from .layout import (
    CROWD_KEYS,
    GRASS_DROP_M,
    GRASS_MARGIN_M,
    HOARDING_TEXT_PROUD_M,
    PROP_WALL_CLEARANCE_M,
    SCENERY_LIFT_M,
    TREE_SPACING_M,
    TREE_WALL_CLEARANCE_M,
    WALL_THICKNESS_M,
    ByMaterial,
    Disc,
    Layout,
    Lettering,
    Pivot,
    Placement,
    Prop,
    SceneryMesh,
    Wood,
    locate,
)
from .sponsors import ARCH_SPONSOR

# The View's sun stands at (20, 40, 10) in the glTF frame: toward +X and -Y
# here, high; the track tests hold it to `race-view.ts`. A facet is lit when
# it faces up and not away from it.
SUN_AZIMUTH = (20.0 / math.hypot(20.0, 10.0), -10.0 / math.hypot(20.0, 10.0))
# How far a tree's canopy reaches from its trunk, which is what keeps its
# clearance from the wall.
CANOPY_RADIUS_M = {"pine": 2.24, "round": 2.0, "cherry": 2.4}
# Every third round tree in a wood is young.
YOUNG_EVERY = 3
TRUNK_HALF_M = 0.2
# A pine: two six-sided cones up its trunk, each as (base height, apex height, base radius).
PINE_CONES = ((1.96, 4.55, 2.24), (3.5, 7.0, 1.68))
PINE_TRUNK_M = 2.1
CONE_SIDES = 6
# A round tree: an eight-facet lump, a bipyramid, at (centre height, radius).
ROUND_LUMP = (3.0, 2.0)
ROUND_TRUNK_M = 1.5
YOUNG_LUMP = (1.92, 1.08)
YOUNG_TRUNK_M = 0.9
LUMP_SIDES = 8
# A cherry: three lumps, each as (across the trunk, centre height, radius),
# the middle one highest so the crown reads from every side.
CHERRY_LUMPS = ((-0.9, 2.7, 1.3), (1.0, 2.8, 1.3), (0.0, 3.5, 1.5))
CHERRY_TRUNK_M = 2.4
# A row's trees stray this far either way from their spot on the line.
ROW_JITTER_M = 1.5
# A disc keeps drawing spots this many times its count before giving up on
# a crowded patch.
TRIES_PER_TREE = 15

# The Circuit Wheel, the style guide's card in three dimensions: a rim of
# sixteen sides about a hub, a gondola hung level from every rim corner,
# and the hub carried on an A-frame either side of the wheel's plane. Its
# footprint is the disc the legs, the rim and the gondolas all fit in.
WHEEL_RADIUS_M = 12.0
"""From the hub to the middle of the rim's section."""
WHEEL_HUB_HEIGHT_M = 14.0
WHEEL_GONDOLAS = 16
WHEEL_FOOTPRINT_M = 13.0
# The rim's section: this deep along a spoke, this wide along the axle.
WHEEL_RIM_DEPTH_M = 0.5
WHEEL_RIM_WIDTH_M = 1.2
# A set of spokes either side of the hub along the axle, each a square bar.
WHEEL_SPOKE_OFFSET_M = 0.45
WHEEL_SPOKE_M = 0.3
# The hub: an eight-sided drum along the axle, long enough to take the legs.
WHEEL_HUB_RADIUS_M = 1.0
WHEEL_HUB_LENGTH_M = 4.0
WHEEL_HUB_SIDES = 8
# Each A-frame: two square legs from feet this far ahead of and behind the
# hub along the wheel's plane, this far out along the axle, up to the hub,
# with a brace between them at this height.
WHEEL_LEG_FOOT_M = 8.5
WHEEL_LEG_OUT_M = 1.7
WHEEL_LEG_M = 0.7
WHEEL_BRACE_HEIGHT_M = 6.0
WHEEL_BRACE_M = 0.4
# A gondola: a six-sided drum along the axle on a square hanger down from
# its rim corner, short enough that the lowest clears the grass under the
# 2 m the rim leaves.
WHEEL_HANGER_M = 0.4
WHEEL_HANGER_BAR_M = 0.2
GONDOLA_RADIUS_M = 0.7
GONDOLA_DEPTH_M = 1.6
GONDOLA_SIDES = 6

# The arch, the style guide's card in three dimensions: a tyre stood across
# the track with its centre at the grass, cut at the ground, so its crown is
# its radius up, and a wall-white abutment hiding each foot. Only the half
# above ground is built, in segments of the full tyre's polygon.
ARCH_RADIUS_M = 16.0
"""To the tread; the crown's height over the grass and half the span."""
ARCH_RING_M = 2.5
"""From the tread in to the lining."""
ARCH_DEPTH_M = 2.0
"""Along the track, face to face."""
ARCH_SIDES = 48
"""Of the full tyre's polygon; the arch is the half of them above ground."""
# Each flat face, from the tread in: a tyre-face lit edge, tyre black, the
# kerb-white belt the sponsor reads round, tyre black to the lining.
ARCH_EDGE_M = 0.7
ARCH_BELT_RADIUS_M = 14.65
"""To the belt's middle, where the lettering's baseline bends."""
ARCH_BELT_M = 0.9
ARCH_TEXT_SIZE_M = 1.2
# The abutments: a block round each foot of the ring, this much past the
# ring on every side, with a stand-plinth band at its base.
ARCH_ABUTMENT_HEIGHT_M = 2.5
ARCH_ABUTMENT_MARGIN_M = 0.5
ARCH_ABUTMENT_BAND_M = 0.4

# The pit building, the card in three dimensions: a flat-roofed block the
# layout's prop gives the length of, its front face toward the track, with
# the parapet the top of its walls and a door for each livery from the line
# forward, where the grid's chase cameras look, under a flag on a pole set
# back from the front edge of the roof.
PITS_DEPTH_M = 10.0
PITS_HEIGHT_M = 6.0
"""To the roof."""
PITS_PARAPET_M = 0.6
PITS_DOOR_WIDTH_M = 4.0
PITS_DOOR_HEIGHT_M = 4.0
PITS_DOOR_PITCH_M = 5.5
PITS_DOOR_FIRST_M = 2.75
"""From the line forward to the first door's middle; the rest follow at the pitch."""
PITS_DOOR_PROUD_M = 0.05
PITS_DOOR_BAND_M = 0.5
"""The lit band across the top of a door."""
PITS_POLE_M = 0.15
PITS_POLE_HEIGHT_M = 4.0
"""Above the roof."""
PITS_POLE_FROM_FRONT_M = 0.5
PITS_FLAG_LENGTH_M = 1.6
PITS_FLAG_HEIGHT_M = 0.9


@dataclass(frozen=True)
class Tree:
    """One scattered tree: its kind, its trunk's foot in track metres, the way its facets turn."""

    kind: str
    x: float
    y: float
    yaw: float
    young: bool = False


@dataclass(frozen=True)
class Footprint:
    """The ground one piece of a prop of `kind` stands on, in track metres: the disc of
    `radius` about (`x`, `y`), or without one the rectangle reaching `half_along` either way
    along `heading` and `half_across` either side of it. A tree keeps its canopy out of it,
    and the prop keeps `clearance` metres of grass between it and every wall: the 8 m rule
    for a tall prop, or none for a span's feet, which only have to stand on grass."""

    kind: str
    x: float
    y: float
    clearance: float
    radius: float = 0.0
    heading: float = 0.0
    half_along: float = 0.0
    half_across: float = 0.0

    def distance(self, x: float, y: float) -> float:
        """From the point to the footprint's edge, negative inside it."""
        if self.radius:
            return math.dist((x, y), (self.x, self.y)) - self.radius
        dx, dy = x - self.x, y - self.y
        cos, sin = math.cos(self.heading), math.sin(self.heading)
        along, across = dx * cos + dy * sin, -dx * sin + dy * cos
        out_along = abs(along) - self.half_along
        out_across = abs(across) - self.half_across
        if out_along > 0.0 or out_across > 0.0:
            return math.hypot(max(out_along, 0.0), max(out_across, 0.0))
        return max(out_along, out_across)

    def bounds(self) -> tuple[float, float, float, float]:
        """The ground it covers, as (min x, min y, max x, max y)."""
        if self.radius:
            return self.x - self.radius, self.y - self.radius, self.x + self.radius, self.y + self.radius
        cos, sin = math.cos(self.heading), math.sin(self.heading)
        corners = [
            (self.x + a * cos - b * sin, self.y + a * sin + b * cos)
            for a in (-self.half_along, self.half_along)
            for b in (-self.half_across, self.half_across)
        ]
        return min(x for x, _ in corners), min(y for _, y in corners), max(x for x, _ in corners), max(y for _, y in corners)


def trees(layout: Layout) -> list[Tree]:
    """Every wood's trees, in the layout's order, each wood keeping clear of the ones before it."""
    placed: list[Tree] = []
    for wood in layout.decor.woods:
        placed += scatter(layout, wood, placed)
    return placed


def scatter(layout: Layout, wood: Wood, placed: Sequence[Tree] = ()) -> list[Tree]:
    """Where `wood`'s trees stand: up to its count, from its own random stream, so the same
    layout scatters the same trees every export.

    A spot within `TREE_WALL_CLEARANCE_M` of a wall, canopy and all, or `TREE_SPACING_M` of
    another tree, `placed` or this wood's own, or whose canopy reaches into a prop's
    footprint, is dropped. A disc draws again in its place; a row leaves the gap.
    """
    rng = random.Random(wood.seed)
    canopy = CANOPY_RADIUS_M[wood.kind]
    reach = canopy + TREE_WALL_CLEARANCE_M
    wall = layout.runoff_half_width + WALL_THICKNESS_M
    patch = wood.patch
    (x0, y0, x1, y1) = patch_bounds(wood)
    centre = ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    radius = math.dist(centre, (x1, y1))
    # Only the centreline near the patch can put a wall within reach of a tree.
    near = [(s.x, s.y) for s in layout.path.samples if math.dist((s.x, s.y), centre) <= radius + wall + reach]
    taken = footprints(layout)

    def clear(x: float, y: float) -> bool:
        return all(math.dist((x, y), p) >= wall + reach for p in near) and all(
            footprint.distance(x, y) >= canopy for footprint in taken
        )

    mine: list[Tree] = []
    if isinstance(patch, Disc):
        spots = _disc_spots(rng, patch, wood.count * TRIES_PER_TREE)
    else:
        spots = _row_spots(rng, patch.a, patch.b, wood.count)
    for x, y in spots:
        if len(mine) == wood.count:
            break
        yaw = rng.random() * math.tau
        if not clear(x, y) or any(math.dist((x, y), (t.x, t.y)) < TREE_SPACING_M for t in (*placed, *mine)):
            continue
        young = wood.kind == "round" and len(mine) % YOUNG_EVERY == YOUNG_EVERY - 1
        mine.append(Tree(wood.kind, x, y, yaw, young))
    return mine


def patch_bounds(wood: Wood) -> tuple[float, float, float, float]:
    """The ground a wood's trees can cover, canopies and all, as (min x, min y, max x, max y)."""
    reach = CANOPY_RADIUS_M[wood.kind]
    patch = wood.patch
    if isinstance(patch, Disc):
        return patch.x - patch.r - reach, patch.y - patch.r - reach, patch.x + patch.r + reach, patch.y + patch.r + reach
    stray = ROW_JITTER_M + reach
    return (
        min(patch.a[0], patch.b[0]) - stray,
        min(patch.a[1], patch.b[1]) - stray,
        max(patch.a[0], patch.b[0]) + stray,
        max(patch.a[1], patch.b[1]) + stray,
    )


def _disc_spots(rng: random.Random, disc: Disc, tries: int) -> Iterator[tuple[float, float]]:
    """Spots spread evenly over the disc, as many as asked."""
    for _ in range(tries):
        angle = rng.random() * math.tau
        d = math.sqrt(rng.random()) * disc.r
        yield disc.x + d * math.cos(angle), disc.y + d * math.sin(angle)


def _row_spots(
    rng: random.Random, a: tuple[float, float], b: tuple[float, float], count: int
) -> Iterator[tuple[float, float]]:
    """`count` spots evenly along the row from `a` to `b`, each strayed a little."""
    for k in range(count):
        f = (k + 0.5) / count
        yield (
            a[0] + (b[0] - a[0]) * f + (rng.random() - 0.5) * 2.0 * ROW_JITTER_M,
            a[1] + (b[1] - a[1]) * f + (rng.random() - 0.5) * 2.0 * ROW_JITTER_M,
        )


def woods(layout: Layout) -> list[SceneryMesh]:
    """Every wood's trees, one mesh per material for the lot, and grass under any wood
    that reaches past the slab's edge."""
    parts = ByMaterial("Woods")
    for tree in trees(layout):
        for material, piece in pieces_of(tree):
            parts.add(material, piece)
    return parts.scenery() + ground(layout)


def ground(layout: Layout) -> list[SceneryMesh]:
    """Grass under the woods and the props beyond the slab the sim stands on: a hair under it,
    so it shows only past the slab's edge, and scenery, so the sidecar never grows for a prop."""
    if not layout.decor.woods and not layout.decor.props:
        return []
    min_x, min_y, max_x, max_y = layout.path.bounds()
    slab = (min_x - GRASS_MARGIN_M, min_y - GRASS_MARGIN_M, max_x + GRASS_MARGIN_M, max_y + GRASS_MARGIN_M)
    covered = [patch_bounds(wood) for wood in layout.decor.woods]
    covered += [footprint.bounds() for footprint in footprints(layout)]
    reach = (
        min(slab[0], *(c[0] for c in covered)),
        min(slab[1], *(c[1] for c in covered)),
        max(slab[2], *(c[2] for c in covered)),
        max(slab[3], *(c[3] for c in covered)),
    )
    if reach == slab:
        return []
    return [SceneryMesh("Woods.grass", "grass", mesh.slab(*reach, -GRASS_DROP_M - SCENERY_LIFT_M))]


def pieces_of(tree: Tree) -> Iterator[tuple[str, mesh.Mesh]]:
    """A tree's faces by material, standing at the grass's level."""
    return BUILDERS[tree.kind](tree)


def _pine(tree: Tree) -> Iterator[tuple[str, mesh.Mesh]]:
    ground = -GRASS_DROP_M
    yield from _trunk(tree, ground, PINE_TRUNK_M)
    for base, apex, radius in PINE_CONES:
        yield from _cone(tree, "canopy-pine", ground + base, ground + apex, radius)


def _round(tree: Tree) -> Iterator[tuple[str, mesh.Mesh]]:
    ground = -GRASS_DROP_M
    (centre, radius), trunk = (YOUNG_LUMP, YOUNG_TRUNK_M) if tree.young else (ROUND_LUMP, ROUND_TRUNK_M)
    yield from _trunk(tree, ground, trunk)
    yield from _lump(tree, "canopy", 0.0, ground + centre, radius)


def _cherry(tree: Tree) -> Iterator[tuple[str, mesh.Mesh]]:
    ground = -GRASS_DROP_M
    yield from _trunk(tree, ground, CHERRY_TRUNK_M)
    for across, centre, radius in CHERRY_LUMPS:
        yield from _lump(tree, "blossom", across, ground + centre, radius)


BUILDERS: dict[str, Callable[[Tree], Iterator[tuple[str, mesh.Mesh]]]] = {
    "pine": _pine,
    "round": _round,
    "cherry": _cherry,
}


def _turned(tree: Tree, across: float, along: float, z: float) -> mesh.Vertex:
    """A point `across` and `along` the tree's own frame, turned by its yaw, at height `z`."""
    cos, sin = math.cos(tree.yaw), math.sin(tree.yaw)
    return (tree.x + across * cos - along * sin, tree.y + across * sin + along * cos, z)


def _trunk(tree: Tree, ground: float, top: float) -> Iterator[tuple[str, mesh.Mesh]]:
    """A four-sided post with a top, its bottom on the grass and never seen."""

    def corner(i: int, j: int, k: int) -> mesh.Vertex:
        return _turned(tree, (-TRUNK_HALF_M, TRUNK_HALF_M)[i], (-TRUNK_HALF_M, TRUNK_HALF_M)[j], (ground, ground + top)[k])

    faces = mesh.cuboid(corner)
    for side in ("x0", "x1", "y0", "y1", "z1"):
        yield "trunk", faces[side]


def _cone(tree: Tree, material: str, base: float, apex: float, radius: float) -> Iterator[tuple[str, mesh.Mesh]]:
    """A six-sided cone with its base closed in shade, each side lit or shaded by where it faces."""
    ring = [
        _turned(tree, radius * math.cos(math.tau * k / CONE_SIDES), radius * math.sin(math.tau * k / CONE_SIDES), base)
        for k in range(CONE_SIDES)
    ]
    tip = (tree.x, tree.y, apex)
    for k in range(CONE_SIDES):
        a, b = ring[k], ring[(k + 1) % CONE_SIDES]
        yield _toned(material, [a, b, tip])
    yield f"{material}-shade", (list(reversed(ring)), [tuple(range(CONE_SIDES))])


def _lump(tree: Tree, material: str, across: float, centre: float, radius: float) -> Iterator[tuple[str, mesh.Mesh]]:
    """An eight-facet lump: a bipyramid with its ring at `centre`, `across` the trunk's frame."""
    ring = [
        _turned(
            tree,
            across + radius * math.cos(math.tau * k / LUMP_SIDES),
            radius * math.sin(math.tau * k / LUMP_SIDES),
            centre,
        )
        for k in range(LUMP_SIDES)
    ]
    top = _turned(tree, across, 0.0, centre + radius)
    bottom = _turned(tree, across, 0.0, centre - radius)
    for k in range(LUMP_SIDES):
        a, b = ring[k], ring[(k + 1) % LUMP_SIDES]
        yield _toned(material, [a, b, top])
        yield _toned(material, [b, a, bottom])


def _toned(material: str, face: list[mesh.Vertex], shade: str | None = None) -> tuple[str, mesh.Mesh]:
    """The face, wound as given, in `material` when it faces up and toward the sun, else in
    `shade`, the colour's own shade tone unless named."""
    a, b, c = face[0], face[1], face[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    normal = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    lit = normal[2] >= 0.0 and normal[0] * SUN_AZIMUTH[0] + normal[1] * SUN_AZIMUTH[1] >= 0.0
    return material if lit else (shade or f"{material}-shade"), (face, [tuple(range(len(face)))])


def footprints(layout: Layout) -> list[Footprint]:
    """Every prop's footprints in the layout's order: the ground each stands on, which keeps
    its clearance from the walls and the trees out. The wheel's is the disc its legs, rim
    and gondolas all fit in; the pits' is the building's plan; the arch's are its two feet."""
    found: list[Footprint] = []
    for prop in layout.decor.props:
        found += FOOTPRINTS[prop.kind](layout, prop)
    return found


def _wheel_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    x, y, _ = locate(layout, prop.at)
    return [Footprint(prop.kind, x, y, PROP_WALL_CLEARANCE_M, radius=WHEEL_FOOTPRINT_M)]


def _arch_footprints(layout: Layout, prop: Prop) -> list[Footprint]:
    """The two abutments, one round each foot of the ring: a span's feet keep no set grass
    from the wall, only the ground they stand on."""
    x, y, heading = locate(layout, prop.at)
    left = (-math.sin(heading), math.cos(heading))
    foot_in = ARCH_RADIUS_M - ARCH_RING_M - ARCH_ABUTMENT_MARGIN_M
    foot_out = ARCH_RADIUS_M + ARCH_ABUTMENT_MARGIN_M
    middle = (foot_in + foot_out) / 2.0
    return [
        Footprint(
            prop.kind,
            x + side * middle * left[0],
            y + side * middle * left[1],
            0.0,
            heading=heading,
            half_along=ARCH_DEPTH_M / 2.0 + ARCH_ABUTMENT_MARGIN_M,
            half_across=(foot_out - foot_in) / 2.0,
        )
        for side in (1.0, -1.0)
    ]


def _pits_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    x, y, heading = locate(layout, prop.at)
    if prop.length is None:
        raise ValueError("the pits need a length")
    return [
        Footprint(
            prop.kind, x, y, PROP_WALL_CLEARANCE_M, heading=heading, half_along=prop.length / 2.0, half_across=PITS_DEPTH_M / 2.0
        )
    ]


FOOTPRINTS: dict[str, Callable[[Layout, Prop], list[Footprint]]] = {
    "circuit-wheel": _wheel_footprint,
    "arch": _arch_footprints,
    "pits": _pits_footprint,
}


def props(layout: Layout) -> tuple[list[SceneryMesh], list[Lettering]]:
    """Every prop's scenery and lettering: its still parts one mesh per material per kind,
    each part the View moves a pivoted mesh of its own, and each text a lettering of its
    own, the props numbered from one among their kind in the layout's order."""
    still = {kind: ByMaterial(f"Prop.{kind}") for kind in PROP_BUILDERS}
    moving: list[SceneryMesh] = []
    lettering: list[Lettering] = []
    numbers: dict[str, int] = {}
    for prop in layout.decor.props:
        numbers[prop.kind] = numbers.get(prop.kind, 0) + 1
        nodes, texts = PROP_BUILDERS[prop.kind](layout, prop, numbers[prop.kind], still[prop.kind])
        moving += nodes
        lettering += texts
    return [piece for family in still.values() for piece in family.scenery()] + moving, lettering


class Node:
    """The faces of one mesh the View moves, gathered by material in face order, for a
    `SceneryMesh` named `name` pivoted at `pivot`."""

    def __init__(self, name: str, pivot: Pivot) -> None:
        self.name = name
        self.pivot = pivot
        self.pieces: list[tuple[str, mesh.Mesh]] = []

    def add(self, material: str, piece: mesh.Mesh) -> None:
        self.pieces.append((material, piece))

    def scenery(self) -> SceneryMesh:
        merged = mesh.merge([piece for _, piece in self.pieces])
        materials = tuple(material for material, (_, faces) in self.pieces for _ in faces)
        return SceneryMesh(self.name, materials[0], merged, self.pivot, materials)


def circuit_wheel(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The Circuit Wheel at `prop`: its legs and hub into `still`, and the rim with its sixteen
    gondolas as the nodes `Prop.circuit-wheel.<number>.rim` and `.gondola.1` to `.16`. It
    carries no name.

    The wheel's plane runs along the heading; its axle is the left normal, which the pivot's
    frame puts along local y. A gondola's node hangs under the rim's, its origin at its
    hanger, so the View turns it back by the rim's angle and it stays level."""
    x, y, heading = locate(layout, prop.at)
    ground = -GRASS_DROP_M
    hub = (x, y, ground + WHEEL_HUB_HEIGHT_M)
    along = (math.cos(heading), math.sin(heading), 0.0)
    axle = (-math.sin(heading), math.cos(heading), 0.0)
    up = (0.0, 0.0, 1.0)

    def at(centre: mesh.Vertex, a: float, b: float, c: float) -> mesh.Vertex:
        """`a` metres along the plane, `b` along the axle and `c` up from `centre`."""
        return tuple(centre[i] + a * along[i] + b * axle[i] + c * up[i] for i in range(3))

    def bar(start: mesh.Vertex, end: mesh.Vertex, side: float, u: mesh.Vertex, v: mesh.Vertex) -> dict[str, mesh.Mesh]:
        """A square bar of `side` from `start` to `end`, its section spanned by the units `u` and `v`."""
        half = side / 2.0

        def corner(i: int, j: int, k: int) -> mesh.Vertex:
            p = (start, end)[k]
            return tuple(p[n] + (-half, half)[i] * u[n] + (-half, half)[j] * v[n] for n in range(3))

        return mesh.cuboid(corner)

    def toned_sides(faces: dict[str, mesh.Mesh], material: str, shade: str | None, into: ByMaterial | Node, ends: tuple[str, ...] = ()) -> None:
        """The upright faces of a `bar`, and `ends`, each wound out of it as the cuboid winds them."""
        for key in ("x0", "x1", "y0", "y1", *ends):
            vertices, (face,) = faces[key]
            into.add(*_toned(material, [vertices[i] for i in face], shade))

    rim_name = f"Prop.circuit-wheel.{number}.rim"
    rim = Node(rim_name, Pivot(*hub, heading))
    corners = [math.tau * k / WHEEL_GONDOLAS for k in range(WHEEL_GONDOLAS)]

    def ring(radius: float, b: float) -> list[mesh.Vertex]:
        return [at(hub, radius * math.cos(a), b, radius * math.sin(a)) for a in corners]

    # The rim: a box section round the sixteen-sided ring, each face toned by where it faces.
    inner, outer = WHEEL_RADIUS_M - WHEEL_RIM_DEPTH_M / 2.0, WHEEL_RADIUS_M + WHEEL_RIM_DEPTH_M / 2.0
    near, far = -WHEEL_RIM_WIDTH_M / 2.0, WHEEL_RIM_WIDTH_M / 2.0
    rails = {(r, b): ring(r, b) for r in (inner, outer) for b in (near, far)}
    spine = ring(WHEEL_RADIUS_M, 0.0)
    for k in range(WHEEL_GONDOLAS):
        j = (k + 1) % WHEEL_GONDOLAS
        quads = (
            # Outer face, inner face, near side, far side.
            [rails[outer, near][k], rails[outer, near][j], rails[outer, far][j], rails[outer, far][k]],
            [rails[inner, far][k], rails[inner, far][j], rails[inner, near][j], rails[inner, near][k]],
            [rails[inner, near][k], rails[inner, near][j], rails[outer, near][j], rails[outer, near][k]],
            [rails[outer, far][k], rails[outer, far][j], rails[inner, far][j], rails[inner, far][k]],
        )
        # Each wound out of the section, away from its own middle: the inner face looks at the hub.
        section = tuple((spine[k][i] + spine[j][i]) / 2.0 for i in range(3))
        for quad in quads:
            rim.add(*_toned("wall", _outward(quad, section), "beam-shade"))
    # Two sets of spokes, one either side of the hub, each from the hub to a rim corner.
    for b in (-WHEEL_SPOKE_OFFSET_M, WHEEL_SPOKE_OFFSET_M):
        for a in corners:
            radial = (math.cos(a), math.sin(a))
            u = axle
            v = at((0.0, 0.0, 0.0), -radial[1], 0.0, radial[0])
            spoke = bar(at(hub, 0.0, b, 0.0), at(hub, WHEEL_RADIUS_M * radial[0], b, WHEEL_RADIUS_M * radial[1]), WHEEL_SPOKE_M, u, v)
            toned_sides(spoke, "wall", "beam-shade", rim)
    # The hub: a drum along the axle, and the A-frames carrying it.
    hub_faces = _drum(at(hub, 0.0, -WHEEL_HUB_LENGTH_M / 2.0, 0.0), axle, WHEEL_HUB_RADIUS_M, WHEEL_HUB_LENGTH_M, WHEEL_HUB_SIDES, along)
    for face in hub_faces:
        still.add(*_toned("carbon", face))
    for b in (-WHEEL_LEG_OUT_M, WHEEL_LEG_OUT_M):
        for a in (-WHEEL_LEG_FOOT_M, WHEEL_LEG_FOOT_M):
            leg = bar(at(hub, a, b, -WHEEL_HUB_HEIGHT_M), at(hub, 0.0, b, 0.0), WHEEL_LEG_M, along, axle)
            toned_sides(leg, "carbon", None, still)
        rise = WHEEL_BRACE_HEIGHT_M / WHEEL_HUB_HEIGHT_M
        reach = WHEEL_LEG_FOOT_M * (1.0 - rise)
        brace = bar(at(hub, -reach, b, WHEEL_BRACE_HEIGHT_M - WHEEL_HUB_HEIGHT_M), at(hub, reach, b, WHEEL_BRACE_HEIGHT_M - WHEEL_HUB_HEIGHT_M), WHEEL_BRACE_M, axle, up)
        toned_sides(brace, "carbon", None, still)
    # The gondolas: one per rim corner, two per key colour, each its own node at its hanger.
    gondolas: list[SceneryMesh] = []
    for k, a in enumerate(corners, start=1):
        hanger = at(hub, WHEEL_RADIUS_M * math.cos(a), 0.0, WHEEL_RADIUS_M * math.sin(a))
        node = Node(f"Prop.circuit-wheel.{number}.gondola.{k}", Pivot(*hanger, heading, parent=rim_name))
        foot = at(hanger, 0.0, 0.0, -WHEEL_HANGER_M)
        toned_sides(bar(hanger, foot, WHEEL_HANGER_BAR_M, along, axle), "carbon", "carbon", node)
        colour = f"crowd-{(k - 1) % len(CROWD_KEYS)}"
        body = at(foot, 0.0, -GONDOLA_DEPTH_M / 2.0, -GONDOLA_RADIUS_M)
        for face in _drum(body, axle, GONDOLA_RADIUS_M, GONDOLA_DEPTH_M, GONDOLA_SIDES, up):
            node.add(colour, (face, [tuple(range(len(face)))]))
        gondolas.append(node.scenery())
    return [rim.scenery(), *gondolas], []


def _block(
    origin: mesh.Vertex,
    along: mesh.Vertex,
    across: mesh.Vertex,
    a: tuple[float, float],
    b: tuple[float, float],
    c: tuple[float, float],
) -> dict[str, mesh.Mesh]:
    """A box in the frame at `origin`: from `a[0]` to `a[1]` along `along`, `b` along `across`
    and `c` up, its faces keyed x along, y across and z up, each wound out of the box."""

    def corner(i: int, j: int, k: int) -> mesh.Vertex:
        return (
            origin[0] + a[i] * along[0] + b[j] * across[0],
            origin[1] + a[i] * along[1] + b[j] * across[1],
            origin[2] + c[k],
        )

    return mesh.cuboid(corner)


def _sides(faces: dict[str, mesh.Mesh], material: str, into: ByMaterial, shade: str | None = None, top: bool = False) -> None:
    """A block's four upright faces, and its top when asked, into `into`: toned by the sun
    against `shade` when one is named, else flat in `material`."""
    for key in ("x0", "x1", "y0", "y1", *(("z1",) if top else ())):
        vertices, (face,) = faces[key]
        quad = [vertices[i] for i in face]
        if shade is None:
            into.add(material, (quad, [(0, 1, 2, 3)]))
        else:
            into.add(*_toned(material, quad, shade))


def arch(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The arch at `prop`, a tyre stood across the track: the half of a `ARCH_SIDES`-sided ring
    above the grass, its tread and lining toned by the sun, its flat faces banded from the
    tread in (lit edge, black, the belt, black), an abutment round each foot, and the
    sponsor bent round the belt on both faces as `Prop.arch.<number>.belt.front` and
    `.back`, the front the face a kart approaches."""
    x, y, heading = locate(layout, prop.at)
    ground = -GRASS_DROP_M
    centre = (x, y, ground)
    along = (math.cos(heading), math.sin(heading), 0.0)
    across = (-math.sin(heading), math.cos(heading), 0.0)

    def at(a: float, b: float, c: float) -> mesh.Vertex:
        """`a` metres across the track, `b` along it and `c` up from the ring's centre."""
        return (x + a * across[0] + b * along[0], y + a * across[1] + b * along[1], ground + c)

    segments = ARCH_SIDES // 2
    corners = [math.pi * k / segments for k in range(segments + 1)]
    outer, inner = ARCH_RADIUS_M, ARCH_RADIUS_M - ARCH_RING_M
    near, far = -ARCH_DEPTH_M / 2.0, ARCH_DEPTH_M / 2.0
    bands = (
        (outer, "tyre-face"),
        (outer - ARCH_EDGE_M, "tyre"),
        (ARCH_BELT_RADIUS_M + ARCH_BELT_M / 2.0, "kerb-white"),
        (ARCH_BELT_RADIUS_M - ARCH_BELT_M / 2.0, "tyre"),
        (inner, ""),
    )

    def ring(radius: float, b: float) -> list[mesh.Vertex]:
        return [at(radius * math.cos(angle), b, radius * math.sin(angle)) for angle in corners]

    rails = {(radius, b): ring(radius, b) for radius, _ in bands for b in (near, far)}
    spine = ring((outer + inner) / 2.0, 0.0)
    for k in range(segments):
        j = k + 1
        section = tuple((spine[k][i] + spine[j][i]) / 2.0 for i in range(3))
        tread = [rails[outer, near][k], rails[outer, near][j], rails[outer, far][j], rails[outer, far][k]]
        lining = [rails[inner, near][k], rails[inner, near][j], rails[inner, far][j], rails[inner, far][k]]
        for quad in (tread, lining):
            still.add(*_toned("tyre", _outward(quad, section)))
        for (r0, material), (r1, _) in zip(bands, bands[1:]):
            for b in (near, far):
                quad = [rails[r0, b][k], rails[r0, b][j], rails[r1, b][j], rails[r1, b][k]]
                still.add(material, (_outward(quad, section), [(0, 1, 2, 3)]))
    # An abutment round each foot: a plinth band at the grass and wall white above it.
    for footprint in _arch_footprints(layout, prop):
        origin = (footprint.x, footprint.y, ground)
        a = (-footprint.half_along, footprint.half_along)
        b = (-footprint.half_across, footprint.half_across)
        _sides(_block(origin, along, across, a, b, (0.0, ARCH_ABUTMENT_BAND_M)), "stand-plinth", still)
        _sides(_block(origin, along, across, a, b, (ARCH_ABUTMENT_BAND_M, ARCH_ABUTMENT_HEIGHT_M)), "wall", still, top=True)
    # The sponsor round the belt, just proud of each face, read from the side it is on.
    name, ink = ARCH_SPONSOR
    proud = ARCH_DEPTH_M / 2.0 + HOARDING_TEXT_PROUD_M
    lettering = [
        Lettering(
            f"Prop.arch.{number}.belt.{face}",
            name,
            ink,
            ARCH_TEXT_SIZE_M,
            Placement(*at(0.0, side * proud, 0.0), heading + (math.pi if side < 0 else 0.0)),
            arc=ARCH_BELT_RADIUS_M,
        )
        for face, side in (("front", -1.0), ("back", 1.0))
    ]
    return [], lettering


def pits(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The pit building at `prop`, `prop.length` long along the track with its front face
    toward it: stand-riser walls toned against stand plinth under a stand-plinth parapet and
    a lit roof, a door for each livery on the front face from the line forward in the
    crowd's order, so every grid slot faces them, each in its key colour with a lit band
    along its top, and a still flag on a carbon pole over each door, the flags in the same
    colours the other way round, as the style guide's card has them. No number and no kart:
    the doors are colours only."""
    if prop.length is None:
        raise ValueError("the pits need a length")
    x, y, heading = locate(layout, prop.at)
    ground = -GRASS_DROP_M
    origin = (x, y, ground)
    along = (math.cos(heading), math.sin(heading), 0.0)
    across = (-math.sin(heading), math.cos(heading), 0.0)
    half = prop.length / 2.0
    # The track is to the left of the building, so its front face is its +across side.
    front = PITS_DEPTH_M / 2.0
    body = (0.0, PITS_HEIGHT_M - PITS_PARAPET_M)
    _sides(_block(origin, along, across, (-half, half), (-front, front), body), "stand-riser", still, shade="stand-plinth")
    parapet = _block(origin, along, across, (-half, half), (-front, front), (body[1], PITS_HEIGHT_M))
    _sides(parapet, "stand-plinth", still)
    vertices, (face,) = parapet["z1"]
    still.add("wall", ([vertices[i] for i in face], [(0, 1, 2, 3)]))

    def at(a: float, b: float, c: float) -> mesh.Vertex:
        return (x + a * along[0] + b * across[0], y + a * along[1] + b * across[1], ground + c)

    slots = len(CROWD_KEYS)
    for slot in range(slots):
        middle = PITS_DOOR_FIRST_M + slot * PITS_DOOR_PITCH_M
        a0, a1 = middle - PITS_DOOR_WIDTH_M / 2.0, middle + PITS_DOOR_WIDTH_M / 2.0
        b = front + PITS_DOOR_PROUD_M
        band = PITS_DOOR_HEIGHT_M - PITS_DOOR_BAND_M
        # Wound to face +across, toward the track.
        still.add(f"crowd-{slot}", ([at(a1, b, 0.0), at(a0, b, 0.0), at(a0, b, band), at(a1, b, band)], [(0, 1, 2, 3)]))
        still.add(
            f"crowd-{slot}-lit",
            ([at(a1, b, band), at(a0, b, band), at(a0, b, PITS_DOOR_HEIGHT_M), at(a1, b, PITS_DOOR_HEIGHT_M)], [(0, 1, 2, 3)]),
        )
        pole_b = front - PITS_POLE_FROM_FRONT_M
        top = PITS_HEIGHT_M + PITS_POLE_HEIGHT_M
        pole = _block(
            origin,
            along,
            across,
            (middle - PITS_POLE_M / 2.0, middle + PITS_POLE_M / 2.0),
            (pole_b - PITS_POLE_M / 2.0, pole_b + PITS_POLE_M / 2.0),
            (PITS_HEIGHT_M, top),
        )
        _sides(pole, "carbon", still, top=True)
        # The flag: a pennant flying forward along the track from the pole's top, in the
        # plane along the track so the grid sees it face on.
        hoist = middle + PITS_POLE_M / 2.0
        flag = [at(hoist, pole_b, top), at(hoist, pole_b, top - PITS_FLAG_HEIGHT_M), at(hoist + PITS_FLAG_LENGTH_M, pole_b, top - PITS_FLAG_HEIGHT_M / 2.0)]
        still.add(f"crowd-{slots - 1 - slot}", (flag, [(0, 1, 2)]))
    return [], []


def _outward(quad: list[mesh.Vertex], centre: mesh.Vertex) -> list[mesh.Vertex]:
    """`quad` wound so its normal points away from `centre`."""
    a, b, c = quad[0], quad[1], quad[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    normal = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    middle = tuple(sum(p[i] for p in quad) / len(quad) for i in range(3))
    away = sum(normal[i] * (middle[i] - centre[i]) for i in range(3))
    return quad if away >= 0.0 else list(reversed(quad))


def _drum(face: mesh.Vertex, axis: mesh.Vertex, radius: float, depth: float, sides: int, first: mesh.Vertex) -> list[list[mesh.Vertex]]:
    """A closed drum of `sides` flat sides: a prism of `radius` running `depth` from the centre
    of its `face` along the unit `axis`, a corner along `first`, every face wound to point out."""
    u = tuple(first[i] - sum(first[j] * axis[j] for j in range(3)) * axis[i] for i in range(3))
    length = math.sqrt(sum(c * c for c in u))
    u = (u[0] / length, u[1] / length, u[2] / length)
    v = (axis[1] * u[2] - axis[2] * u[1], axis[2] * u[0] - axis[0] * u[2], axis[0] * u[1] - axis[1] * u[0])
    near = [
        tuple(face[i] + radius * (math.cos(math.tau * k / sides) * u[i] + math.sin(math.tau * k / sides) * v[i]) for i in range(3))
        for k in range(sides)
    ]
    far = [tuple(p[i] + depth * axis[i] for i in range(3)) for p in near]
    centre = tuple(face[i] + depth / 2.0 * axis[i] for i in range(3))
    faces = [[near[k], near[(k + 1) % sides], far[(k + 1) % sides], far[k]] for k in range(sides)]
    faces.append(list(reversed(near)))
    faces.append(far)
    return [_outward(f, centre) for f in faces]


PROP_BUILDERS: dict[str, Callable[[Layout, Prop, int, ByMaterial], tuple[list[SceneryMesh], list[Lettering]]]] = {
    "circuit-wheel": circuit_wheel,
    "arch": arch,
    "pits": pits,
}

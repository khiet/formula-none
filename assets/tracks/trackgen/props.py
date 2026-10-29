"""Trackside props: scenery a layout declares in its decor and the builder
places, never collided with and never in the sidecar (ADR 0050). Today the
woods: a patch of trees the builder scatters, each tree the style guide's
Woods card in three dimensions, flat facets in a lit and a shade tone on a
trunk, standing flat at the grass's level."""

from __future__ import annotations

import math
import random
from collections.abc import Callable, Iterator, Sequence
from dataclasses import dataclass

from . import mesh
from .layout import (
    GRASS_DROP_M,
    GRASS_MARGIN_M,
    SCENERY_LIFT_M,
    TREE_SPACING_M,
    TREE_WALL_CLEARANCE_M,
    WALL_THICKNESS_M,
    ByMaterial,
    Disc,
    Layout,
    SceneryMesh,
    Wood,
)

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


@dataclass(frozen=True)
class Tree:
    """One scattered tree: its kind, its trunk's foot in track metres, the way its facets turn."""

    kind: str
    x: float
    y: float
    yaw: float
    young: bool = False


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
    another tree, `placed` or this wood's own, is dropped. A disc draws again in its place; a
    row leaves the gap.
    """
    rng = random.Random(wood.seed)
    reach = CANOPY_RADIUS_M[wood.kind] + TREE_WALL_CLEARANCE_M
    wall = layout.runoff_half_width + WALL_THICKNESS_M
    patch = wood.patch
    (x0, y0, x1, y1) = patch_bounds(wood)
    centre = ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    radius = math.dist(centre, (x1, y1))
    # Only the centreline near the patch can put a wall within reach of a tree.
    near = [(s.x, s.y) for s in layout.path.samples if math.dist((s.x, s.y), centre) <= radius + wall + reach]

    def clear(x: float, y: float) -> bool:
        return all(math.dist((x, y), p) >= wall + reach for p in near)

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
    """Grass under the woods beyond the slab the sim stands on: a hair under it, so it shows
    only past the slab's edge, and scenery, so the sidecar never grows for a wood."""
    if not layout.decor.woods:
        return []
    min_x, min_y, max_x, max_y = layout.path.bounds()
    slab = (min_x - GRASS_MARGIN_M, min_y - GRASS_MARGIN_M, max_x + GRASS_MARGIN_M, max_y + GRASS_MARGIN_M)
    covered = [patch_bounds(wood) for wood in layout.decor.woods]
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


# One builder per kind of tree.
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


def _toned(material: str, triangle: list[mesh.Vertex]) -> tuple[str, mesh.Mesh]:
    """The triangle, wound as given, in `material` when it faces up and toward the sun, else its shade."""
    a, b, c = triangle
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    normal = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    lit = normal[2] >= 0.0 and normal[0] * SUN_AZIMUTH[0] + normal[1] * SUN_AZIMUTH[1] >= 0.0
    return material if lit else f"{material}-shade", (triangle, [(0, 1, 2)])

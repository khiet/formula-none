"""Trackside props: scenery a layout declares in its decor and the builder
places, never collided with and never in the sidecar (ADR 0050). The
woods: a patch of trees the builder scatters, each tree the style guide's
Woods card in three dimensions, flat facets in a lit and a shade tone on a
trunk, standing flat at the grass's level. The Circuit Wheel: the card's
Ferris wheel outside the first curve, its rim and gondolas nodes of their
own, pivoted where the View turns them. The arch: a giant tyre spanning
Dunlop, lettered with its sponsor on both faces. The pit building: a row
of garages beside the start straight, a door and a flag in each livery's
key colour, which the grid faces through the countdown. The hangars: a row
of barrel-roofed sheds outside Goldstone's Hangar Straight. The airship: a
twelve-sided hull circling high over the infield, a node of its own the
View flies round its circle, its sponsor riding on both flanks. The
helicopter: Monarch's red-and-white helicopter circling the harbour, a body
node the View flies and dips with two rotor nodes it turns under it, its
sponsor riding on both cabin flanks. The display jets: Goldstone's three red
jets in an arrowhead, each trailing a smoke ribbon, one node the View flies
past over the hangars. The harbour:
Monarch's basin of water inside a quay, yachts moored along it. The casino:
Monarch's grand house outside Casino Square, on a terrace level with the
crest."""

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
    SPAN_FOOT_CLEARANCE_M,
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
    Spot,
    Wood,
    elevation,
    locate,
)
from .sponsors import AIRSHIP_SPONSOR, ARCH_SPONSOR, HELICOPTER_SPONSOR

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
"""To the belt's middle, which the lettering, centred on its height, bends round."""
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

# A row of airfield hangars, Goldstone's nod to the airfield it stands on:
# sheds side by side along the track, each a box to the eaves under a barrel
# roof of `HANGAR_ROOF_FACETS` flat facets, its gable end toward the track
# with a dark door in it. The layout's prop gives the row's length; as many
# hangars as fit stand at the pitch, the row centred on its spot.
HANGAR_WIDTH_M = 18.0
"""Along the track, wall to wall."""
HANGAR_DEPTH_M = 24.0
"""Across the track, gable to gable."""
HANGAR_EAVES_M = 6.0
HANGAR_RIDGE_M = 10.0
HANGAR_PITCH_M = 22.0
HANGAR_ROOF_FACETS = 6
HANGAR_DOOR_WIDTH_M = 12.0
HANGAR_DOOR_HEIGHT_M = 5.0
HANGAR_DOOR_PROUD_M = 0.05

# The airship, the card in three dimensions: a hull of `AIRSHIP_SIDES` flat
# sides round its axis, swelling from the nose to a cylinder amidships and
# tapering to the tail, with a flat at the top, the bottom and each flank so
# the gondola hangs from a flat and the name sits on one. Its node's origin
# is the nose, on the axis, and its rest heading is the tangent of the
# circle the View flies it round, the circle's centre to its left. The
# layout's prop gives the circle and the height of the axis over the grass;
# the track tests hold its lowest point `AIRSHIP_CLEARANCE_M` over every road
# under its circle.
AIRSHIP_LENGTH_M = 24.0
AIRSHIP_RADIUS_M = 4.0
"""To a corner of the widest ring; the hull is twice this across."""
AIRSHIP_SIDES = 12
AIRSHIP_APOTHEM_M = AIRSHIP_RADIUS_M * math.cos(math.pi / AIRSHIP_SIDES)
"""From the axis to a flat of the widest ring."""
# The hull's profile from the nose, as (metres from the nose, radius there);
# the first and last are the nose and the tail, points on the axis.
AIRSHIP_PROFILE = ((0.0, 0.0), (2.5, 2.4), (5.0, AIRSHIP_RADIUS_M), (19.0, AIRSHIP_RADIUS_M), (22.0, 2.2), (AIRSHIP_LENGTH_M, 0.0))
AIRSHIP_CLEARANCE_M = 30.0
"""The least the lowest point keeps over any road under its circle."""
# Four fins at the tail, one up, one down and one each flank, each a thin
# plate swept back, its root buried in the hull: its outline in (metres from
# the nose, metres from the axis), root first, read round.
AIRSHIP_FIN = ((18.0, 1.0), (22.5, 1.0), (23.5, 6.5), (20.5, 6.5))
AIRSHIP_FIN_M = 0.3
# The gondola: a carbon box hung under the bottom flat amidships, spanning
# these metres from the nose, this wide, this tall, this far under the flat,
# on two square struts of this side at these metres from the nose.
AIRSHIP_GONDOLA_SPAN = (9.0, 15.0)
AIRSHIP_GONDOLA_WIDTH_M = 2.0
AIRSHIP_GONDOLA_HEIGHT_M = 1.6
AIRSHIP_GONDOLA_DROP_M = 0.6
AIRSHIP_STRUTS_AT = (10.0, 14.0)
AIRSHIP_STRUT_M = 0.25
# The name amidships on each flank, centred this far from the nose, just
# proud of the flat.
AIRSHIP_TEXT_AT_M = 12.0
AIRSHIP_TEXT_SIZE_M = 1.2

# The helicopter, the sky-objects mockup's Monarch card in three dimensions:
# a cabin of one side profile across its width, a tapering boom to a kerb-red
# fin, carbon glass, skids and rotors, and a kerb-red stripe down each flank.
# Every profile is (metres back from the nose, metres up from the cabin's
# floor). Its node's origin is the nose, its rest heading the tangent of the
# circle the View flies it round, the circle's centre to its left; the
# layout's prop gives the circle and the nose's height over the grass. The
# View dips the nose `HELICOPTER_DIP_DEG` about it, and the track tests hold
# the lowest point, dipped and with the rotors at any angle,
# `HELICOPTER_CLEARANCE_M` over every road under its circle.
HELICOPTER_NOSE_UP_M = 1.2
"""The nose over the cabin's floor."""
HELICOPTER_CABIN = ((0.0, 1.2), (0.8, 2.6), (2.4, 3.4), (5.2, 3.4), (6.0, 2.6), (6.0, 0.6), (4.8, 0.0), (1.2, 0.0))
"""The cabin's side profile, round from the nose over the roof; the first edge is the windscreen."""
HELICOPTER_CABIN_WIDTH_M = 2.2
HELICOPTER_BOOM = ((5.8, 1.6, 2.9, 0.8), (12.0, 2.25, 2.75, 0.4))
"""The boom's root, buried in the cabin, and its tail end: back from the nose, bottom, top, width."""
HELICOPTER_FIN = ((11.0, 2.6), (12.3, 2.6), (12.9, 4.6), (12.1, 4.6))
"""Root first, buried in the boom, read round."""
HELICOPTER_FIN_M = 0.2
HELICOPTER_LENGTH_M = 12.9
"""From the nose to the fin's tip."""
HELICOPTER_PANEL_PROUD_M = 0.03
HELICOPTER_STRIPE = ((0.3, 6.0), (1.15, 1.6))
"""Back from the nose and up from the floor, on each flank."""
HELICOPTER_WINDSCREEN = ((0.3, 1.6), (0.9, 2.5), (2.3, 3.05), (2.55, 1.6))
"""The windscreen's flank, each side, wrapping round from the nose's glass; it starts at the
stripe's top, since the two stand equally proud and would fight where they overlapped."""
HELICOPTER_WINDOW = ((3.1, 4.9), (1.9, 2.9))
HELICOPTER_MAST = ((2.7, 3.3), 0.6, (3.4, 4.05))
"""Back from the nose, square across, from the roof up to the hub."""
HELICOPTER_MAIN_HUB = (3.0, 4.1)
HELICOPTER_MAIN_BLADE = (5.5, 0.3, 0.08)
"""Reach from the hub, chord and thickness of each of the two blades: an 11 m rotor."""
HELICOPTER_TAIL_HUB = (12.4, 3.2, 0.25)
"""Back from the nose, up from the floor, and to the left of the fin's plane."""
HELICOPTER_TAIL_BLADE = (1.05, 0.15, 0.05)
HELICOPTER_SKIDS = ((0.4, 5.8), 1.3, -0.45, 0.15)
"""Back from the nose, either side of the middle and clear of the cabin, the bar's middle under the floor, and its side."""
HELICOPTER_STRUTS_AT = (1.3, 4.7)
HELICOPTER_STRUT_M = 0.12
HELICOPTER_TEXT_AT = (3.0, 0.8)
"""The name's middle on each flank, between the skids' line and the stripe."""
HELICOPTER_TEXT_SIZE_M = 0.42
HELICOPTER_DIP_DEG = 5.0
"""The View's nose-down pitch; the track tests hold it to `scenery-motion.ts`."""
HELICOPTER_CLEARANCE_M = 30.0
"""The least the lowest point keeps over any road under its circle."""

# The display jets, the sky-objects mockup's Goldstone card in three
# dimensions: three kerb-red jets in an arrowhead, each a square-sectioned
# fuselage to a pointed nose with swept wings, a fin, a carbon canopy and a
# wall flash down each flank, trailing a fixed smoke ribbon that swells from
# a point at its tail and closes again. The formation is one node with its
# origin at the leader's nose and its local x along the noses; the View only
# slides it along that axis. Every colour is flat, as the airship's fins and
# the helicopter's stripe and fin are: neither kerb red nor ink blue has a
# shade tone, and one ribbon toned beside two flat ones would not match. Every
# jet's parts are in metres back from its nose, to the left of its axis and
# up from it. The layout's prop gives the pass's length, centred on `at`, and
# the leader's height over the grass; the track tests hold the lowest point,
# ribbons included, `JETS_CLEARANCE_M` over every road the pass crosses.
JET_LENGTH_M = 9.0
"""From the nose to the tail, where the ribbon starts."""
JET_NOSE_M = 1.8
"""From the point to the full section."""
JET_BODY_M = 0.9
"""The fuselage's square section, side to side and top to bottom."""
JET_WING = ((3.0, 6.5), (5.8, 7.0), 4.2, 0.12)
"""The root's and the tip's span back from the nose, the tip's reach left or right, and the thickness."""
JET_FIN = ((6.8, 8.6), (8.3, 9.2), 2.6, 0.12)
"""The root's and the tip's span back from the nose, the tip's height over the axis, and the thickness."""
JET_CANOPY = ((2.0, 4.2), 0.6, 0.4)
"""Back from the nose, across, and up from the fuselage's top."""
JET_FLASH = ((JET_NOSE_M, JET_LENGTH_M), (-0.25, -0.05))
"""Back from the nose and up from the axis, on each flank."""
JET_PANEL_PROUD_M = 0.03
JETS_FORMATION = ((0.0, 0.0, 0.0), (12.0, 8.0, 4.5), (12.0, -8.0, -4.5))
"""Each jet's nose back from the leader's, left of it and up from it: the leader, then a
wingman stepped up on the left and one stepped down on the right."""
JETS_SMOKE = ("wall", "ink-blue", "kerb-red")
"""Each jet's ribbon, in `JETS_FORMATION`'s order."""
JETS_RIBBON = ((JET_LENGTH_M, 0.0), (56.0, 1.7), (60.0, 0.0))
"""Back from the jet's nose and the ribbon's half-width there, up, down and to each side."""
JETS_CLEARANCE_M = 30.0
"""The least the lowest point keeps over any road under the pass."""

# The harbour, Monarch's basin of sea water: a flat sheet of water a hair
# over the grass, ringed by a stone quay standing proud of it. The layout's
# prop gives the quay's outer edge as an outline.
QUAY_WIDTH_M = 2.0
QUAY_HEIGHT_M = 0.6
"""Over the water, so the quay's inner face reads as the drop to the sea."""
# The yachts moor stern-to along every quay whose middle lies within
# `YACHT_MOORING_REACH_M` of the lap, one every `YACHT_PITCH_M`, their
# lengths cycling through `YACHT_LENGTHS_M`. As a wood drops a tree, the
# harbour drops a yacht that would touch one already moored or come nearer
# the quay than its stern does, so two rows never meet in a corner. A yacht
# is a pointed hull with a cabin and a flybridge on it, each as a share of
# its length.
YACHT_PITCH_M = 7.0
YACHT_LENGTHS_M = (14.0, 18.0, 12.0)
YACHT_MOORING_REACH_M = 40.0
YACHT_OFF_QUAY_M = 1.0
YACHT_BEAM = 0.3
YACHT_BOW = 0.3
"""The share of the length the bow tapers over."""
YACHT_FREEBOARD_M = 1.4
YACHT_CABIN = (0.2, 0.65, 1.0, 1.3)
"""From and to as shares of the length from the stern, inset from each side, and tall."""
YACHT_FLYBRIDGE = (0.3, 0.55, 1.6, 0.9)
"""As the cabin's, standing on its roof."""
YACHT_WINDOW_BAND = (0.35, 0.8)
"""The cabin's window band, from and to as shares of its height."""
YACHT_WINDOW_PROUD_M = 0.03

# The casino, Monarch's grand house outside Casino Square: a stucco block
# under a copper hip roof, a square tower at each front corner under a copper
# pyramid, and a copper dome on an eight-sided drum over the door, standing on
# a stone terrace that rises from the grass to the road's height at its spot,
# so it stands level with the crest. Its front faces the track with a dark
# door and two rows of dark windows.
CASINO_LENGTH_M = 40.0
"""Along the track."""
CASINO_DEPTH_M = 20.0
CASINO_EAVES_M = 12.0
"""Over the terrace, as every height of the casino's is."""
CASINO_RIDGE_M = 16.0
CASINO_TOWER_M = 6.0
"""Square, flush with the front and the end."""
CASINO_TOWER_EAVES_M = 18.0
CASINO_TOP_M = 24.0
"""The towers' apexes, the casino's highest point."""
CASINO_DRUM_RADIUS_M = 3.5
CASINO_DRUM_BACK_M = 1.0
"""From the front to the drum's nearest edge."""
CASINO_DRUM_TOP_M = 15.0
CASINO_DOME_TOP_M = 21.0
CASINO_DRUM_SIDES = 8
CASINO_DOOR_WIDTH_M = 4.0
CASINO_DOOR_HEIGHT_M = 5.0
CASINO_WINDOW_WIDTH_M = 1.6
CASINO_WINDOWS_AT_M = (4.5, 8.5, 12.5)
"""Each window's middle from the door's, either way along the front, all clear of the door."""
CASINO_WINDOW_ROWS = ((1.5, 5.0), (7.0, 10.5))
"""Each row's sill and head."""
CASINO_PANEL_PROUD_M = 0.05


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
    `radius` about (`x`, `y`), the counter-clockwise `outline` when it has one, or else the
    rectangle reaching `half_along` either way along `heading` and `half_across` either side
    of it. A tree keeps its canopy out of it,
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
    outline: tuple[tuple[float, float], ...] = ()

    def distance(self, x: float, y: float) -> float:
        """From the point to the footprint's edge, negative inside it."""
        if self.outline:
            edge = _distance_to_outline(self.outline, x, y)
            return -edge if _inside(self.outline, x, y) else edge
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
        if self.outline:
            xs, ys = [x for x, _ in self.outline], [y for _, y in self.outline]
            return min(xs), min(ys), max(xs), max(ys)
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


def _no_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    """A prop that flies stands on no ground: its plan position is free, since it passes
    over the track."""
    return []


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
            SPAN_FOOT_CLEARANCE_M,
            heading=heading,
            half_along=ARCH_DEPTH_M / 2.0 + ARCH_ABUTMENT_MARGIN_M,
            half_across=(foot_out - foot_in) / 2.0,
        )
        for side in (1.0, -1.0)
    ]


def _pits_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    return [_row_footprint(layout, prop, PITS_DEPTH_M, "the pits need a length")]


def _hangars_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    return [_row_footprint(layout, prop, HANGAR_DEPTH_M, "the hangars need a length")]


def _row_footprint(layout: Layout, prop: Prop, depth: float, missing: str) -> Footprint:
    """The plan of a prop that runs `prop.length` along the track, `depth` deep across it."""
    x, y, heading = locate(layout, prop.at)
    if prop.length is None:
        raise ValueError(missing)
    return Footprint(prop.kind, x, y, PROP_WALL_CLEARANCE_M, heading=heading, half_along=prop.length / 2.0, half_across=depth / 2.0)


def _harbour_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    """The basin, quay and all: yachts moor in it, so it keeps the 8 m rule."""
    x, y, heading = locate(layout, prop.at)
    if not prop.outline:
        raise ValueError("the harbour needs an outline")
    cos, sin = math.cos(heading), math.sin(heading)
    outline = tuple((x + a * cos - b * sin, y + a * sin + b * cos) for a, b in prop.outline)
    return [Footprint(prop.kind, x, y, PROP_WALL_CLEARANCE_M, outline=outline)]


def _casino_footprint(layout: Layout, prop: Prop) -> list[Footprint]:
    """The casino's plan. It stands on the lap, since its terrace rises to the road's height there."""
    if not isinstance(prop.at, Spot):
        raise ValueError("the casino stands on a spot on the lap")
    x, y, heading = locate(layout, prop.at)
    return [Footprint(prop.kind, x, y, PROP_WALL_CLEARANCE_M, heading=heading, half_along=CASINO_LENGTH_M / 2.0, half_across=CASINO_DEPTH_M / 2.0)]


def _facing_frame(prop: Prop, heading: float) -> tuple[mesh.Vertex, mesh.Vertex]:
    """The (along, across) axes of a prop that runs along the track, `across` pointing from
    the prop toward the track whichever side it stands on, so its front face is +across."""
    along = (math.cos(heading), math.sin(heading), 0.0)
    left = (-math.sin(heading), math.cos(heading), 0.0)
    toward_track = -1.0 if isinstance(prop.at, Spot) and prop.at.offset > 0.0 else 1.0
    return along, (toward_track * left[0], toward_track * left[1], 0.0)


FOOTPRINTS: dict[str, Callable[[Layout, Prop], list[Footprint]]] = {
    "circuit-wheel": _wheel_footprint,
    "arch": _arch_footprints,
    "pits": _pits_footprint,
    "hangars": _hangars_footprint,
    "airship": _no_footprint,
    "helicopter": _no_footprint,
    "jets": _no_footprint,
    "harbour": _harbour_footprint,
    "casino": _casino_footprint,
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


def _sides(faces: dict[str, mesh.Mesh], material: str, into: ByMaterial | Node, shade: str | None = None, top: bool = False) -> None:
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
    the doors are colours only. A `white` building, Goldstone's, has wall-white walls over beam
    shade under a beam-top parapet instead. It faces the track from either side."""
    (plan,) = _pits_footprint(layout, prop)
    x, y, heading = plan.x, plan.y, plan.heading
    ground = -GRASS_DROP_M
    origin = (x, y, ground)
    along, across = _facing_frame(prop, heading)
    half = plan.half_along
    front = plan.half_across
    walls, shade, rim = ("wall", "beam-shade", "beam-top") if prop.white else ("stand-riser", "stand-plinth", "stand-plinth")
    body = (0.0, PITS_HEIGHT_M - PITS_PARAPET_M)
    _sides(_block(origin, along, across, (-half, half), (-front, front), body), walls, still, shade=shade)
    parapet = _block(origin, along, across, (-half, half), (-front, front), (body[1], PITS_HEIGHT_M))
    _sides(parapet, rim, still)
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
        behind = at(middle, 0.0, 0.0)
        door = [at(a1, b, 0.0), at(a0, b, 0.0), at(a0, b, band), at(a1, b, band)]
        still.add(f"crowd-{slot}", (_outward(door, behind), [(0, 1, 2, 3)]))
        lit = [at(a1, b, band), at(a0, b, band), at(a0, b, PITS_DOOR_HEIGHT_M), at(a1, b, PITS_DOOR_HEIGHT_M)]
        still.add(f"crowd-{slot}-lit", (_outward(lit, behind), [(0, 1, 2, 3)]))
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
        # plane along the track and wound to face it, so the grid sees it face on whether
        # or not the material is drawn from both sides.
        hoist = middle + PITS_POLE_M / 2.0
        flag = [at(hoist, pole_b, top - PITS_FLAG_HEIGHT_M), at(hoist, pole_b, top), at(hoist + PITS_FLAG_LENGTH_M, pole_b, top - PITS_FLAG_HEIGHT_M / 2.0)]
        still.add(f"crowd-{slots - 1 - slot}", (_outward(flag, at(hoist, -front, top)), [(0, 1, 2)]))
    return [], []


def hangars(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """A row of airfield hangars at `prop`, `prop.length` long along the track: as many as fit
    at `HANGAR_PITCH_M`, centred on the spot, each with its gable end toward the track. Walls
    and gables in stand riser where the sun reaches and stand plinth elsewhere, the barrel
    roof's facets in beam top lit over beam shade, and a carbon door on the front gable. No
    lettering: the hangars carry no sponsor."""
    (plan,) = _hangars_footprint(layout, prop)
    ground = -GRASS_DROP_M
    along, across = _facing_frame(prop, plan.heading)
    count = int((plan.half_along * 2.0 - HANGAR_WIDTH_M) // HANGAR_PITCH_M) + 1
    front = plan.half_across
    half = HANGAR_WIDTH_M / 2.0
    # The roof's arc across the hangar's width: a circle through both eaves and the ridge.
    rise = HANGAR_RIDGE_M - HANGAR_EAVES_M
    radius = (half * half + rise * rise) / (2.0 * rise)
    sweep = math.asin(half / radius)
    profile = [
        (radius * math.sin(t), HANGAR_RIDGE_M - radius + radius * math.cos(t))
        for t in (-sweep + 2.0 * sweep * k / HANGAR_ROOF_FACETS for k in range(HANGAR_ROOF_FACETS + 1))
    ]

    def at(a: float, b: float, c: float) -> mesh.Vertex:
        return (plan.x + a * along[0] + b * across[0], plan.y + a * along[1] + b * across[1], ground + c)

    for k in range(count):
        middle = (k - (count - 1) / 2.0) * HANGAR_PITCH_M
        hub = at(middle, 0.0, HANGAR_EAVES_M / 2.0)
        for a in (middle - half, middle + half):
            wall = [at(a, -front, 0.0), at(a, front, 0.0), at(a, front, HANGAR_EAVES_M), at(a, -front, HANGAR_EAVES_M)]
            still.add(*_toned("stand-riser", _outward(wall, hub), "stand-plinth"))
        # Each gable: the wall to the eaves with the roof's arc over it, one convex face.
        for b in (-front, front):
            gable = [at(middle + half, b, 0.0), at(middle - half, b, 0.0)] + [at(middle + u, b, z) for u, z in profile]
            still.add(*_toned("stand-riser", _outward(gable, hub), "stand-plinth"))
        for (u0, z0), (u1, z1) in zip(profile, profile[1:]):
            facet = [at(middle + u0, -front, z0), at(middle + u1, -front, z1), at(middle + u1, front, z1), at(middle + u0, front, z0)]
            still.add(*_toned("beam-top", _outward(facet, at(middle, 0.0, 0.0)), "beam-shade"))
        b = front + HANGAR_DOOR_PROUD_M
        d = HANGAR_DOOR_WIDTH_M / 2.0
        door = [at(middle - d, b, 0.0), at(middle + d, b, 0.0), at(middle + d, b, HANGAR_DOOR_HEIGHT_M), at(middle - d, b, HANGAR_DOOR_HEIGHT_M)]
        still.add("carbon", (_outward(door, hub), [(0, 1, 2, 3)]))
    return [], []


def airship(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The airship at `prop`: nothing into `still`, and the whole of it as the one node
    `Prop.airship.<number>.hull`, its origin at the nose and its local x along the nose, at
    rest `prop.radius` metres from `prop.at` along +x with the circle's centre to its left,
    the axis `prop.height` metres over the grass. The hull in wall white where the sun
    reaches and beam shade elsewhere, as it hangs at rest; the fins kerb red; the gondola
    and struts carbon. The sponsor from `sponsors.py` on both flanks, each lettering hung
    under the hull's node so it rides with it."""
    if prop.radius is None or prop.height is None:
        raise ValueError("the airship needs a radius and a height")
    cx, cy, _ = locate(layout, prop.at)
    nose = (cx + prop.radius, cy, -GRASS_DROP_M + prop.height)
    heading = math.pi / 2.0
    along = (math.cos(heading), math.sin(heading), 0.0)
    left = (-math.sin(heading), math.cos(heading), 0.0)

    def at(s: float, b: float, c: float) -> mesh.Vertex:
        """`s` metres back from the nose, `b` to the left of the axis and `c` up from it."""
        return (nose[0] - s * along[0] + b * left[0], nose[1] - s * along[1] + b * left[1], nose[2] + c)

    name = f"Prop.airship.{number}.hull"
    hull = Node(name, Pivot(*nose, heading))
    # The hull: a ring at each station of the profile, its corners a half side off the
    # flanks, closed to a point at the nose and the tail; every face wound out of the axis.
    corners = [math.pi / AIRSHIP_SIDES + math.tau * k / AIRSHIP_SIDES for k in range(AIRSHIP_SIDES)]
    rings = [[at(s, r * math.cos(a), r * math.sin(a)) for a in corners] for s, r in AIRSHIP_PROFILE[1:-1]]
    stations = [s for s, _ in AIRSHIP_PROFILE[1:-1]]
    for k in range(AIRSHIP_SIDES):
        j = (k + 1) % AIRSHIP_SIDES
        hull.add(*_toned("wall", _outward([at(0.0, 0.0, 0.0), rings[0][k], rings[0][j]], at(stations[0], 0.0, 0.0)), "beam-shade"))
        for i in range(len(rings) - 1):
            axis = at((stations[i] + stations[i + 1]) / 2.0, 0.0, 0.0)
            hull.add(*_toned("wall", _outward([rings[i][k], rings[i][j], rings[i + 1][j], rings[i + 1][k]], axis), "beam-shade"))
        hull.add(*_toned("wall", _outward([rings[-1][j], rings[-1][k], at(AIRSHIP_LENGTH_M, 0.0, 0.0)], at(stations[-1], 0.0, 0.0)), "beam-shade"))
    # The fins: a plate either side of its plane and a face along each outer edge; the root
    # edge is inside the hull.
    for a in (0.0, math.pi / 2.0, math.pi, 3.0 * math.pi / 2.0):
        radial = (math.cos(a), math.sin(a))
        thick = (-math.sin(a) * AIRSHIP_FIN_M / 2.0, math.cos(a) * AIRSHIP_FIN_M / 2.0)
        plates = [
            [at(s, r * radial[0] + side * thick[0], r * radial[1] + side * thick[1]) for s, r in AIRSHIP_FIN]
            for side in (1.0, -1.0)
        ]
        middle = at(sum(s for s, _ in AIRSHIP_FIN) / len(AIRSHIP_FIN), 0.0, 0.0)
        for plate in plates:
            hull.add("kerb-red", (_outward(plate, middle), [(0, 1, 2, 3)]))
        for i in range(1, len(AIRSHIP_FIN)):
            j = (i + 1) % len(AIRSHIP_FIN)
            hull.add("kerb-red", (_outward([plates[0][i], plates[0][j], plates[1][j], plates[1][i]], middle), [(0, 1, 2, 3)]))
    # The gondola under the bottom flat, every face since it is seen from below, and the
    # struts up to it.
    astern = (-along[0], -along[1], 0.0)
    top = -AIRSHIP_APOTHEM_M - AIRSHIP_GONDOLA_DROP_M
    half = AIRSHIP_GONDOLA_WIDTH_M / 2.0
    gondola = _block(nose, astern, left, AIRSHIP_GONDOLA_SPAN, (-half, half), (top - AIRSHIP_GONDOLA_HEIGHT_M, top))
    for vertices, (face,) in gondola.values():
        hull.add(*_toned("carbon", [vertices[i] for i in face]))
    for s in AIRSHIP_STRUTS_AT:
        strut = _block(nose, astern, left, (s - AIRSHIP_STRUT_M / 2.0, s + AIRSHIP_STRUT_M / 2.0), (-AIRSHIP_STRUT_M / 2.0, AIRSHIP_STRUT_M / 2.0), (top, -AIRSHIP_APOTHEM_M))
        _sides(strut, "carbon", hull, shade="carbon-shade")
    # The name on each flank, just proud of the flat, read from that side.
    text, ink = AIRSHIP_SPONSOR
    proud = AIRSHIP_APOTHEM_M + HOARDING_TEXT_PROUD_M
    lettering = [
        Lettering(
            f"Prop.airship.{number}.flank.{flank}",
            text,
            ink,
            AIRSHIP_TEXT_SIZE_M,
            Placement(*at(AIRSHIP_TEXT_AT_M, side * proud, 0.0), heading + side * math.pi / 2.0),
            parent=name,
        )
        for flank, side in (("left", 1.0), ("right", -1.0))
    ]
    return [hull.scenery()], lettering


def helicopter(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The helicopter at `prop`: nothing into `still`, and the body as the node
    `Prop.helicopter.<number>.body`, its origin at the nose and its local x along the nose, at
    rest `prop.radius` metres from `prop.at` along +x with the circle's centre to its left,
    the nose `prop.height` metres over the grass. Under it hang `.main-rotor`, two blades
    turning about its local z, and `.tail-rotor`, two blades turning about its local y, each
    with its origin at its hub. The cabin and boom in wall white where the sun reaches and
    beam shade elsewhere, as it hangs at rest; the stripe and fin kerb red; the glass, mast,
    skids and rotors carbon. The sponsor from `sponsors.py` on both flanks, each lettering
    hung under the body's node so it rides with it."""
    if prop.radius is None or prop.height is None:
        raise ValueError("the helicopter needs a radius and a height")
    cx, cy, _ = locate(layout, prop.at)
    nose = (cx + prop.radius, cy, -GRASS_DROP_M + prop.height)
    heading = math.pi / 2.0
    along = (math.cos(heading), math.sin(heading), 0.0)
    left = (-math.sin(heading), math.cos(heading), 0.0)
    astern = (-along[0], -along[1], 0.0)
    floor = -HELICOPTER_NOSE_UP_M

    def at(s: float, b: float, up: float) -> mesh.Vertex:
        """`s` metres back from the nose, `b` to the left of the middle and `up` from the floor."""
        return (nose[0] - s * along[0] + b * left[0], nose[1] - s * along[1] + b * left[1], nose[2] + floor + up)

    name = f"Prop.helicopter.{number}.body"
    body = Node(name, Pivot(*nose, heading))
    # The cabin: its profile on each flank and a face along each edge between them, the
    # windscreen's in glass.
    half = HELICOPTER_CABIN_WIDTH_M / 2.0
    middle = at(3.0, 0.0, 1.7)
    for side in (1.0, -1.0):
        body.add(*_toned("wall", _outward([at(s, side * half, u) for s, u in HELICOPTER_CABIN], middle), "beam-shade"))
    for k, (s0, u0) in enumerate(HELICOPTER_CABIN):
        s1, u1 = HELICOPTER_CABIN[(k + 1) % len(HELICOPTER_CABIN)]
        edge = _outward([at(s0, half, u0), at(s1, half, u1), at(s1, -half, u1), at(s0, -half, u0)], middle)
        body.add(*(("carbon", (edge, [(0, 1, 2, 3)])) if k == 0 else _toned("wall", edge, "beam-shade")))
    # Panels just proud of each flank: the stripe, the windscreen's sides and the window.
    proud = half + HELICOPTER_PANEL_PROUD_M
    (s0, s1), (u0, u1) = HELICOPTER_STRIPE
    (w0, w1), (v0, v1) = HELICOPTER_WINDOW
    for side in (1.0, -1.0):
        for material, outline in (
            ("kerb-red", ((s0, u0), (s1, u0), (s1, u1), (s0, u1))),
            ("carbon", HELICOPTER_WINDSCREEN),
            ("carbon", ((w0, v0), (w1, v0), (w1, v1), (w0, v1))),
        ):
            body.add(material, (_outward([at(s, side * proud, u) for s, u in outline], middle), [(0, 1, 2, 3)]))

    # The boom: a box tapering from its root inside the cabin to the tail.
    def boom(i: int, j: int, k: int) -> mesh.Vertex:
        s, bottom, top, width = HELICOPTER_BOOM[i]
        return at(s, (j - 0.5) * width, (bottom, top)[k])

    for vertices, (face,) in mesh.cuboid(boom).values():
        body.add(*_toned("wall", [vertices[i] for i in face], "beam-shade"))
    # The fin: a plate either side of the boom's plane and a face along each outer edge.
    plates = [[at(s, side * HELICOPTER_FIN_M / 2.0, u) for s, u in HELICOPTER_FIN] for side in (1.0, -1.0)]
    fin_middle = at(sum(s for s, _ in HELICOPTER_FIN) / len(HELICOPTER_FIN), 0.0, sum(u for _, u in HELICOPTER_FIN) / len(HELICOPTER_FIN))
    for plate in plates:
        body.add("kerb-red", (_outward(plate, fin_middle), [(0, 1, 2, 3)]))
    for i in range(1, len(HELICOPTER_FIN)):
        j = (i + 1) % len(HELICOPTER_FIN)
        body.add("kerb-red", (_outward([plates[0][i], plates[0][j], plates[1][j], plates[1][i]], fin_middle), [(0, 1, 2, 3)]))
    # The mast, the skids and their struts, in carbon toned against its shade.
    (m0, m1), width, (mu0, mu1) = HELICOPTER_MAST
    mast = _block(nose, astern, left, (m0, m1), (-width / 2.0, width / 2.0), (floor + mu0, floor + mu1))
    _sides(mast, "carbon", body, shade="carbon-shade")
    (k0, k1), apart, under, bar = HELICOPTER_SKIDS
    for side in (1.0, -1.0):
        skid = _block(nose, astern, left, (k0, k1), (side * apart - bar / 2.0, side * apart + bar / 2.0), (floor + under - bar / 2.0, floor + under + bar / 2.0))
        for vertices, (face,) in skid.values():
            body.add(*_toned("carbon", [vertices[i] for i in face]))
        for s in HELICOPTER_STRUTS_AT:
            strut = _block(
                nose,
                astern,
                left,
                (s - HELICOPTER_STRUT_M / 2.0, s + HELICOPTER_STRUT_M / 2.0),
                (side * apart - HELICOPTER_STRUT_M / 2.0, side * apart + HELICOPTER_STRUT_M / 2.0),
                (floor + under + bar / 2.0, floor),
            )
            _sides(strut, "carbon", body, shade="carbon-shade")
    # The rotors, flat carbon, since a tone baked at rest would be wrong once they turn.
    main_hub = at(HELICOPTER_MAIN_HUB[0], 0.0, HELICOPTER_MAIN_HUB[1])
    main_reach, main_chord, main_thick = HELICOPTER_MAIN_BLADE
    main = _rotor(f"Prop.helicopter.{number}.main-rotor", Pivot(*main_hub, heading, parent=name), along, left, main_reach, main_chord, main_thick)
    ts, tu, tb = HELICOPTER_TAIL_HUB
    tail_reach, tail_chord, tail_thick = HELICOPTER_TAIL_BLADE
    # The tail rotor's blades lie in the fin's upright plane: thin across, their chord upright.
    tail = _rotor(f"Prop.helicopter.{number}.tail-rotor", Pivot(*at(ts, tb, tu), heading, parent=name), astern, left, tail_reach, tail_thick, tail_chord)
    # The name on each flank, just proud of the cabin, read from that side.
    text, ink = HELICOPTER_SPONSOR
    lettering = [
        Lettering(
            f"Prop.helicopter.{number}.flank.{flank}",
            text,
            ink,
            HELICOPTER_TEXT_SIZE_M,
            Placement(*at(HELICOPTER_TEXT_AT[0], side * (half + HOARDING_TEXT_PROUD_M), HELICOPTER_TEXT_AT[1]), heading + side * math.pi / 2.0),
            parent=name,
        )
        for flank, side in (("left", 1.0), ("right", -1.0))
    ]
    # Parents before children, as the build script places them.
    return [body.scenery(), main.scenery(), tail.scenery()], lettering


def _rotor(name: str, pivot: Pivot, along: mesh.Vertex, across: mesh.Vertex, reach: float, width: float, height: float) -> Node:
    """Two carbon blades from the hub at `pivot`, each `reach` long, one either way along
    `along`, `width` across and `height` up."""
    hub = (pivot.x, pivot.y, pivot.z)
    rotor = Node(name, pivot)
    for blade in ((0.0, reach), (-reach, 0.0)):
        for vertices, (face,) in _block(hub, along, across, blade, (-width / 2.0, width / 2.0), (-height / 2.0, height / 2.0)).values():
            rotor.add("carbon", (vertices, [face]))
    return rotor


def jets(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The display jets at `prop`: nothing into `still`, and the whole formation, ribbons and
    all, as the one node `Prop.jets.<number>.formation`, its origin at the leader's nose and
    its local x along the noses, which point against the way `at` faces, so on a spot they fly
    against the race. At rest the leader's nose is over `at`, `prop.height` metres over the
    grass, in the middle of the `prop.length` pass. No lettering: it would not read at the
    height it flies."""
    if prop.length is None or prop.height is None:
        raise ValueError("the jets need a pass length and a height")
    x, y, facing = locate(layout, prop.at)
    heading = facing + math.pi
    leader = (x, y, -GRASS_DROP_M + prop.height)
    along = (math.cos(heading), math.sin(heading))
    left = (-along[1], along[0])
    formation = Node(f"Prop.jets.{number}.formation", Pivot(*leader, heading))
    half = JET_BODY_M / 2.0
    for (s0, b0, c0), smoke in zip(JETS_FORMATION, JETS_SMOKE):

        def at(s: float, b: float, c: float, s0: float = s0, b0: float = b0, c0: float = c0) -> mesh.Vertex:
            """`s` metres back from this jet's nose, `b` to the left of its axis and `c` up from it."""
            return (
                leader[0] - (s0 + s) * along[0] + (b0 + b) * left[0],
                leader[1] - (s0 + s) * along[1] + (b0 + b) * left[1],
                leader[2] + c0 + c,
            )

        def flat(material: str, faces: dict[str, mesh.Mesh], buried: str) -> None:
            """A box's faces but the one `buried` in the fuselage, flat in `material`."""
            for key, (vertices, (face,)) in faces.items():
                if key != buried:
                    formation.add(material, ([vertices[i] for i in face], [(0, 1, 2, 3)]))

        # The fuselage: a pyramid nose onto a square section running back to the tail.
        axis = at(JET_LENGTH_M / 2.0, 0.0, 0.0)
        corners = ((half, half), (-half, half), (-half, -half), (half, -half))
        for k, (b, c) in enumerate(corners):
            nb, nc = corners[(k + 1) % 4]
            formation.add("kerb-red", (_outward([at(0.0, 0.0, 0.0), at(JET_NOSE_M, b, c), at(JET_NOSE_M, nb, nc)], axis), [(0, 1, 2)]))
            side = [at(JET_NOSE_M, b, c), at(JET_LENGTH_M, b, c), at(JET_LENGTH_M, nb, nc), at(JET_NOSE_M, nb, nc)]
            formation.add("kerb-red", (_outward(side, axis), [(0, 1, 2, 3)]))
        formation.add("kerb-red", (_outward([at(JET_LENGTH_M, b, c) for b, c in corners], axis), [(0, 1, 2, 3)]))
        # The wings, swept, their roots in the fuselage's flanks.
        root, tip, reach, thick = JET_WING
        for side in (1.0, -1.0):
            wing = mesh.cuboid(lambda i, j, k, side=side: at((root, tip)[j][i], side * (half, reach)[j], (k - 0.5) * thick))
            flat("kerb-red", wing, "y0")
        root, tip, height, thick = JET_FIN
        flat("kerb-red", mesh.cuboid(lambda i, j, k: at((root, tip)[j][i], (k - 0.5) * thick, (half, height)[j])), "y0")
        (g0, g1), width, rise = JET_CANOPY
        flat("carbon", mesh.cuboid(lambda i, j, k: at((g0, g1)[i], (j - 0.5) * width, half + k * rise)), "z0")
        # The flash down each flank, just proud of it.
        (f0, f1), (u0, u1) = JET_FLASH
        for side in (1.0, -1.0):
            b = side * (half + JET_PANEL_PROUD_M)
            formation.add("wall", (_outward([at(f0, b, u0), at(f1, b, u0), at(f1, b, u1), at(f0, b, u1)], axis), [(0, 1, 2, 3)]))
        # The ribbon: a point at the tail swelling to a diamond section and closing again.
        (r0, _), (r1, spread), (r2, _) = JETS_RIBBON
        ring = [at(r1, spread * b, spread * c) for b, c in ((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0))]
        for point, middle in ((at(r0, 0.0, 0.0), at((r0 + r1) / 2.0, 0.0, 0.0)), (at(r2, 0.0, 0.0), at((r1 + r2) / 2.0, 0.0, 0.0))):
            for k in range(4):
                formation.add(smoke, (_outward([point, ring[k], ring[(k + 1) % 4]], middle), [(0, 1, 2)]))
    return [formation.scenery()], []


def harbour(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The harbour at `prop`: a sheet of `water` filling the basin inside a stone quay, the quay
    `QUAY_WIDTH_M` wide inside the outline with stand-plinth faces under a beam-top coping,
    and yachts moored stern-to along the quays near the lap, bows out into the basin. No
    lettering."""
    (plan,) = _harbour_footprint(layout, prop)
    outer = list(plan.outline)
    inner = _inset(outer, QUAY_WIDTH_M)
    ground = -GRASS_DROP_M
    sea = ground + SCENERY_LIFT_M
    top = sea + QUAY_HEIGHT_M
    count = len(outer)
    for k in range(count):
        (a, b), (c, d) = (outer[k], outer[(k + 1) % count]), (inner[k], inner[(k + 1) % count])
        dx, dy = b[0] - a[0], b[1] - a[1]
        # The outline runs counter-clockwise, so the basin is to each edge's left.
        out = (dy, -dx, 0.0)
        still.add("stand-plinth", mesh.quad_facing((*a, ground), (*b, ground), (*b, top), (*a, top), out))
        still.add("stand-plinth", mesh.quad_facing((*c, sea), (*d, sea), (*d, top), (*c, top), (-dy, dx, 0.0)))
        still.add("beam-top", mesh.quad_facing((*a, top), (*b, top), (*d, top), (*c, top), (0.0, 0.0, 1.0)))
    still.add("water", ([(x, y, sea) for x, y in inner], [tuple(range(count))]))

    moored: list[list[tuple[float, float]]] = []
    for k in range(count):
        a, b = inner[k], inner[(k + 1) % count]
        middle = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0)
        if min(math.dist(middle, (q.x, q.y)) for q in layout.path.samples) > YACHT_MOORING_REACH_M:
            continue
        length = math.dist(a, b)
        along = ((b[0] - a[0]) / length, (b[1] - a[1]) / length)
        inward = (-along[1], along[0])
        berths = int(length // YACHT_PITCH_M)
        for berth in range(berths):
            at = (berth - (berths - 1) / 2.0) * YACHT_PITCH_M + length / 2.0
            stern = (a[0] + along[0] * at + inward[0] * YACHT_OFF_QUAY_M, a[1] + along[1] * at + inward[1] * YACHT_OFF_QUAY_M)
            size = YACHT_LENGTHS_M[len(moored) % len(YACHT_LENGTHS_M)]
            hull = _hull_plan(stern, inward, size)
            # Its stern corners stand the berth's distance off the quay; nothing may stand nearer.
            if any(not _inside(inner, x, y) or _distance_to_outline(inner, x, y) < YACHT_OFF_QUAY_M - 1e-9 for x, y in hull):
                continue
            if any(_convex_overlap(hull, other) for other in moored):
                continue
            moored.append(hull)
            _yacht(still, (*stern, sea), inward, size)
    return [], []


def casino(layout: Layout, prop: Prop, number: int, still: ByMaterial) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The casino at `prop`, its front toward the track: a stand-plinth terrace with a beam-top
    paving from the grass to the road's height at its spot, and on it the stucco block, towers
    and drum toned against stucco shade, the roofs, pyramids and dome in copper toned against
    copper shade, and a carbon door and windows just proud of the front. No lettering."""
    (plan,) = _casino_footprint(layout, prop)
    road = elevation(layout.elevation, prop.at.s)
    grass = -GRASS_DROP_M
    terrace = road + grass
    along, across = _facing_frame(prop, plan.heading)
    half, front = plan.half_along, plan.half_across
    terrace_block = _block((plan.x, plan.y, grass), along, across, (-half, half), (-front, front), (0.0, terrace - grass))
    _sides(terrace_block, "stand-plinth", still)
    vertices, (face,) = terrace_block["z1"]
    still.add("beam-top", ([vertices[i] for i in face], [(0, 1, 2, 3)]))
    origin = (plan.x, plan.y, terrace)

    def at(a: float, b: float, c: float) -> mesh.Vertex:
        return (plan.x + a * along[0] + b * across[0], plan.y + a * along[1] + b * across[1], terrace + c)

    def copper(face: list[mesh.Vertex], centre: mesh.Vertex) -> None:
        still.add(*_toned("copper", _outward(face, centre)))

    def pyramid(base: list[mesh.Vertex], apex: mesh.Vertex, centre: mesh.Vertex) -> None:
        for k, a in enumerate(base):
            copper([a, base[(k + 1) % len(base)], apex], centre)

    # The block under its hip roof: a ridge along the middle, inset half the depth from each end.
    body = _block(origin, along, across, (-half, half), (-front, front), (0.0, CASINO_EAVES_M))
    _sides(body, "stucco", still, shade="stucco-shade")
    inset = half - front
    left, right = at(-inset, 0.0, CASINO_RIDGE_M), at(inset, 0.0, CASINO_RIDGE_M)
    eaves = [at(-half, -front, CASINO_EAVES_M), at(half, -front, CASINO_EAVES_M), at(half, front, CASINO_EAVES_M), at(-half, front, CASINO_EAVES_M)]
    hub = at(0.0, 0.0, CASINO_EAVES_M)
    back_left, back_right, front_right, front_left = eaves
    copper([back_left, back_right, right, left], hub)
    copper([front_right, front_left, left, right], hub)
    copper([back_right, front_right, right], hub)
    copper([front_left, back_left, left], hub)
    # A tower at each front corner under its pyramid.
    for end in (-1.0, 1.0):
        a0, a1 = sorted((end * half, end * (half - CASINO_TOWER_M)))
        b0, b1 = front - CASINO_TOWER_M, front
        tower = _block(origin, along, across, (a0, a1), (b0, b1), (CASINO_EAVES_M, CASINO_TOWER_EAVES_M))
        _sides(tower, "stucco", still, shade="stucco-shade")
        base = [at(a0, b0, CASINO_TOWER_EAVES_M), at(a1, b0, CASINO_TOWER_EAVES_M), at(a1, b1, CASINO_TOWER_EAVES_M), at(a0, b1, CASINO_TOWER_EAVES_M)]
        pyramid(base, at((a0 + a1) / 2.0, (b0 + b1) / 2.0, CASINO_TOP_M), at((a0 + a1) / 2.0, (b0 + b1) / 2.0, CASINO_TOWER_EAVES_M))
    # The dome over the door: a drum on the roof and a cone of copper facets on it.
    drum_b = front - CASINO_DRUM_BACK_M - CASINO_DRUM_RADIUS_M
    corners = [math.tau * (k + 0.5) / CASINO_DRUM_SIDES for k in range(CASINO_DRUM_SIDES)]
    low = [at(CASINO_DRUM_RADIUS_M * math.cos(t), drum_b + CASINO_DRUM_RADIUS_M * math.sin(t), CASINO_EAVES_M) for t in corners]
    high = [(x, y, terrace + CASINO_DRUM_TOP_M) for x, y, _ in low]
    centre = at(0.0, drum_b, CASINO_EAVES_M)
    for k in range(CASINO_DRUM_SIDES):
        j = (k + 1) % CASINO_DRUM_SIDES
        still.add(*_toned("stucco", _outward([low[k], low[j], high[j], high[k]], centre), "stucco-shade"))
    pyramid(high, at(0.0, drum_b, CASINO_DOME_TOP_M), at(0.0, drum_b, CASINO_DRUM_TOP_M))
    # The door and the windows, just proud of the front and wound toward the track.
    b = front + CASINO_PANEL_PROUD_M
    behind = at(0.0, 0.0, CASINO_EAVES_M / 2.0)
    panels = [(-CASINO_DOOR_WIDTH_M / 2.0, CASINO_DOOR_WIDTH_M / 2.0, 0.0, CASINO_DOOR_HEIGHT_M)]
    for middle in CASINO_WINDOWS_AT_M:
        for side in (-1.0, 1.0):
            for sill, head in CASINO_WINDOW_ROWS:
                m = side * middle
                panels.append((m - CASINO_WINDOW_WIDTH_M / 2.0, m + CASINO_WINDOW_WIDTH_M / 2.0, sill, head))
    for a0, a1, c0, c1 in panels:
        quad = [at(a0, b, c0), at(a1, b, c0), at(a1, b, c1), at(a0, b, c1)]
        still.add("carbon", (_outward(quad, behind), [(0, 1, 2, 3)]))
    return [], []


def _yacht(still: ByMaterial, stern: mesh.Vertex, fore: tuple[float, float], length: float) -> None:
    """A yacht with its stern's middle at `stern` on the water, its bow along the unit `fore`:
    a wall-white hull under a beam-top deck, a wall-white cabin with a band of carbon windows
    round it, and a wall-white flybridge under a beam-top one. The hull is not toned: the lap sees
    its shaded side, and white is what reads as a yacht from there."""
    port = (-fore[1], fore[0], 0.0)
    ahead = (fore[0], fore[1], 0.0)
    half = length * YACHT_BEAM / 2.0
    shoulder = length * (1.0 - YACHT_BOW)

    def point(f: float, p: float, z: float) -> mesh.Vertex:
        return (stern[0] + f * ahead[0] + p * port[0], stern[1] + f * ahead[1] + p * port[1], stern[2] + z)

    plan = [(0.0, -half), (shoulder, -half), (length, 0.0), (shoulder, half), (0.0, half)]
    middle = point(length / 2.0, 0.0, YACHT_FREEBOARD_M / 2.0)
    for (f0, p0), (f1, p1) in zip(plan, plan[1:] + plan[:1]):
        side = [point(f0, p0, 0.0), point(f1, p1, 0.0), point(f1, p1, YACHT_FREEBOARD_M), point(f0, p0, YACHT_FREEBOARD_M)]
        still.add("wall", (_outward(side, middle), [(0, 1, 2, 3)]))
    deck = [point(f, p, YACHT_FREEBOARD_M) for f, p in plan]
    still.add("beam-top", (_outward(deck, middle), [tuple(range(len(deck)))]))
    for (start, end, inset, tall), roof, z in (
        (YACHT_CABIN, "wall", YACHT_FREEBOARD_M),
        (YACHT_FLYBRIDGE, "beam-top", YACHT_FREEBOARD_M + YACHT_CABIN[3]),
    ):
        box = _block(point(0.0, 0.0, 0.0), ahead, port, (start * length, end * length), (inset - half, half - inset), (z, z + tall))
        _sides(box, "wall", still)
        vertices, (face,) = box["z1"]
        still.add(roof, ([vertices[i] for i in face], [(0, 1, 2, 3)]))
    # The cabin's windows: a band of dark glass round it, just proud of each side.
    start, end, inset, tall = YACHT_CABIN
    sill, head = YACHT_FREEBOARD_M + tall * YACHT_WINDOW_BAND[0], YACHT_FREEBOARD_M + tall * YACHT_WINDOW_BAND[1]
    a0, a1 = start * length - YACHT_WINDOW_PROUD_M, end * length + YACHT_WINDOW_PROUD_M
    b0, b1 = inset - half - YACHT_WINDOW_PROUD_M, half - inset + YACHT_WINDOW_PROUD_M
    band = _block(point(0.0, 0.0, 0.0), ahead, port, (a0, a1), (b0, b1), (sill, head))
    _sides(band, "carbon", still)


def _hull_plan(stern: tuple[float, float], fore: tuple[float, float], length: float) -> list[tuple[float, float]]:
    """A yacht's hull in plan, counter-clockwise from its stern's starboard corner."""
    half = length * YACHT_BEAM / 2.0
    shoulder = length * (1.0 - YACHT_BOW)
    port = (-fore[1], fore[0])
    return [
        (stern[0] + f * fore[0] + p * port[0], stern[1] + f * fore[1] + p * port[1])
        for f, p in ((0.0, -half), (shoulder, -half), (length, 0.0), (shoulder, half), (0.0, half))
    ]


def _inset(outline: list[tuple[float, float]], by: float) -> list[tuple[float, float]]:
    """The counter-clockwise outline moved `by` metres in along every edge, each corner where
    two moved edges meet. Every edge must be long enough not to vanish."""
    moved = []
    for (ax, ay), (bx, by_) in zip(outline, outline[1:] + outline[:1]):
        length = math.dist((ax, ay), (bx, by_))
        nx, ny = -(by_ - ay) / length * by, (bx - ax) / length * by
        moved.append(((ax + nx, ay + ny), (bx + nx, by_ + ny)))
    corners = []
    for k in range(len(moved)):
        (p0, p1), (q0, q1) = moved[k - 1], moved[k]
        d1 = (p1[0] - p0[0], p1[1] - p0[1])
        d2 = (q1[0] - q0[0], q1[1] - q0[1])
        cross = d1[0] * d2[1] - d1[1] * d2[0]
        t = ((q0[0] - p0[0]) * d2[1] - (q0[1] - p0[1]) * d2[0]) / cross
        corners.append((p0[0] + t * d1[0], p0[1] + t * d1[1]))
    return corners


def _distance_to_outline(outline: Sequence[tuple[float, float]], x: float, y: float) -> float:
    """From the point to the nearest edge of the closed outline."""
    best = math.inf
    for (ax, ay), (bx, by) in zip(outline, [*outline[1:], outline[0]]):
        dx, dy = bx - ax, by - ay
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.dist((x, y), (ax + t * dx, ay + t * dy)))
    return best


def _inside(outline: Sequence[tuple[float, float]], x: float, y: float) -> bool:
    crossings = 0
    for (ax, ay), (bx, by) in zip(outline, [*outline[1:], outline[0]]):
        if (ay > y) != (by > y) and x < ax + (y - ay) * (bx - ax) / (by - ay):
            crossings += 1
    return crossings % 2 == 1


def _convex_overlap(a: Sequence[tuple[float, float]], b: Sequence[tuple[float, float]]) -> bool:
    """Whether two convex outlines overlap, by the separating axis test."""
    for outline in (a, b):
        for (x0, y0), (x1, y1) in zip(outline, [*outline[1:], outline[0]]):
            axis = (y0 - y1, x1 - x0)
            pa = [axis[0] * x + axis[1] * y for x, y in a]
            pb = [axis[0] * x + axis[1] * y for x, y in b]
            if max(pa) <= min(pb) or max(pb) <= min(pa):
                return False
    return True


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
    "hangars": hangars,
    "airship": airship,
    "helicopter": helicopter,
    "jets": jets,
    "harbour": harbour,
    "casino": casino,
}

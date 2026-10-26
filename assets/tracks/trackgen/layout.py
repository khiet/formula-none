"""What a track author writes: the path plus everything hung on it, and the
builder that turns it into meshes and placements for Blender."""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass, field

import math

from . import mesh
from .path import Feature, Path, elevation
from .sponsors import SPONSORS

# Surface tags the sim understands; the fifth, out of world, is the absence
# of any surface below a point.
ASPHALT = "asphalt"
SAND = "sand"
GRASS = "grass"
WALL = "wall"

# The grass slab sits this far below the asphalt and the sand slopes down to
# it, so no two surfaces are coplanar and the surface under a point is never
# ambiguous.
GRASS_DROP_M = 0.05
GRASS_MARGIN_M = 40.0
WALL_THICKNESS_M = 0.5
WALL_HEIGHT_M = 1.0
PIPE_RADIUS_M = 1.5
PIPE_HEIGHT_M = 2.0

# Scenery, drawn and never collided with. It floats a hair over the surface
# it decorates so it draws on top without changing what the sim stands on.
SCENERY_LIFT_M = 0.01
KERB_WIDTH_M = 0.6
KERB_STRIPE_SAMPLES = 2
START_LINE_SQUARE_M = 1.0
START_LINE_ROWS = 2
# A hoarding is a board on the far side of the wall, facing the track, with
# a sponsor's name on it; the gantry spans the asphalt over the start line.
HOARDING_SETBACK_M = 0.6
HOARDING_SPACING_M = 12.0
HOARDING_LENGTH_M = 6.0
HOARDING_BOTTOM_M = 0.4
HOARDING_TOP_M = 2.2
HOARDING_TEXT_HEIGHT_M = 0.8
# Lettering stands clear of its board by more than its own depth, so its
# back never shares the board's plane and shows through from behind.
HOARDING_TEXT_PROUD_M = 0.06
# The stands and the gantry are the asset catalog's Grandstand and Start
# gantry cards in three dimensions, at the cards' 14 px to the metre.
# A stand is a plinth and four white tiers, each stepped back and in from
# the one below, a crowd of single pixels on every tread, and a kerb-red
# canopy on two carbon posts at the back.
GRANDSTAND_SETBACK_M = 4.0
"""From the back of the wall to the front of the stand."""
GRANDSTAND_WIDTH_M = 12.0
GRANDSTAND_PLINTH_M = 0.43
GRANDSTAND_TIERS = 4
GRANDSTAND_TIER_RISE_M = 1.29
GRANDSTAND_TIER_TREAD_M = 1.2
GRANDSTAND_TIER_INSET_M = 0.14
GRANDSTAND_POST_M = 0.43
GRANDSTAND_POST_IN_M = 0.29
GRANDSTAND_CANOPY_BOTTOM_M = 7.57
GRANDSTAND_CANOPY_DEPTH_M = 0.43
GRANDSTAND_CANOPY_OVERHANG_M = 0.7
# The crowd: a row on each tread, its colours cycling through the key
# colours so no two neighbours match, a little further in on each tier.
CROWD_PER_TIER = 22
CROWD_SPACING_M = 0.5
CROWD_FIRST_IN_M = 0.57
CROWD_STEP_IN_M = 0.14
CROWD_WIDTH_M = 0.29
CROWD_HEIGHT_M = 0.36
CROWD_FROM_RISER_M = 0.4
# The eight liveries' key colours in slot order, sRGB; they mirror `key` in
# `apps/web/src/livery.ts` and the track tests hold them to it.
CROWD_KEYS = (0xD00A12, 0x1F3FB0, 0xC8CCD0, 0xFF7A00, 0x00665A, 0x37BEDD, 0xF282B4, 0x2A2A2E)
# The gantry: two carbon pillars just off the asphalt holding a white beam
# with two rows of the track's chequer on both faces, lit on top and
# shaded underneath.
GANTRY_PAST_LINE_M = 1.0
GANTRY_BEYOND_ASPHALT_M = 1.0
"""From the edge of the asphalt to the outer face of a pillar."""
GANTRY_PILLAR_M = 0.85
GANTRY_TOP_M = 7.0
GANTRY_BEAM_HEIGHT_M = 1.71
GANTRY_BEAM_SHADE_M = 0.29
GANTRY_CHEQUER_M = 0.5
GANTRY_CHEQUER_ROWS = 2
# A boost pad is drawn on the rectangle the sim boosts on, so these mirror
# `BOOST_PAD_HALF_*` in `crates/sim/src/tuning.rs`; the track tests hold
# them to it. The rest is the asset catalog's Boost pad card: a carbon
# plate lit along its front edge and shaded along its back, under three
# chevrons in the mini-turbo orange, each shaded along its right arm.
BOOST_PAD_HALF_LENGTH_M = 2.0
BOOST_PAD_HALF_WIDTH_M = 3.0
# The card's measures, at its 18 px to the metre.
BOOST_PAD_BAND_M = 0.22
BOOST_PAD_PLATE_MATERIALS = ("carbon", "carbon-lit", "carbon-shade")
CHEVRON_HALF_SPAN_M = 1.89
CHEVRON_RISE_M = 1.0
CHEVRON_THICKNESS_M = 0.56
CHEVRON_SHADE_M = 0.22
CHEVRON_SPACING_M = 1.0
CHEVRON_FIRST_TIP_M = 1.67
CHEVRON_COUNT = 3

# The eight-slot spawn grid: two staggered columns of four, pole in front on
# the left, the right column set back. Slot order is grid order. Placements
# snap to the nearest centreline sample, so these distances hold to within
# half a `SAMPLE_STEP_M`.
GRID_SLOTS = 8
GRID_ROW_SPACING_M = 5.0
GRID_COLUMN_OFFSET_M = 2.0
GRID_STAGGER_M = 2.0
GRID_FRONT_ROW_BEHIND_LINE_M = 4.0

@dataclass(frozen=True)
class Pipe:
    """An upright pipe obstacle at arc length `s`, `offset` metres left of the centreline."""

    s: float
    offset: float


@dataclass(frozen=True)
class ItemRow:
    """Four item boxes abreast at arc length `s`."""

    s: float
    offsets: tuple[float, float, float, float] = (-4.5, -1.5, 1.5, 4.5)


@dataclass(frozen=True)
class BoostPad:
    s: float
    offset: float = 0.0


@dataclass(frozen=True)
class Hoardings:
    """Sponsor boards along the outer edge of a range of segments."""

    segments: tuple[str, ...]
    side: int


@dataclass(frozen=True)
class Grandstands:
    """Stands behind the hoardings along a range of segments, one every stand's width."""

    segments: tuple[str, ...]
    side: int


@dataclass(frozen=True)
class Decor:
    """The scenery a layout asks for beyond what every track gets (kerbs and the start line)."""

    hoardings: list[Hoardings] = field(default_factory=list)
    grandstands: list[Grandstands] = field(default_factory=list)
    gantry: bool = False


@dataclass
class Layout:
    name: str
    path: Path
    asphalt_half_width: float
    runoff_width: float
    checkpoint_count: int
    pipes: list[Pipe]
    item_rows: list[ItemRow]
    boost_pads: list[BoostPad]
    elevation: list[Feature] = field(default_factory=list)
    decor: Decor = field(default_factory=Decor)

    @property
    def runoff_half_width(self) -> float:
        return self.asphalt_half_width + self.runoff_width


@dataclass(frozen=True)
class Placement:
    """Something placed on the track: position on the surface and the direction it faces."""

    x: float
    y: float
    z: float
    heading: float
    half_width: float = 0.0


@dataclass(frozen=True)
class SurfaceMesh:
    name: str
    surface: str
    material: str
    mesh: mesh.Mesh
    uvs: list[tuple[float, float]] | None = None
    """One per vertex when the surface takes decals: tyre marks need the asphalt's."""


@dataclass(frozen=True)
class SceneryMesh:
    """A mesh drawn and never collided with: not in the sidecar."""

    name: str
    material: str
    mesh: mesh.Mesh


@dataclass(frozen=True)
class TextProp:
    """Lettering Blender sets in the scenery: `size` metres tall, in `material`, read from `at.heading`."""

    name: str
    text: str
    material: str
    size: float
    at: Placement


@dataclass
class TrackGeometry:
    """Everything the Blender build script places in the scene."""

    centreline: list[tuple[float, float, float]]
    surfaces: list[SurfaceMesh]
    checkpoints: list[Placement]
    item_boxes: list[Placement]
    boost_pads: list[Placement]
    spawn_grid: list[Placement]
    scenery: list[SceneryMesh] = field(default_factory=list)
    props: list[TextProp] = field(default_factory=list)



def build(layout: Layout) -> TrackGeometry:
    path = layout.path
    height = [elevation(layout.elevation, sample.s) for sample in path.samples]

    def rail(offset: float, drop: float = 0.0) -> list[mesh.Vertex]:
        return [
            (*sample.offset(offset), h - drop)
            for sample, h in zip(path.samples, height)
        ]

    half = layout.asphalt_half_width
    runoff = layout.runoff_half_width
    # Across the asphalt from its left edge, and along it in metres of arc
    # length, so a tyre mark laid in these coordinates lands where the kart was.
    asphalt_uvs = [(0.0, sample.s) for sample in path.samples] + [
        (1.0, sample.s) for sample in path.samples
    ]
    surfaces = [
        SurfaceMesh(
            "Asphalt",
            ASPHALT,
            ASPHALT,
            mesh.ribbon(rail(half), rail(-half), closed=True),
            uvs=asphalt_uvs,
        ),
        SurfaceMesh(
            "SandLeft", SAND, SAND, mesh.ribbon(rail(runoff, GRASS_DROP_M), rail(half), closed=True)
        ),
        SurfaceMesh(
            "SandRight", SAND, SAND, mesh.ribbon(rail(-half), rail(-runoff, GRASS_DROP_M), closed=True)
        ),
    ]
    min_x, min_y, max_x, max_y = path.bounds()
    surfaces.append(
        SurfaceMesh(
            "Grass",
            GRASS,
            GRASS,
            mesh.slab(
                min_x - GRASS_MARGIN_M,
                min_y - GRASS_MARGIN_M,
                max_x + GRASS_MARGIN_M,
                max_y + GRASS_MARGIN_M,
                -GRASS_DROP_M,
            ),
        )
    )

    # Track limits: a wall along each edge of the run-off the whole way
    # round, standing on the sand's outer edge so it follows the hill and
    # the ramp with the track. A kart can go off the asphalt but never off
    # the course, so no corner can be cut (ADR 0022).
    for name, side in (("WallLeft", 1), ("WallRight", -1)):
        inner = rail(side * runoff, GRASS_DROP_M)
        outer = rail(side * (runoff + WALL_THICKNESS_M), GRASS_DROP_M)
        surfaces.append(
            SurfaceMesh(name, WALL, WALL, mesh.box_rail(inner, outer, WALL_HEIGHT_M, closed=True))
        )

    for number, pipe in enumerate(layout.pipes):
        sample = path.at(pipe.s)
        base = (*sample.offset(pipe.offset), elevation(layout.elevation, sample.s))
        surfaces.append(
            SurfaceMesh(
                f"Pipe.{number:02d}", WALL, "pipe", mesh.cylinder(base, PIPE_RADIUS_M, PIPE_HEIGHT_M)
            )
        )

    def place(s: float, offset: float = 0.0, half_width: float = 0.0) -> Placement:
        sample = path.at(s)
        x, y = sample.offset(offset)
        return Placement(x, y, elevation(layout.elevation, sample.s), sample.heading, half_width)

    # Evenly spaced from the start line, each spanning the run-off, so a kart
    # between the walls crosses every one in order.
    checkpoints = [
        place(k * path.length / layout.checkpoint_count, half_width=runoff)
        for k in range(layout.checkpoint_count)
    ]
    item_boxes = [place(row.s, offset) for row in layout.item_rows for offset in row.offsets]
    boost_pads = [place(pad.s, pad.offset) for pad in layout.boost_pads]
    spawn_grid = [
        place(
            -GRID_FRONT_ROW_BEHIND_LINE_M
            - GRID_ROW_SPACING_M * (slot // 2)
            - GRID_STAGGER_M * (slot % 2),
            GRID_COLUMN_OFFSET_M if slot % 2 == 0 else -GRID_COLUMN_OFFSET_M,
        )
        for slot in range(GRID_SLOTS)
    ]

    centreline = [(sample.x, sample.y, h) for sample, h in zip(path.samples, height)]
    scenery, props = decorate(layout, height)
    return TrackGeometry(
        centreline, surfaces, checkpoints, item_boxes, boost_pads, spawn_grid, scenery, props
    )


def decorate(layout: Layout, height: list[float]) -> tuple[list[SceneryMesh], list[TextProp]]:
    """The scenery: kerbs on every corner, the start line, and what the layout's decor asks for."""
    scenery = kerbs(layout, height) + start_line(layout) + boost_pads(layout)
    boards, lettering = hoardings(layout)
    scenery += boards + grandstands(layout)
    if layout.decor.gantry:
        scenery += gantry(layout)
    return scenery, lettering


def kerbs(layout: Layout, height: list[float]) -> list[SceneryMesh]:
    """Red-and-white stripes along both edges of the asphalt through every corner."""
    path = layout.path
    half = layout.asphalt_half_width

    def lifted(index: int, offset: float) -> mesh.Vertex:
        sample = path.samples[index]
        return (*sample.offset(offset), height[index] + SCENERY_LIFT_M)

    result: list[SceneryMesh] = []
    for segment in path.segments:
        if segment.turn == 0:
            continue
        indices = path.indices_between(segment.s_start, segment.s_end)
        for side, label in ((1, "L"), (-1, "R")):
            outer = [lifted(i, side * half) for i in indices]
            inner = [lifted(i, side * (half - KERB_WIDTH_M)) for i in indices]
            left, right = (outer, inner) if side == 1 else (inner, outer)
            red, white = mesh.striped(mesh.ribbon(left, right, closed=False), KERB_STRIPE_SAMPLES)
            result.append(SceneryMesh(f"Kerb.{segment.name}.{label}.red", "kerb-red", red))
            result.append(SceneryMesh(f"Kerb.{segment.name}.{label}.white", "kerb-white", white))
    return result


def start_line(layout: Layout) -> list[SceneryMesh]:
    """Checkers across the asphalt from the start line forward."""
    line = layout.path.at(0.0)
    half = layout.asphalt_half_width
    forward = (math.cos(line.heading), math.sin(line.heading))
    z = elevation(layout.elevation, 0.0) + SCENERY_LIFT_M

    def corner(d: float, along: float) -> mesh.Vertex:
        x, y = line.offset(d)
        return (x + forward[0] * along, y + forward[1] * along, z)

    columns = int(2 * half / START_LINE_SQUARE_M)
    dark: list[mesh.Mesh] = []
    light: list[mesh.Mesh] = []
    for row in range(START_LINE_ROWS):
        for column in range(columns):
            d0 = half - column * START_LINE_SQUARE_M
            d1 = d0 - START_LINE_SQUARE_M
            along0 = row * START_LINE_SQUARE_M
            along1 = along0 + START_LINE_SQUARE_M
            square = mesh.quad(
                corner(d0, along0), corner(d1, along0), corner(d1, along1), corner(d0, along1)
            )
            (dark if (row + column) % 2 == 0 else light).append(square)
    return [
        SceneryMesh("StartLine.dark", "checker-dark", mesh.merge(dark)),
        SceneryMesh("StartLine.light", "checker-light", mesh.merge(light)),
    ]


def boost_pads(layout: Layout) -> list[SceneryMesh]:
    """A plate over each pad's rectangle with its chevrons on top, pointing along the track."""
    result: list[SceneryMesh] = []
    for number, pad in enumerate(layout.boost_pads):
        sample = layout.path.at(pad.s)
        x, y = sample.offset(pad.offset)
        cos, sin = math.cos(sample.heading), math.sin(sample.heading)

        def at(ahead: float, right: float, lift: float) -> mesh.Vertex:
            return (
                x + ahead * cos + right * sin,
                y + ahead * sin - right * cos,
                elevation(layout.elevation, sample.s + ahead) + lift,
            )

        def flat(corners: list[tuple[float, float]], lift: float) -> mesh.Mesh:
            # Corners run counter-clockwise in (ahead, right), a mirror of the
            # ground's frame, so they are reversed to face up.
            return [at(a, r, lift) for a, r in reversed(corners)], [tuple(range(len(corners)))]

        def band(back: float, front: float) -> mesh.Mesh:
            side = BOOST_PAD_HALF_WIDTH_M
            return flat([(back, -side), (front, -side), (front, side), (back, side)], SCENERY_LIFT_M)

        half_length = BOOST_PAD_HALF_LENGTH_M
        inner_back, inner_front = -half_length + BOOST_PAD_BAND_M, half_length - BOOST_PAD_BAND_M
        plate = {
            "carbon-shade": band(-half_length, inner_back),
            "carbon": band(inner_back, inner_front),
            "carbon-lit": band(inner_front, half_length),
        }

        # The chevrons sit over the plate so they never share its plane.
        lift = 2.0 * SCENERY_LIFT_M
        span, rise = CHEVRON_HALF_SPAN_M, CHEVRON_RISE_M
        lit: list[mesh.Mesh] = []
        shade: list[mesh.Mesh] = []
        for k in range(CHEVRON_COUNT):
            tip = CHEVRON_FIRST_TIP_M - k * CHEVRON_SPACING_M

            def edge(depth: float) -> tuple[tuple[float, float], tuple[float, float], tuple[float, float]]:
                """The left end, the tip and the right end of the chevron's leading edge, pushed `depth` back."""
                return (tip - rise - depth, -span), (tip - depth, 0.0), (tip - rise - depth, span)

            front_left, front_tip, front_right = edge(0.0)
            _, seam_tip, seam_right = edge(CHEVRON_THICKNESS_M - CHEVRON_SHADE_M)
            back_left, back_tip, back_right = edge(CHEVRON_THICKNESS_M)
            lit.append(flat([back_left, front_left, front_tip, back_tip], lift))
            lit.append(flat([seam_tip, front_tip, front_right, seam_right], lift))
            shade.append(flat([back_tip, seam_tip, seam_right, back_right], lift))

        pieces = {**plate, "chevron": mesh.merge(lit), "chevron-shade": mesh.merge(shade)}
        for material, piece in pieces.items():
            result.append(SceneryMesh(f"BoostPad.{number:02d}.{material}", material, piece))
    return result


def along_outside(
    layout: Layout, segments: tuple[str, ...], side: int, setback: float, spacing: float
) -> Iterator[Placement]:
    """Spots every `spacing` metres along `segments`, `setback` metres beyond the wall on `side`, facing the track.

    The first spot is half a spacing in and the last at least half a spacing
    short, so nothing overhangs the range; the range wraps past the start
    line like a wall's.
    """
    path = layout.path
    s_start = path.segment(segments[0]).s_start
    s_end = path.segment(segments[-1]).s_end
    if s_end < s_start:
        s_end += path.length
    offset = side * (layout.runoff_half_width + WALL_THICKNESS_M + setback)
    s = s_start + spacing / 2.0
    while s + spacing / 2.0 <= s_end:
        sample = path.at(s)
        x, y = sample.offset(offset)
        nx, ny = sample.left_normal
        # Towards the track: against the left normal on the left, with it on the right.
        facing = math.atan2(-side * ny, -side * nx)
        yield Placement(x, y, elevation(layout.elevation, sample.s), facing)
        s += spacing


def hoardings(layout: Layout) -> tuple[list[SceneryMesh], list[TextProp]]:
    """White boards facing the track with a sponsor's name on each, cycling through the sponsors."""
    boards: list[SceneryMesh] = []
    lettering: list[TextProp] = []
    for run in layout.decor.hoardings:
        for spot in along_outside(layout, run.segments, run.side, HOARDING_SETBACK_M, HOARDING_SPACING_M):
            number = len(boards)
            # The board runs across the facing, so along the track.
            along = (-math.sin(spot.heading), math.cos(spot.heading))
            ends = (-HOARDING_LENGTH_M / 2.0, HOARDING_LENGTH_M / 2.0)
            (ax, ay), (bx, by) = [(spot.x + along[0] * e, spot.y + along[1] * e) for e in ends]
            board = mesh.quad(
                (ax, ay, spot.z + HOARDING_BOTTOM_M),
                (bx, by, spot.z + HOARDING_BOTTOM_M),
                (bx, by, spot.z + HOARDING_TOP_M),
                (ax, ay, spot.z + HOARDING_TOP_M),
            )
            boards.append(SceneryMesh(f"Hoarding.{number:02d}", "board", board))
            name, ink = SPONSORS[number % len(SPONSORS)]
            proud = HOARDING_TEXT_PROUD_M
            lettering.append(
                TextProp(
                    f"Sponsor.{number:02d}",
                    name,
                    ink,
                    HOARDING_TEXT_HEIGHT_M,
                    Placement(
                        spot.x + math.cos(spot.heading) * proud,
                        spot.y + math.sin(spot.heading) * proud,
                        spot.z + (HOARDING_BOTTOM_M + HOARDING_TOP_M) / 2.0,
                        spot.heading,
                    ),
                )
            )
    return boards, lettering


def grandstands(layout: Layout) -> list[SceneryMesh]:
    """Stands shoulder to shoulder behind the hoardings, on the grass, facing the track.

    Every stand's boxes are merged by material, so the stands cost a draw
    call per colour however many there are.
    """
    by_material: dict[str, list[mesh.Mesh]] = {}

    def add(material: str, piece: mesh.Mesh) -> None:
        by_material.setdefault(material, []).append(piece)

    for run in layout.decor.grandstands:
        for front in along_outside(layout, run.segments, run.side, GRANDSTAND_SETBACK_M, GRANDSTAND_WIDTH_M):
            cos, sin = math.cos(front.heading), math.sin(front.heading)
            ground = front.z - GRASS_DROP_M

            def box(left: float, right: float, near: float, far: float, bottom: float, top: float) -> dict[str, mesh.Mesh]:
                """A box `left`..`right` along the stand, `near`..`far` back from its front, `bottom`..`top` up.

                Its faces are keyed x across, y back from the track and z up.
                """

                def corner(i: int, j: int, k: int) -> mesh.Vertex:
                    along = (left, right)[i]
                    back = (near, far)[j]
                    return (
                        front.x - sin * along - cos * back,
                        front.y + cos * along - sin * back,
                        ground + (bottom, top)[k],
                    )

                return mesh.cuboid(corner)

            half = GRANDSTAND_WIDTH_M / 2.0
            depth = GRANDSTAND_TIERS * GRANDSTAND_TIER_TREAD_M
            plinth = box(-half, half, 0.0, depth, 0.0, GRANDSTAND_PLINTH_M)
            # The plinth's top and every tier's bottom are hidden by the tier on them.
            for side in ("x0", "x1", "y0", "y1"):
                add("stand-plinth", plinth[side])
            for tier in range(GRANDSTAND_TIERS):
                inset = half - tier * GRANDSTAND_TIER_INSET_M
                bottom = GRANDSTAND_PLINTH_M + tier * GRANDSTAND_TIER_RISE_M
                top = bottom + GRANDSTAND_TIER_RISE_M
                near = tier * GRANDSTAND_TIER_TREAD_M
                faces = box(-inset, inset, near, depth, bottom, top)
                add("wall", faces["z1"])
                for side in ("x0", "x1", "y0", "y1"):
                    add("stand-riser", faces[side])
                for seat in range(CROWD_PER_TIER):
                    left = -half + CROWD_FIRST_IN_M + tier * CROWD_STEP_IN_M + seat * CROWD_SPACING_M
                    back = near + CROWD_FROM_RISER_M
                    person = box(left, left + CROWD_WIDTH_M, back, back + CROWD_WIDTH_M, top, top + CROWD_HEIGHT_M)
                    slot = (seat * 5 + tier * 3) % len(CROWD_KEYS)
                    for side, face in person.items():
                        if side != "z0":
                            add(f"crowd-{slot}", face)

            post_top = GRANDSTAND_CANOPY_BOTTOM_M
            for side in (-1, 1):
                outer = side * (half - GRANDSTAND_POST_IN_M)
                inner = outer - side * GRANDSTAND_POST_M
                post = box(min(inner, outer), max(inner, outer), depth - GRANDSTAND_POST_M, depth, GRANDSTAND_PLINTH_M, post_top)
                for face_side in ("x0", "x1", "y0", "y1"):
                    add("carbon", post[face_side])

            reach = half + GRANDSTAND_CANOPY_OVERHANG_M
            canopy = box(
                -reach, reach, -GRANDSTAND_CANOPY_OVERHANG_M, depth, post_top, post_top + GRANDSTAND_CANOPY_DEPTH_M
            )
            add("kerb-red", canopy["z1"])
            for side in ("x0", "x1", "y0", "y1", "z0"):
                add("canopy-shade", canopy[side])

    return [
        SceneryMesh(f"Grandstand.{material}", material, mesh.merge(pieces))
        for material, pieces in by_material.items()
    ]


def gantry(layout: Layout) -> list[SceneryMesh]:
    """The gantry spanning the asphalt just past the start line, its chequer facing both ways along the track."""
    sample = layout.path.at(GANTRY_PAST_LINE_M)
    ground = elevation(layout.elevation, sample.s)
    cos, sin = math.cos(sample.heading), math.sin(sample.heading)

    def box(right: float, left: float, behind: float, ahead: float, bottom: float, top: float) -> dict[str, mesh.Mesh]:
        """A box `right`..`left` metres left of the centreline, `behind`..`ahead` along the track, `bottom`..`top` up.

        Its faces are keyed x across, y along the track and z up.
        """

        def corner(i: int, j: int, k: int) -> mesh.Vertex:
            across = (right, left)[i]
            along = (behind, ahead)[j]
            return (
                sample.x + cos * along - sin * across,
                sample.y + sin * along + cos * across,
                ground + (bottom, top)[k],
            )

        return mesh.cuboid(corner)

    by_material: dict[str, list[mesh.Mesh]] = {}

    def add(material: str, piece: mesh.Mesh) -> None:
        by_material.setdefault(material, []).append(piece)

    span = layout.asphalt_half_width + GANTRY_BEYOND_ASPHALT_M
    thick = GANTRY_PILLAR_M / 2.0
    beam_bottom = GANTRY_TOP_M - GANTRY_BEAM_HEIGHT_M
    # The pillars stand on the sand's lowest edge so they never float on its slope.
    for side in (-1, 1):
        outer, inner = side * span, side * (span - GANTRY_PILLAR_M)
        pillar = box(min(inner, outer), max(inner, outer), -thick, thick, -GRASS_DROP_M, beam_bottom)
        # Lit on the faces up and down the track, the ones the grid sees.
        add("carbon-lit", pillar["y0"])
        add("carbon-lit", pillar["y1"])
        add("carbon", pillar["x0"])
        add("carbon", pillar["x1"])

    beam = box(-span, span, -thick, thick, beam_bottom, GANTRY_TOP_M)
    add("beam-top", beam["z1"])
    add("beam-shade", beam["z0"])
    for side in ("x0", "x1", "y0", "y1"):
        add("wall", beam[side])

    # Tiles a hair proud of both faces, so they never share the beam's
    # planes; only a tile's outer face is drawn.
    lift = SCENERY_LIFT_M
    shade_top = beam_bottom + GANTRY_BEAM_SHADE_M
    columns = round(2.0 * span / GANTRY_CHEQUER_M)
    for behind, ahead, outer in ((-thick - lift, -thick, "y0"), (thick, thick + lift, "y1")):

        def flat(right: float, left: float, bottom: float, top: float) -> mesh.Mesh:
            return box(right, left, behind, ahead, bottom, top)[outer]

        add("beam-shade", flat(-span, span, beam_bottom, shade_top))
        for row in range(GANTRY_CHEQUER_ROWS):
            bottom = shade_top + row * GANTRY_CHEQUER_M
            for column in range(columns):
                right = -span + column * GANTRY_CHEQUER_M
                material = "checker-dark" if (row + column) % 2 == 0 else "checker-light"
                add(material, flat(right, right + GANTRY_CHEQUER_M, bottom, bottom + GANTRY_CHEQUER_M))

    return [
        SceneryMesh(f"Gantry.{material}", material, mesh.merge(pieces))
        for material, pieces in by_material.items()
    ]

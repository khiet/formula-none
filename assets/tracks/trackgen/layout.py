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
GRANDSTAND_SETBACK_M = 4.0
GRANDSTAND_WIDTH_M = 12.0
GANTRY_PAST_LINE_M = 1.0

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
class Wall:
    """A barrier along the outer edge of the run-off over a range of segments."""

    segments: tuple[str, ...]
    side: int
    """+1 left of the direction of travel, -1 right."""


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
    walls: list[Wall]
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


@dataclass(frozen=True)
class ModelProp:
    """A Kenney kit model Blender adds to the scenery: `size` metres wide, its front toward `at.heading`."""

    name: str
    model: str
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
    props: list[TextProp | ModelProp] = field(default_factory=list)


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

    for number, wall in enumerate(layout.walls):
        s_start = path.segment(wall.segments[0]).s_start
        s_end = path.segment(wall.segments[-1]).s_end
        if s_end < s_start:
            s_end += path.length
        indices = path.indices_between(s_start, s_end)
        near = wall.side * runoff
        far = wall.side * (runoff + WALL_THICKNESS_M)
        inner = [(*path.samples[i].offset(near), -GRASS_DROP_M) for i in indices]
        outer = [(*path.samples[i].offset(far), -GRASS_DROP_M) for i in indices]
        surfaces.append(
            SurfaceMesh(
                f"Wall.{number:02d}", WALL, WALL, mesh.box_rail(inner, outer, WALL_HEIGHT_M)
            )
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


def decorate(layout: Layout, height: list[float]) -> tuple[list[SceneryMesh], list[TextProp | ModelProp]]:
    """The scenery: kerbs on every corner, the start line, and what the layout's decor asks for."""
    scenery = kerbs(layout, height) + start_line(layout)
    props: list[TextProp | ModelProp] = []
    boards, lettering = hoardings(layout)
    scenery += boards
    props += lettering
    props += grandstands(layout)
    if layout.decor.gantry:
        props.append(gantry(layout))
    return scenery, props


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


def grandstands(layout: Layout) -> list[ModelProp]:
    """Kenney stands shoulder to shoulder behind the hoardings, on the grass, facing the track."""
    stands: list[ModelProp] = []
    setback = GRANDSTAND_SETBACK_M + GRANDSTAND_WIDTH_M / 2.0
    for run in layout.decor.grandstands:
        for spot in along_outside(layout, run.segments, run.side, setback, GRANDSTAND_WIDTH_M):
            on_grass = Placement(spot.x, spot.y, spot.z - GRASS_DROP_M, spot.heading)
            stands.append(
                ModelProp(f"Grandstand.{len(stands):02d}", "grandStand", GRANDSTAND_WIDTH_M, on_grass)
            )
    return stands


def gantry(layout: Layout) -> ModelProp:
    """The Kenney gantry spanning the asphalt just past the start line, its front down the track."""
    sample = layout.path.at(GANTRY_PAST_LINE_M)
    at = Placement(sample.x, sample.y, elevation(layout.elevation, sample.s), sample.heading)
    return ModelProp("Gantry", "overhead", 2.0 * (layout.asphalt_half_width + 1.0), at)

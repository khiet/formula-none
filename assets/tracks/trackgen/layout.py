"""What a track author writes: the path plus everything hung on it, and the
builder that turns it into meshes and placements for Blender."""

from __future__ import annotations

from dataclasses import dataclass, field

from . import mesh
from .path import Feature, Path, elevation

# Surface tags the sim understands; the fifth, out of world, is the absence
# of any surface below a point.
ASPHALT = "asphalt"
SAND = "sand"
GRASS = "grass"
WALL = "wall"

# The grass slab sits this far below the asphalt so the sand run-off slopes
# down to it instead of lying in the same plane, which would make "which
# surface is under this point" ambiguous.
GRASS_DROP_M = 0.05
GRASS_MARGIN_M = 40.0
WALL_THICKNESS_M = 0.5
WALL_HEIGHT_M = 1.0
PIPE_RADIUS_M = 1.5
PIPE_HEIGHT_M = 2.0

# The eight-slot spawn grid: two staggered columns of four, pole in front on
# the left, each row five metres behind the last, the right column half a
# row back. Slot order is grid order. Placements snap to the nearest
# centreline sample, so distances along the track are whole metres.
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


@dataclass
class TrackGeometry:
    """Everything the Blender build script places in the scene."""

    centreline: list[tuple[float, float, float]]
    surfaces: list[SurfaceMesh]
    checkpoints: list[Placement]
    item_boxes: list[Placement]
    boost_pads: list[Placement]
    spawn_grid: list[Placement]


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
    surfaces = [
        SurfaceMesh("Asphalt", ASPHALT, ASPHALT, mesh.ribbon(rail(half), rail(-half), closed=True)),
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
    return TrackGeometry(centreline, surfaces, checkpoints, item_boxes, boost_pads, spawn_grid)

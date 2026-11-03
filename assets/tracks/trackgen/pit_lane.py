"""A pit lane (ADR 0081), built from a layout's `PitLane` on any course.

The lane is the run-off turned to asphalt at its fork: its two edges start on
the run-off's two edges and sweep out to their places behind the wall, so the
fork needs no gate to read as one. The stretch of the wall that side of the
course between the two lines is left standing as the pit wall; the wall opens
at the fork and at the merge, and the lane's outer wall runs on from the edges
of those gaps, so no way off the course opens with them.
"""

from __future__ import annotations

from dataclasses import dataclass

from . import mesh
from .layout import (
    ASPHALT,
    GRASS_DROP_M,
    SCENERY_LIFT_M,
    WALL,
    WALL_HEIGHT_M,
    WALL_THICKNESS_M,
    Layout,
    Placement,
    SceneryMesh,
    SurfaceMesh,
)
from .path import elevation

LANE_WIDTH_M = 7.0
"""A fast lane and an apron, each half of this."""
LANE_LIFT_M = 0.01
"""Over the grass and, at its outer edge, over the sand; nothing over the asphalt it meets."""
LINE_DEPTH_M = 0.6
"""How far along the lane the painted entry and exit lines reach."""
PIT_BOXES = 8


@dataclass(frozen=True)
class PitLaneData:
    """What the sidecar says of a lane; the sim reads this and never the meshes."""

    path: list[tuple[float, float, float]]
    half_width: float
    entry: Placement
    exit: Placement
    boxes: list[Placement]


def _smoothstep(t: float) -> float:
    t = min(1.0, max(0.0, t))
    return t * t * (3.0 - 2.0 * t)


class _Lane:
    """The lane's edges as distances from the centreline, on its side of it."""

    def __init__(self, layout: Layout) -> None:
        self.layout = layout
        self.lane = layout.pit_lane
        self.path = layout.path
        self.asphalt = layout.asphalt_half_width
        self.runoff = layout.runoff_half_width
        self.wall_back = self.runoff + WALL_THICKNESS_M
        self.indices = self.path.indices_between(self.lane.s_fork, self.lane.s_merge)

    def along(self, s: float) -> float:
        """Metres on from the fork, whichever side of the start line `s` is read from."""
        return (s - self.lane.s_fork) % self.path.length

    def swept(self, s: float) -> float:
        """How far the lane has swept out at `s`: 0 at the fork and the merge, 1 line to line."""
        lane = self.lane
        along = self.along(s)
        entering = along / (lane.s_entry - lane.s_fork)
        leaving = (lane.s_merge - lane.s_fork - along) / (lane.s_merge - lane.s_exit)
        return _smoothstep(min(entering, leaving))

    def inner(self, s: float) -> float:
        return self.asphalt + (self.wall_back - self.asphalt) * self.swept(s)

    def outer(self, s: float) -> float:
        return self.runoff + (self.wall_back + LANE_WIDTH_M - self.runoff) * self.swept(s)

    def centre(self, s: float) -> float:
        return (self.inner(s) + self.outer(s)) / 2.0

    def height(self, s: float, offset: float) -> float:
        """The lane's surface at `offset` from the centreline: flush with the asphalt's edge, and
        a hair over the sand's fall from there to the grass, level beyond."""
        out = min(1.0, (offset - self.asphalt) / (self.runoff - self.asphalt))
        return elevation(self.layout.elevation, s) - (GRASS_DROP_M - LANE_LIFT_M) * out

    def rail(self, offset_at, drop: float | None = None) -> list[mesh.Vertex]:
        rail = []
        for index in self.indices:
            sample = self.path.samples[index]
            offset = offset_at(sample.s)
            z = (
                self.height(sample.s, offset)
                if drop is None
                else elevation(self.layout.elevation, sample.s) - drop
            )
            rail.append((*sample.offset(self.lane.side * offset), z))
        return rail

    def place(self, s: float, offset: float, half_width: float = 0.0) -> Placement:
        sample = self.path.at(s)
        return Placement(
            *sample.offset(self.lane.side * offset), self.height(s, offset), sample.heading, half_width
        )


def _wall_rail(layout: Layout, indices: list[int], side: int, offset: float) -> list[mesh.Vertex]:
    return [
        (
            *layout.path.samples[i].offset(side * offset),
            elevation(layout.elevation, layout.path.samples[i].s) - GRASS_DROP_M,
        )
        for i in indices
    ]


def _wall(layout: Layout, name: str, s_start: float, s_end: float, side: int) -> SurfaceMesh:
    """The run-off's wall on `side` from `s_start` to `s_end`, its ends open."""
    indices = layout.path.indices_between(s_start, s_end)
    inner = _wall_rail(layout, indices, side, layout.runoff_half_width)
    outer = _wall_rail(layout, indices, side, layout.runoff_half_width + WALL_THICKNESS_M)
    return SurfaceMesh(name, WALL, WALL, mesh.box_rail(inner, outer, WALL_HEIGHT_M))


def surfaces(layout: Layout) -> list[SurfaceMesh]:
    """The lane's asphalt and outer wall, and the two walls that replace the closed one on its side."""
    lane = layout.pit_lane
    edges = _Lane(layout)
    near = edges.rail(edges.inner)
    far = edges.rail(edges.outer)
    left, right = (far, near) if lane.side > 0 else (near, far)
    uvs = [(0.0, edges.path.samples[i].s) for i in edges.indices] + [
        (1.0, edges.path.samples[i].s) for i in edges.indices
    ]
    # The same side's wall name as the closed wall has, so "a wall" still
    # reads off a name's first letters; the lane's own walls say which is which.
    side_name = "WallLeft" if lane.side > 0 else "WallRight"
    return [
        SurfaceMesh("PitLane", ASPHALT, ASPHALT, mesh.ribbon(left, right, closed=False), uvs=uvs),
        _wall(layout, side_name, lane.s_merge, lane.s_fork, lane.side),
        _wall(layout, "WallPit", lane.s_entry, lane.s_exit, lane.side),
        SurfaceMesh(
            "WallPitLane",
            WALL,
            WALL,
            mesh.box_rail(
                edges.rail(edges.outer, GRASS_DROP_M),
                edges.rail(lambda s: edges.outer(s) + WALL_THICKNESS_M, GRASS_DROP_M),
                WALL_HEIGHT_M,
            ),
        ),
    ]


def outer_face(layout: Layout) -> float:
    """How far from the centreline the lane's outer wall reaches, where a checkpoint along it must."""
    return layout.runoff_half_width + WALL_THICKNESS_M + LANE_WIDTH_M + WALL_THICKNESS_M


def data(layout: Layout) -> PitLaneData:
    """The lane as the sidecar has it: its path, its lines and a pit box in front of each door."""
    # `props` imports `layout`, so it is imported here.
    from .props import PITS_DOOR_FIRST_M, PITS_DOOR_PITCH_M

    lane = layout.pit_lane
    edges = _Lane(layout)
    half = LANE_WIDTH_M / 2.0
    apron = edges.wall_back + LANE_WIDTH_M * 0.75
    path = edges.rail(edges.centre)
    return PitLaneData(
        path=path,
        half_width=half,
        entry=edges.place(lane.s_entry, edges.centre(lane.s_entry), half),
        exit=edges.place(lane.s_exit, edges.centre(lane.s_exit), half),
        boxes=[edges.place(PITS_DOOR_FIRST_M + k * PITS_DOOR_PITCH_M, apron) for k in range(PIT_BOXES)],
    )


def lines(layout: Layout) -> list[SceneryMesh]:
    """The painted entry and exit lines across the lane, a hair over its surface."""
    lane = layout.pit_lane
    edges = _Lane(layout)
    pieces = []
    for name, s, ahead in (("Entry", lane.s_entry, 1.0), ("Exit", lane.s_exit, -1.0)):
        sample = edges.path.at(s)
        forward = (sample.left_normal[1], -sample.left_normal[0])
        depth = ahead * LINE_DEPTH_M
        z = edges.height(s, edges.wall_back) + SCENERY_LIFT_M

        def corner(offset: float, along: float) -> mesh.Vertex:
            x, y = sample.offset(lane.side * offset)
            return (x + forward[0] * along, y + forward[1] * along, z)

        near, far = edges.wall_back, edges.wall_back + LANE_WIDTH_M
        pieces.append(
            SceneryMesh(
                f"PitLane.{name}",
                "kerb-white",
                mesh.quad_facing(
                    corner(near, 0.0),
                    corner(far, 0.0),
                    corner(far, depth),
                    corner(near, depth),
                    (0.0, 0.0, 1.0),
                ),
            )
        )
    return pieces

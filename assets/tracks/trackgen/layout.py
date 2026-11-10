"""What a track author writes: the path plus everything hung on it, and the
builder that turns it into meshes and placements for Blender."""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass, field, replace
from typing import TYPE_CHECKING

import math

from . import mesh
from .path import SAMPLE_STEP_M, Feature, Path, Ramp, elevation
from .sponsors import SPONSORS

if TYPE_CHECKING:
    from .pit_lane import PitLaneData

# Surface tags the sim understands; the sixth, out of world, is the absence
# of any surface below a point.
ASPHALT = "asphalt"
SAND = "sand"
GRASS = "grass"
WALL = "wall"
TYRES = "tyres"

# The grass slab sits this far below the asphalt and the sand slopes down to
# it, so no two surfaces are coplanar and the surface under a point is never
# ambiguous.
GRASS_DROP_M = 0.05
GRASS_MARGIN_M = 40.0
WALL_THICKNESS_M = 0.5
WALL_HEIGHT_M = 1.0
# A tyre wall collides as a prism with this footprint; the tyres drawn over
# it are scenery, two rings of them stacked to its height.
TYRE_WALL_RADIUS_M = 1.5
TYRE_WALL_HEIGHT_M = 1.2
TYRE_WALL_RINGS = 2
TYRES_PER_RING = 5
TYRE_DIAMETER_M = 0.6
TYRE_TREAD_M = 0.25
# The tyres' outer faces and the belts stand this far proud of the prism so
# they draw over it.
TYRE_PROUD_M = 0.03
TYRE_BELT_HEIGHT_M = 0.18

# Scenery, drawn and never collided with. It floats a hair over the surface
# it decorates so it draws on top without changing what the sim stands on.
SCENERY_LIFT_M = 0.01
# A `mesh.cuboid`'s upright faces, when its third axis is up.
SIDES = ("x0", "x1", "y0", "y1")
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
# The stands and the gantry are the style guide's Grandstand and Start
# gantry drawings in three dimensions, at the drawings' 14 px to the metre.
# A stand is a plinth and four tiers with white treads, each stepped back and in from
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
GRANDSTAND_CANOPY_THICKNESS_M = 0.43
GRANDSTAND_CANOPY_OVERHANG_M = 0.7
# The crowd: a row on each tread, a little further in on each tier, its
# colours stepping five slots a seat and three a tier through the eight key
# colours, so neighbours along a row or up a column never match.
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
# with two rows of the start line's checkers on both faces, lit on top and
# shaded underneath.
GANTRY_PAST_LINE_M = 1.0
GANTRY_BEYOND_ASPHALT_M = 1.0
"""From the edge of the asphalt to the outer face of a pillar."""
GANTRY_PILLAR_M = 0.85
GANTRY_TOP_M = 7.0
GANTRY_BEAM_HEIGHT_M = 1.71
GANTRY_BEAM_SHADE_M = 0.29
GANTRY_CHECKER_M = 0.5
GANTRY_CHECKER_ROWS = 2
# A boost pad is drawn on the rectangle the sim boosts on, so these mirror
# `BOOST_PAD_HALF_*` in `crates/sim/src/tuning.rs`; the track tests hold
# them to it. The rest is the style guide's Boost pad drawing: a carbon
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
# A sausage kerb is drawn on the rectangle the sim launches from, so these
# mirror `KERB_HALF_*` in `crates/sim/src/tuning.rs`; the track tests hold
# them to it. It is a capsule seen from above, its ends rounded inside that
# rectangle, standing proud of the asphalt as a squashed half-round, yellow
# with black stripes across it.
SAUSAGE_KERB_HALF_LENGTH_M = 4.0
SAUSAGE_KERB_HALF_WIDTH_M = 0.5
SAUSAGE_KERB_PROUD_M = 0.15
SAUSAGE_KERB_STRIPES = 6
SAUSAGE_KERB_STRIPE_PITCH_M = 1.3
SAUSAGE_KERB_STRIPE_WIDTH_M = 0.45
SAUSAGE_KERB_ACROSS_STEPS = 6
SAUSAGE_KERB_END_STEPS = 4
# The hazard zone painted over a wheel crossing: wall to wall and as deep
# either side as the wheel can reach, `WHEEL_LANDING_SPREAD_M` plus
# `WHEEL_RADIUS_M` in `crates/sim/src/tuning.rs`; the sim's tests hold the
# sidecar to it. A band of yellow and black chevrons across each end, a
# light yellow tint between: paint only, no surface tag.
HAZARD_HALF_DEPTH_M = 9.2
HAZARD_BAND_M = 1.0
HAZARD_CHEVRON_M = 1.0
# How far a chevron leans across the track over the band's depth.
HAZARD_CHEVRON_LEAN_M = 0.6
# The tint's share of yellow over the ground it lies on.
HAZARD_TINT = 0.14
# The marshal post beside a wheel crossing: a low white platform on the
# outside run-off, short of the wheel's path, banded orange under its top
# with an orange lamp on its face to the track. The View hangs the flag on
# its top.
MARSHAL_POST_BEFORE_CROSSING_M = 3.0
MARSHAL_POST_INTO_RUNOFF_M = 3.0
MARSHAL_POST_WIDTH_M = 1.4
MARSHAL_POST_HEIGHT_M = 1.0
MARSHAL_POST_BAND_M = 0.12
MARSHAL_LAMP_SIZE_M = 0.35
MARSHAL_LAMP_PROUD_M = 0.1
MARSHAL_LAMP_BELOW_TOP_M = 0.3

# The bridge where a course crosses itself: a deck this thick over the
# underpass, and its span wide enough to leave this much open ground beyond
# each of the underpass's walls.
BRIDGE_DECK_THICKNESS_M = 1.0
BRIDGE_SPAN_CLEARANCE_M = 2.0
# The deck's underside, in the tunnel's dark: girders this wide along the
# raised leg, the outer two flush with the deck's sides, their feet at the
# deck's full thickness, and a soffit this far up into the deck between
# them. The feet stay where a flat underside would be, so the girders take
# nothing from the chase camera's clearance under the deck.
BRIDGE_GIRDERS = 5
BRIDGE_GIRDER_WIDTH_M = 1.2
BRIDGE_SOFFIT_RECESS_M = 0.5
# A tunnel: a side standing against the back of each wall from the grass to
# the roof, the roof spanning between them, and a row of lamps along each
# side's inside face. The chase camera rides `ABOVE_M` over its kart in
# `apps/web/src/chase-camera.ts`, which this mirrors and the track tests hold
# it to; the roof's underside stands well clear of it, so the camera never
# cuts through it.
CHASE_CAMERA_ABOVE_M = 3.2
TUNNEL_ROOF_M = 6.0
TUNNEL_ROOF_THICKNESS_M = 0.6
TUNNEL_SIDE_THICKNESS_M = 0.6
TUNNEL_LAMP_SPACING_M = 12.0
TUNNEL_LAMP_HEIGHT_M = 4.4
"""From the road to the middle of a lamp."""
TUNNEL_LAMP_LENGTH_M = 1.6
TUNNEL_LAMP_TALL_M = 0.3
TUNNEL_LAMP_PROUD_M = 0.05

# A wood's trees keep this much grass between a canopy's edge and the wall,
# and this much between trunks, however many the layout asks for.
TREE_WALL_CLEARANCE_M = 3.0
TREE_SPACING_M = 5.0
# A prop keeps this much grass between its footprint and the wall, the
# rule for anything over 3 m tall: the chase camera hangs over the wall on
# the outside of a corner, and a tall thing nearer would stand between it
# and the kart.
PROP_WALL_CLEARANCE_M = 8.0
# The pit building's doors, one per livery at this pitch, middle to middle.
PITS_DOOR_PITCH_M = 5.5
# The first door's middle from the building's middle, by default: a building
# centred on the line has its doors run forward from it, where the grid's
# chase cameras look.
PITS_DOOR_FIRST_M = 2.75
# The first door's middle that centres them all on the building: for a lane
# whose exit line comes too soon after the building's middle for the doors to
# run forward from there.
PITS_DOORS_CENTRED_M = -(len(CROWD_KEYS) - 1) * PITS_DOOR_PITCH_M / 2.0
# A span, the arch or the hangar shell, is held to the span rule instead:
# its underside keeps the gantry's height over the asphalt and its feet
# only have to stand on grass, so they keep no set distance from the wall.
SPAN_FOOT_CLEARANCE_M = 0.0

# The eight-slot spawn grid: two staggered columns of four, pole in front on
# the left, the right column set back. Slot order is grid order. Placements
# snap to the nearest centreline sample, so these distances hold to within
# half a `SAMPLE_STEP_M`.
# Metres between the coins of a line, centre to centre: wider than the length
# the sim collects a coin over, `COIN_HALF_LENGTH_M` either side, so a kart
# takes them one at a time.
COIN_LINE_SPACING_M = 4.0
GRID_SLOTS = 8
GRID_ROW_SPACING_M = 5.0
GRID_COLUMN_OFFSET_M = 2.0
GRID_STAGGER_M = 2.0
GRID_FRONT_ROW_BEHIND_LINE_M = 4.0

@dataclass(frozen=True)
class TyreWall:
    """A stack of tyres at arc length `s`, `offset` metres left of the centreline."""

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
class CoinLine:
    """`count` coins a kart collects (GLOSSARY: coin), in a line along the direction of travel
    from arc length `s`, `COIN_LINE_SPACING_M` apart, `offset` metres left of the centreline."""

    s: float
    count: int
    offset: float = 0.0


@dataclass(frozen=True)
class Kerb:
    """A sausage kerb at arc length `s`, `offset` metres left of the centreline, lying along the track."""

    s: float
    offset: float


@dataclass(frozen=True)
class WheelCrossing:
    """Where the runaway wheel crosses the track at arc length `s`, square to it, wall to wall.

    `outside` is the side the marshal post stands on: 1 left of the centreline, -1 right.
    """

    s: float
    outside: int


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
class Bridge:
    """Where the course crosses itself: the leg from `s_start` to `s_end` rides an
    embankment over the grass, cut by a span over the leg below.

    `deck_s` is where the raised leg passes over the leg below, `underpass_s`
    where that leg passes under it. Scenery only: the raised road's own
    surfaces carry the karts.
    """

    deck_s: float
    underpass_s: float
    s_start: float
    s_end: float


@dataclass(frozen=True)
class Tunnel:
    """A roof over the road from `s_start` to `s_end` on sides behind the walls, lit along
    both sides. Scenery only: the walls under it are what a kart meets."""

    s_start: float
    s_end: float

    def __post_init__(self) -> None:
        if self.s_end <= self.s_start:
            raise ValueError("a tunnel runs forward along the lap")


@dataclass(frozen=True)
class Disc:
    """A round patch of grass, `r` metres about (`x`, `y`) in track metres."""

    x: float
    y: float
    r: float


@dataclass(frozen=True)
class Line:
    """A row from `a` to `b`, each a point in track metres."""

    a: tuple[float, float]
    b: tuple[float, float]


@dataclass(frozen=True)
class Wood:
    """A patch of trees of one `kind` (`pine`, `round` or `cherry`) the builder scatters over
    `patch`: up to `count` of them, from the random stream `seed` starts, dropping any tree
    too near a wall or another tree. A layout never places a tree by hand."""

    kind: str
    patch: Disc | Line
    count: int
    seed: int


@dataclass(frozen=True)
class Spot:
    """A place hung on the lap: `s` metres along it and `offset` metres left of the centreline,
    facing the way the lap runs there, as every placement is."""

    s: float
    offset: float = 0.0


@dataclass(frozen=True)
class World:
    """A place set in the world: track metres, facing `heading` radians counter-clockwise from +X."""

    x: float
    y: float
    heading: float = 0.0


@dataclass(frozen=True)
class Prop:
    """A trackside prop of one `kind` (`circuit-wheel`, `arch`, `pits`, `hangars`,
    `hangar-shell`, `control-tower`, `airship`, `helicopter`, `jets`, `harbour`, `casino`,
    `block` or `pool`) placed by `at`, built by `trackgen.props`: scenery in the glTF only,
    which no kart meets, standing beyond the walls or flying over them (ADR 0050). A kind that
    runs along the track, the pits, a row of hangars or the hangar shell over it, is `length` metres long, centred on
    `at`, on whichever side of the track `at` is, facing it. A kind that flies, the airship or
    the helicopter, circles `at` at `radius` metres, its nose `height` metres over the grass;
    the jets fly a straight pass `length` metres long centred on `at`, against the way it
    faces, the leader's nose `height` metres over the grass. The harbour's
    `outline` is its quay's outer edge, counter-clockwise, each corner in metres along `at`'s
    heading and to its left; `spray` names the lap segments whose stretch of its quay throws
    spray. A `block` of hotels or flats is `length` metres along its front and `height` metres
    tall; a `pool` is `length` metres long. `white` paints the pits' walls wall white."""

    kind: str
    at: Spot | World
    length: float | None = None
    radius: float | None = None
    height: float | None = None
    white: bool = False
    outline: tuple[tuple[float, float], ...] = ()
    spray: tuple[str, ...] = ()


@dataclass(frozen=True)
class Decor:
    """The scenery a layout asks for beyond what every track gets (kerbs and the start line)."""

    hoardings: list[Hoardings] = field(default_factory=list)
    grandstands: list[Grandstands] = field(default_factory=list)
    gantry: bool = False
    bridges: list[Bridge] = field(default_factory=list)
    tunnels: list[Tunnel] = field(default_factory=list)
    woods: list[Wood] = field(default_factory=list)
    props: list[Prop] = field(default_factory=list)


@dataclass(frozen=True)
class PitLane:
    """A pit lane on `side` of the centreline (1 left, -1 right), in metres of arc length: it
    forks off the run-off at `s_fork`, the pit limiter holds from `s_entry` to `s_exit` (ADR
    0081) and it is back on the run-off at `s_merge`. `s_fork` and `s_entry` may be negative,
    behind the start line. `s_building` is the pit building's middle, the prop's `Spot.s`, which
    the pit boxes line up on; props never reach the sidecar, so the layout says it twice.
    `door_first` is the first door's middle, in metres on from `s_building`, the rest following
    at the pitch, so it says where both the building's doors and the pit boxes stand."""

    side: int
    s_fork: float
    s_entry: float
    s_exit: float
    s_merge: float
    s_building: float = 0.0
    door_first: float = PITS_DOOR_FIRST_M


@dataclass(frozen=True)
class Taper:
    """The asphalt's half width changing linearly in s from `s_start` to `s_end`: from the width
    of the segment ending at or holding `s_start` to that of the segment starting at or holding
    `s_end`, so the edge never steps. A taper overrides the segments it lies over."""

    s_start: float
    s_end: float


@dataclass
class Layout:
    name: str
    path: Path
    asphalt_half_width: float
    runoff_width: float
    checkpoint_count: int
    tyre_walls: list[TyreWall]
    item_rows: list[ItemRow]
    boost_pads: list[BoostPad]
    coin_lines: list[CoinLine] = field(default_factory=list)
    kerbs: list[Kerb] = field(default_factory=list)
    wheel_crossings: list[WheelCrossing] = field(default_factory=list)
    elevation: list[Feature] = field(default_factory=list)
    ramps: list[Ramp] = field(default_factory=list)
    decor: Decor = field(default_factory=Decor)
    pit_lane: PitLane | None = None
    half_widths: dict[str, float] = field(default_factory=dict)
    """The asphalt's half width over the segments so named, where it is not `asphalt_half_width`.
    The run-off keeps `runoff_width` beyond the local edge, so the walls follow it in."""
    tapers: list[Taper] = field(default_factory=list)

    def __post_init__(self) -> None:
        for name in self.half_widths:
            self.path.segment(name)

    @property
    def runoff_half_width(self) -> float:
        """The default's: where a layout narrows, `runoff_half_width_at` is the wall's place."""
        return self.asphalt_half_width + self.runoff_width

    def half_width_at(self, s: float) -> float:
        """The asphalt's half width at arc length `s`, which may wrap around the loop."""
        if not self.half_widths:
            return self.asphalt_half_width
        length = self.path.length
        for taper in self.tapers:
            span = (taper.s_end - taper.s_start) % length
            into = (s - taper.s_start) % length
            if into <= span:
                before = self._segment_half_width(taper.s_start, before=True)
                after = self._segment_half_width(taper.s_end, before=False)
                return before + (after - before) * into / span
        return self._segment_half_width(s, before=False)

    def runoff_half_width_at(self, s: float) -> float:
        return self.half_width_at(s) + self.runoff_width

    def _segment_half_width(self, s: float, before: bool) -> float:
        """The half width of the segment at `s`; on a boundary, the one ending there when `before`,
        else the one starting there."""
        length = self.path.length
        s %= length
        if before and s == 0.0:
            s = length
        for segment in self.path.segments:
            inside = segment.s_start < s <= segment.s_end if before else segment.s_start <= s < segment.s_end
            if inside:
                return self.half_widths.get(segment.name, self.asphalt_half_width)
        return self.asphalt_half_width

    def width_is_uniform(self, s_start: float, s_end: float) -> bool:
        """Whether the half width holds from `s_start` to `s_end`, wrapping past the start line."""
        span = (s_end - s_start) % self.path.length
        steps = max(1, math.ceil(span / SAMPLE_STEP_M))
        widths = {self.half_width_at(s_start + span * k / steps) for k in range(steps + 1)}
        return len(widths) == 1


@dataclass(frozen=True)
class Placement:
    """Something placed on the track: position on the surface and the direction it faces."""

    x: float
    y: float
    z: float
    heading: float
    half_width: float = 0.0
    hazard_half_depth: float = 0.0


@dataclass(frozen=True)
class SurfaceMesh:
    name: str
    surface: str
    material: str
    mesh: mesh.Mesh
    uvs: list[tuple[float, float]] | None = None
    """One per vertex when the surface takes decals: tyre marks need the asphalt's."""


@dataclass(frozen=True)
class Pivot:
    """The node of its own a mesh the View moves gets: its origin at (`x`, `y`, `z`) with its
    local x along `heading` and its local y to the left of it, hung under the node named
    `parent` when it moves with that one too. The build script sets the origin and the
    parent; the View turns the node about its own axes."""

    x: float
    y: float
    z: float
    heading: float
    parent: str | None = None


@dataclass(frozen=True)
class SceneryMesh:
    """A mesh drawn and never collided with: not in the sidecar."""

    name: str
    material: str
    mesh: mesh.Mesh
    pivot: Pivot | None = None
    """Set for a mesh the View moves: exported as a node named `name` with its origin at the
    pivot, never merged into its material's family."""
    materials: tuple[str, ...] = ()
    """One material per face when a pivoted node mixes them; `material` is then its first.
    A still family never mixes: `ByMaterial` splits it instead."""


def locate(layout: Layout, at: Spot | World) -> tuple[float, float, float]:
    """Where `at` stands in track metres, and the heading it faces, in radians."""
    if isinstance(at, World):
        return at.x, at.y, at.heading
    sample = layout.path.at(at.s)
    nx, ny = sample.left_normal
    return sample.x + at.offset * nx, sample.y + at.offset * ny, sample.heading


@dataclass(frozen=True)
class Lettering:
    """Lettering Blender sets in the scenery: `size` metres tall, in `material`, read from `at.heading`.

    With `arc`, the text is bent round a circle of that radius about `at` in the plane it
    faces, reading left to right over the circle's top, as the arch's belt is lettered. With
    `parent`, the name of a pivoted scenery mesh, the text is hung under that node so it rides
    wherever the View moves it, as the airship's name rides its hull; `at` is still where the
    text stands at rest."""

    name: str
    text: str
    material: str
    size: float
    at: Placement
    arc: float | None = None
    parent: str | None = None


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
    lettering: list[Lettering] = field(default_factory=list)
    kerbs: list[Placement] = field(default_factory=list)
    wheel_crossings: list[Placement] = field(default_factory=list)
    coins: list[Placement] = field(default_factory=list)
    pit_lane: PitLaneData | None = None
    ramp_lips: list[Placement] = field(default_factory=list)


def uniform_widths(layout: Layout) -> None:
    """Refuse a layout that changes its width under what is built to one width: a tunnel, a
    bridge, a ramp, a pit lane or a wheel crossing's hazard zone."""
    spans = [(f"tunnel at {t.s_start}", t.s_start, t.s_end) for t in layout.decor.tunnels]
    spans += [(f"bridge at {b.deck_s}", b.s_start, b.s_end) for b in layout.decor.bridges]
    spans += [(f"ramp at {r.s_start}", r.s_start, r.s_end) for r in layout.ramps]
    spans += [
        (f"wheel crossing at {c.s}", c.s - HAZARD_HALF_DEPTH_M, c.s + HAZARD_HALF_DEPTH_M)
        for c in layout.wheel_crossings
    ]
    if layout.pit_lane is not None:
        spans.append(("pit lane", layout.pit_lane.s_fork, layout.pit_lane.s_merge))
    for what, s_start, s_end in spans:
        if not layout.width_is_uniform(s_start, s_end):
            raise ValueError(f"{layout.name}: the road's width changes under the {what}")


def build(layout: Layout) -> TrackGeometry:
    uniform_widths(layout)
    path = layout.path
    height = [elevation(layout.elevation, sample.s) for sample in path.samples]

    halves = [layout.half_width_at(sample.s) for sample in path.samples]

    def rail(side: int, beyond: float = 0.0, drop: float = 0.0) -> list[mesh.Vertex]:
        """Along the lap on `side` (1 left, -1 right), `beyond` metres out from the local edge of the asphalt."""
        return [
            (*sample.offset(side * (half + beyond)), h - drop)
            for sample, half, h in zip(path.samples, halves, height)
        ]

    runoff = layout.runoff_width
    # Across the asphalt from its left edge, and along it in metres of arc
    # length, so a tyre mark laid in these coordinates lands where the kart was.
    asphalt_uvs = [(0.0, sample.s) for sample in path.samples] + [
        (1.0, sample.s) for sample in path.samples
    ]
    asphalt = mesh.ribbon(rail(1), rail(-1), closed=True)
    for ramp in layout.ramps:
        asphalt, asphalt_uvs = ramp_in_the_asphalt(layout, ramp, asphalt, asphalt_uvs)
    surfaces = [
        SurfaceMesh("Asphalt", ASPHALT, ASPHALT, asphalt, uvs=asphalt_uvs),
        SurfaceMesh(
            "SandLeft", SAND, SAND, mesh.ribbon(rail(1, runoff, GRASS_DROP_M), rail(1), closed=True)
        ),
        SurfaceMesh(
            "SandRight", SAND, SAND, mesh.ribbon(rail(-1), rail(-1, runoff, GRASS_DROP_M), closed=True)
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

    # `pit_lane` imports this module, so it is imported here, once everything
    # it needs is defined.
    from . import pit_lane

    lane = layout.pit_lane
    if lane is not None:
        surfaces += pit_lane.surfaces(layout)

    # Track limits: a wall along each edge of the run-off the whole way
    # round, standing on the sand's outer edge so it follows the hill with
    # the track. A kart can go off the asphalt but never off
    # the course, so no corner can be cut (ADR 0022). A pit lane's side has
    # its own walls, with the gaps the lane comes and goes by.
    for name, side in (("WallLeft", 1), ("WallRight", -1)):
        if lane is not None and side == lane.side:
            continue
        inner = rail(side, runoff, GRASS_DROP_M)
        outer = rail(side, runoff + WALL_THICKNESS_M, GRASS_DROP_M)
        surfaces.append(
            SurfaceMesh(name, WALL, WALL, mesh.box_rail(inner, outer, WALL_HEIGHT_M, closed=True))
        )

    for number, wall in enumerate(layout.tyre_walls):
        surfaces.append(
            SurfaceMesh(
                f"TyreWall.{number:02d}",
                TYRES,
                "tyre-shade",
                mesh.cylinder(tyre_wall_base(layout, wall), TYRE_WALL_RADIUS_M, TYRE_WALL_HEIGHT_M),
            )
        )

    def place(s: float, offset: float = 0.0, half_width: float = 0.0, hazard_half_depth: float = 0.0) -> Placement:
        sample = path.at(s)
        x, y = sample.offset(offset)
        return Placement(x, y, elevation(layout.elevation, sample.s), sample.heading, half_width, hazard_half_depth)

    def checkpoint_reach(s: float) -> float:
        """To the walls' outer faces, or along a pit lane to its outer wall's, on both sides as
        every checkpoint is (ADR 0079): a lap through the pits crosses these too. Read at the
        sample the checkpoint snaps to, where a taper's width is the wall's."""
        s = path.at(s).s
        if lane is not None and (s - lane.s_fork) % path.length <= lane.s_merge - lane.s_fork:
            return pit_lane.outer_face(layout)
        return layout.runoff_half_width_at(s) + WALL_THICKNESS_M

    # Evenly spaced from the start line, each reaching the walls' outer faces,
    # so a kart between the walls or in the air over one crosses every one in
    # order (ADR 0079).
    checkpoints = [
        place(s, half_width=checkpoint_reach(s))
        for s in (k * path.length / layout.checkpoint_count for k in range(layout.checkpoint_count))
    ]
    item_boxes = [place(row.s, offset) for row in layout.item_rows for offset in row.offsets]
    boost_pads = [place(pad.s, pad.offset) for pad in layout.boost_pads]
    kerbs = [place(kerb.s, kerb.offset) for kerb in layout.kerbs]
    coins = [
        place(line.s + k * COIN_LINE_SPACING_M, line.offset) for line in layout.coin_lines for k in range(line.count)
    ]
    # At the height of the lip's top edge, across the ramp's bounds: the sim judges a trick by them.
    ramp_lips = [
        replace(
            place(ramp.s_end, (ramp.left + ramp.right) / 2.0, half_width=(ramp.left - ramp.right) / 2.0),
            z=elevation(layout.elevation, ramp.s_end) + ramp.height,
        )
        for ramp in layout.ramps
    ]
    # Wall to wall: the run-off's edges, where the walls stand.
    wheel_crossings = [
        place(crossing.s, half_width=layout.runoff_half_width_at(crossing.s), hazard_half_depth=HAZARD_HALF_DEPTH_M)
        for crossing in layout.wheel_crossings
    ]
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
    scenery, lettering = decorate(layout, height)
    return TrackGeometry(
        centreline,
        surfaces,
        checkpoints,
        item_boxes,
        boost_pads,
        spawn_grid,
        scenery,
        lettering,
        kerbs,
        wheel_crossings,
        coins,
        pit_lane.data(layout) if lane is not None else None,
        ramp_lips,
    )


def ramp_stations(layout: Layout, ramp: Ramp) -> tuple[list[int], list[float]]:
    """The centreline samples a ramp spans and its rise at each, from nothing at the foot to its
    full height at the lip, whichever samples lie nearest its two ends."""
    half = layout.half_width_at(ramp.s_start)
    if not -half <= ramp.right < ramp.left <= half:
        raise ValueError(f"a ramp from {ramp.right} to {ramp.left} m is not on {half} m of asphalt")
    indices = layout.path.indices_between(ramp.s_start, ramp.s_end)
    first, last = (layout.path.samples[i].s for i in (indices[0], indices[-1]))
    return indices, [ramp.height * (layout.path.samples[i].s - first) / (last - first) for i in indices]


def ramp_in_the_asphalt(
    layout: Layout,
    ramp: Ramp,
    asphalt: mesh.Mesh,
    uvs: list[tuple[float, float]],
) -> tuple[mesh.Mesh, list[tuple[float, float]]]:
    """The asphalt ribbon with a ramp's stretch rebuilt so the slope rises out of it.

    In the one mesh, sharing vertices along the foot, because the sim smooths a seam only between
    a mesh's own triangles: a wedge laid on the road as a mesh of its own throws a kart off the
    edge of its foot. The slope has no faces for its sides or lip, which catch a kart on the road
    beside them and stop it dead, nor road beneath it, which takes the lift off its launch: the
    sim's ramp is the slope alone, and `ramp_markings` draws the rest. A kart that comes in from
    the side rides up it, or runs under its high end on the grass there, and is never held.
    """
    half = layout.half_width_at(ramp.s_start)
    path = layout.path
    vertices, faces = list(asphalt[0]), list(asphalt[1])
    uvs = list(uvs)
    count = len(path.samples)
    indices, rise = ramp_stations(layout, ramp)
    first, last = indices[0], indices[-1]

    def add(index: int, offset: float, up: float) -> int:
        sample = path.samples[index]
        vertices.append((*sample.offset(offset), elevation(layout.elevation, sample.s) + up))
        uvs.append(((half - offset) / (2.0 * half), sample.s))
        return len(vertices) - 1

    # Left to right across the asphalt: the ribbon's own rails, with one more
    # at each ramp edge inside them. The foot's slope is the road's own row.
    stops = [half] + [offset for offset in (ramp.left, ramp.right) if abs(offset) < half] + [-half]

    def road_vertex(index: int, offset: float) -> int:
        return index if offset == half else count + index

    road = {
        i: {o: road_vertex(i, o) if abs(o) == half else add(i, o, 0.0) for o in stops} for i in indices
    }
    slope = {
        i: {o: add(i, o, up) if up > 0.0 else road[i][o] for o in (ramp.left, ramp.right)}
        for i, up in zip(indices, rise)
    }

    # The ribbon's quads from the row before the foot through the lip's row go.
    def ribbon_quad(i: int) -> mesh.Face:
        j = (i + 1) % count
        return (i, count + i, count + j, j)

    before, after = (first - 1) % count, (last + 1) % count
    gone = {ribbon_quad((before + k) % count) for k in range(len(indices) + 1)}
    faces = [face for face in faces if face not in gone]

    # Onto the foot's row and on from the lip's, each split where the ramp's edges run.
    row = [road[first][o] for o in stops]
    faces.append((before, count + before, row[-1]))
    faces += [(before, right, left) for left, right in zip(row, row[1:])]
    row = [road[last][o] for o in stops]
    faces += [(left, right, after) for left, right in zip(row, row[1:])]
    faces.append((row[-1], count + after, after))

    for here, on in zip(indices, indices[1:]):
        faces.append((slope[here][ramp.left], slope[here][ramp.right], slope[on][ramp.right], slope[on][ramp.left]))
        # The flat road beside the slope.
        for left, right in zip(stops, stops[1:]):
            if left != ramp.left:
                faces.append((road[here][left], road[here][right], road[on][right], road[on][left]))
    return (vertices, faces), uvs


def ramp_markings(layout: Layout) -> list[SceneryMesh]:
    """Kerb stripes down both edges of a ramp's slope and its sides and a white lip, so a driver
    tells the ramp's half of the road from the flat half in time to choose."""
    result: list[SceneryMesh] = []
    for number, ramp in enumerate(layout.ramps):
        indices, rise = ramp_stations(layout, ramp)
        samples = [layout.path.samples[i] for i in indices]

        def on_slope(offset: float, lift: float = SCENERY_LIFT_M) -> list[mesh.Vertex]:
            return [
                (*sample.offset(offset), elevation(layout.elevation, sample.s) + h + lift)
                for sample, h in zip(samples, rise)
            ]

        def on_road(offset: float) -> list[mesh.Vertex]:
            return [(*sample.offset(offset), elevation(layout.elevation, sample.s)) for sample in samples]

        # Proud of where the slope's sides stand, as its stripes are of its top.
        left, right = ramp.left + SCENERY_LIFT_M, ramp.right - SCENERY_LIFT_M
        ribbons = {
            "L": mesh.ribbon(on_slope(ramp.left), on_slope(ramp.left - KERB_WIDTH_M), closed=False),
            "R": mesh.ribbon(on_slope(ramp.right + KERB_WIDTH_M), on_slope(ramp.right), closed=False),
            "side.L": mesh.ribbon(on_road(left), on_slope(left, 0.0), closed=False),
            "side.R": mesh.ribbon(on_slope(right, 0.0), on_road(right), closed=False),
        }
        for label, ribbon in ribbons.items():
            red, white = mesh.striped(ribbon, KERB_STRIPE_SAMPLES)
            result.append(SceneryMesh(f"Ramp.{number:02d}.{label}.red", "kerb-red", red))
            result.append(SceneryMesh(f"Ramp.{number:02d}.{label}.white", "kerb-white", white))
        last = samples[-1]
        ground = elevation(layout.elevation, last.s)
        ahead = (last.left_normal[1], -last.left_normal[0], 0.0)
        along = (ahead[0] * SCENERY_LIFT_M, ahead[1] * SCENERY_LIFT_M)

        def on_lip(offset: float, z: float) -> mesh.Vertex:
            x, y = last.offset(offset)
            return (x + along[0], y + along[1], z)

        result.append(
            SceneryMesh(
                f"Ramp.{number:02d}.lip",
                "kerb-white",
                mesh.quad_facing(
                    on_lip(ramp.left, ground + rise[-1]),
                    on_lip(ramp.right, ground + rise[-1]),
                    on_lip(ramp.right, ground),
                    on_lip(ramp.left, ground),
                    ahead,
                ),
            )
        )
    return result


def decorate(layout: Layout, height: list[float]) -> tuple[list[SceneryMesh], list[Lettering]]:
    """The scenery: kerbs on every corner, the start line, and what the layout's decor asks for."""
    scenery = kerbs(layout, height) + start_line(layout) + boost_pads(layout) + tyre_stacks(layout)
    scenery += sausage_kerbs(layout) + hazard_zones(layout) + marshal_posts(layout) + ramp_markings(layout)
    if layout.pit_lane is not None:
        from . import pit_lane

        scenery += pit_lane.lines(layout)
    boards, lettering = hoardings(layout)
    scenery += boards + grandstands(layout)
    if layout.decor.gantry:
        scenery += gantry(layout)
    scenery += bridges(layout, height) + tunnels(layout, height)
    # `props` imports this module, so it is imported here, once everything
    # it needs is defined.
    from .props import props, woods

    placed, lettered = props(layout)
    scenery += woods(layout) + placed
    return scenery, lettering + lettered


def kerbs(layout: Layout, height: list[float]) -> list[SceneryMesh]:
    """Red-and-white stripes along both edges of the asphalt through every corner."""
    path = layout.path

    def lifted(index: int, offset: float) -> mesh.Vertex:
        sample = path.samples[index]
        return (*sample.offset(offset), height[index] + SCENERY_LIFT_M)

    result: list[SceneryMesh] = []
    for segment in path.segments:
        if segment.turn == 0:
            continue
        indices = path.indices_between(segment.s_start, segment.s_end)
        for side, label in ((1, "L"), (-1, "R")):
            halves = [layout.half_width_at(path.samples[i].s) for i in indices]
            outer = [lifted(i, side * half) for i, half in zip(indices, halves)]
            inner = [lifted(i, side * (half - KERB_WIDTH_M)) for i, half in zip(indices, halves)]
            left, right = (outer, inner) if side == 1 else (inner, outer)
            red, white = mesh.striped(mesh.ribbon(left, right, closed=False), KERB_STRIPE_SAMPLES)
            result.append(SceneryMesh(f"Kerb.{segment.name}.{label}.red", "kerb-red", red))
            result.append(SceneryMesh(f"Kerb.{segment.name}.{label}.white", "kerb-white", white))
    return result


def tyre_wall_base(layout: Layout, wall: TyreWall) -> mesh.Vertex:
    """The centre of the tyre wall's footprint, on the asphalt."""
    sample = layout.path.at(wall.s)
    return (*sample.offset(wall.offset), elevation(layout.elevation, sample.s))


def tyre_stacks(layout: Layout) -> list[SceneryMesh]:
    """The tyres over each tyre wall's prism: rings of tyres standing on their
    treads, axles pointing out, each ring turned half a tyre's spacing from
    the one below and belted round its middle in alternating red and white."""
    parts = ByMaterial("TyreStack")
    face_radius = TYRE_WALL_RADIUS_M + TYRE_PROUD_M
    # Far enough out that the belt's flat sides clear the tyres' faces.
    belt_radius = face_radius + TYRE_PROUD_M
    belt_sides = 4 * TYRES_PER_RING
    for wall in layout.tyre_walls:
        cx, cy, cz = tyre_wall_base(layout, wall)
        for ring in range(TYRE_WALL_RINGS):
            middle = cz + (ring + 0.5) * TYRE_DIAMETER_M
            turn = math.pi / TYRES_PER_RING * ring
            for k in range(TYRES_PER_RING):
                angle = turn + math.tau * k / TYRES_PER_RING
                out = (math.cos(angle), math.sin(angle), 0.0)
                centre = (cx + out[0] * face_radius, cy + out[1] * face_radius, middle)
                rubber, face = mesh.drum(centre, (-out[0], -out[1], 0.0), TYRE_DIAMETER_M / 2.0, TYRE_TREAD_M)
                parts.add("tyre", rubber)
                parts.add("tyre-face", face)
            bottom = middle - TYRE_BELT_HEIGHT_M / 2.0
            for k in range(belt_sides):
                a0 = turn + math.tau * k / belt_sides
                a1 = turn + math.tau * (k + 1) / belt_sides
                corners = [
                    (cx + belt_radius * math.cos(a), cy + belt_radius * math.sin(a), z)
                    for a, z in ((a0, bottom), (a1, bottom))
                ]
                top = [(x, y, z + TYRE_BELT_HEIGHT_M) for x, y, z in corners]
                parts.add(
                    "kerb-red" if k % 2 == 0 else "kerb-white",
                    mesh.quad(corners[0], corners[1], top[1], top[0]),
                )
    return parts.scenery()


def start_line(layout: Layout) -> list[SceneryMesh]:
    """Checkers across the asphalt from the start line forward."""
    line = layout.path.at(0.0)
    half = layout.half_width_at(0.0)
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


def sausage_kerbs(layout: Layout) -> list[SceneryMesh]:
    """A yellow capsule over each sausage kerb's rectangle, striped black across."""
    half_length, half_width = SAUSAGE_KERB_HALF_LENGTH_M, SAUSAGE_KERB_HALF_WIDTH_M
    body = half_length - half_width
    # Stations along the kerb: round its back end, then every stripe's edges
    # along the body, then round its front end, so each band between two
    # stations is one colour.
    end = [
        body + half_width * math.sin(math.pi / 2.0 * k / SAUSAGE_KERB_END_STEPS)
        for k in range(1, SAUSAGE_KERB_END_STEPS + 1)
    ]
    first = -SAUSAGE_KERB_STRIPE_PITCH_M * (SAUSAGE_KERB_STRIPES - 1) / 2.0
    centres = [first + k * SAUSAGE_KERB_STRIPE_PITCH_M for k in range(SAUSAGE_KERB_STRIPES)]
    edges = [c + d * SAUSAGE_KERB_STRIPE_WIDTH_M / 2.0 for c in centres for d in (-1.0, 1.0)]
    stations = [-a for a in reversed(end)] + [-body] + edges + [body] + end

    def black(back: float, front: float) -> bool:
        middle = (back + front) / 2.0
        return any(abs(middle - c) < SAUSAGE_KERB_STRIPE_WIDTH_M / 2.0 for c in centres)

    result: list[SceneryMesh] = []
    for number, kerb in enumerate(layout.kerbs):
        sample = layout.path.at(kerb.s)
        x, y = sample.offset(kerb.offset)
        cos, sin = math.cos(sample.heading), math.sin(sample.heading)
        base = elevation(layout.elevation, sample.s) + SCENERY_LIFT_M

        def ring(ahead: float) -> list[mesh.Vertex]:
            """The kerb's cross-section `ahead` metres along it, from its left foot over to its right."""
            rounded = max(abs(ahead) - body, 0.0)
            scale = math.sqrt(max(half_width * half_width - rounded * rounded, 0.0)) / half_width
            points = []
            for k in range(SAUSAGE_KERB_ACROSS_STEPS + 1):
                angle = math.pi * k / SAUSAGE_KERB_ACROSS_STEPS
                right = -math.cos(angle) * half_width * scale
                up = math.sin(angle) * SAUSAGE_KERB_PROUD_M * scale
                points.append((x + ahead * cos + right * sin, y + ahead * sin - right * cos, base + up))
            return points

        parts = {"kerb-yellow": [], "kerb-black": []}
        for back, front in zip(stations, stations[1:]):
            # Left to right along the back, then back along the front, so
            # each quad faces up and out.
            a, b = ring(back), ring(front)
            faces = [mesh.quad(a[k], a[k + 1], b[k + 1], b[k]) for k in range(SAUSAGE_KERB_ACROSS_STEPS)]
            parts["kerb-black" if black(back, front) else "kerb-yellow"].append(mesh.merge(faces))
        for material, pieces in parts.items():
            result.append(SceneryMesh(f"SausageKerb.{number:02d}.{material}", material, mesh.merge(pieces)))
    return result


def hazard_zones(layout: Layout) -> list[SceneryMesh]:
    """Paint over each wheel crossing, wall to wall and `HAZARD_HALF_DEPTH_M` either side: chevrons across
    each end, a tint between, on the asphalt and the sand falling to the walls alike."""
    result: list[SceneryMesh] = []
    for number, crossing in enumerate(layout.wheel_crossings):
        half, runoff = layout.half_width_at(crossing.s), layout.runoff_half_width_at(crossing.s)
        sample = layout.path.at(crossing.s)
        cos, sin = math.cos(sample.heading), math.sin(sample.heading)

        def at(ahead: float, right: float) -> mesh.Vertex:
            # The sand falls `GRASS_DROP_M` from the asphalt's edge to the wall.
            drop = GRASS_DROP_M * max(abs(right) - half, 0.0) / (runoff - half)
            return (
                sample.x + ahead * cos + right * sin,
                sample.y + ahead * sin - right * cos,
                elevation(layout.elevation, sample.s + ahead) - drop + SCENERY_LIFT_M,
            )

        def flat(corners: list[tuple[float, float]]) -> mesh.Mesh:
            # Corners counter-clockwise in (ahead, right), a mirror of the
            # ground's frame, so reversed to face up.
            return mesh.quad(*(at(a, r) for a, r in reversed(corners)))

        parts = ByMaterial(f"HazardZone.{number:02d}")
        inner = HAZARD_HALF_DEPTH_M - HAZARD_BAND_M
        # The tint in strips at the asphalt's edges, so it lies on the sand's fall.
        for material, left, right in (
            ("hazard-sand", -runoff, -half),
            ("hazard-asphalt", -half, half),
            ("hazard-sand", half, runoff),
        ):
            parts.add(material, flat([(-inner, left), (inner, left), (inner, right), (-inner, right)]))
        # Each chevron leans the same way at both ends, cut square at the walls.
        chevrons = int(math.ceil((2.0 * runoff + HAZARD_CHEVRON_LEAN_M) / HAZARD_CHEVRON_M))
        for back, front in ((-HAZARD_HALF_DEPTH_M, -inner), (inner, HAZARD_HALF_DEPTH_M)):
            for k in range(chevrons):
                left = -runoff - HAZARD_CHEVRON_LEAN_M + k * HAZARD_CHEVRON_M
                right = left + HAZARD_CHEVRON_M
                clip = lambda r: min(max(r, -runoff), runoff)
                lean = HAZARD_CHEVRON_LEAN_M
                corners = [
                    (back, clip(left)),
                    (front, clip(left + lean)),
                    (front, clip(right + lean)),
                    (back, clip(right)),
                ]
                if corners[0][1] == corners[3][1] and corners[1][1] == corners[2][1]:
                    continue
                parts.add("kerb-black" if k % 2 else "kerb-yellow", flat(corners))
        result += parts.scenery()
    return result


def marshal_posts(layout: Layout) -> list[SceneryMesh]:
    """A low white platform on the outside run-off beside each wheel crossing, banded orange under its top,
    an orange lamp on its face to the track."""
    result: list[SceneryMesh] = []
    for number, crossing in enumerate(layout.wheel_crossings):
        sample = layout.path.at(crossing.s - MARSHAL_POST_BEFORE_CROSSING_M)
        # A run-off too narrow for the usual setback has the post in its
        # middle, so it always stands inside the wall.
        into = min(MARSHAL_POST_INTO_RUNOFF_M, layout.runoff_width / 2.0)
        offset = crossing.outside * (layout.half_width_at(sample.s) + into)
        x, y = sample.offset(offset)
        cos, sin = math.cos(sample.heading), math.sin(sample.heading)
        ground = elevation(layout.elevation, sample.s) - GRASS_DROP_M
        # Toward the track from the post: across the heading, back to the centreline.
        inward = -crossing.outside

        def box(along: tuple[float, float], toward: tuple[float, float], up: tuple[float, float]) -> dict[str, mesh.Mesh]:
            """A box `along` the track, `toward` it from the post's centre and `up` from the ground."""

            def corner(i: int, j: int, k: int) -> mesh.Vertex:
                a, t = along[i], toward[j] * inward
                # Left of the heading, seen from above, is (-sin, cos).
                return (x + cos * a - sin * t, y + sin * a + cos * t, ground + up[k])

            return mesh.cuboid(corner)

        half = MARSHAL_POST_WIDTH_M / 2.0
        parts = ByMaterial(f"MarshalPost.{number:02d}")
        band_bottom = MARSHAL_POST_HEIGHT_M - MARSHAL_POST_BAND_M
        post = box((-half, half), (-half, half), (0.0, band_bottom))
        for side in SIDES:
            parts.add("board", post[side])
        band = box((-half, half), (-half, half), (band_bottom, MARSHAL_POST_HEIGHT_M))
        for side in SIDES:
            parts.add("post-band", band[side])
        parts.add("board", band["z1"])
        lamp_half = MARSHAL_LAMP_SIZE_M / 2.0
        lamp_top = MARSHAL_POST_HEIGHT_M - MARSHAL_LAMP_BELOW_TOP_M
        lamp = box(
            (-lamp_half, lamp_half),
            (half, half + MARSHAL_LAMP_PROUD_M),
            (lamp_top - MARSHAL_LAMP_SIZE_M, lamp_top),
        )
        for side in (*SIDES, "z0", "z1"):
            parts.add("chevron", lamp[side])
        result += parts.scenery()
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
    s = s_start + spacing / 2.0
    while s + spacing / 2.0 <= s_end:
        sample = path.at(s)
        x, y = sample.offset(side * (layout.runoff_half_width_at(sample.s) + WALL_THICKNESS_M + setback))
        nx, ny = sample.left_normal
        # Towards the track: against the left normal on the left, with it on the right.
        facing = math.atan2(-side * ny, -side * nx)
        yield Placement(x, y, elevation(layout.elevation, sample.s), facing)
        s += spacing


def hoardings(layout: Layout) -> tuple[list[SceneryMesh], list[Lettering]]:
    """White boards facing the track with a sponsor's name on each, cycling through the sponsors."""
    boards: list[SceneryMesh] = []
    lettering: list[Lettering] = []
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
                Lettering(
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


class ByMaterial:
    """Pieces gathered by material, then merged into one scenery mesh per material named `<family>.<material>`,
    so a family costs a draw call per colour however many pieces it has."""

    def __init__(self, family: str) -> None:
        self.family = family
        self.pieces: dict[str, list[mesh.Mesh]] = {}

    def add(self, material: str, piece: mesh.Mesh) -> None:
        self.pieces.setdefault(material, []).append(piece)

    def scenery(self) -> list[SceneryMesh]:
        return [
            SceneryMesh(f"{self.family}.{material}", material, mesh.merge(pieces))
            for material, pieces in self.pieces.items()
        ]


def grandstands(layout: Layout) -> list[SceneryMesh]:
    """Stands shoulder to shoulder behind the hoardings, on the grass, facing the track."""
    parts = ByMaterial("Grandstand")

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
            for side in SIDES:
                parts.add("stand-plinth", plinth[side])
            for tier in range(GRANDSTAND_TIERS):
                tier_half = half - tier * GRANDSTAND_TIER_INSET_M
                bottom = GRANDSTAND_PLINTH_M + tier * GRANDSTAND_TIER_RISE_M
                top = bottom + GRANDSTAND_TIER_RISE_M
                near = tier * GRANDSTAND_TIER_TREAD_M
                faces = box(-tier_half, tier_half, near, depth, bottom, top)
                parts.add("wall", faces["z1"])
                for side in SIDES:
                    parts.add("stand-riser", faces[side])
                for seat in range(CROWD_PER_TIER):
                    left = -half + CROWD_FIRST_IN_M + tier * CROWD_STEP_IN_M + seat * CROWD_SPACING_M
                    back = near + CROWD_FROM_RISER_M
                    person = box(left, left + CROWD_WIDTH_M, back, back + CROWD_WIDTH_M, top, top + CROWD_HEIGHT_M)
                    slot = (seat * 5 + tier * 3) % len(CROWD_KEYS)
                    for side, face in person.items():
                        if side != "z0":
                            parts.add(f"crowd-{slot}", face)

            canopy_bottom = GRANDSTAND_CANOPY_BOTTOM_M
            for side in (-1, 1):
                outer = side * (half - GRANDSTAND_POST_IN_M)
                inner = outer - side * GRANDSTAND_POST_M
                post = box(min(inner, outer), max(inner, outer), depth - GRANDSTAND_POST_M, depth, GRANDSTAND_PLINTH_M, canopy_bottom)
                for face_side in SIDES:
                    parts.add("carbon", post[face_side])

            reach = half + GRANDSTAND_CANOPY_OVERHANG_M
            canopy = box(
                -reach, reach, -GRANDSTAND_CANOPY_OVERHANG_M, depth, canopy_bottom, canopy_bottom + GRANDSTAND_CANOPY_THICKNESS_M
            )
            parts.add("kerb-red", canopy["z1"])
            for side in (*SIDES, "z0"):
                parts.add("stand-canopy-shade", canopy[side])

    return parts.scenery()


def gantry(layout: Layout) -> list[SceneryMesh]:
    """The gantry spanning the asphalt just past the start line, its checkers facing both ways along the track."""
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

    parts = ByMaterial("Gantry")
    span = layout.half_width_at(sample.s) + GANTRY_BEYOND_ASPHALT_M
    half_depth = GANTRY_PILLAR_M / 2.0
    beam_bottom = GANTRY_TOP_M - GANTRY_BEAM_HEIGHT_M
    # The pillars stand on the sand's lowest edge so they never float on its slope.
    for side in (-1, 1):
        outer, inner = side * span, side * (span - GANTRY_PILLAR_M)
        pillar = box(min(inner, outer), max(inner, outer), -half_depth, half_depth, -GRASS_DROP_M, beam_bottom)
        # Lit on the faces up and down the track, the ones the grid sees.
        parts.add("carbon-lit", pillar["y0"])
        parts.add("carbon-lit", pillar["y1"])
        parts.add("carbon", pillar["x0"])
        parts.add("carbon", pillar["x1"])

    beam = box(-span, span, -half_depth, half_depth, beam_bottom, GANTRY_TOP_M)
    parts.add("beam-top", beam["z1"])
    parts.add("beam-shade", beam["z0"])
    for side in SIDES:
        parts.add("wall", beam[side])

    # Tiles a hair proud of both faces, so they never share the beam's
    # planes; only a tile's outer face is drawn.
    lift = SCENERY_LIFT_M
    shade_top = beam_bottom + GANTRY_BEAM_SHADE_M
    columns = round(2.0 * span / GANTRY_CHECKER_M)
    for behind, ahead, outer in ((-half_depth - lift, -half_depth, "y0"), (half_depth, half_depth + lift, "y1")):

        def flat(right: float, left: float, bottom: float, top: float) -> mesh.Mesh:
            return box(right, left, behind, ahead, bottom, top)[outer]

        parts.add("beam-shade", flat(-span, span, beam_bottom, shade_top))
        for row in range(GANTRY_CHECKER_ROWS):
            bottom = shade_top + row * GANTRY_CHECKER_M
            for column in range(columns):
                right = -span + column * GANTRY_CHECKER_M
                material = "checker-dark" if (row + column) % 2 == 0 else "checker-light"
                parts.add(material, flat(right, right + GANTRY_CHECKER_M, bottom, bottom + GANTRY_CHECKER_M))

    return parts.scenery()


def bridge_half_span(layout: Layout, bridge: Bridge) -> float:
    """Half the span's length along the raised leg: past the underpass's walls by the clearance,
    however squarely the legs cross."""
    deck = layout.path.at(bridge.deck_s)
    under = layout.path.at(bridge.underpass_s)
    clear = layout.runoff_half_width_at(bridge.underpass_s) + WALL_THICKNESS_M + BRIDGE_SPAN_CLEARANCE_M
    return clear / abs(math.sin(deck.heading - under.heading))


def bridges(layout: Layout, height: list[float]) -> list[SceneryMesh]:
    """Each bridge's embankment, down from the raised road's walls to the grass, its two abutments
    and the deck between them, with girders under a recessed soffit over the underpass."""
    path = layout.path
    ground = -GRASS_DROP_M
    parts = ByMaterial("Bridge")

    def rail(indices: list[int], share: float, lift: float | None = None) -> list[mesh.Vertex]:
        """`share` of the way from the centreline to the wall's outer foot, left when positive, at
        the road's level or at `lift` metres under it, or on the grass when `lift` is None."""
        return [
            (*path.samples[i].offset(share * edge), ground if lift is None else height[i] - GRASS_DROP_M - lift)
            for i in indices
        ]

    for bridge in layout.decor.bridges:
        # The raised leg is one width end to end: `uniform_widths` holds it to that.
        edge = layout.runoff_half_width_at(bridge.deck_s) + WALL_THICKNESS_M
        half_span = bridge_half_span(layout, bridge)
        # The span's ends land on samples, outward so it is never narrower than asked.
        span_start = path.index_at(bridge.deck_s - half_span - SAMPLE_STEP_M / 2.0)
        span_end = path.index_at(bridge.deck_s + half_span + SAMPLE_STEP_M / 2.0)
        banks = (
            path.indices_between(bridge.s_start, path.samples[span_start].s),
            path.indices_between(path.samples[span_end].s, bridge.s_end),
        )
        span = path.indices_between(path.samples[span_start].s, path.samples[span_end].s)
        for bank in banks:
            for side in (1, -1):
                top, foot = rail(bank, side, 0.0), rail(bank, side)
                parts.add("grass", mesh.ribbon(top, foot, closed=False) if side == 1 else mesh.ribbon(foot, top, closed=False))
        for end in (span_start, span_end):
            (lt,), (lf,) = rail([end], 1, 0.0), rail([end], 1)
            (rt,), (rf,) = rail([end], -1, 0.0), rail([end], -1)
            parts.add("wall", mesh.quad(lf, rf, rt, lt))
        for side in (1, -1):
            top, under = rail(span, side, 0.0), rail(span, side, BRIDGE_DECK_THICKNESS_M)
            parts.add("wall", mesh.ribbon(top, under, closed=False) if side == 1 else mesh.ribbon(under, top, closed=False))
        # Each girder's edges, as shares of the way from the centreline to a side; a ribbon from
        # a lower share to a higher faces down, and a side from the foot to the soffit faces up
        # the shares.
        soffit = BRIDGE_DECK_THICKNESS_M - BRIDGE_SOFFIT_RECESS_M
        width = BRIDGE_GIRDER_WIDTH_M / edge
        pitch = (2.0 - width) / (BRIDGE_GIRDERS - 1)
        girders = [(-1.0 + k * pitch, -1.0 + k * pitch + width) for k in range(BRIDGE_GIRDERS)]
        for (_, near), (far, _) in zip(girders, girders[1:]):
            parts.add("carbon-shade", mesh.ribbon(rail(span, near, soffit), rail(span, far, soffit), closed=False))
        for k, (low, high) in enumerate(girders):
            parts.add("carbon", mesh.ribbon(rail(span, low, BRIDGE_DECK_THICKNESS_M), rail(span, high, BRIDGE_DECK_THICKNESS_M), closed=False))
            # The outer girders' outer faces are the deck's sides.
            if k > 0:
                parts.add("carbon", mesh.ribbon(rail(span, low, soffit), rail(span, low, BRIDGE_DECK_THICKNESS_M), closed=False))
            if k < BRIDGE_GIRDERS - 1:
                parts.add("carbon", mesh.ribbon(rail(span, high, BRIDGE_DECK_THICKNESS_M), rail(span, high, soffit), closed=False))
    return parts.scenery()


def tunnels(layout: Layout, height: list[float]) -> list[SceneryMesh]:
    """Each tunnel's two sides, standing against the back of the walls from the grass to the top
    of the roof, the roof between them with its underside at `TUNNEL_ROOF_M` over the road, its
    two portal faces, and a lamp every `TUNNEL_LAMP_SPACING_M` along each side's inside."""
    path = layout.path
    roof_top = TUNNEL_ROOF_M + TUNNEL_ROOF_THICKNESS_M
    parts = ByMaterial("Tunnel")

    def rail(indices: list[int], offset: float, up: float) -> list[mesh.Vertex]:
        return [(*path.samples[i].offset(offset), height[i] + up) for i in indices]

    for tunnel in layout.decor.tunnels:
        # A hair off the wall's back, so the side never shares its plane; one width end to end,
        # since `uniform_widths` holds the tunnel to that.
        inside = layout.runoff_half_width_at(tunnel.s_start) + WALL_THICKNESS_M + SCENERY_LIFT_M
        outside = inside + TUNNEL_SIDE_THICKNESS_M
        indices = path.indices_between(tunnel.s_start, tunnel.s_end)
        for side in (1, -1):
            parts.add(
                "beam-shade",
                mesh.box_rail(
                    rail(indices, side * inside, -GRASS_DROP_M),
                    rail(indices, side * outside, -GRASS_DROP_M),
                    roof_top + GRASS_DROP_M,
                ),
            )
        parts.add("carbon", mesh.ribbon(rail(indices, outside, roof_top), rail(indices, -outside, roof_top), closed=False))
        parts.add("carbon-shade", mesh.ribbon(rail(indices, -inside, TUNNEL_ROOF_M), rail(indices, inside, TUNNEL_ROOF_M), closed=False))
        for end, outward in ((indices[0], -1.0), (indices[-1], 1.0)):
            (lb,), (rb,) = rail([end], inside, TUNNEL_ROOF_M), rail([end], -inside, TUNNEL_ROOF_M)
            (lt,), (rt,) = rail([end], inside, roof_top), rail([end], -inside, roof_top)
            heading = path.samples[end].heading
            ahead = (outward * math.cos(heading), outward * math.sin(heading), 0.0)
            parts.add("carbon", mesh.quad_facing(lb, rb, rt, lt, ahead))

        half_long, half_tall = TUNNEL_LAMP_LENGTH_M / 2.0, TUNNEL_LAMP_TALL_M / 2.0
        s = tunnel.s_start + TUNNEL_LAMP_SPACING_M / 2.0
        while s < tunnel.s_end:
            sample = path.at(s)
            cos, sin = math.cos(sample.heading), math.sin(sample.heading)
            z = elevation(layout.elevation, s) + TUNNEL_LAMP_HEIGHT_M
            for side in (1, -1):
                x, y = sample.offset(side * (inside - TUNNEL_LAMP_PROUD_M))
                a, b, c, d = (
                    (x + cos * along, y + sin * along, z + up)
                    for along, up in ((-half_long, -half_tall), (half_long, -half_tall), (half_long, half_tall), (-half_long, half_tall))
                )
                # Lit toward the road, the way the side's inside faces.
                toward_road = (side * sin, -side * cos, 0.0)
                parts.add("lamp", mesh.quad_facing(a, b, c, d, toward_road))
            s += TUNNEL_LAMP_SPACING_M
    return parts.scenery()

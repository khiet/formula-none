"""The plain oval: a test fixture, not a race track.

Two 125 m straights joined by 30 m radius turns, run counter-clockwise, so
the lap is 2 * 125 + 2 * pi * 30 = 438.5 m. The start line is at s = 0 on
the right-hand straight, 25 m after the second turn, and the straight runs
along +Y (glTF -Z), so karts spawn with identity rotation. Sim tests rely
on the numbers below:

- the front straight is clear for 100 m past the start line, enough for a
  kart to reach top speed and coast to a stop;
- one tyre wall stands 8 m into the first turn, centred on spawn slot 0's
  column, so a kart that drives straight from slot 0 meets it head on;
- the walls stand 12 m from the centreline, so a kart that drives straight
  from slot 1 scrapes along the first turn's outside;
- the front straight's boost pad covers the right half of the asphalt, 60 m
  past the line, so a kart from slot 1 crosses it and one from slot 0 does
  not; the back straight's pad is centred 45 m in, far enough before the
  second turn that a pad boost has run out by the turn.
"""

from ..layout import GRID_COLUMN_OFFSET_M, BoostPad, ItemRow, Layout, TyreWall
from ..path import Sample, Turtle

STRAIGHT_M = 125.0
LINE_AFTER_TURN_M = 25.0
TURN_RADIUS_M = 30.0
TYRE_WALL_INTO_TURN_M = 8.0
LEFT_GRID_COLUMN_X = TURN_RADIUS_M - GRID_COLUMN_OFFSET_M
# Where the front straight's pad is centred: halfway across the right half of
# the asphalt, so with the sim's tuned pad width it covers that half.
FRONT_PAD_RIGHT_OF_CENTRE_M = 3.0


def offset_reaching_x(sample: Sample, x: float) -> float:
    """Metres left of `sample` at which the offset point has the given x."""
    return (x - sample.x) / sample.left_normal[0]


path = (
    Turtle(x=TURN_RADIUS_M, y=0.0, heading_deg=90.0)
    .straight("front-out", STRAIGHT_M - LINE_AFTER_TURN_M)
    .arc("turn-1", TURN_RADIUS_M, 180.0)
    .straight("back", STRAIGHT_M)
    .arc("turn-2", TURN_RADIUS_M, 180.0)
    .close("front-in")
)

tyre_wall_1_s = path.segment("turn-1").s_start + TYRE_WALL_INTO_TURN_M

LAYOUT = Layout(
    name="oval",
    path=path,
    asphalt_half_width=6.0,
    runoff_width=6.0,
    checkpoint_count=12,
    tyre_walls=[
        TyreWall(tyre_wall_1_s, offset=offset_reaching_x(path.at(tyre_wall_1_s), LEFT_GRID_COLUMN_X)),
        TyreWall(path.segment("turn-2").s_start + TYRE_WALL_INTO_TURN_M, offset=1.0),
    ],
    item_rows=[
        ItemRow(path.segment("back").s_start + 30.0),
        ItemRow(path.segment("back").s_start + 60.0),
    ],
    boost_pads=[
        BoostPad(path.segment("back").s_start + 45.0),
        BoostPad(path.segment("front-out").s_start + 60.0, offset=-FRONT_PAD_RIGHT_OF_CENTRE_M),
    ],
)

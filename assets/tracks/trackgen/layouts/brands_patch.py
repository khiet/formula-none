"""Brands Patch: the race track.

A counter-clockwise loop of about 800 m: the start/finish straight up the
east side, hairpin one at the top, a run back down into a right-hander, the
hill on the westbound straight, the chicane, hairpin two at the west end, the
jump ramp on the long eastbound straight and a sweeping final corner. Pipes
stand on the inside of both hairpins. The start line is at s = 0 on the
east straight, which runs along +Y (glTF -Z). The scenery: grandstands and
sponsor hoardings along the outside of the start straight and the return,
hoardings round the final corner, and the gantry over the line.
"""

from ..layout import BoostPad, Decor, Grandstands, Hoardings, ItemRow, Layout, Pipe, Wall
from ..path import Hill, Ramp, Turtle

EAST_STRAIGHT_X = 80.0
HAIRPIN_RADIUS_M = 18.0
CHICANE_RADIUS_M = 14.0

path = (
    Turtle(x=EAST_STRAIGHT_X, y=0.0, heading_deg=90.0)
    .straight("start", 70.0)
    .arc("hairpin-1", HAIRPIN_RADIUS_M, 180.0)
    .straight("return", 100.0)
    .arc("corner-3", 22.0, -90.0)
    .straight("hill", 110.0)
    .arc("chicane-in", CHICANE_RADIUS_M, 45.0)
    .arc("chicane-mid", CHICANE_RADIUS_M, -90.0)
    .arc("chicane-out", CHICANE_RADIUS_M, 45.0)
    .straight_until_x("approach", -150.0)
    .arc("hairpin-2", HAIRPIN_RADIUS_M, 180.0)
    .straight_until_x("jump", EAST_STRAIGHT_X - 34.0)
    .arc("final", 34.0, 90.0)
    .close("grid")
)

HILL_CREST_M = 2.0
RAMP_HEIGHT_M = 1.5
RAMP_LENGTH_M = 12.0

hill = path.segment("hill")
jump = path.segment("jump")
ramp_start = jump.s_start + 70.0

LAYOUT = Layout(
    name="brands-patch",
    path=path,
    asphalt_half_width=6.0,
    runoff_width=6.0,
    checkpoint_count=12,
    # Hairpin one's 57 m arc falls between two even gates 36 m apart across
    # the infield grass; the gate at its apex makes the lap go round it.
    apex_gates=("hairpin-1",),
    walls=[
        Wall(("grid", "start"), side=-1),
        Wall(("hairpin-1",), side=-1),
        Wall(("corner-3",), side=1),
        Wall(("hairpin-2",), side=-1),
        Wall(("final",), side=-1),
    ],
    pipes=[
        Pipe(path.segment("hairpin-1").s_mid - 8.0, offset=4.5),
        Pipe(path.segment("hairpin-1").s_mid + 8.0, offset=4.5),
        Pipe(path.segment("hairpin-2").s_mid - 8.0, offset=4.5),
        Pipe(path.segment("hairpin-2").s_mid + 8.0, offset=4.5),
    ],
    item_rows=[
        ItemRow(hill.s_start + 15.0),
        ItemRow(jump.s_start + 30.0),
    ],
    boost_pads=[
        BoostPad(path.segment("return").s_mid),
        BoostPad(ramp_start + RAMP_LENGTH_M + 40.0),
    ],
    elevation=[
        Hill(hill.s_start + 30.0, hill.s_end - 30.0, HILL_CREST_M),
        Ramp(ramp_start, ramp_start + RAMP_LENGTH_M, RAMP_HEIGHT_M),
    ],
    decor=Decor(
        hoardings=[
            Hoardings(("grid", "start"), side=-1),
            Hoardings(("return",), side=1),
            Hoardings(("final",), side=-1),
        ],
        grandstands=[Grandstands(("grid", "start"), side=-1)],
        gantry=True,
    ),
)

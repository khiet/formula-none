"""Suzuqa Patch: the race track, a figure eight of about 1306 m after Suzuka.

It keeps the `brands-patch` id it had as Brands Patch. The start/finish
straight runs south-east into the first curve and turn 2, then the esses,
Dunlop and the two Degners; the run to the hairpin passes under the bridge,
then the 200R, the double-apex Spoon and the back straight, which climbs
over the bridge before 130R, the chicane and the final curve. Tyre walls
stand outside turn 2 and inside the hairpin and Spoon's second apex, sausage kerbs
inside esses 3 and the chicane, the runaway wheel crosses the run from the
bridge to the hairpin, its marshal post on the outside, and the jump sits at
Spoon's exit. The scenery: grandstands along the outside of the start
straight and the hairpin exit, hoardings round Spoon, 130R and the final
curve, the gantry over the line, the bridge where the course crosses
itself, woods along the back section and the Circuit Wheel outside the
first curve.
"""

from ..layout import (
    BoostPad,
    Bridge,
    Decor,
    Disc,
    Grandstands,
    Hoardings,
    ItemRow,
    Kerb,
    Layout,
    Line,
    Prop,
    Spot,
    TyreWall,
    WheelCrossing,
    Wood,
)
from ..path import Hill, Ramp, Turtle

# Just in from the painted kerb on the inside of a corner.
KERB_INSIDE_M = 4.9
# Left of the centreline, where every tyre wall stands clear of the bots' line.
TYRE_WALL_LEFT_M = 4.5

path = (
    Turtle(x=0.0, y=0.0, heading_deg=-50.0)
    .straight("start", 60.0)
    .arc("first-curve", 50.0, -51.0)
    .straight("turn-2-approach", 26.0)
    .arc("turn-2", 21.0, -111.0)
    .straight("esses-run", 27.0)
    .arc("esses-1", 30.0, 57.0)
    .arc("esses-2", 20.0, -71.0)
    .straight("esses-link", 9.0)
    .arc("esses-3", 21.0, 55.0)
    .arc("esses-4", 21.0, -122.0)
    .arc("dunlop", 34.0, 102.0)
    .arc("dunlop-rise", 41.0, 58.0)
    .arc("degner-1", 24.0, -70.0)
    .straight("degner-link", 53.0)
    .arc("degner-2", 18.0, -75.0)
    .straight("underpass", 85.3)
    .arc("hairpin-kink", 43.0, -28.0)
    .straight("hairpin-approach", 12.0)
    .arc("hairpin", 17.0, 180.0)
    .straight("hairpin-exit", 21.0)
    .arc("two-hundred-r", 43.0, -99.0)
    .straight("spoon-entry", 8.0)
    .arc("spoon-1", 17.0, 109.0)
    .straight("spoon-link", 20.0)
    .arc("spoon-2", 29.0, 108.0)
    .straight("back-straight", 190.14)
    .arc("one-thirty-r", 52.0, 40.0)
    .straight("chicane-approach", 13.0)
    .arc("chicane-in", 12.0, -76.0)
    .arc("chicane-out", 13.0, 86.0)
    .arc("final-curve", 40.0, -92.0)
    .close("grid")
)

# Where the back straight passes over the underpass, metres along each leg.
BRIDGE_UNDERPASS_S = 573.8
BRIDGE_BACK_STRAIGHT_S = 1020.7
BRIDGE_DECK_HEIGHT_M = 5.0
BRIDGE_HILL_HALF_LENGTH_M = 55.0
RAMP_HEIGHT_M = 1.5
RAMP_LENGTH_M = 12.0

back = path.segment("back-straight")
hairpin = path.segment("hairpin")
ramp_start = back.s_start + 14.0
# The road's climb over the bridge, which the embankment stands under.
bridge_hill = Hill(
    BRIDGE_BACK_STRAIGHT_S - BRIDGE_HILL_HALF_LENGTH_M,
    BRIDGE_BACK_STRAIGHT_S + BRIDGE_HILL_HALF_LENGTH_M,
    BRIDGE_DECK_HEIGHT_M,
)

LAYOUT = Layout(
    name="brands-patch",
    path=path,
    asphalt_half_width=6.0,
    runoff_width=6.0,
    # Fifteen keeps every checkpoint at least 24 m from the bridge on both legs.
    checkpoint_count=15,
    tyre_walls=[
        # Outside turn 2: a right-hander's inside is on the bots' line.
        TyreWall(path.segment("turn-2").s_mid, offset=TYRE_WALL_LEFT_M),
        TyreWall(hairpin.s_mid - 8.0, offset=TYRE_WALL_LEFT_M),
        TyreWall(hairpin.s_mid + 8.0, offset=TYRE_WALL_LEFT_M),
        TyreWall(path.segment("spoon-2").s_mid, offset=TYRE_WALL_LEFT_M),
    ],
    item_rows=[
        ItemRow(path.segment("esses-run").s_mid),
        ItemRow(path.segment("degner-link").s_mid),
        ItemRow(back.s_end - 15.0),
    ],
    boost_pads=[
        BoostPad(path.segment("dunlop-rise").s_mid),
        BoostPad(path.segment("hairpin-exit").s_mid, offset=3.0),
    ],
    kerbs=[
        Kerb(path.segment("esses-3").s_mid, offset=KERB_INSIDE_M),
        Kerb(path.segment("chicane-in").s_mid, offset=-KERB_INSIDE_M),
    ],
    # The hairpin turns left, so its outside is the right.
    wheel_crossings=[WheelCrossing(BRIDGE_UNDERPASS_S + 28.0, outside=-1)],
    elevation=[
        Ramp(ramp_start, ramp_start + RAMP_LENGTH_M, RAMP_HEIGHT_M),
        bridge_hill,
    ],
    decor=Decor(
        hoardings=[
            Hoardings(("grid", "start"), side=1),
            Hoardings(("spoon-2",), side=-1),
            Hoardings(("one-thirty-r",), side=-1),
            Hoardings(("final-curve",), side=1),
        ],
        grandstands=[
            Grandstands(("grid", "start"), side=1),
            Grandstands(("hairpin-exit",), side=-1),
        ],
        gantry=True,
        bridges=[
            Bridge(
                deck_s=BRIDGE_BACK_STRAIGHT_S,
                underpass_s=BRIDGE_UNDERPASS_S,
                s_start=bridge_hill.s_start,
                s_end=bridge_hill.s_end,
            )
        ],
        # The Circuit Wheel outside the first curve, where the start straight
        # points: docs/mockups/trackside-objects.html's spot.
        props=[Prop("circuit-wheel", Spot(s=86.0, offset=34.0))],
        # The patches, counts and seeds of docs/mockups/trackside-objects.html.
        woods=[
            # Pines outside Degner.
            Wood("pine", Disc(-215.0, -70.0, 35.0), count=40, seed=1),
            # Round trees on the slope between Dunlop and 130R.
            Wood("round", Disc(-150.0, 5.0, 12.0), count=15, seed=2),
            # Round trees in the 200R pocket.
            Wood("round", Disc(-300.0, 72.0, 14.0), count=12, seed=3),
            # Pines outside the hairpin.
            Wood("pine", Disc(-230.0, 190.0, 25.0), count=25, seed=4),
            # Pines beyond Spoon.
            Wood("pine", Disc(-400.0, 100.0, 25.0), count=30, seed=5),
            # Round trees in the middle of the east infield.
            Wood("round", Disc(-60.0, -45.0, 20.0), count=15, seed=6),
            # The cherry row outside the esses.
            Wood("cherry", Line((-5.0, -150.0), (-105.0, -130.0)), count=12, seed=7),
        ],
    ),
)

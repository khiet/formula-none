"""Bell Deer: the race track, a figure eight of about 1306 m after Suzuka.

The start/finish straight runs south-east into the first curve and turn 2,
then the esses, Dunlop and the two Degners; the run to the hairpin passes under the bridge,
then the 200R, the double-apex Spoon and the back straight, which climbs
over the bridge before 130R, the chicane and the final curve. Tyre walls
stand outside turn 2 and inside the hairpin and Spoon's second apex, sausage kerbs
inside esses 3 and the chicane, the runaway wheel crosses the run from the
bridge to the hairpin, its marshal post on the outside, and a ramp on the
left half of the road at Spoon's exit, a pad leading into it and the right half flat. The scenery: grandstands along the outside of the start
straight and the hairpin exit, hoardings round Spoon, 130R and the final
curve, the gantry over the line, the bridge where the course crosses
itself, woods along the back section, the Circuit Wheel outside the
first curve, the tyre arch over the middle of Dunlop, the pit building
the grid faces across the start straight and the airship circling over
the east infield. The pit lane leaves the inside of the final curve, runs
between the pit wall and the pit building and rejoins inside the first curve.
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
    PitLane,
    Prop,
    Spot,
    CoinLine,
    TyreWall,
    WheelCrossing,
    Wood,
    World,
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
# Low enough that a kart off the pad lands while its boost still runs (ADR 0101).
RAMP_HEIGHT_M = 0.5
RAMP_LENGTH_M = 8.0

back = path.segment("back-straight")
hairpin = path.segment("hairpin")
# Past the checkpoint a kart would otherwise be in the air over.
ramp_start = back.s_start + 36.0
# The back straight's row, this far along it.
BACK_STRAIGHT_ROW_M = 80.0
# The road's climb over the bridge, which the embankment stands under.
bridge_hill = Hill(
    BRIDGE_BACK_STRAIGHT_S - BRIDGE_HILL_HALF_LENGTH_M,
    BRIDGE_BACK_STRAIGHT_S + BRIDGE_HILL_HALF_LENGTH_M,
    BRIDGE_DECK_HEIGHT_M,
)

LAYOUT = Layout(
    name="bell-deer",
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
        # Past where a kart boosted off the ramp with a full purse lands, and
        # clear of the bridge, so the item lands before 130R (ADR 0097).
        ItemRow(back.s_start + BACK_STRAIGHT_ROW_M),
    ],
    boost_pads=[
        BoostPad(path.segment("dunlop-rise").s_mid),
        BoostPad(path.segment("hairpin-exit").s_mid, offset=3.0),
        # The run-up to the ramp, on its half of the road.
        BoostPad(ramp_start - 2.0, offset=3.0),
    ],
    # Ten on the centreline, down the start straight and through the
    # underpass short of the wheel's crossing, so a clean lap takes a full
    # set; five off it, out wide before turn 2 and between the Spoon's two
    # curves, for a wider line.
    coin_lines=[
        CoinLine(path.segment("start").s_start + 10.0, count=5),
        CoinLine(path.segment("underpass").s_start + 6.0, count=5),
        CoinLine(path.segment("turn-2-approach").s_start + 6.0, count=2, offset=-4.0),
        # Out wide between the Spoon's apexes, so the shorter inside line stays
        # clear and away from spoon-2's tyre wall.
        CoinLine(path.segment("spoon-link").s_start + 6.0, count=3, offset=-4.0),
    ],
    kerbs=[
        Kerb(path.segment("esses-3").s_mid, offset=KERB_INSIDE_M),
        Kerb(path.segment("chicane-in").s_mid, offset=-KERB_INSIDE_M),
    ],
    # The hairpin turns left, so its outside is the right.
    wheel_crossings=[WheelCrossing(BRIDGE_UNDERPASS_S + 28.0, outside=-1)],
    elevation=[bridge_hill],
    # The inside of Spoon's exit: a left-hander's exit runs out to the right,
    # along the flat half, and the bots' line stays there too.
    ramps=[Ramp(ramp_start, ramp_start + RAMP_LENGTH_M, RAMP_HEIGHT_M, right=0.0, left=6.0)],
    # 30 m of fork each end, inside curves that already lean a kart that way,
    # and 140 m between the lines (ADR 0081).
    pit_lane=PitLane(side=-1, s_fork=-80.0, s_entry=-50.0, s_exit=90.0, s_merge=120.0),
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
        # points, the arch over Dunlop's middle, the pit building inside the
        # start straight and the airship circling the east infield over Dunlop
        # and the esses: docs/mockups/trackside-objects.html's spots.
        props=[
            Prop("circuit-wheel", Spot(s=86.0, offset=34.0)),
            Prop("arch", Spot(s=path.segment("dunlop").s_mid)),
            Prop("pits", Spot(s=0.0, offset=-26.0), length=90.0),
            Prop("airship", World(-60.0, -40.0), radius=60.0, height=45.0),
        ],
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

"""Goldstone: a flat, fast, clockwise circuit of about 1323 m after Silverstone.

The grid sits on the Hamilton Straight by the pits. The lap runs through
Abbey and Farm to Village, The Loop and Aintree, down the Wellington Straight
to Brooklands and the long right of Luffield, past Woodcote onto the National
Straight, then Copse, the Maggotts and Becketts esses and Chapel, the Hangar
Straight to Stowe, and Vale and Club back onto the grid. The ground is an old
airfield, so there is no elevation; the lap is about speed and the esses.
The scenery: the pits white inside the start straight, between Club and
Abbey as Silverstone's are, with the grandstands across from them, hoardings round Luffield, Copse, Becketts and
Stowe, the gantry over the line, a row of airfield hangars outside the
Hangar Straight, and the display jets' flypast over them. The layout is docs/mockups/silverstone-course.html's.
"""

from ..layout import (
    BoostPad,
    Decor,
    Grandstands,
    Hoardings,
    ItemRow,
    Kerb,
    Layout,
    Prop,
    Spot,
    CoinLine,
    PitLane,
    PITS_DOORS_CENTRED_M,
    TyreWall,
    WheelCrossing,
)
from ..path import Turtle

# Just in from the painted kerb on the inside of a corner.
KERB_INSIDE_M = 4.9
# Left of the centreline, where every tyre wall stands clear of the bots' line.
TYRE_WALL_LEFT_M = 4.5
# Where the pits' middle stands, its doors centred on it: the longest building
# the infield between Club and Abbey holds by the 8 m rule stands here.
PITS_S = -6.0

path = (
    Turtle(x=0.0, y=0.0, heading_deg=52.5)
    .straight("start", 29.0)
    .arc("abbey", 25.0, -57.0)
    .straight("abbey-exit", 13.0)
    .arc("farm", 33.0, 39.0)
    .straight("village-approach", 31.0)
    .arc("village", 15.0, -103.0)
    .arc("the-loop", 12.0, 134.0)
    .arc("aintree", 39.0, 64.0)
    .straight("wellington", 145.0)
    .arc("brooklands", 17.0, 113.0)
    .straight("brooklands-exit", 10.0)
    .arc("luffield", 18.0, -193.0)
    .straight("woodcote-approach", 18.0)
    .arc("woodcote", 47.0, -54.0)
    .straight("national", 82.0)
    .arc("copse", 22.0, -66.0)
    .straight("copse-exit", 80.0)
    .arc("maggotts", 47.0, 22.0)
    .arc("becketts-right", 27.0, -46.0)
    .arc("becketts-left", 20.0, 56.0)
    .arc("becketts", 24.0, -95.0)
    .arc("chapel", 39.0, 31.0)
    .straight("hangar", 175.74)
    .arc("stowe", 37.0, -125.0)
    .straight("vale-approach", 102.0)
    .arc("vale", 15.0, 72.0)
    .arc("club", 20.0, -152.0)
    .close("grid")
)

seg = path.segment

LAYOUT = Layout(
    name="goldstone",
    path=path,
    asphalt_half_width=6.0,
    runoff_width=6.0,
    checkpoint_count=15,
    tyre_walls=[
        # Outside the right-handers: their inside is on the bots' line.
        TyreWall(seg("luffield").s_mid - 10.0, offset=TYRE_WALL_LEFT_M),
        TyreWall(seg("luffield").s_mid + 10.0, offset=TYRE_WALL_LEFT_M),
        TyreWall(seg("stowe").s_mid, offset=TYRE_WALL_LEFT_M),
    ],
    item_rows=[
        ItemRow(seg("wellington").s_mid),
        ItemRow(seg("national").s_mid),
        ItemRow(seg("copse-exit").s_mid),
    ],
    boost_pads=[
        BoostPad(seg("abbey-exit").s_mid, offset=3.0),
        BoostPad(seg("hangar").s_start + 40.0),
    ],
    # Ten on the centreline, off the grid and down the Hangar Straight past
    # its boost pad, so a clean lap takes a full set; three off it, out
    # wide on the run to Vale for a wider entry, short of the wheel's
    # crossing.
    coin_lines=[
        CoinLine(seg("start").s_start + 10.0, count=5),
        CoinLine(seg("hangar").s_mid + 30.0, count=5),
        CoinLine(seg("vale-approach").s_start + 12.0, count=3, offset=-4.0),
    ],
    kerbs=[
        Kerb(seg("the-loop").s_mid, offset=KERB_INSIDE_M),
        Kerb(seg("becketts-right").s_mid, offset=-KERB_INSIDE_M),
        Kerb(seg("becketts-left").s_mid, offset=KERB_INSIDE_M),
        Kerb(seg("becketts").s_mid, offset=-KERB_INSIDE_M),
        Kerb(seg("vale").s_mid, offset=KERB_INSIDE_M),
    ],
    # Vale turns left, so the approach's outside is the right.
    wheel_crossings=[WheelCrossing(seg("vale-approach").s_mid, outside=-1)],
    elevation=[],
    # Right of the Hamilton Straight, as Silverstone's lane is (ADR 0089): the
    # fork at Club's start and the sweep round its inside, where a bot called in
    # slows for the lane's turn; past the line round the inside of Abbey, and
    # the merge on the run to Village. 140 m between the lines (ADR 0081).
    pit_lane=PitLane(
        side=-1,
        s_fork=-108.0,
        s_entry=-52.0,
        s_exit=92.0,
        s_merge=115.0,
        s_building=PITS_S,
        door_first=PITS_DOORS_CENTRED_M,
    ),
    decor=Decor(
        hoardings=[
            Hoardings(("luffield",), side=1),
            Hoardings(("copse",), side=1),
            Hoardings(("becketts",), side=1),
            Hoardings(("stowe",), side=1),
        ],
        grandstands=[
            # Outside the Hamilton Straight: the pits stand inside it, facing the grid.
            Grandstands(("grid", "start"), side=1),
            Grandstands(("luffield",), side=-1),
            Grandstands(("copse-exit",), side=-1),
            Grandstands(("vale-approach",), side=1),
        ],
        gantry=True,
        # The pits drawn white inside the Hamilton Straight, a nod to the Wing,
        # 70 m so they keep the 8 m rule from Club and Abbey (ADR 0089). A row
        # of five airfield hangars outside the Hangar Straight, fronts 21 m
        # from the centreline as the pits' are.
        props=[
            Prop("pits", Spot(s=PITS_S, offset=-26.0), length=70.0, white=True),
            Prop("hangars", Spot(s=seg("hangar").s_mid, offset=33.0), length=110.0),
            # The display jets' flypast over the hangars, against the race so drivers on the
            # straight see them coming.
            Prop("jets", Spot(s=seg("hangar").s_mid, offset=33.0), length=600.0, height=30.0),
        ],
    ),
)

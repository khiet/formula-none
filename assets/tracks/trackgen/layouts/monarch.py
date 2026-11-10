"""Monarch: a slow, walled-in, clockwise street circuit of about 1301 m after Monaco.

The start/finish straight runs north into Sainte Devote, then the climb up
Beau Rivage to Massenet and Casino Square, down to Mirabeau, round the
Grand Hotel hairpin, Mirabeau Bas and Portier, through the tunnel to the
chicane, along the harbour to Tabac, the two swimming pool chicanes, the
quay, La Rascasse and Anthony Noghes. The run-off is a 2 m strip, so the
walls stand close as they do in the streets. The road is 12 m wide but for
Mirabeau to Mirabeau Bas and La Rascasse, narrowed toward Monaco's own,
with the run-off and the walls following its edge in. The road climbs 8 m from
Sainte Devote to its crest at Casino Square and falls again by the tunnel;
there is no ramp, since one on a street circuit lands against a wall. The
layout is docs/mockups/monaco-course.html's.
"""

import math

from ..layout import (
    BoostPad,
    CoinLine,
    Decor,
    Grandstands,
    Hoardings,
    ItemRow,
    Kerb,
    PITS_DOORS_CENTRED_M,
    Layout,
    PitLane,
    Prop,
    Spot,
    Taper,
    Tunnel,
    TyreWall,
    WheelCrossing,
    World,
)
from ..path import Hill, Turtle

# Just in from the painted kerb on the inside of a corner.
KERB_INSIDE_M = 4.9
# Left of the centreline, where every tyre wall stands clear of the bots' line.
TYRE_WALL_LEFT_M = 4.5
CREST_HEIGHT_M = 8.0
# Where the pits' middle stands: far enough on that its back keeps the 8 m rule
# from the second pool chicane's wall behind it.
PITS_S = 34.0
# The harbour quay's outer edge, counter-clockwise, in metres east and north
# of the harbour prop's place: the north edge behind the harbour straight's
# stands, the west edge behind the pool's, then the quay and Rascasse.
HARBOUR_OUTLINE = (
    (100.0, 98.0),
    (-65.0, 98.0),
    (-65.0, 12.0),
    (-89.0, 4.0),
    (-91.0, -15.0),
    (-83.0, -35.0),
    (-73.5, -55.0),
    (-77.5, -70.0),
    (-88.5, -85.0),
    (-104.0, -95.0),
    (100.0, -95.0),
)
# The casino's middle, this far left of the centreline, puts its front 10 m
# behind the wall, past the 8 m rule.
CASINO_LEFT_M = 28.5
# The town's blocks on the hillside behind the climb, in rows stepping up
# away from it: each as (segment, share of it, metres left of the
# centreline, length, height). Every front keeps the 8 m rule; the rows on
# Beau Rivage leave a wedge open from the climb's chase cameras to the
# casino's dome, so the blocks frame it, and the two on each side of the
# Casino approach line the road up to it.
CLIMB_ROWS = (
    # Behind the foot of Beau Rivage, west of the wedge.
    ("beau-rivage", 0.05, 24.0, 16.0, 11.0),
    ("beau-rivage", 0.03, 42.0, 18.0, 15.0),
    ("beau-rivage", 0.14, 42.0, 14.0, 14.0),
    ("beau-rivage", 0.04, 60.0, 20.0, 19.0),
    ("beau-rivage", 0.15, 62.0, 14.0, 18.0),
    # Behind the top of Beau Rivage, east of it, between the climb and the casino.
    ("beau-rivage", 0.72, 24.0, 14.0, 10.0),
    ("beau-rivage", 0.81, 24.0, 14.0, 12.0),
    ("beau-rivage", 0.92, 24.0, 14.0, 11.0),
    ("beau-rivage", 0.84, 42.0, 14.0, 15.0),
    ("beau-rivage", 0.94, 42.0, 12.0, 14.0),
    ("beau-rivage", 0.93, 60.0, 14.0, 18.0),
    # Either side of the Casino approach.
    ("casino-approach", 0.2, 24.0, 16.0, 12.0),
    ("casino-approach", 0.65, 24.0, 16.0, 14.0),
    ("casino-approach", 0.2, -24.0, 18.0, 13.0),
    ("casino-approach", 0.7, -24.0, 16.0, 11.0),
)
# Round the harbour's far quays, fronts to the water: (x, y, heading, length,
# height) in the world, the heading's left the front.
SHORE_BLOCKS = (
    (258.0, -60.0, math.pi / 2.0, 20.0, 16.0),
    (258.0, -30.0, math.pi / 2.0, 16.0, 12.0),
    (258.0, 0.0, math.pi / 2.0, 18.0, 18.0),
    (258.0, 30.0, math.pi / 2.0, 16.0, 13.0),
    (120.0, -133.0, 0.0, 18.0, 14.0),
    (150.0, -133.0, 0.0, 16.0, 11.0),
    (180.0, -133.0, 0.0, 20.0, 17.0),
    (210.0, -133.0, 0.0, 16.0, 12.0),
)
# The pool, west of the pool chicanes between their wall and the pits' back,
# its long side toward them along the start straight's heading.
POOL_AT = World(33.0, 20.0, math.radians(98.0) - math.pi)
POOL_LENGTH_M = 30.0
# The roof starts past the tunnel mouth and ends short of the chicane, so
# both portals stand on the flat.
TUNNEL_PAST_MOUTH_M = 10.0
TUNNEL_SHORT_OF_EXIT_M = 15.0
# The road narrows from its 12 m toward Monaco's own, about 7.5 m at its tightest
# (ADR 0106): 9 m from Mirabeau to the hairpin's exit and at Rascasse, 8 m round
# the Grand Hotel hairpin, and 10 m at Mirabeau Bas, where a boosted kart at 9 m
# cleared the wall and flew past Portier's checkpoint. Each narrowing tapers in
# over this much of the road before it.
NARROW_HALF_WIDTH_M = 4.5
HAIRPIN_HALF_WIDTH_M = 4.0
MIRABEAU_BAS_HALF_WIDTH_M = 5.0
TAPER_IN_M = 15.0
# The hairpin closes the last half metre over its own first and last few metres,
# and Mirabeau Bas opens its half metre over its first few.
HALF_METRE_TAPER_M = 5.0

path = (
    Turtle(x=0.0, y=0.0, heading_deg=98.0)
    .straight("start", 93.0)
    .arc("sainte-devote", 13.0, -84.0)
    .straight("beau-rivage", 163.12)
    .arc("massenet", 37.0, 93.0)
    .straight("casino-approach", 39.0)
    .arc("casino", 26.0, -84.0)
    .straight("casino-descent", 52.0)
    .arc("mirabeau", 14.0, -140.0)
    .arc("mirabeau-kink", 25.0, 27.0)
    .straight("hairpin-approach", 14.0)
    .arc("grand-hotel", 14.0, 195.0)
    .straight("hairpin-exit", 7.0)
    .arc("mirabeau-bas", 13.0, -104.0)
    .straight("portier-approach", 10.0)
    .arc("portier", 13.0, -97.0)
    .straight("tunnel-entry", 34.0)
    .arc("tunnel", 122.0, -81.0)
    .arc("chicane-in", 14.0, 69.0)
    .arc("chicane-out", 12.0, -97.0)
    .arc("chicane-exit", 15.0, 25.0)
    .straight("harbour", 69.0)
    .arc("tabac", 17.0, 54.0)
    .arc("tabac-exit", 100.0, 27.0)
    .arc("pool-1-left", 13.0, 47.0)
    .arc("pool-1-right", 22.0, -45.0)
    .straight("pool-straight", 17.0)
    .arc("pool-2-right", 12.0, -71.0)
    .arc("pool-2-left", 12.0, 73.0)
    .arc("quay", 90.0, 39.0)
    .arc("rascasse", 12.0, -134.0)
    .straight("rascasse-exit", 15.0)
    .arc("noghes", 14.0, -78.0)
    .arc("noghes-flick", 48.0, 27.0)
    .arc("pit-bend", 40.0, -21.0)
    .close("grid")
)

seg = path.segment
# Sainte Devote to the tunnel mouth, so the crest falls in Casino Square.
climb = Hill(seg("sainte-devote").s_start, seg("tunnel-entry").s_end, CREST_HEIGHT_M)

LAYOUT = Layout(
    name="monarch",
    path=path,
    asphalt_half_width=6.0,
    runoff_width=2.0,
    half_widths={
        **{
            name: NARROW_HALF_WIDTH_M
            for name in ("mirabeau", "mirabeau-kink", "hairpin-approach", "hairpin-exit", "rascasse")
        },
        "grand-hotel": HAIRPIN_HALF_WIDTH_M,
        "mirabeau-bas": MIRABEAU_BAS_HALF_WIDTH_M,
    },
    tapers=[
        # In after the wheel crossing, out over Portier so the tunnel entry's box row has the full width.
        Taper(seg("casino-descent").s_end - TAPER_IN_M, seg("mirabeau").s_start),
        Taper(seg("grand-hotel").s_start, seg("grand-hotel").s_start + HALF_METRE_TAPER_M),
        Taper(seg("grand-hotel").s_end - HALF_METRE_TAPER_M, seg("grand-hotel").s_end),
        Taper(seg("mirabeau-bas").s_start, seg("mirabeau-bas").s_start + HALF_METRE_TAPER_M),
        Taper(seg("portier-approach").s_start, seg("tunnel-entry").s_start),
        # In after the quay's box row, out down the exit straight, well before the pit lane's fork.
        Taper(seg("quay").s_end - TAPER_IN_M, seg("rascasse").s_start),
        Taper(seg("rascasse-exit").s_start, seg("rascasse-exit").s_end),
    ],
    checkpoint_count=15,
    tyre_walls=[
        # Outside Sainte Devote: a right-hander's inside is on the bots' line.
        TyreWall(seg("sainte-devote").s_mid, offset=TYRE_WALL_LEFT_M),
        # None in the Grand Hotel hairpin, though the mockup put two inside it:
        # bots lost laps on them wherever they stood (ADR 0064).
    ],
    item_rows=[
        ItemRow(seg("beau-rivage").s_mid),
        ItemRow(seg("tunnel-entry").s_mid),
        ItemRow(seg("quay").s_mid),
    ],
    boost_pads=[
        # Left of the centreline: the outside of the tunnel's bend and the
        # inside of the chicane's left-hander, so the boost costs the chicane's
        # entry, and the right half is the tow and the safe line in. The left
        # leaves a boosted kart the most road short of the wall (ADR 0097).
        BoostPad(seg("tunnel").s_mid, offset=3.0),
        # Left of the centreline, away from the fork on the right, so a kart called
        # in for the pit lane passes it by and no boost throws it at the pit wall.
        BoostPad(seg("pit-bend").s_start, offset=3.0),
    ],
    # Ten on the centreline, down the start straight and the harbour; five
    # off it, wide on the climb and on the pool straight. The climb's three
    # stand on the approach to Casino Square: Beau Rivage holds a box row.
    coin_lines=[
        CoinLine(seg("start").s_start + 10.0, count=5),
        CoinLine(seg("harbour").s_start + 10.0, count=5),
        CoinLine(seg("casino-approach").s_start + 8.0, count=3, offset=-4.0),
        CoinLine(seg("pool-straight").s_start + 4.0, count=2, offset=4.0),
    ],
    kerbs=[
        Kerb(seg("chicane-out").s_mid, offset=-KERB_INSIDE_M),
        Kerb(seg("pool-1-left").s_mid, offset=KERB_INSIDE_M),
        Kerb(seg("pool-2-right").s_mid, offset=-KERB_INSIDE_M),
    ],
    # Mirabeau turns right, so the descent's outside is the left.
    wheel_crossings=[WheelCrossing(seg("casino-descent").s_mid, outside=1)],
    elevation=[climb],
    # Right of the start straight, as Monaco's lane is (ADR 0088). The fork
    # leaves on the noghes flick, after Anthony Noghes, and the merge ends at
    # Sainte Devote, not past it as Monaco's does: a lane can fit inside neither
    # corner's 13-14 m radius. 120 m between the lines (ADR 0081), the doors
    # centred on the building so every pit box is short of the exit line.
    pit_lane=PitLane(
        side=-1,
        s_fork=-75.0,
        s_entry=-50.0,
        s_exit=70.0,
        s_merge=93.0,
        s_building=PITS_S,
        door_first=PITS_DOORS_CENTRED_M,
    ),
    decor=Decor(
        hoardings=[
            Hoardings(("casino",), side=-1),
            Hoardings(("portier",), side=1),
            Hoardings(("pool-1-left", "pool-1-right"), side=-1),
            Hoardings(("rascasse",), side=1),
        ],
        grandstands=[
            Grandstands(("grid", "start"), side=1),
            Grandstands(("harbour",), side=1),
            Grandstands(("pool-straight", "pool-2-right"), side=1),
        ],
        gantry=True,
        props=[
            # Across the straight from the stands, its front 1 m behind the lane's wall.
            Prop("pits", Spot(s=PITS_S, offset=-22.0), length=90.0),
            # The basin outside the harbour straight, the pool and the quay: behind the
            # stands beside the first two, then close round the quay and Rascasse, the
            # 8 m rule's width from their walls.
            # Waves throw spray over its quay beside the lap's quay.
            Prop("harbour", World(150.0, -30.0), outline=HARBOUR_OUTLINE, spray=("quay",)),
            # Outside Casino Square at the crest, a right-hander, so on the left.
            Prop("casino", Spot(seg("casino").s_mid, offset=CASINO_LEFT_M)),
            # Circling over the basin and within about 20 m of the casino. 40 m up, since
            # the road climbs to 8 m at the crest under it and the skids keep 30 m over
            # every road.
            Prop("helicopter", World(170.0, 120.0), radius=100.0, height=40.0),
            *(
                Prop("block", Spot(seg(name).s_start + share * seg(name).length, offset), length=length, height=height)
                for name, share, offset, length, height in CLIMB_ROWS
            ),
            *(Prop("block", World(x, y, heading), length=length, height=height) for x, y, heading, length, height in SHORE_BLOCKS),
            Prop("pool", POOL_AT, length=POOL_LENGTH_M),
        ],
        tunnels=[
            Tunnel(
                seg("tunnel-entry").s_end + TUNNEL_PAST_MOUTH_M,
                seg("tunnel").s_end - TUNNEL_SHORT_OF_EXIT_M,
            )
        ],
    ),
)

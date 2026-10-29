"""The bridge where Suzuqa Patch crosses itself, checked without Blender:
nothing the sim reads sits near the other leg, the deck clears the underpass,
and the scenery stands the deck on an embankment with a span over the road
below.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from trackgen.layout import (
    BRIDGE_DECK_THICKNESS_M,
    GRASS_DROP_M,
    GRID_FRONT_ROW_BEHIND_LINE_M,
    WALL_THICKNESS_M,
    build,
)
from trackgen.layouts import load
from trackgen.path import elevation

# Gates and placements are height-blind in the sim, so each stays this far,
# in plan, from the other leg's centreline.
CLEARANCE_M = 20.0
# Metres of arc length either side of the crossing that count as a leg at
# the bridge: as far as the embankment reaches.
LEG_REACH_M = 60.0
# The span leaves at least this much open ground under the deck, wide enough
# that both of the underpass's walls stand clear of the abutments.
MIN_SPAN_M = 28.0


def brands_patch():
    layout = load("brands-patch")
    return layout, build(layout), layout.decor.bridges[0]


def wrapped(layout, a: float, b: float) -> float:
    """Metres of arc length between `a` and `b`, the short way round the loop."""
    gap = abs(a - b) % layout.path.length
    return min(gap, layout.path.length - gap)


def leg(layout, s: float):
    path = layout.path
    return [path.samples[i] for i in path.indices_between(s - LEG_REACH_M, s + LEG_REACH_M)]


def placements(layout, geometry):
    """Everything the sim reads off the sidecar, as (what, arc length, points in plan)."""
    path = layout.path
    for number, checkpoint in enumerate(geometry.checkpoints):
        across = (-math.sin(checkpoint.heading), math.cos(checkpoint.heading))
        ends = [
            (checkpoint.x + side * checkpoint.half_width * across[0], checkpoint.y + side * checkpoint.half_width * across[1])
            for side in (1, -1)
        ]
        yield f"checkpoint {number}", number * path.length / layout.checkpoint_count, [(checkpoint.x, checkpoint.y), *ends]
    boxes = iter(geometry.item_boxes)
    for row in layout.item_rows:
        for offset in row.offsets:
            box = next(boxes)
            yield f"item box at {row.s:.0f} m, {offset} m", row.s, [(box.x, box.y)]
    for pad, placed in zip(layout.boost_pads, geometry.boost_pads, strict=True):
        yield f"boost pad at {pad.s:.0f} m", pad.s, [(placed.x, placed.y)]
    for kerb, placed in zip(layout.kerbs, geometry.kerbs, strict=True):
        yield f"kerb at {kerb.s:.0f} m", kerb.s, [(placed.x, placed.y)]
    for crossing, placed in zip(layout.wheel_crossings, geometry.wheel_crossings, strict=True):
        # The wheel's path, wall to wall; its hazard zone is paint the sim never reads.
        across = (-math.sin(placed.heading), math.cos(placed.heading))
        path_ends = [(placed.x + d * across[0], placed.y + d * across[1]) for d in (-placed.half_width, placed.half_width)]
        yield f"wheel crossing at {crossing.s:.0f} m", crossing.s, [(placed.x, placed.y), *path_ends]
    for wall in layout.tyre_walls:
        x, y = path.at(wall.s).offset(wall.offset)
        yield f"tyre wall at {wall.s:.0f} m", wall.s, [(x, y)]
    for slot, spot in enumerate(geometry.spawn_grid):
        yield f"grid slot {slot}", -GRID_FRONT_ROW_BEHIND_LINE_M, [(spot.x, spot.y)]


class Clearance(unittest.TestCase):
    def test_nothing_the_sim_reads_sits_within_20_m_in_plan_of_the_other_leg_at_the_bridge(self) -> None:
        layout, geometry, bridge = brands_patch()
        for own, other in ((bridge.deck_s, bridge.underpass_s), (bridge.underpass_s, bridge.deck_s)):
            other_leg = leg(layout, other)
            for what, s, points in placements(layout, geometry):
                if wrapped(layout, s, own) > LEG_REACH_M:
                    continue
                nearest = min(math.dist(p, (q.x, q.y)) for p in points for q in other_leg)
                with self.subTest(what):
                    self.assertGreaterEqual(nearest, CLEARANCE_M)

    def test_the_legs_cross_where_the_bridge_says(self) -> None:
        layout, _, bridge = brands_patch()
        deck = layout.path.at(bridge.deck_s)
        under = layout.path.at(bridge.underpass_s)
        self.assertLess(math.dist((deck.x, deck.y), (under.x, under.y)), 1.0)


class Deck(unittest.TestCase):
    def test_the_deck_stands_5_m_over_the_underpass(self) -> None:
        layout, _, bridge = brands_patch()
        self.assertAlmostEqual(elevation(layout.elevation, bridge.deck_s), 5.0, delta=0.01)
        self.assertEqual(elevation(layout.elevation, bridge.underpass_s), 0.0)

    def test_the_span_leaves_28_m_open_under_the_deck_with_both_underpass_walls_clear(self) -> None:
        layout, geometry, bridge = brands_patch()
        under = layout.path.at(bridge.underpass_s)
        across = under.left_normal
        # The abutments' faces, as metres left of the underpass's centreline.
        abutments = [
            (x - under.x) * across[0] + (y - under.y) * across[1]
            for piece in geometry.scenery
            if piece.name == "Bridge.wall"
            for x, y, z in piece.mesh[0]
            if z < 0.0
        ]
        near = max(a for a in abutments if a < 0.0)
        far = min(a for a in abutments if a > 0.0)
        self.assertGreaterEqual(far - near, MIN_SPAN_M)
        walls = layout.runoff_half_width + WALL_THICKNESS_M
        self.assertGreater(-near, walls)
        self.assertGreater(far, walls)

    def test_the_underside_clears_a_kart_on_the_underpass(self) -> None:
        _, geometry, _ = brands_patch()
        (underside,) = [p for p in geometry.scenery if p.name == "Bridge.stand-plinth"]
        lowest = min(z for _, _, z in underside.mesh[0])
        self.assertGreater(lowest, 3.0)
        top = max(z for _, _, z in underside.mesh[0])
        self.assertLess(top, 5.0 - BRIDGE_DECK_THICKNESS_M)

    def test_the_embankment_runs_from_the_road_down_to_the_grass(self) -> None:
        _, geometry, _ = brands_patch()
        (bank,) = [p for p in geometry.scenery if p.name == "Bridge.grass"]
        heights = [z for _, _, z in bank.mesh[0]]
        self.assertAlmostEqual(min(heights), -GRASS_DROP_M, places=6)
        self.assertGreater(max(heights), 3.0)


if __name__ == "__main__":
    unittest.main()

"""Where the rows, pads and coin lines set up the next fight (ADR 0097), checked
without Blender: a box row about every third of Goldstone's lap, the last on
the Stowe exit short of the wheel; the Hangar Straight's and Monarch's tunnel
pads on one half of the road, the other half clear for the tow; one sausage
kerb left in Becketts, with a coin out wide on each exit; Bell Deer's
back-straight row past where a boosted kart lands off the ramp; Spoon's
coins on the wide line; and every row clear of a wheel's hazard zone.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from tests.test_boost_pads import tuning_constant
from tests.test_coins import tuning_count
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import HAZARD_HALF_DEPTH_M, build
from trackgen.layouts import NAMES, load
from trackgen.path import Ramp, elevation

BOX_HALF_LENGTH_M = tuning_constant("ITEM_BOX_HALF_LENGTH_M")
PAD_HALF_WIDTH_M = tuning_constant("BOOST_PAD_HALF_WIDTH_M")
# The fastest a kart goes down a straight: on a boost pad's boost with a full purse.
BOOSTED_SPEED_M_PER_S = (
    tuning_constant("BOOST_PAD_FACTOR")
    * tuning_constant("TOP_SPEED_M_PER_S")
    * (1.0 + tuning_count("COIN_CAP") * tuning_constant("COIN_TOP_SPEED_FACTOR"))
)
# The sim keeps gravity negative, pulling down its +Y.
GRAVITY_M_PER_S2 = -tuning_constant("GRAVITY_M_PER_S2")


def segment_at(layout, s: float):
    return next(seg for seg in layout.path.segments if seg.s_start <= s < seg.s_end)


def pad_across(pad) -> tuple[float, float]:
    """The metres left of the centreline that `pad`'s plate spans, right edge first."""
    return pad.offset - PAD_HALF_WIDTH_M, pad.offset + PAD_HALF_WIDTH_M


def landing_s(layout, ramp: Ramp, speed: float) -> float:
    """Where a kart that leaves `ramp`'s lip at `speed` up its slope comes down on the road."""
    slope = math.atan2(ramp.height, ramp.s_end - ramp.s_start)
    along, up = speed * math.cos(slope), speed * math.sin(slope)
    lip = elevation(layout.elevation, ramp.s_end) + ramp.height
    t = 0.0
    while True:
        t += 0.001
        s = ramp.s_end + along * t
        if lip + up * t - GRAVITY_M_PER_S2 * t * t / 2.0 <= elevation(layout.elevation, s):
            return s


class Goldstone(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load("goldstone")
        self.seg = self.layout.path.segment

    def test_a_box_row_stands_about_every_third_of_the_lap(self) -> None:
        lap = self.layout.path.length
        rows = sorted(row.s for row in self.layout.item_rows)
        gaps = [(b - a) % lap for a, b in zip(rows, rows[1:] + rows[:1])]
        self.assertEqual(len(gaps), 3)
        for gap in gaps:
            with self.subTest(gap=round(gap)):
                self.assertGreater(gap, 0.25 * lap)
                self.assertLess(gap, 0.40 * lap)

    def test_the_last_row_stands_on_the_stowe_exit_short_of_the_wheels_hazard_zone(self) -> None:
        (crossing,) = self.layout.wheel_crossings
        row = max(self.layout.item_rows, key=lambda r: r.s)
        self.assertEqual(segment_at(self.layout, row.s).name, "vale-approach")
        self.assertLess(row.s - self.seg("vale-approach").s_start, 20.0, "just past Stowe")
        self.assertGreater(crossing.s - HAZARD_HALF_DEPTH_M - (row.s + BOX_HALF_LENGTH_M), 20.0)

    def test_the_hangar_straights_pad_covers_the_right_half_and_leaves_the_left_for_the_tow(self) -> None:
        hangar = self.seg("hangar")
        (pad,) = [p for p in self.layout.boost_pads if hangar.s_start <= p.s < hangar.s_end]
        right, left = pad_across(pad)
        self.assertLessEqual(left, 0.0, "the left half is clear")
        self.assertGreaterEqual(right, -self.layout.asphalt_half_width)
        self.assertLess(self.seg("stowe").turn, 0, "Stowe turns right, so the braking line runs on the left")

    def test_becketts_keeps_only_its_first_kerb(self) -> None:
        esses = [self.seg(name) for name in ("maggotts", "becketts-right", "becketts-left", "becketts", "chapel")]
        kerbs = [k for k in self.layout.kerbs if esses[0].s_start <= k.s < esses[-1].s_end]
        self.assertEqual([segment_at(self.layout, k.s).name for k in kerbs], ["becketts-right"])

    def test_a_coin_marks_the_outside_of_each_becketts_exit(self) -> None:
        for name in ("becketts-right", "becketts-left", "becketts"):
            corner = self.seg(name)
            lines = [line for line in self.layout.coin_lines if corner.s_start <= line.s < corner.s_end]
            with self.subTest(name):
                (line,) = lines
                self.assertEqual(line.count, 1)
                self.assertGreater(line.s, corner.s_mid, "on the exit")
                # A left-hander's outside is its right, and the other way round.
                self.assertEqual(math.copysign(1.0, line.offset), -corner.turn)


class Monarch(unittest.TestCase):
    def test_the_tunnels_pad_covers_the_left_half_and_leaves_the_right_for_the_tow(self) -> None:
        layout = load("monarch")
        tunnel = layout.path.segment("tunnel")
        (pad,) = [p for p in layout.boost_pads if tunnel.s_start <= p.s < tunnel.s_end]
        right, left = pad_across(pad)
        self.assertGreaterEqual(right, 0.0, "the right half is clear")
        self.assertLessEqual(left, layout.asphalt_half_width)
        self.assertGreater(layout.path.segment("chicane-in").turn, 0, "the chicane opens left, so the pad's side is its inside")


class BellDeer(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load("bell-deer")

    def test_the_back_straights_row_stands_past_where_a_boosted_kart_lands_off_the_ramp(self) -> None:
        back = self.layout.path.segment("back-straight")
        (ramp,) = self.layout.ramps
        (row,) = [r for r in self.layout.item_rows if back.s_start <= r.s < back.s_end]
        lands = landing_s(self.layout, ramp, BOOSTED_SPEED_M_PER_S)
        self.assertGreater(row.s - BOX_HALF_LENGTH_M, lands)
        self.assertLess(row.s, back.s_start + 0.5 * (back.s_end - back.s_start), "in the straight's first half")

    def test_spoons_coins_trace_the_wide_line_between_its_apexes(self) -> None:
        link = self.layout.path.segment("spoon-link")
        (line,) = [line for line in self.layout.coin_lines if link.s_start <= line.s < link.s_end]
        for apex in ("spoon-1", "spoon-2"):
            self.assertGreater(self.layout.path.segment(apex).turn, 0, f"{apex} turns left")
        self.assertLess(line.offset, 0.0, "right of the centreline: the outside, the tyre wall's other side")


class EveryCourse(unittest.TestCase):
    def test_no_box_row_lies_in_a_wheels_hazard_zone(self) -> None:
        for name in NAMES:
            layout = load(name)
            lap = layout.path.length
            for crossing in layout.wheel_crossings:
                for row in layout.item_rows:
                    gap = abs((row.s - crossing.s + lap / 2.0) % lap - lap / 2.0)
                    with self.subTest(name, row=round(row.s)):
                        self.assertGreater(gap, HAZARD_HALF_DEPTH_M + BOX_HALF_LENGTH_M)

    def test_every_offset_pad_lies_on_the_asphalt(self) -> None:
        for name in NAMES:
            layout = load(name)
            geometry = build(layout)
            for pad, placed in zip(layout.boost_pads, geometry.boost_pads, strict=True):
                with self.subTest(name, s=round(pad.s)):
                    right, left = pad_across(pad)
                    self.assertGreaterEqual(right, -layout.asphalt_half_width)
                    self.assertLessEqual(left, layout.asphalt_half_width)
                    self.assertAlmostEqual(left_of_centre(layout, placed.x, placed.y, pad.s), pad.offset, places=3)


if __name__ == "__main__":
    unittest.main()

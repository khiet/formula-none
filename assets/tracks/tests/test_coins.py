"""Coins placed where a kart collects them, checked without Blender: every line along a straight
no box row uses, every coin's rectangle on the asphalt, clear of every item box, boost pad and grid slot, enough on the centreline for a
clean lap to collect a full purse, and about ten to twelve on the oval, thirteen on Goldstone and fifteen on Bell
Deer.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest

from tests.test_boost_pads import TUNING_RS, tuning_constant
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import COIN_LINE_SPACING_M, build
from trackgen.layouts import NAMES, load

HALF_LENGTH_M = tuning_constant("COIN_HALF_LENGTH_M")
HALF_WIDTH_M = tuning_constant("COIN_HALF_WIDTH_M")
BOX_HALF_LENGTH_M = tuning_constant("ITEM_BOX_HALF_LENGTH_M")
BOX_HALF_WIDTH_M = tuning_constant("ITEM_BOX_HALF_WIDTH_M")
PAD_HALF_LENGTH_M = tuning_constant("BOOST_PAD_HALF_LENGTH_M")
PAD_HALF_WIDTH_M = tuning_constant("BOOST_PAD_HALF_WIDTH_M")
KART_HALF_LENGTH_M = tuning_constant("KART_HALF_LENGTH_M")
KART_HALF_WIDTH_M = tuning_constant("KART_HALF_WIDTH_M")


def tuning_count(name: str) -> int:
    match = re.search(rf"pub const {name}: u32 = ([0-9]+);", TUNING_RS.read_text())
    if match is None:
        raise AssertionError(f"{name} is not a u32 constant in {TUNING_RS}")
    return int(match.group(1))


COIN_CAP = tuning_count("COIN_CAP")

# How many coins each layout lays: the oval about ten to twelve, Bell Deer fifteen or so, Goldstone thirteen.
COUNTS = {"oval": range(10, 13), "bell-deer": range(14, 17), "goldstone": range(12, 15)}


def corners(placement, half_length: float, half_width: float) -> list[tuple[float, float]]:
    """The rectangle `half_length` along `placement`'s heading and `half_width` across it, in plan."""
    ahead = (math.cos(placement.heading), math.sin(placement.heading))
    left = (-ahead[1], ahead[0])
    return [
        (
            placement.x + along * half_length * ahead[0] + across * half_width * left[0],
            placement.y + along * half_length * ahead[1] + across * half_width * left[1],
        )
        for along, across in ((1, 1), (-1, 1), (-1, -1), (1, -1))
    ]


def overlap(a: list[tuple[float, float]], b: list[tuple[float, float]]) -> bool:
    """Whether two rectangles in plan overlap: no edge of either separates them."""
    for shape in (a, b):
        for p, q in zip(shape, shape[1:] + shape[:1]):
            axis = (q[1] - p[1], p[0] - q[0])
            project = [x * axis[0] + y * axis[1] for x, y in a], [x * axis[0] + y * axis[1] for x, y in b]
            if max(project[0]) <= min(project[1]) or max(project[1]) <= min(project[0]):
                return False
    return True


def every_coin():
    """Each coin of every shipped layout, as (track, layout, arc length, offset, placement)."""
    for name in NAMES:
        layout = load(name)
        geometry = build(layout)
        lines = [(line.s + k * COIN_LINE_SPACING_M, line.offset) for line in layout.coin_lines for k in range(line.count)]
        for (s, offset), coin in zip(lines, geometry.coins, strict=True):
            yield name, layout, geometry, s, offset, coin


class Coins(unittest.TestCase):
    def test_each_layout_lays_about_the_coins_a_lap_asks_for(self) -> None:
        for name in NAMES:
            with self.subTest(name):
                self.assertIn(len(build(load(name)).coins), COUNTS[name])

    def test_a_clean_lap_down_the_centreline_collects_a_full_set(self) -> None:
        for name in NAMES:
            with self.subTest(name):
                layout = load(name)
                on_line = sum(line.count for line in layout.coin_lines if line.offset == 0.0)
                off_line = sum(line.count for line in layout.coin_lines if line.offset != 0.0)
                self.assertGreaterEqual(on_line, COIN_CAP)
                self.assertGreater(off_line, 0, "some reward a wider line")

    def test_every_line_runs_along_a_straight_no_box_row_uses(self) -> None:
        for name in NAMES:
            layout = load(name)
            for line in layout.coin_lines:
                with self.subTest(name, s=round(line.s), offset=line.offset):
                    first, last = line.s - HALF_LENGTH_M, line.s + (line.count - 1) * COIN_LINE_SPACING_M + HALF_LENGTH_M
                    segment = next(seg for seg in layout.path.segments if seg.s_start <= line.s < seg.s_end)
                    self.assertEqual(segment.turn, 0, f"{segment.name} is a curve")
                    self.assertLessEqual(segment.s_start, first)
                    self.assertLessEqual(last, segment.s_end, f"runs off the end of {segment.name}")
                    rows = [row.s for row in layout.item_rows if segment.s_start <= row.s < segment.s_end]
                    self.assertEqual(rows, [], f"{segment.name} has a box row")

    def test_a_line_spaces_its_coins_so_a_kart_takes_them_one_at_a_time(self) -> None:
        self.assertGreater(COIN_LINE_SPACING_M, 2.0 * HALF_LENGTH_M)

    def test_every_coin_lies_on_the_asphalt(self) -> None:
        for name, layout, _, s, offset, coin in every_coin():
            with self.subTest(name, s=round(s), offset=offset):
                for x, y in corners(coin, HALF_LENGTH_M, HALF_WIDTH_M):
                    self.assertLessEqual(abs(left_of_centre(layout, x, y, s)), layout.asphalt_half_width)

    def test_no_coin_lies_on_an_item_box_a_boost_pad_or_a_grid_slot(self) -> None:
        for name, _, geometry, s, offset, coin in every_coin():
            with self.subTest(name, s=round(s), offset=offset):
                rectangle = corners(coin, HALF_LENGTH_M, HALF_WIDTH_M)
                for box in geometry.item_boxes:
                    self.assertFalse(overlap(rectangle, corners(box, BOX_HALF_LENGTH_M, BOX_HALF_WIDTH_M)), box)
                for pad in geometry.boost_pads:
                    self.assertFalse(overlap(rectangle, corners(pad, PAD_HALF_LENGTH_M, PAD_HALF_WIDTH_M)), pad)
                # A kart on the grid takes nothing at Go.
                for slot in geometry.spawn_grid:
                    self.assertFalse(overlap(rectangle, corners(slot, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M)), slot)


if __name__ == "__main__":
    unittest.main()

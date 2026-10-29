"""Tyre walls checked without Blender: each layout's tyre walls stand on the
asphalt and clear of the line the bots drive, so a bot never meets one.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from tests.test_boost_pads import tuning_constant
from trackgen.layout import TYRES, build
from trackgen.layouts import NAMES, load

# Footprints are checked against the centreline samples, a metre apart, so
# an edge on the asphalt's edge reads a hair either side of it.
SAMPLE_SLACK_M = 0.01
# Metres of arc length either side of a placement's own `s` searched for the
# centreline sample nearest to it: past anything a placement draws, and short
# of the other leg where a figure eight crosses itself.
NEAR_S_M = 30.0


def nearest_sample(layout, x: float, y: float, s: float):
    """The centreline sample nearest (x, y) within `NEAR_S_M` of arc length `s`."""
    path = layout.path
    near = [path.samples[i] for i in path.indices_between(s - NEAR_S_M, s + NEAR_S_M)]
    return min(near, key=lambda sample: (sample.x - x) ** 2 + (sample.y - y) ** 2)


def left_of_centre(layout, x: float, y: float, s: float) -> float:
    """How far (x, y) lies left of the centreline near arc length `s`, in metres."""
    sample = nearest_sample(layout, x, y, s)
    nx, ny = sample.left_normal
    return (x - sample.x) * nx + (y - sample.y) * ny


class TyreWalls(unittest.TestCase):
    def test_every_layout_has_one_tyres_mesh_per_tyre_wall(self) -> None:
        for name in NAMES:
            with self.subTest(name):
                layout = load(name)
                tyres = [s for s in build(layout).surfaces if s.surface == TYRES]
                self.assertGreater(len(layout.tyre_walls), 0)
                self.assertEqual(len(tyres), len(layout.tyre_walls))

    def test_every_tyre_wall_stands_on_the_asphalt_clear_of_the_bots_line(self) -> None:
        bot_line = -tuning_constant("BOT_LINE_RIGHT_OF_CENTRE_M")
        kart_half_width = tuning_constant("KART_HALF_WIDTH_M")
        for name in NAMES:
            layout = load(name)
            tyres = [surface for surface in build(layout).surfaces if surface.surface == TYRES]
            for wall, surface in zip(layout.tyre_walls, tyres, strict=True):
                with self.subTest(name, wall=surface.name):
                    offsets = [left_of_centre(layout, x, y, wall.s) for x, y, _ in surface.mesh[0]]
                    self.assertLessEqual(
                        max(abs(d) for d in offsets), layout.asphalt_half_width + SAMPLE_SLACK_M
                    )
                    self.assertGreater(min(offsets) - bot_line, kart_half_width)


if __name__ == "__main__":
    unittest.main()

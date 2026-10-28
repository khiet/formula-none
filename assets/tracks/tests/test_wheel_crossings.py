"""Runaway wheel crossings checked without Blender: one across the middle of
the Brands Patch approach to hairpin two, wall to wall, with its marshal
post on the outside run-off clear of the wheel's path, and none on the oval.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from tests.test_boost_pads import in_pad_frame
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import build
from trackgen.layouts import load


def brands_patch():
    layout = load("brands-patch")
    return layout, build(layout)


def post_pieces(geometry, number: int):
    prefix = f"MarshalPost.{number:02d}."
    return [p for p in geometry.scenery if p.name.startswith(prefix)]


class WheelCrossings(unittest.TestCase):
    def test_brands_patch_has_one_crossing_across_the_middle_of_the_approach_wall_to_wall(self) -> None:
        layout, geometry = brands_patch()
        self.assertEqual(len(geometry.wheel_crossings), 1)
        crossing = geometry.wheel_crossings[0]
        approach = layout.path.segment("approach")
        middle = layout.path.at(approach.s_mid)
        self.assertAlmostEqual(crossing.x, middle.x, places=6)
        self.assertAlmostEqual(crossing.y, middle.y, places=6)
        self.assertAlmostEqual(crossing.heading, middle.heading, places=6)
        self.assertEqual(crossing.half_width, layout.runoff_half_width)
        self.assertAlmostEqual(2.0 * crossing.half_width, 24.0)

    def test_the_marshal_post_stands_on_the_outside_run_off_short_of_the_wheels_path(self) -> None:
        layout, geometry = brands_patch()
        crossing = geometry.wheel_crossings[0]
        hairpin = layout.path.segment("hairpin-2")
        self.assertGreater(hairpin.turn, 0, "hairpin two turns left, so its outside is the right")
        vertices = [v for piece in post_pieces(geometry, 0) for v in piece.mesh[0]]
        self.assertTrue(vertices)
        for x, y, _ in vertices:
            right = -left_of_centre(layout, x, y)
            self.assertGreater(right, layout.asphalt_half_width, "off the asphalt")
            self.assertLess(right, layout.runoff_half_width, "inside the walls")
            ahead, _ = in_pad_frame(crossing, (x, y))
            self.assertLess(ahead, -1.0, "short of the wheel's path")

    def test_the_post_is_white_with_an_orange_lamp_on_its_face_to_the_track(self) -> None:
        layout, geometry = brands_patch()
        pieces = {p.material: p for p in post_pieces(geometry, 0)}
        self.assertEqual(set(pieces), {"board", "chevron"})
        lamp = [v for v in pieces["chevron"].mesh[0]]
        post = [v for v in pieces["board"].mesh[0]]
        nearest = lambda vertices: min(-left_of_centre(layout, x, y) for x, y, _ in vertices)
        self.assertLess(nearest(lamp), nearest(post), "the lamp stands proud toward the track")
        self.assertLess(max(v[2] for v in lamp), max(v[2] for v in post), "under the post's top")

    def test_the_oval_has_no_crossing(self) -> None:
        geometry = build(load("oval"))
        self.assertEqual(geometry.wheel_crossings, [])
        self.assertFalse([p for p in geometry.scenery if p.name.startswith("MarshalPost.")])


if __name__ == "__main__":
    unittest.main()

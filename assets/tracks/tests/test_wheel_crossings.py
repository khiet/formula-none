"""Runaway wheel crossings checked without Blender: one across the middle of
the Brands Patch approach to hairpin two, wall to wall, painted as a hazard
zone over all the wheel can reach, with its marshal post on the outside
run-off clear of the wheel's path, and none on the oval.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from tests.test_boost_pads import in_pad_frame, signed_area
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import build
from trackgen.layouts import load


def brands_patch():
    layout = load("brands-patch")
    return layout, build(layout)


def post_pieces(geometry, number: int):
    prefix = f"MarshalPost.{number:02d}."
    return [p for p in geometry.scenery if p.name.startswith(prefix)]


def zone_pieces(geometry, number: int):
    prefix = f"HazardZone.{number:02d}."
    return [p for p in geometry.scenery if p.name.startswith(prefix)]


def in_crossing_frame(crossing, pieces):
    """Every vertex of `pieces` as (metres ahead of the crossing, metres right of it)."""
    return [in_pad_frame(crossing, v) for piece in pieces for v in piece.mesh[0]]


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

    def test_the_post_is_a_low_white_platform_banded_orange_under_its_top_with_an_orange_lamp_on_its_face_to_the_track(self) -> None:
        layout, geometry = brands_patch()
        pieces = {p.material: p for p in post_pieces(geometry, 0)}
        self.assertEqual(set(pieces), {"board", "post-band", "chevron"})
        band = pieces["post-band"].mesh[0]
        lamp = pieces["chevron"].mesh[0]
        post = pieces["board"].mesh[0]
        ground = min(v[2] for v in post)
        self.assertAlmostEqual(max(v[2] for v in post) - ground, 1.0, places=6, msg="1 m tall")
        across = [-left_of_centre(layout, x, y) for x, y, _ in post]
        self.assertAlmostEqual(max(across) - min(across), 1.4, delta=0.01, msg="1.4 m wide")
        self.assertAlmostEqual(min(v[2] for v in band) - ground, 0.88, places=6, msg="0.12 m under the top")
        self.assertAlmostEqual(max(v[2] for v in band) - ground, 1.0, places=6)
        self.assertLess(max(v[2] for v in lamp) - ground, 0.88, "the lamp under the band")
        nearest = lambda vertices: min(-left_of_centre(layout, x, y) for x, y, _ in vertices)
        self.assertLess(nearest(lamp), nearest(post), "the lamp stands proud toward the track")

    def test_the_crossing_reaches_as_deep_as_the_hazard_zone(self) -> None:
        _, geometry = brands_patch()
        self.assertAlmostEqual(geometry.wheel_crossings[0].hazard_half_depth, 9.2)

    def test_the_hazard_zone_is_painted_wall_to_wall_and_as_deep_as_the_wheel_reaches(self) -> None:
        layout, geometry = brands_patch()
        crossing = geometry.wheel_crossings[0]
        pieces = zone_pieces(geometry, 0)
        points = in_crossing_frame(crossing, pieces)
        aheads = [ahead for ahead, _ in points]
        rights = [right for _, right in points]
        self.assertAlmostEqual(min(aheads), -9.2, delta=0.01)
        self.assertAlmostEqual(max(aheads), 9.2, delta=0.01)
        self.assertAlmostEqual(min(rights), -layout.runoff_half_width, delta=0.01)
        self.assertAlmostEqual(max(rights), layout.runoff_half_width, delta=0.01)

    def test_the_hazard_zone_has_a_metre_of_yellow_and_black_chevrons_across_each_end_and_a_tint_between(self) -> None:
        _, geometry = brands_patch()
        crossing = geometry.wheel_crossings[0]
        by_material = {}
        for piece in zone_pieces(geometry, 0):
            by_material.setdefault(piece.material, []).append(piece)
        self.assertIn("kerb-yellow", by_material)
        self.assertIn("kerb-black", by_material)
        for material in ("kerb-yellow", "kerb-black"):
            aheads = [ahead for ahead, _ in in_crossing_frame(crossing, by_material[material])]
            for ahead in aheads:
                self.assertGreaterEqual(abs(ahead), 8.2 - 0.01, f"{material} only in the end bands")
            self.assertLess(min(aheads), 0.0, f"{material} at the near end")
            self.assertGreater(max(aheads), 0.0, f"{material} at the far end")
        tints = [p for m, ps in by_material.items() if m.startswith("hazard-") for p in ps]
        self.assertTrue(tints, "a tint between the bands")
        aheads = [ahead for ahead, _ in in_crossing_frame(crossing, tints)]
        self.assertAlmostEqual(min(aheads), -8.2, delta=0.01)
        self.assertAlmostEqual(max(aheads), 8.2, delta=0.01)

    def test_the_hazard_zone_is_paint_and_faces_up(self) -> None:
        _, geometry = brands_patch()
        self.assertFalse([s for s in geometry.surfaces if s.name.startswith("HazardZone")])
        for piece in zone_pieces(geometry, 0):
            vertices, faces = piece.mesh
            for face in faces:
                points = [vertices[i][:2] for i in face]
                with self.subTest(piece.name):
                    self.assertGreater(signed_area(points), 0.0)

    def test_the_oval_has_no_crossing(self) -> None:
        geometry = build(load("oval"))
        self.assertEqual(geometry.wheel_crossings, [])
        self.assertFalse([p for p in geometry.scenery if p.name.startswith("MarshalPost.")])
        self.assertFalse([p for p in geometry.scenery if p.name.startswith("HazardZone.")])


if __name__ == "__main__":
    unittest.main()

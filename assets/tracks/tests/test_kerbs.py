"""Sausage kerbs checked without Blender: drawn on the rectangle the sim
launches from, standing proud of the asphalt inside the Brands Patch
chicane's apex, and nowhere on the oval.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from tests.test_boost_pads import faces_of, in_pad_frame, signed_area, tuning_constant
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import SAUSAGE_KERB_PROUD_M, SCENERY_LIFT_M, build
from trackgen.layouts import load

HALF_LENGTH_M = tuning_constant("KERB_HALF_LENGTH_M")
HALF_WIDTH_M = tuning_constant("KERB_HALF_WIDTH_M")


def brands_patch():
    layout = load("brands-patch")
    return layout, build(layout)


def pieces_of(geometry, number: int):
    prefix = f"SausageKerb.{number:02d}."
    return [p for p in geometry.scenery if p.name.startswith(prefix)]


class SausageKerbs(unittest.TestCase):
    def test_brands_patch_has_one_kerb_inside_the_chicanes_apex_on_the_asphalt(self) -> None:
        layout, geometry = brands_patch()
        self.assertEqual(len(geometry.kerbs), 1)
        kerb = geometry.kerbs[0]
        apex = layout.path.segment("chicane-mid")
        self.assertLess(apex.turn, 0, "the chicane's middle is a right-hander")
        nearest = min(layout.path.samples, key=lambda s: (s.x - kerb.x) ** 2 + (s.y - kerb.y) ** 2)
        self.assertAlmostEqual(nearest.s, apex.s_mid, delta=1.0)
        # The kerb's corners sit right of the centreline, on the inside, and
        # in from the edge of the asphalt.
        for vertex in (v for piece in pieces_of(geometry, 0) for v in piece.mesh[0]):
            right = -left_of_centre(layout, vertex[0], vertex[1])
            self.assertGreater(right, 0.0)
            self.assertLess(right, layout.asphalt_half_width)

    def test_the_oval_has_no_kerb(self) -> None:
        geometry = build(load("oval"))
        self.assertEqual(geometry.kerbs, [])
        self.assertFalse([p for p in geometry.scenery if p.name.startswith("SausageKerb.")])

    def test_the_kerb_is_drawn_on_the_rectangle_the_sim_launches_from(self) -> None:
        _, geometry = brands_patch()
        kerb = geometry.kerbs[0]
        local = [in_pad_frame(kerb, v) for piece in pieces_of(geometry, 0) for v in piece.mesh[0]]
        along = [a for a, _ in local]
        across = [c for _, c in local]
        self.assertAlmostEqual(min(along), -HALF_LENGTH_M, places=6)
        self.assertAlmostEqual(max(along), HALF_LENGTH_M, places=6)
        self.assertAlmostEqual(min(across), -HALF_WIDTH_M, places=6)
        self.assertAlmostEqual(max(across), HALF_WIDTH_M, places=6)

    def test_the_kerb_stands_proud_of_the_asphalt_in_yellow_and_black(self) -> None:
        _, geometry = brands_patch()
        kerb = geometry.kerbs[0]
        pieces = pieces_of(geometry, 0)
        self.assertEqual({p.material for p in pieces}, {"kerb-yellow", "kerb-black"})
        heights = [v[2] - kerb.z for piece in pieces for v in piece.mesh[0]]
        self.assertAlmostEqual(min(heights), SCENERY_LIFT_M, places=6)
        self.assertAlmostEqual(max(heights), SCENERY_LIFT_M + SAUSAGE_KERB_PROUD_M, places=6)

    def test_every_face_of_the_kerb_faces_away_from_the_ground(self) -> None:
        _, geometry = brands_patch()
        for piece in pieces_of(geometry, 0):
            for face in faces_of(piece):
                with self.subTest(piece=piece.name):
                    self.assertGreater(signed_area(face), -1e-9)


if __name__ == "__main__":
    unittest.main()

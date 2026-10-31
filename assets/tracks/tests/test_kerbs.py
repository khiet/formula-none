"""Sausage kerbs checked without Blender: drawn on the rectangle the sim
launches from, standing proud of the asphalt inside esses 3 and the chicane
on Bell Deer, and nowhere on the oval.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from tests.test_boost_pads import faces_of, in_pad_frame, signed_area, tuning_constant
from tests.test_tyre_walls import left_of_centre, nearest_sample
from trackgen.layout import SAUSAGE_KERB_PROUD_M, SCENERY_LIFT_M, build
from trackgen.layouts import load

HALF_LENGTH_M = tuning_constant("KERB_HALF_LENGTH_M")
HALF_WIDTH_M = tuning_constant("KERB_HALF_WIDTH_M")


def bell_deer():
    layout = load("bell-deer")
    return layout, build(layout)


def pieces_of(geometry, number: int):
    prefix = f"SausageKerb.{number:02d}."
    return [p for p in geometry.scenery if p.name.startswith(prefix)]


# Each kerb on Bell Deer, by the corner whose apex it sits inside.
CORNERS = ("esses-3", "chicane-in")


class SausageKerbs(unittest.TestCase):
    def test_bell_deer_has_a_kerb_inside_the_apex_of_esses_3_and_the_chicane_on_the_asphalt(self) -> None:
        layout, geometry = bell_deer()
        self.assertEqual(len(geometry.kerbs), len(CORNERS))
        for number, (corner, kerb) in enumerate(zip(CORNERS, geometry.kerbs)):
            with self.subTest(corner):
                apex = layout.path.segment(corner)
                nearest = nearest_sample(layout, kerb.x, kerb.y, apex.s_mid)
                self.assertAlmostEqual(nearest.s, apex.s_mid, delta=1.0)
                # All of the kerb sits on the corner's inside, and in from
                # the edge of the asphalt.
                for vertex in (v for piece in pieces_of(geometry, number) for v in piece.mesh[0]):
                    inside = apex.turn * left_of_centre(layout, vertex[0], vertex[1], apex.s_mid)
                    self.assertGreater(inside, 0.0)
                    self.assertLess(inside, layout.asphalt_half_width)

    def test_the_oval_has_no_kerb(self) -> None:
        geometry = build(load("oval"))
        self.assertEqual(geometry.kerbs, [])
        self.assertFalse([p for p in geometry.scenery if p.name.startswith("SausageKerb.")])

    def test_each_kerb_is_drawn_on_the_rectangle_the_sim_launches_from(self) -> None:
        _, geometry = bell_deer()
        for number, kerb in enumerate(geometry.kerbs):
            with self.subTest(kerb=number):
                local = [in_pad_frame(kerb, v) for piece in pieces_of(geometry, number) for v in piece.mesh[0]]
                along = [a for a, _ in local]
                across = [c for _, c in local]
                self.assertAlmostEqual(min(along), -HALF_LENGTH_M, places=6)
                self.assertAlmostEqual(max(along), HALF_LENGTH_M, places=6)
                self.assertAlmostEqual(min(across), -HALF_WIDTH_M, places=6)
                self.assertAlmostEqual(max(across), HALF_WIDTH_M, places=6)

    def test_each_kerb_stands_proud_of_the_asphalt_in_yellow_and_black(self) -> None:
        _, geometry = bell_deer()
        for number, kerb in enumerate(geometry.kerbs):
            with self.subTest(kerb=number):
                pieces = pieces_of(geometry, number)
                self.assertEqual({p.material for p in pieces}, {"kerb-yellow", "kerb-black"})
                heights = [v[2] - kerb.z for piece in pieces for v in piece.mesh[0]]
                self.assertAlmostEqual(min(heights), SCENERY_LIFT_M, places=6)
                self.assertAlmostEqual(max(heights), SCENERY_LIFT_M + SAUSAGE_KERB_PROUD_M, places=6)

    def test_every_face_of_each_kerb_faces_away_from_the_ground(self) -> None:
        _, geometry = bell_deer()
        for number in range(len(geometry.kerbs)):
            for piece in pieces_of(geometry, number):
                for face in faces_of(piece):
                    with self.subTest(piece=piece.name):
                        self.assertGreater(signed_area(face), -1e-9)


if __name__ == "__main__":
    unittest.main()

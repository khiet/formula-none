"""The ramp bounded across the road (ADR 0101), checked without Blender: the
surface a kart meets, read off the same meshes the sidecar exports; the flat
line beside it; and the walls and centreline, which never rise for it.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from tests.test_boost_pads import tuning_constant
from trackgen.layout import WALL_HEIGHT_M, Layout, build
from trackgen.layouts import load
from trackgen.path import Ramp, Turtle

PAD_HALF_LENGTH_M = tuning_constant("BOOST_PAD_HALF_LENGTH_M")
PAD_HALF_WIDTH_M = tuning_constant("BOOST_PAD_HALF_WIDTH_M")


def triangles(faces):
    for face in faces:
        for k in range(1, len(face) - 1):
            yield face[0], face[k], face[k + 1]


def height_on_triangle(point, a, b, c) -> float | None:
    """The height of the triangle `a b c` over `point`, or `None` when it is not over it."""
    denominator = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
    if abs(denominator) < 1e-9:
        return None
    u = ((b[1] - c[1]) * (point[0] - c[0]) + (c[0] - b[0]) * (point[1] - c[1])) / denominator
    v = ((c[1] - a[1]) * (point[0] - c[0]) + (a[0] - c[0]) * (point[1] - c[1])) / denominator
    w = 1.0 - u - v
    if min(u, v, w) < -1e-9:
        return None
    return u * a[2] + v * b[2] + w * c[2]


def road_height(geometry, point) -> float:
    """The topmost asphalt over `point`: what a kart driving there stands on."""
    heights = []
    for surface in geometry.surfaces:
        if surface.surface != "asphalt":
            continue
        vertices, faces = surface.mesh
        for i, j, k in triangles(faces):
            height = height_on_triangle(point, vertices[i], vertices[j], vertices[k])
            if height is not None:
                heights.append(height)
    return max(heights)


def normal(a, b, c):
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    return (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])


class BellDeersRamp(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load("bell-deer")
        self.geometry = build(self.layout)
        (self.ramp,) = self.layout.ramps

    def height(self, s: float, offset: float) -> float:
        return road_height(self.geometry, self.layout.path.at(s).offset(offset))

    def test_the_surface_climbs_on_the_ramps_side_and_stays_flat_on_the_other(self) -> None:
        ramp = self.ramp
        middle = (ramp.s_start + ramp.s_end) / 2.0
        for offset in (0.5, 3.0, 5.5):
            self.assertAlmostEqual(self.height(ramp.s_start, offset), 0.0, delta=0.01)
            self.assertAlmostEqual(self.height(middle, offset), ramp.height / 2.0, delta=0.01)
            self.assertAlmostEqual(self.height(ramp.s_end, offset), ramp.height, delta=0.01)
            self.assertAlmostEqual(self.height(ramp.s_end + 1.0, offset), 0.0, delta=0.01)
        for offset in (-0.5, -3.0, -5.5):
            for s in (ramp.s_start, middle, ramp.s_end):
                self.assertAlmostEqual(self.height(s, offset), 0.0, delta=0.01)

    def test_it_covers_one_half_of_the_asphalt_at_spoons_exit(self) -> None:
        back = self.layout.path.segment("back-straight")
        self.assertEqual((self.ramp.right, self.ramp.left), (0.0, self.layout.asphalt_half_width))
        self.assertTrue(back.s_start < self.ramp.s_start and self.ramp.s_end < back.s_mid)

    def test_a_boost_pad_on_its_side_of_the_road_leads_into_its_foot(self) -> None:
        ramp = self.ramp
        (pad,) = [p for p in self.layout.boost_pads if abs(p.s - ramp.s_start) < 2.0 * PAD_HALF_LENGTH_M]
        self.assertEqual(pad.s + PAD_HALF_LENGTH_M, ramp.s_start)
        self.assertEqual(pad.offset - PAD_HALF_WIDTH_M, ramp.right)
        self.assertEqual(pad.offset + PAD_HALF_WIDTH_M, ramp.left)

    def test_the_walls_and_the_centreline_do_not_rise_for_it(self) -> None:
        ramp = self.ramp
        foot = self.layout.path.at(ramp.s_start)
        cos, sin = math.cos(foot.heading), math.sin(foot.heading)

        def beside_it(x: float, y: float) -> bool:
            dx, dy = x - foot.x, y - foot.y
            along, across = dx * cos + dy * sin, -dx * sin + dy * cos
            return 0.0 <= along <= ramp.s_end - ramp.s_start and abs(across) < 20.0

        tops = [
            z
            for surface in self.geometry.surfaces
            if surface.name.startswith("Wall")
            for x, y, z in surface.mesh[0]
            if beside_it(x, y)
        ]
        self.assertTrue(tops, "the walls run beside it")
        self.assertLessEqual(max(tops), WALL_HEIGHT_M)
        self.assertEqual([z for x, y, z in self.geometry.centreline if beside_it(x, y) and z != 0.0], [])

    def test_every_face_there_points_up_with_none_degenerate(self) -> None:
        # The physics discards a back face's contact, and the slope's sides and lip are scenery.
        ramp = self.ramp
        (asphalt,) = [s for s in self.geometry.surfaces if s.name == "Asphalt"]
        vertices, faces = asphalt.mesh
        foot = self.layout.path.at(ramp.s_start)
        cos, sin = math.cos(foot.heading), math.sin(foot.heading)
        length = ramp.s_end - ramp.s_start
        seen = 0
        for i, j, k in triangles(faces):
            a, b, c = vertices[i], vertices[j], vertices[k]
            dx, dy = (a[0] + b[0] + c[0]) / 3.0 - foot.x, (a[1] + b[1] + c[1]) / 3.0 - foot.y
            if -2.0 <= dx * cos + dy * sin <= length + 2.0 and abs(-dx * sin + dy * cos) < 7.0:
                seen += 1
                self.assertGreater(normal(a, b, c)[2], 0.0)
        self.assertGreater(seen, 0)

    def test_the_tyre_marks_map_covers_every_vertex(self) -> None:
        (asphalt,) = [s for s in self.geometry.surfaces if s.name == "Asphalt"]
        self.assertEqual(len(asphalt.uvs), len(asphalt.mesh[0]))


class RampBounds(unittest.TestCase):
    def layout_with(self, ramp: Ramp) -> Layout:
        path = (
            Turtle(x=0.0, y=0.0, heading_deg=0.0)
            .straight("a", 200.0)
            .arc("b", 50.0, 180.0)
            .straight("c", 200.0)
            .arc("d", 50.0, 180.0)
            .close("e")
        )
        return Layout(
            name="test",
            path=path,
            asphalt_half_width=6.0,
            runoff_width=6.0,
            checkpoint_count=4,
            tyre_walls=[],
            item_rows=[],
            boost_pads=[],
            ramps=[ramp],
        )

    def test_a_ramp_past_the_asphalts_edge_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            build(self.layout_with(Ramp(10.0, 20.0, 1.0, right=0.0, left=7.0)))

    def test_height_is_zero_outside_its_bounds(self) -> None:
        ramp = Ramp(10.0, 20.0, 1.0, right=-2.0, left=2.0)
        self.assertEqual(ramp.height_at(15.0, 2.5), 0.0)
        self.assertEqual(ramp.height_at(15.0, -2.5), 0.0)
        self.assertEqual(ramp.height_at(9.0, 0.0), 0.0)
        self.assertEqual(ramp.height_at(21.0, 0.0), 0.0)
        self.assertEqual(ramp.height_at(15.0, 0.0), 0.5)


if __name__ == "__main__":
    unittest.main()

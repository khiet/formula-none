"""Monarch, the course after Monaco, checked without Blender: the lap
docs/mockups/monaco-course.html decided, walled in close by a 2 m run-off,
with one 8 m crest at Casino Square and no ramp; and the tunnel, walls and a
roof over the road high enough to clear the chase camera, lit along both sides.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from pathlib import Path

from tests.test_props import across_track, build_materials, materials_used
from trackgen.layout import (
    CHASE_CAMERA_ABOVE_M,
    TUNNEL_LAMP_SPACING_M,
    TUNNEL_ROOF_M,
    WALL_THICKNESS_M,
    Tunnel,
    build,
)
from trackgen.layouts import load
from trackgen.path import Hill, Ramp, elevation

NAME = "monarch"
TUNNEL = "Tunnel."
CHASE_CAMERA_TS = Path(__file__).resolve().parents[3] / "apps" / "web" / "src" / "chase-camera.ts"


class Lap(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        self.geometry = build(self.layout)

    def test_it_closes_at_about_1301_m_clockwise(self) -> None:
        self.assertAlmostEqual(self.layout.path.length, 1301.3, delta=0.5)
        points = [(q.x, q.y) for q in self.layout.path.samples]
        area = sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1])) / 2.0
        self.assertLess(area, 0.0, "clockwise")

    def test_it_lays_what_the_mockup_placed(self) -> None:
        g = self.geometry
        counts = (len(g.item_boxes), len(g.boost_pads), len(g.kerbs), len(g.coins), len(g.checkpoints), len(g.spawn_grid))
        self.assertEqual(counts, (12, 2, 3, 15, 15, 8))
        self.assertEqual(self.layout.asphalt_half_width, 6.0)

    def test_its_walls_stand_2_m_off_the_asphalt(self) -> None:
        self.assertEqual(self.layout.runoff_width, 2.0)

    def test_one_8_m_crest_from_sainte_devote_to_the_tunnel_mouth_tops_out_in_casino_square(self) -> None:
        (hill,) = self.layout.elevation
        self.assertIsInstance(hill, Hill)
        self.assertNotIsInstance(hill, Ramp)
        seg = self.layout.path.segment
        self.assertEqual(hill.s_start, seg("sainte-devote").s_start)
        self.assertEqual(hill.s_end, seg("tunnel-entry").s_end)
        self.assertAlmostEqual(hill.s_end - hill.s_start, 576.0, delta=1.0)
        crest = (hill.s_start + hill.s_end) / 2.0
        casino = seg("casino")
        self.assertTrue(casino.s_start <= crest <= casino.s_end, "the crest falls in Casino Square")
        self.assertAlmostEqual(elevation(self.layout.elevation, crest), 8.0)
        heights = [elevation(self.layout.elevation, q.s) for q in self.layout.path.samples]
        steepest = max(abs(b - a) / (q1.s - q0.s) for a, b, q0, q1 in zip(heights, heights[1:], self.layout.path.samples, self.layout.path.samples[1:]))
        self.assertAlmostEqual(steepest, 0.044, delta=0.001)


class TheTunnel(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.tunnel,) = self.layout.decor.tunnels
        self.pieces = [p for p in build(self.layout).scenery if p.name.startswith(TUNNEL)]

    def frame(self, v):
        """A point as (along, left of, up) the centreline's frame at the sample nearest it: the
        tunnel bends, so no one frame serves its whole length."""
        path = self.layout.path
        nearest = min(path.indices_between(self.tunnel.s_start - 5.0, self.tunnel.s_end + 5.0), key=lambda i: (path.samples[i].x - v[0]) ** 2 + (path.samples[i].y - v[1]) ** 2)
        along, left = across_track(self.layout, path.samples[nearest].s, v[0], v[1])
        return along, left, v[2]

    def vertices(self, material: str | None = None):
        return [v for p in self.pieces if material in (None, p.material) for v in p.mesh[0]]

    def test_it_covers_about_150_m_of_the_tunnel_bend_on_the_flat(self) -> None:
        tunnel = self.layout.path.segment("tunnel")
        self.assertLessEqual(tunnel.s_start, self.tunnel.s_start)
        self.assertLessEqual(self.tunnel.s_end, tunnel.s_end)
        self.assertAlmostEqual(self.tunnel.s_end - self.tunnel.s_start, 150.0, delta=5.0)
        self.assertEqual(elevation(self.layout.elevation, self.tunnel.s_start), 0.0)

    def test_the_mirror_of_the_chase_cameras_height_matches_the_client(self) -> None:
        match = re.search(r"const ABOVE_M = ([0-9.]+);", CHASE_CAMERA_TS.read_text())
        self.assertIsNotNone(match)
        self.assertEqual(float(match.group(1)), CHASE_CAMERA_ABOVE_M)

    def test_its_roof_clears_the_chase_camera_by_metres(self) -> None:
        self.assertGreaterEqual(TUNNEL_ROOF_M, CHASE_CAMERA_ABOVE_M + 2.0)
        roof = self.vertices("carbon-shade")
        self.assertTrue(roof)
        self.assertGreaterEqual(min(z for *_, z in roof), TUNNEL_ROOF_M - 1e-9)

    def test_its_sides_stand_behind_the_walls_from_the_grass_to_the_roof(self) -> None:
        wall_back = self.layout.runoff_half_width + WALL_THICKNESS_M
        sides = [self.frame(v) for v in self.vertices("beam-shade")]
        self.assertGreaterEqual(min(abs(b) for _, b, _ in sides), wall_back, "behind the walls")
        self.assertTrue(any(b > 0 for _, b, _ in sides) and any(b < 0 for _, b, _ in sides), "one each side")
        self.assertLessEqual(min(c for *_, c in sides), 0.0)
        self.assertGreaterEqual(max(c for *_, c in sides), TUNNEL_ROOF_M - 1e-9)

    def test_nothing_hangs_over_the_road_lower_than_the_roof(self) -> None:
        edge = self.layout.runoff_half_width
        for p in self.pieces:
            for v in p.mesh[0]:
                _, b, c = self.frame(v)
                if abs(b) < edge:
                    self.assertGreaterEqual(c, TUNNEL_ROOF_M - 1e-9, p.name)

    def test_lamps_line_both_sides_under_the_roof(self) -> None:
        lamps = [p for p in self.pieces if p.material == "lamp"]
        self.assertEqual(len(lamps), 1)
        vertices, faces = lamps[0].mesh
        centres = [[sum(vertices[i][k] for i in f) / len(f) for k in range(3)] for f in faces]
        length = self.tunnel.s_end - self.tunnel.s_start
        per_side = math.floor(length / TUNNEL_LAMP_SPACING_M)
        for side in (1, -1):
            on_side = [c for c in centres if self.frame(c)[1] * side > 0]
            self.assertGreaterEqual(len(on_side), per_side, f"side {side}")
            self.assertTrue(all(0.0 < c[2] < TUNNEL_ROOF_M for c in on_side))

    def test_it_paints_only_from_the_palette(self) -> None:
        self.assertTrue(materials_used(self.pieces) <= build_materials())

    def test_it_never_reaches_the_sidecar(self) -> None:
        self.assertFalse(any(s.name.startswith(TUNNEL) for s in build(self.layout).surfaces))

    def test_a_tunnel_must_run_forward(self) -> None:
        with self.assertRaises(ValueError):
            Tunnel(s_start=10.0, s_end=5.0)


if __name__ == "__main__":
    unittest.main()

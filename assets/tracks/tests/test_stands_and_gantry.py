"""The grandstands and the start gantry, built from boxes and checked
without Blender: the catalog's Grandstand and Start gantry cards in three
dimensions.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from pathlib import Path

from trackgen import mesh
from trackgen.layout import (
    CROWD_KEYS,
    GANTRY_PAST_LINE_M,
    GRANDSTAND_SETBACK_M,
    GRASS_DROP_M,
    WALL_THICKNESS_M,
    build,
)
from trackgen.layouts import load
from trackgen.path import elevation

LIVERY_TS = Path(__file__).resolve().parents[3] / "apps" / "web" / "src" / "livery.ts"
# The only shipped track with stands and a gantry; the oval stays plain.
DRESSED = "brands-patch"


def normal(face: list[mesh.Vertex]) -> mesh.Vertex:
    a, b, c = face[0], face[1], face[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    length = math.sqrt(sum(c * c for c in n))
    return n[0] / length, n[1] / length, n[2] / length


def faces_of(data: mesh.Mesh) -> list[list[mesh.Vertex]]:
    vertices, faces = data
    return [[vertices[v] for v in face] for face in faces]


def stand_samples(layout):
    """The centreline samples along the runs the stands line, so a stand is
    measured from its own leg and never from another leg behind it."""
    path = layout.path
    return [
        path.samples[i]
        for run in layout.decor.grandstands
        for i in path.indices_between(path.segment(run.segments[0]).s_start, path.segment(run.segments[-1]).s_end)
    ]


def pieces(family: str) -> dict[str, object]:
    """The scenery of `family` on the dressed track, by material."""
    geometry = build(load(DRESSED))
    found = {p.material: p for p in geometry.scenery if p.name.startswith(f"{family}.")}
    if not found:
        raise AssertionError(f"{DRESSED} draws no {family}")
    return found


class Cuboid(unittest.TestCase):
    def test_every_face_points_out_of_the_box_whichever_way_its_axes_run(self) -> None:
        # A left-handed frame, as the gantry's across-then-along axes give.
        def corner(i: int, j: int, k: int) -> mesh.Vertex:
            return (3.0 - 2.0 * i, 1.0 + 0.5 * j, 4.0 * k)

        box = mesh.cuboid(corner)
        self.assertEqual(set(box), {"x0", "x1", "y0", "y1", "z0", "z1"})
        centre = (2.0, 1.25, 2.0)
        for side, piece in box.items():
            with self.subTest(side=side):
                (face,) = faces_of(piece)
                middle = [sum(v[axis] for v in face) / 4.0 for axis in range(3)]
                outward = [m - c for m, c in zip(middle, centre)]
                self.assertGreater(sum(n * o for n, o in zip(normal(face), outward)), 0.0)


class Grandstands(unittest.TestCase):
    def test_the_crowd_wears_the_eight_key_colours_of_the_liveries(self) -> None:
        keys = [int(h, 16) for h in re.findall(r"^\s+key: 0x([0-9a-f]{6}),$", LIVERY_TS.read_text(), re.M)]
        self.assertEqual(len(keys), 8)
        self.assertEqual(list(CROWD_KEYS), keys)
        crowd = {m for m in pieces("Grandstand") if m.startswith("crowd-")}
        self.assertEqual(crowd, {f"crowd-{slot}" for slot in range(8)})

    def test_the_stands_are_white_tiers_under_a_kerb_red_canopy(self) -> None:
        stands = pieces("Grandstand")
        self.assertIn("wall", stands)
        canopy_top = max(v[2] for v in stands["kerb-red"].mesh[0])
        self.assertEqual(canopy_top, max(v[2] for p in stands.values() for v in p.mesh[0]))
        for face in faces_of(stands["kerb-red"].mesh):
            self.assertAlmostEqual(normal(face)[2], 1.0, places=6, msg="the canopy's top")

    def test_the_tiers_step_up_away_from_the_track(self) -> None:
        layout = load(DRESSED)
        samples = stand_samples(layout)
        # Each tread's distance from the centreline, by its height.
        levels: dict[float, list[float]] = {}
        for face in faces_of(pieces("Grandstand")["wall"].mesh):
            if normal(face)[2] > 0.99:
                middle = [sum(v[axis] for v in face) / len(face) for axis in range(2)]
                distance = min(math.dist(middle, (s.x, s.y)) for s in samples)
                levels.setdefault(round(face[0][2], 6), []).append(distance)
        self.assertEqual(len(levels), 4, "four tiers")
        mean_distances = [sum(d) / len(d) for _, d in sorted(levels.items())]
        self.assertEqual(mean_distances, sorted(mean_distances), "each higher tier is further back")

    def test_the_stands_stand_on_the_grass_behind_the_wall(self) -> None:
        layout = load(DRESSED)
        wall_back = layout.runoff_half_width + WALL_THICKNESS_M
        vertices = [v for p in pieces("Grandstand").values() for v in p.mesh[0]]
        samples = stand_samples(layout)
        nearest = min(min(math.dist(v[:2], (s.x, s.y)) for s in samples) for v in vertices)
        # The canopy reaches over the front of the stand, still well clear of the wall.
        self.assertGreater(nearest, wall_back + GRANDSTAND_SETBACK_M / 2.0)
        self.assertAlmostEqual(min(v[2] for v in vertices), -GRASS_DROP_M, places=6)


class Gantry(unittest.TestCase):
    def setUp(self) -> None:
        layout = load(DRESSED)
        self.half = layout.asphalt_half_width
        sample = layout.path.at(GANTRY_PAST_LINE_M)
        self.ground = elevation(layout.elevation, sample.s)
        self.x, self.y, self.heading = sample.x, sample.y, sample.heading
        self.gantry = pieces("Gantry")

    def across(self, v: mesh.Vertex) -> float:
        """Metres left of the centreline at the gantry."""
        return -(v[0] - self.x) * math.sin(self.heading) + (v[1] - self.y) * math.cos(self.heading)

    def test_two_carbon_pillars_stand_off_the_asphalt_on_either_side(self) -> None:
        pillars = [v for m in ("carbon", "carbon-lit") for v in self.gantry[m].mesh[0]]
        for side in (1, -1):
            with self.subTest(side=side):
                feet = [v for v in pillars if side * self.across(v) > 0]
                self.assertTrue(feet)
                self.assertGreater(min(side * self.across(v) for v in feet), self.half)
                self.assertLessEqual(min(v[2] for v in feet), self.ground)

    def test_the_beam_clears_the_karts_over_the_asphalt(self) -> None:
        over = [v for p in self.gantry.values() for v in p.mesh[0] if abs(self.across(v)) < self.half]
        self.assertTrue(over)
        self.assertGreater(min(v[2] for v in over), self.ground + 4.0)

    def test_the_beam_shows_the_checkers_up_and_down_the_track(self) -> None:
        forward = (math.cos(self.heading), math.sin(self.heading), 0.0)
        for material in ("checker-dark", "checker-light"):
            facings = {
                round(sum(n * f for n, f in zip(normal(face), forward)))
                for face in faces_of(self.gantry[material].mesh)
            }
            self.assertEqual(facings, {-1, 1}, material)

    def test_the_beam_is_lit_on_top_and_shaded_underneath(self) -> None:
        self.assertTrue(all(normal(f)[2] > 0.99 for f in faces_of(self.gantry["beam-top"].mesh)))
        self.assertTrue(any(normal(f)[2] < -0.99 for f in faces_of(self.gantry["beam-shade"].mesh)))


if __name__ == "__main__":
    unittest.main()

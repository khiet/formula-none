"""Monarch, the course after Monaco, checked without Blender: the lap
docs/mockups/monaco-course.html decided, walled in close by a 2 m run-off,
with one 8 m crest at Casino Square and no ramp; the tunnel, walls and a
roof over the road high enough to clear the chase camera, lit along both sides;
the harbour's water and yachts outside the harbour straight and the pool; and
the casino outside Casino Square, level with the crest.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from pathlib import Path

from dataclasses import replace

from tests.test_goldstone import faces_right
from tests.test_props import across_track, build_materials, faces_up_and_toward_the_sun, footprint_clearance, materials_used
from trackgen.layout import (
    CHASE_CAMERA_ABOVE_M,
    GRANDSTAND_CANOPY_BOTTOM_M,
    GRANDSTAND_SETBACK_M,
    GRANDSTAND_WIDTH_M,
    GRASS_DROP_M,
    PROP_WALL_CLEARANCE_M,
    TUNNEL_LAMP_SPACING_M,
    TUNNEL_ROOF_M,
    WALL_THICKNESS_M,
    Prop,
    Tunnel,
    World,
    along_outside,
    build,
)
from trackgen.layouts import load
from trackgen.props import QUAY_WIDTH_M, YACHT_LENGTHS_M, YACHT_PITCH_M, footprints
from trackgen.path import Hill, Ramp, elevation

NAME = "monarch"
TUNNEL = "Tunnel."
HARBOUR = "Prop.harbour"
CASINO = "Prop.casino"
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


def family(geometry, prefix: str):
    return {p.name: p for p in geometry.scenery if p.name.startswith(f"{prefix}.")}


def vertices_of(pieces) -> list[tuple[float, float, float]]:
    return [v for piece in pieces for v in piece.mesh[0]]


class Props(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        self.geometry = build(self.layout)

    def test_every_footprint_keeps_8_m_of_grass_from_every_wall(self) -> None:
        found = footprints(self.layout)
        self.assertEqual({f.kind for f in found}, {"harbour", "casino"})
        for footprint in found:
            with self.subTest(footprint=footprint):
                self.assertEqual(footprint.clearance, PROP_WALL_CLEARANCE_M)
                self.assertGreaterEqual(footprint_clearance(self.layout, footprint), PROP_WALL_CLEARANCE_M - 1e-9)

    def test_no_grandstand_stands_in_a_footprint(self) -> None:
        for run in self.layout.decor.grandstands:
            for front in along_outside(self.layout, run.segments, run.side, GRANDSTAND_SETBACK_M, GRANDSTAND_WIDTH_M):
                for footprint in footprints(self.layout):
                    with self.subTest(stand=front, footprint=footprint.kind):
                        # Half a stand's width covers its depth and the canopy's overhang too.
                        self.assertGreater(footprint.distance(front.x, front.y), GRANDSTAND_WIDTH_M / 2.0)

    def test_its_props_paint_only_from_the_palette(self) -> None:
        props = [p for p in self.geometry.scenery if p.name.startswith("Prop.")]
        self.assertTrue(props)
        self.assertTrue(materials_used(props) <= build_materials())

    def test_no_prop_reaches_the_sidecar(self) -> None:
        bare = build(replace(self.layout, decor=replace(self.layout.decor, props=[])))
        for what in ("centreline", "surfaces", "checkpoints", "item_boxes", "boost_pads", "spawn_grid", "kerbs", "wheel_crossings", "coins"):
            with self.subTest(what):
                self.assertEqual(getattr(self.geometry, what), getattr(bare, what))


class Harbour(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "harbour"]
        self.pieces = family(build(self.layout), HARBOUR)
        self.water = self.pieces[f"{HARBOUR}.water"]

    def test_the_water_lies_outside_the_harbour_straight_and_the_pool_close_behind_their_stands(self) -> None:
        water = self.water.mesh[0]
        for name in ("harbour", "pool-straight"):
            segment = self.layout.path.segment(name)
            with self.subTest(name):
                self.assertEqual(segment.turn, 0)
                lefts = [across_track(self.layout, segment.s_mid, x, y)[1] for x, y, _ in water]
                # The lap runs clockwise, so the outside is the left; the stands fill the first 18 m.
                self.assertGreater(min(lefts), 18.0)
                self.assertLess(min(lefts), 30.0)

    def test_it_lies_at_sea_level_a_hair_over_the_grass_inside_a_raised_quay(self) -> None:
        for name in ("harbour", "pool-straight"):
            self.assertEqual(elevation(self.layout.elevation, self.layout.path.segment(name).s_mid), 0.0, name)
        heights = {round(z, 6) for _, _, z in self.water.mesh[0]}
        self.assertEqual(len(heights), 1, "flat")
        (sea,) = heights
        self.assertGreater(sea, -GRASS_DROP_M)
        self.assertLess(sea, -GRASS_DROP_M + 0.05)
        quay = vertices_of([self.pieces[f"{HARBOUR}.stand-plinth"]])
        self.assertGreater(max(z for *_, z in quay), sea + 0.3)

    def test_the_quay_rings_the_water(self) -> None:
        def bounds(vertices):
            return (min(v[0] for v in vertices), min(v[1] for v in vertices), max(v[0] for v in vertices), max(v[1] for v in vertices))

        water = bounds(self.water.mesh[0])
        quay = bounds(vertices_of([self.pieces[f"{HARBOUR}.stand-plinth"]]))
        self.assertTrue(quay[0] < water[0] and quay[1] < water[1] and quay[2] > water[2] and quay[3] > water[3])

    def cabins(self) -> list[tuple[float, float]]:
        """The plan middle of each yacht's cabin, from the band of dark windows round it."""
        cabin = self.pieces[f"{HARBOUR}.carbon"]
        vertices, faces = cabin.mesh
        self.assertEqual(len(faces) % 4, 0, "four sides a band")
        middles = []
        for k in range(0, len(faces), 4):
            corners = [vertices[i] for f in faces[k : k + 4] for i in f]
            middles.append((sum(v[0] for v in corners) / len(corners), sum(v[1] for v in corners) / len(corners)))
        return middles

    def test_yachts_moor_along_the_quays_toward_the_lap_as_many_as_each_holds(self) -> None:
        # The basin runs along +x: its north quay faces the harbour straight, its west quay the pool.
        x, y = self.prop.at.x, self.prop.at.y
        north = y + self.prop.width / 2.0 - QUAY_WIDTH_M
        west = x - self.prop.length / 2.0 + QUAY_WIDTH_M
        cabins = self.cabins()
        on_north = [c for c in cabins if north - max(YACHT_LENGTHS_M) < c[1] < north]
        on_west = [c for c in cabins if west < c[0] < west + max(YACHT_LENGTHS_M)]
        self.assertEqual(len(on_north) + len(on_west), len(cabins), "none on the far quays")
        # A 161 m quay holds 18 at a 7 m pitch with 20 m clear at each end; a 174 m one holds 20.
        self.assertEqual((len(on_north), len(on_west)), (18, 20))
        gaps = {round(b[0] - a[0], 6) for a, b in zip(sorted(on_north), sorted(on_north)[1:])}
        self.assertEqual(gaps, {YACHT_PITCH_M})

    def test_every_yacht_floats_inside_the_quay_lower_than_the_stands(self) -> None:
        water = self.water.mesh[0]
        lo = (min(v[0] for v in water), min(v[1] for v in water))
        hi = (max(v[0] for v in water), max(v[1] for v in water))
        sea = water[0][2]
        yachts = [p for name, p in self.pieces.items() if name.split(".")[-1] not in ("water", "stand-plinth", "beam-top")]
        for x, y, z in vertices_of(yachts):
            self.assertTrue(lo[0] < x < hi[0] and lo[1] < y < hi[1], (x, y))
            self.assertTrue(sea <= z < sea + GRANDSTAND_CANOPY_BOTTOM_M)

    def test_it_refuses_a_layout_that_gives_no_size(self) -> None:
        for prop in (Prop("harbour", World(160.0, -20.0)), Prop("harbour", World(160.0, -20.0), length=100.0)):
            short = replace(self.layout, decor=replace(self.layout.decor, props=[prop]))
            with self.subTest(prop=prop), self.assertRaises(ValueError):
                build(short)


class Casino(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "casino"]
        self.pieces = family(build(self.layout), CASINO)
        self.road = elevation(self.layout.elevation, self.prop.at.s)
        self.terrace = self.road - GRASS_DROP_M

    def frame(self, v):
        """(along, left of, up from the terrace) at the casino's spot."""
        along, left = across_track(self.layout, self.prop.at.s, v[0], v[1])
        return along, left, v[2] - self.terrace

    def test_it_stands_outside_casino_square_at_the_crest(self) -> None:
        square = self.layout.path.segment("casino")
        self.assertEqual(square.turn, -1, "a right-hander, so the outside is the left")
        self.assertTrue(square.s_start <= self.prop.at.s <= square.s_end)
        self.assertGreater(self.prop.at.offset, self.layout.runoff_half_width)
        self.assertAlmostEqual(self.road, 8.0, delta=0.1, msg="at the crest")

    def test_its_terrace_rises_from_the_grass_to_the_road_and_the_building_stands_on_it(self) -> None:
        terrace = vertices_of([self.pieces[f"{CASINO}.stand-plinth"]])
        self.assertAlmostEqual(min(z for *_, z in terrace), -GRASS_DROP_M)
        self.assertAlmostEqual(max(z for *_, z in terrace), self.terrace)
        building = vertices_of([p for name, p in self.pieces.items() if name.split(".")[-1].startswith(("stucco", "copper"))])
        self.assertTrue(building)
        self.assertAlmostEqual(min(self.frame(v)[2] for v in building), 0.0)

    def test_its_towers_top_it_24_m_over_the_terrace_under_lit_copper(self) -> None:
        copper = [self.frame(v) for v in vertices_of([p for name, p in self.pieces.items() if ".copper" in name])]
        everything = [self.frame(v) for p in self.pieces.values() for v in p.mesh[0]]
        self.assertAlmostEqual(max(c for *_, c in copper), 24.0, msg="the towers' apexes")
        self.assertAlmostEqual(max(c for *_, c in everything), 24.0)
        lit = self.pieces[f"{CASINO}.copper"]
        self.assertTrue(all(faces_up_and_toward_the_sun([lit.mesh[0][i] for i in f]) for f in lit.mesh[1]))

    def test_its_door_and_windows_face_the_track_on_its_front(self) -> None:
        stucco = [self.frame(v) for name, p in self.pieces.items() if ".stucco" in name for v in p.mesh[0]]
        front = min(b for _, b, _ in stucco)
        dark = self.pieces[f"{CASINO}.carbon"]
        self.assertGreaterEqual(len(dark.mesh[1]), 7, "a door and windows")
        for f in dark.mesh[1]:
            panel = [self.frame(dark.mesh[0][i]) for i in f]
            self.assertLess(max(b for _, b, _ in panel), front, "proud of the front")
            self.assertTrue(faces_right(panel), "toward the track, which is to the right")

    def test_it_letters_nothing(self) -> None:
        self.assertFalse(any(t.name.startswith(f"{CASINO}.") for t in build(self.layout).lettering))


if __name__ == "__main__":
    unittest.main()

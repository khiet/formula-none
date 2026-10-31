"""Monarch, the course after Monaco, checked without Blender: the lap
docs/mockups/monaco-course.html decided, walled in close by a 2 m run-off,
with one 8 m crest at Casino Square and no ramp; the tunnel, walls and a
roof over the road high enough to clear the chase camera, lit along both sides;
the harbour's water and yachts outside the harbour straight and the pool;
the casino outside Casino Square, level with the crest; and the helicopter
circling between them, high enough to clear every road with its rotors at
any angle, its sponsor on both cabin flanks.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from pathlib import Path

from dataclasses import replace

from tests.test_goldstone import faces_right
from tests.test_props import across_track, build_materials, faces_by_material, faces_up_and_toward_the_sun, footprint_clearance, materials_used
from trackgen.layout import (
    CHASE_CAMERA_ABOVE_M,
    GRANDSTAND_CANOPY_BOTTOM_M,
    GRANDSTAND_SETBACK_M,
    GRANDSTAND_WIDTH_M,
    GRASS_DROP_M,
    HOARDING_TEXT_PROUD_M,
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
from trackgen.props import (
    HELICOPTER_CABIN_WIDTH_M,
    HELICOPTER_CLEARANCE_M,
    HELICOPTER_DIP_DEG,
    HELICOPTER_LENGTH_M,
    HELICOPTER_MAIN_BLADE,
    HELICOPTER_SKIDS,
    HELICOPTER_TAIL_BLADE,
    HELICOPTER_TEXT_SIZE_M,
    YACHT_OFF_QUAY_M,
    footprints,
)
from trackgen.sponsors import HELICOPTER_SPONSOR, SPONSORS
from trackgen.path import Hill, Ramp, elevation

NAME = "monarch"
TUNNEL = "Tunnel."
HARBOUR = "Prop.harbour"
CASINO = "Prop.casino"
HELICOPTER = "Prop.helicopter.1"
WEB_SRC = Path(__file__).resolve().parents[3] / "apps" / "web" / "src"
CHASE_CAMERA_TS = WEB_SRC / "chase-camera.ts"
SCENERY_MOTION_TS = WEB_SRC / "scenery-motion.ts"


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


def distance_to_outline(outline, x: float, y: float) -> float:
    """From the point to the nearest edge of the closed outline."""
    best = math.inf
    for (ax, ay), (bx, by) in zip(outline, outline[1:] + outline[:1]):
        dx, dy = bx - ax, by - ay
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy or 1.0)))
        best = min(best, math.dist((x, y), (ax + t * dx, ay + t * dy)))
    return best


def inside(outline, x: float, y: float) -> bool:
    crossings = 0
    for (ax, ay), (bx, by) in zip(outline, outline[1:] + outline[:1]):
        if (ay > y) != (by > y) and x < ax + (y - ay) * (bx - ax) / (by - ay):
            crossings += 1
    return crossings % 2 == 1


def convex_overlap(a, b) -> bool:
    """Whether two convex outlines overlap, by the separating axis test."""
    for outline in (a, b):
        for (x0, y0), (x1, y1) in zip(outline, outline[1:] + outline[:1]):
            axis = (y0 - y1, x1 - x0)
            pa = [axis[0] * x + axis[1] * y for x, y in a]
            pb = [axis[0] * x + axis[1] * y for x, y in b]
            if max(pa) <= min(pb) or max(pb) <= min(pa):
                return False
    return True


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
        water = self.water_outline()
        for name in ("harbour", "pool-straight"):
            segment = self.layout.path.segment(name)
            sample = self.layout.path.at(segment.s_mid)
            with self.subTest(name):
                self.assertEqual(segment.turn, 0)
                # The lap runs clockwise, so the outside is the left; the stands fill the first 18 m.
                self.assertTrue(inside(water, *sample.offset(32.0)))
                self.assertGreater(distance_to_outline(water, sample.x, sample.y), 18.0)
                self.assertLess(distance_to_outline(water, sample.x, sample.y), 30.0)

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

    def water_outline(self) -> list[tuple[float, float]]:
        vertices, (face,) = self.water.mesh
        return [vertices[i][:2] for i in face]

    def decks(self) -> list[list[tuple[float, float]]]:
        """Each yacht's deck in plan: the beam-top faces with five corners, the hull's outline."""
        vertices, faces = self.pieces[f"{HARBOUR}.beam-top"].mesh
        return [[vertices[i][:2] for i in f] for f in faces if len(f) == 5]

    def test_the_water_comes_within_21_m_of_the_quay_and_rascasse(self) -> None:
        # Their walls stand 8.5 m out, then the 8 m rule and the 2 m quay: 18.5 m at the nearest.
        water = self.water_outline()
        for name in ("quay", "rascasse"):
            sample = self.layout.path.at(self.layout.path.segment(name).s_mid)
            with self.subTest(name):
                self.assertLess(distance_to_outline(water, sample.x, sample.y), 21.0)

    def test_yachts_moor_stern_to_along_the_quays_toward_the_lap(self) -> None:
        water = self.water_outline()
        decks = self.decks()
        self.assertEqual(len(decks), len(self.cabins()), "a deck and a cabin each")
        for deck in decks:
            self.assertAlmostEqual(min(distance_to_outline(water, x, y) for x, y in deck), YACHT_OFF_QUAY_M, places=6)
        near_the_quay = [d for d in decks if min(distance_to_outline([(q.x, q.y) for q in self.quay_samples()], x, y) for x, y in d) < 30.0]
        self.assertGreaterEqual(len(near_the_quay), 5, "yachts line the quay the lap runs along")
        nearest_far_quay = min(math.dist(d[0], (200.0, -120.0)) for d in decks)
        self.assertGreater(nearest_far_quay, 40.0, "none on the far quays")

    def quay_samples(self):
        segment = self.layout.path.segment("quay")
        return [self.layout.path.samples[i] for i in self.layout.path.indices_between(segment.s_start, segment.s_end)]

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

    def test_no_two_yachts_touch_and_every_one_floats_inside_the_quay(self) -> None:
        water = self.water_outline()
        decks = self.decks()
        for k, deck in enumerate(decks):
            self.assertTrue(all(inside(water, x, y) for x, y in deck), deck)
            for other in decks[k + 1 :]:
                self.assertFalse(convex_overlap(deck, other), (deck, other))

    def test_every_yacht_stands_lower_than_the_stands(self) -> None:
        sea = self.water.mesh[0][0][2]
        yachts = [p for name, p in self.pieces.items() if name.split(".")[-1] in ("wall", "carbon")]
        for *_, z in vertices_of(yachts):
            self.assertTrue(sea <= z < sea + GRANDSTAND_CANOPY_BOTTOM_M)

    def test_every_yachts_windows_band_its_cabin_over_the_hull(self) -> None:
        sea = self.water.mesh[0][0][2]
        windows = [z - sea for *_, z in self.pieces[f"{HARBOUR}.carbon"].mesh[0]]
        # The hull's freeboard is 1.4 m and the cabin on it 1.3 m tall.
        self.assertGreater(min(windows), 1.4)
        self.assertLess(max(windows), 1.4 + 1.3)

    def test_it_refuses_a_layout_that_gives_no_outline(self) -> None:
        short = replace(self.layout, decor=replace(self.layout.decor, props=[Prop("harbour", World(160.0, -20.0))]))
        with self.assertRaises(ValueError):
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

    def test_it_refuses_a_place_off_the_lap_since_its_terrace_reads_the_road(self) -> None:
        astray = replace(self.layout, decor=replace(self.layout.decor, props=[Prop("casino", World(140.0, 260.0))]))
        with self.assertRaises(ValueError):
            build(astray)


def turned(v, pivot, axis, angle: float):
    """`v` turned by `angle` radians about the unit `axis` through `pivot`, right-handed."""
    d = [v[i] - pivot[i] for i in range(3)]
    cos, sin = math.cos(angle), math.sin(angle)
    dot = sum(d[i] * axis[i] for i in range(3))
    cross = (axis[1] * d[2] - axis[2] * d[1], axis[2] * d[0] - axis[0] * d[2], axis[0] * d[1] - axis[1] * d[0])
    return tuple(pivot[i] + d[i] * cos + cross[i] * sin + axis[i] * dot * (1.0 - cos) for i in range(3))


class Helicopter(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "helicopter"]
        self.geometry = build(self.layout)
        self.pieces = family(self.geometry, HELICOPTER)
        self.body = self.pieces[f"{HELICOPTER}.body"]
        self.main = self.pieces[f"{HELICOPTER}.main-rotor"]
        self.tail = self.pieces[f"{HELICOPTER}.tail-rotor"]
        self.lettering = [t for t in self.geometry.lettering if t.name.startswith(f"{HELICOPTER}.")]
        pivot = self.body.pivot
        self.nose = (pivot.x, pivot.y, pivot.z)
        self.left = (-math.sin(pivot.heading), math.cos(pivot.heading), 0.0)

    def frame(self, v):
        """A vertex as (along the nose's heading, left of it, up from the nose) from the body's pivot."""
        pivot = self.body.pivot
        dx, dy = v[0] - pivot.x, v[1] - pivot.y
        cos, sin = math.cos(pivot.heading), math.sin(pivot.heading)
        return dx * cos + dy * sin, -dx * sin + dy * cos, v[2] - pivot.z

    def test_it_circles_40_m_up_round_a_100_m_circle_turning_left_over_the_harbour_and_near_the_casino(self) -> None:
        self.assertEqual(self.prop.at, World(170.0, 120.0))
        self.assertEqual((self.prop.radius, self.prop.height), (100.0, 40.0))
        pivot = self.body.pivot
        self.assertIsNone(pivot.parent)
        centre = (self.prop.at.x, self.prop.at.y)
        self.assertAlmostEqual(math.dist((pivot.x, pivot.y), centre), self.prop.radius)
        self.assertAlmostEqual(pivot.z, -GRASS_DROP_M + self.prop.height)
        # The nose along the circle with its centre to the left, so it turns anticlockwise.
        along = (math.cos(pivot.heading), math.sin(pivot.heading))
        to_centre = (centre[0] - pivot.x, centre[1] - pivot.y)
        self.assertAlmostEqual(along[0] * to_centre[0] + along[1] * to_centre[1], 0.0)
        self.assertGreater(along[0] * to_centre[1] - along[1] * to_centre[0], 0.0)
        circle = [(centre[0] + self.prop.radius * math.cos(math.tau * k / 720), centre[1] + self.prop.radius * math.sin(math.tau * k / 720)) for k in range(720)]
        found = {f.kind: f for f in footprints(self.layout)}
        self.assertTrue(any(inside(found["harbour"].outline, x, y) for x, y in circle), "over the basin")
        self.assertLess(min(found["casino"].distance(x, y) for x, y in circle), 30.0, "close by the casino")
        # It crosses the track, so the altitude rule below is not idle.
        crossed = [s for s in self.layout.path.samples if abs(math.dist((s.x, s.y), centre) - self.prop.radius) < self.layout.asphalt_half_width]
        self.assertGreater(len(crossed), 0)

    def test_the_mirrors_of_its_circle_and_dip_match_the_client(self) -> None:
        source = SCENERY_MOTION_TS.read_text()
        circle = re.search(r"export const HELICOPTER_CIRCLE_M = ([0-9.]+);", source)
        dip = re.search(r"export const HELICOPTER_DIP_DEG = ([0-9.]+);", source)
        self.assertIsNotNone(circle)
        self.assertIsNotNone(dip)
        self.assertEqual(float(circle.group(1)), self.prop.radius)
        self.assertEqual(float(dip.group(1)), HELICOPTER_DIP_DEG)

    def test_the_body_is_a_node_with_its_origin_at_the_nose_and_both_rotors_hung_under_it(self) -> None:
        body = [self.frame(v) for v in self.body.mesh[0]]
        self.assertAlmostEqual(max(a for a, _, _ in body), 0.0)
        self.assertAlmostEqual(min(a for a, _, _ in body), -HELICOPTER_LENGTH_M)
        _, apart, _, bar = HELICOPTER_SKIDS
        self.assertGreater(apart - bar / 2.0, HELICOPTER_CABIN_WIDTH_M / 2.0)
        self.assertAlmostEqual(max(abs(b) for _, b, _ in body), apart + bar / 2.0, msg="the skids stand clear of the cabin")
        nose = [(b, c) for a, b, c in body if abs(a) < 1e-9]
        self.assertTrue(nose and all(abs(c) < 1e-9 for _, c in nose), "the origin is the nose")
        self.assertEqual(set(self.pieces), {f"{HELICOPTER}.body", f"{HELICOPTER}.main-rotor", f"{HELICOPTER}.tail-rotor"})
        for rotor in (self.main, self.tail):
            with self.subTest(rotor=rotor.name):
                self.assertEqual(rotor.pivot.parent, self.body.name)
                self.assertEqual(rotor.pivot.heading, self.body.pivot.heading)
        # The main rotor over the cabin: two blades in the plane level with its hub, 11 m tip to tip.
        hub = self.frame((self.main.pivot.x, self.main.pivot.y, self.main.pivot.z))
        self.assertGreater(hub[2], max(c for a, _, c in body if abs(a - hub[0]) < 1.0), "on top of the mast")
        blades = [self.frame(v) for v in self.main.mesh[0]]
        reach, _, thick = HELICOPTER_MAIN_BLADE
        self.assertTrue(all(abs(c - hub[2]) <= thick / 2.0 + 1e-9 for _, _, c in blades))
        self.assertAlmostEqual(max(a for a, _, _ in blades) - min(a for a, _, _ in blades), 2.0 * reach)
        self.assertEqual(2.0 * reach, 11.0)
        self.assertEqual(len(self.main.mesh[1]), 12, "two blades of six faces")
        # The tail rotor beside the fin: two blades in the upright plane its hub stands in.
        hub = self.frame((self.tail.pivot.x, self.tail.pivot.y, self.tail.pivot.z))
        blades = [self.frame(v) for v in self.tail.mesh[0]]
        reach, _, thick = HELICOPTER_TAIL_BLADE
        self.assertGreater(hub[1], 0.0, "on the left of the fin")
        self.assertTrue(all(abs(b - hub[1]) <= thick / 2.0 + 1e-9 for _, b, _ in blades))
        self.assertAlmostEqual(max(a for a, _, _ in blades) - min(a for a, _, _ in blades), 2.0 * reach)
        self.assertEqual(len(self.tail.mesh[1]), 12, "two blades of six faces")
        self.assertLess(hub[0], -HELICOPTER_LENGTH_M + 1.0, "at the tail")

    def test_it_never_flies_under_30_m_over_any_road_dipped_with_its_rotors_at_any_angle(self) -> None:
        up = (0.0, 0.0, 1.0)
        main_hub = (self.main.pivot.x, self.main.pivot.y, self.main.pivot.z)
        tail_hub = (self.tail.pivot.x, self.tail.pivot.y, self.tail.pivot.z)
        points = list(self.body.mesh[0])
        for step in range(36):
            angle = math.tau * step / 36
            points += [turned(v, main_hub, up, angle) for v in self.main.mesh[0]]
            points += [turned(v, tail_hub, self.left, angle) for v in self.tail.mesh[0]]
        # A right-handed turn about the left axis puts the nose down.
        dip = math.radians(HELICOPTER_DIP_DEG)
        self.assertLess(turned(self.frame_to_world(1.0, 0.0, 0.0), self.nose, self.left, dip)[2], self.nose[2])
        # Level too, as the export leaves it before the View first dips it.
        lowest = min(min(turned(v, self.nose, self.left, dip)[2] for v in points), min(v[2] for v in points))
        centre = (self.prop.at.x, self.prop.at.y)
        reach = self.prop.radius + HELICOPTER_LENGTH_M + HELICOPTER_MAIN_BLADE[0]
        under = [s for s in self.layout.path.samples if math.dist((s.x, s.y), centre) <= reach]
        self.assertGreater(len(under), 0)
        road = max(elevation(self.layout.elevation, s.s) for s in under)
        self.assertAlmostEqual(road, 8.0, delta=0.1, msg="its circle reaches the crest")
        self.assertGreaterEqual(lowest - road, HELICOPTER_CLEARANCE_M)
        self.assertEqual(HELICOPTER_CLEARANCE_M, 30.0)

    def frame_to_world(self, a: float, b: float, c: float):
        """The world point `a` along the nose's heading, `b` left of it and `c` up from the nose."""
        heading = self.body.pivot.heading
        return (self.nose[0] + a * math.cos(heading) + b * self.left[0], self.nose[1] + a * math.sin(heading) + b * self.left[1], self.nose[2] + c)

    def test_it_is_wall_white_lit_over_beam_shade_with_a_kerb_red_stripe_and_fin_and_carbon_glass_skids_and_rotors(self) -> None:
        by_material = faces_by_material(self.body)
        self.assertEqual(set(by_material), {"wall", "beam-shade", "kerb-red", "carbon", "carbon-shade"})
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())
        self.assertTrue(all(faces_up_and_toward_the_sun(f) for f in by_material["wall"]))
        self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in by_material["beam-shade"]))
        self.assertEqual(materials_used([self.main, self.tail]), {"carbon"})
        # The stripe runs down both flanks and the fin stands at the tail.
        red = [[self.frame(v) for v in f] for f in by_material["kerb-red"]]
        flanks = {round(f[0][1], 3) for f in red if all(abs(b - f[0][1]) < 1e-9 for _, b, _ in f) and abs(f[0][1]) > HELICOPTER_CABIN_WIDTH_M / 2.0}
        self.assertEqual(len(flanks), 2, "a stripe proud of each flank")
        self.assertTrue(any(min(a for a, _, _ in f) < -HELICOPTER_LENGTH_M + 1.0 for f in red), "the fin")
        # The skids are the lowest of it.
        lowest = min(c for f in by_material["carbon"] + by_material["carbon-shade"] for _, _, c in (self.frame(v) for v in f))
        self.assertAlmostEqual(lowest, min(self.frame(v)[2] for v in self.body.mesh[0]))

    def test_box_box_box_logistics_from_sponsors_py_reads_on_both_cabin_flanks_and_rides_the_body(self) -> None:
        self.assertIn(HELICOPTER_SPONSOR, SPONSORS)
        name, ink = HELICOPTER_SPONSOR
        self.assertEqual((name, ink), ("BOX BOX BOX LOGISTICS", "ink-red"))
        self.assertEqual({t.name for t in self.lettering}, {f"{HELICOPTER}.flank.left", f"{HELICOPTER}.flank.right"})
        sides = set()
        cabin = [self.frame(v) for v in self.body.mesh[0] if abs(abs(self.frame(v)[1]) - HELICOPTER_CABIN_WIDTH_M / 2.0) < 1e-9]
        for text in self.lettering:
            with self.subTest(text=text.name):
                self.assertEqual(text.parent, self.body.name)
                self.assertEqual((text.text, text.material, text.size), (name, ink, HELICOPTER_TEXT_SIZE_M))
                self.assertIsNone(text.arc)
                along, left, up = self.frame((text.at.x, text.at.y, text.at.z))
                self.assertAlmostEqual(abs(left), HELICOPTER_CABIN_WIDTH_M / 2.0 + HOARDING_TEXT_PROUD_M)
                # On the cabin's flank, not the boom's.
                self.assertTrue(min(a for a, _, _ in cabin) < along < 0.0)
                self.assertTrue(min(c for _, _, c in cabin) < up < max(c for _, _, c in cabin))
                outward = math.sin(text.at.heading - self.body.pivot.heading)
                self.assertAlmostEqual(outward, math.copysign(1.0, left))
                sides.add(round(left, 3))
        self.assertEqual(len(sides), 2)

    def test_it_refuses_a_layout_that_gives_no_circle_or_no_height(self) -> None:
        for prop in (Prop("helicopter", World(170.0, 120.0), height=40.0), Prop("helicopter", World(170.0, 120.0), radius=100.0)):
            with self.subTest(prop=prop), self.assertRaises(ValueError):
                build(replace(self.layout, decor=replace(self.layout.decor, props=[prop])))

    def test_the_helicopter_costs_about_80_faces(self) -> None:
        faces = sum(len(p.mesh[1]) for p in self.pieces.values())
        self.assertTrue(70 <= faces <= 100, faces)


if __name__ == "__main__":
    unittest.main()

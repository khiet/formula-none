"""Goldstone, the course after Silverstone, checked without Blender: the lap
docs/mockups/silverstone-course.html decided, flat and with no ramp; the
pit building drawn white inside the Hamilton Straight, facing the grid;
and the row of airfield hangars outside the Hangar Straight, each a
barrel-roofed shed with its door toward the track, every footprint keeping
the 8 m rule; the hangar shell the straight drives through, its sides and
parked aircraft beyond the walls, its ribs overhead clear of a boosted
chase camera and its exit framing Stowe; the control tower that ends the
Wellington Straight's view; and the display jets' formation over the
hangars, flying against the race and keeping 30 m over every road its pass
crosses.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from dataclasses import replace
from pathlib import Path

from tests.test_boost_pads import tuning_constant
from tests.test_bridge import triangles_of, unhidden
from tests.test_monarch import WEB_SRC, camera_at, chase_camera_behind_m, half_field_of_view
from tests.test_props import (
    across_track,
    edge_of,
    faces_left,
    faces_right,
    build_materials,
    faces_up_and_toward_the_sun,
    footprint_clearance,
    materials_used,
)
from trackgen.layout import (
    CHASE_CAMERA_ABOVE_M,
    CROWD_KEYS,
    GANTRY_BEAM_HEIGHT_M,
    GANTRY_TOP_M,
    GRASS_DROP_M,
    PROP_WALL_CLEARANCE_M,
    SPAN_FOOT_CLEARANCE_M,
    WALL_THICKNESS_M,
    Prop,
    Spot,
    World,
    build,
    locate,
)
from trackgen.layouts import load
from trackgen.props import (
    HANGAR_DEPTH_M,
    HANGAR_DOOR_PROUD_M,
    HANGAR_EAVES_M,
    HANGAR_PITCH_M,
    HANGAR_RIDGE_M,
    HANGAR_SHELL_EAVES_M,
    HANGAR_SHELL_FRAME_PITCH_M,
    HANGAR_SHELL_RIGHT_M,
    HANGAR_WIDTH_M,
    JET_LENGTH_M,
    JETS_CLEARANCE_M,
    JETS_FORMATION,
    JETS_RIBBON,
    JETS_SMOKE,
    PITS_DEPTH_M,
    PITS_DOOR_PROUD_M,
    TOWER_CAB_M,
    TOWER_CAB_TOP_M,
    TOWER_SHAFT_M,
    TOWER_SHAFT_TOP_M,
    footprints,
)
from trackgen.path import elevation

NAME = "goldstone"
PITS = "Prop.pits"
HANGARS = "Prop.hangars"
SHELL = "Prop.hangar-shell"
TOWER = "Prop.control-tower"
PRESENTATION_ZONES_TS = WEB_SRC / "presentation-zones.ts"
# Under 3 m tall, a prop keeps no 8 m from the wall: the chase camera sees over it.
LOW_PROP_M = 3.0
JETS = "Prop.jets.1"
SCENERY_MOTION_TS = Path(__file__).resolve().parents[3] / "apps" / "web" / "src" / "scenery-motion.ts"


def family(geometry, prefix: str):
    return {p.name: p for p in geometry.scenery if p.name.startswith(f"{prefix}.")}


class Lap(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        self.geometry = build(self.layout)

    def test_it_closes_at_about_1323_m_clockwise(self) -> None:
        self.assertAlmostEqual(self.layout.path.length, 1323.4, delta=0.5)
        points = [(q.x, q.y) for q in self.layout.path.samples]
        area = sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(points, points[1:] + points[:1])) / 2.0
        self.assertLess(area, 0.0, "clockwise")

    def test_it_lays_its_boxes_pads_kerbs_checkpoints_and_grid(self) -> None:
        g = self.geometry
        counts = (len(g.item_boxes), len(g.boost_pads), len(g.kerbs), len(g.checkpoints), len(g.spawn_grid))
        self.assertEqual(counts, (12, 2, 3, 15, 8))
        self.assertEqual(self.layout.asphalt_half_width, 6.0)
        self.assertEqual(self.layout.runoff_width, 6.0)

    def test_it_is_flat_with_no_ramp_or_bridge(self) -> None:
        self.assertEqual(self.layout.elevation, [])
        self.assertEqual(self.layout.decor.bridges, [])

    def test_every_tall_footprint_keeps_8_m_of_grass_from_every_wall(self) -> None:
        found = footprints(self.layout)
        self.assertEqual({f.kind for f in found}, {"pits", "hangars", "hangar-shell", "control-tower"})
        for footprint in (f for f in found if f.kind != "hangar-shell"):
            with self.subTest(footprint=footprint):
                self.assertEqual(footprint.clearance, PROP_WALL_CLEARANCE_M)
                self.assertGreaterEqual(footprint_clearance(self.layout, footprint), PROP_WALL_CLEARANCE_M - 1e-9)

    def test_its_props_paint_only_from_the_palette(self) -> None:
        props = [p for p in self.geometry.scenery if p.name.startswith("Prop.")]
        self.assertTrue(materials_used(props) <= build_materials())


class WhitePits(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "pits"]
        self.pieces = family(build(self.layout), PITS)
        self.ground = -GRASS_DROP_M

    def frame(self, v):
        along, left = across_track(self.layout, self.prop.at.s, v[0], v[1])
        return along, left, v[2] - self.ground

    def test_it_stands_inside_the_hamilton_straight_between_club_and_abbey(self) -> None:
        lap = self.layout.path.length
        self.assertLess(self.prop.at.s + self.prop.length / 2.0, self.layout.path.segment("abbey").s_mid)
        self.assertGreater(self.prop.at.s - self.prop.length / 2.0, self.layout.path.segment("club").s_mid - lap)
        self.assertLess(self.prop.at.offset, -self.layout.runoff_half_width, "right of the track, on the inside")
        vertices = [self.frame(v) for piece in self.pieces.values() for v in piece.mesh[0]]
        self.assertAlmostEqual(min(a for a, _, _ in vertices), -self.prop.length / 2.0)
        self.assertAlmostEqual(max(a for a, _, _ in vertices), self.prop.length / 2.0)
        self.assertLessEqual(max(b for _, b, _ in vertices), self.prop.at.offset + PITS_DEPTH_M / 2.0 + PITS_DOOR_PROUD_M + 1e-6)

    def test_its_walls_are_wall_white_over_beam_shade(self) -> None:
        names = set(self.pieces)
        self.assertIn(f"{PITS}.wall", names)
        self.assertIn(f"{PITS}.beam-shade", names)
        self.assertNotIn(f"{PITS}.stand-riser", names)
        shaded = self.pieces[f"{PITS}.beam-shade"]
        faces = [[shaded.mesh[0][i] for i in f] for f in shaded.mesh[1]]
        self.assertTrue(faces)
        self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in faces))

    def test_its_doors_face_the_track_centred_on_it(self) -> None:
        front = self.prop.at.offset + PITS_DEPTH_M / 2.0 + PITS_DOOR_PROUD_M
        centres = []
        for slot in range(len(CROWD_KEYS)):
            piece = self.pieces[f"{PITS}.crowd-{slot}"]
            (door,) = [f for f in piece.mesh[1] if min(piece.mesh[0][v][2] for v in f) - self.ground < 1e-6]
            panel = [piece.mesh[0][v] for v in door]
            self.assertEqual({round(self.frame(v)[1], 3) for v in panel}, {round(front, 3)})
            self.assertTrue(faces_left([self.frame(v) for v in panel]), f"door {slot} faces the track")
            centres.append(sum(self.frame(v)[0] for v in panel) / len(panel))
        self.assertAlmostEqual(centres[0] + centres[-1], 0.0, places=6)
        self.assertEqual(centres, sorted(centres))

    def test_its_flags_face_the_track(self) -> None:
        for slot in range(len(CROWD_KEYS)):
            piece = self.pieces[f"{PITS}.crowd-{slot}"]
            (flag,) = [f for f in piece.mesh[1] if min(piece.mesh[0][v][2] for v in f) - self.ground > 1.0]
            self.assertTrue(faces_left([self.frame(piece.mesh[0][v]) for v in flag]), f"flag {slot} faces the track")


class Hangars(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "hangars"]
        self.pieces = family(build(self.layout), HANGARS)
        self.ground = -GRASS_DROP_M
        self.straight = self.layout.path.segment("hangar")

    def frame(self, v):
        along, left = across_track(self.layout, self.prop.at.s, v[0], v[1])
        return along, left, v[2] - self.ground

    def test_the_row_stands_outside_the_hangar_straight(self) -> None:
        self.assertEqual(self.straight.turn, 0)
        self.assertGreater(self.prop.at.offset, self.layout.runoff_half_width, "the lap turns right, so the outside is the left")
        half = self.prop.length / 2.0
        self.assertLessEqual(self.straight.s_start, self.prop.at.s - half)
        self.assertLessEqual(self.prop.at.s + half, self.straight.s_end)
        vertices = [self.frame(v) for piece in self.pieces.values() for v in piece.mesh[0]]
        self.assertGreaterEqual(min(c for _, _, c in vertices), -1e-9, "on the grass")
        self.assertAlmostEqual(max(c for _, _, c in vertices), HANGAR_RIDGE_M)
        self.assertGreaterEqual(min(b for _, b, _ in vertices), self.prop.at.offset - HANGAR_DEPTH_M / 2.0 - HANGAR_DOOR_PROUD_M - 1e-6)
        self.assertLessEqual(max(b for _, b, _ in vertices), self.prop.at.offset + HANGAR_DEPTH_M / 2.0 + 1e-6)

    def test_as_many_hangars_as_the_length_holds_stand_at_the_pitch(self) -> None:
        count = math.floor((self.prop.length - HANGAR_WIDTH_M) / HANGAR_PITCH_M) + 1
        self.assertGreaterEqual(count, 4)
        doors = self.pieces[f"{HANGARS}.carbon"]
        self.assertEqual(len(doors.mesh[1]), count, "one door a hangar")
        middles = sorted(sum(self.frame(doors.mesh[0][v])[0] for v in f) / len(f) for f in doors.mesh[1])
        gaps = {round(b - a, 6) for a, b in zip(middles, middles[1:])}
        self.assertEqual(gaps, {HANGAR_PITCH_M})
        self.assertAlmostEqual(middles[0] + middles[-1], 0.0, msg="the row centred on its spot")

    def test_each_door_is_on_the_gable_toward_the_track_and_wound_to_face_it(self) -> None:
        doors = self.pieces[f"{HANGARS}.carbon"]
        front = self.prop.at.offset - HANGAR_DEPTH_M / 2.0 - HANGAR_DOOR_PROUD_M
        for f in doors.mesh[1]:
            panel = [self.frame(doors.mesh[0][v]) for v in f]
            self.assertEqual({round(b, 3) for _, b, _ in panel}, {round(front, 3)})
            self.assertAlmostEqual(min(c for _, _, c in panel), 0.0)
            self.assertLess(max(c for _, _, c in panel), HANGAR_EAVES_M + 1e-9)
            self.assertTrue(faces_right(panel))

    def test_the_barrel_roofs_are_beam_top_lit_over_beam_shade_on_toned_stand_walls(self) -> None:
        self.assertEqual(
            set(self.pieces),
            {f"{HANGARS}.{m}" for m in ("beam-top", "beam-shade", "stand-riser", "stand-plinth", "carbon")},
        )
        top = self.pieces[f"{HANGARS}.beam-top"]
        lit = [[top.mesh[0][i] for i in f] for f in top.mesh[1]]
        self.assertTrue(all(faces_up_and_toward_the_sun(f) for f in lit))
        self.assertTrue(all(min(v[2] for v in f) - self.ground >= HANGAR_EAVES_M - 1e-9 for f in lit), "roof above the eaves")

    def test_every_face_points_out_of_its_hangar(self) -> None:
        doors = self.pieces[f"{HANGARS}.carbon"]
        centres = [sum(self.frame(doors.mesh[0][v])[0] for v in f) / len(f) for f in doors.mesh[1]]
        for piece in self.pieces.values():
            for f in piece.mesh[1]:
                local = [self.frame(piece.mesh[0][v]) for v in f]
                middle = [sum(p[i] for p in local) / len(local) for i in range(3)]
                # The middle of the floor of the hangar the face belongs to.
                hub = (min(centres, key=lambda c: abs(c - middle[0])), self.prop.at.offset, 0.0)
                a, b, c = local[0], local[1], local[2]
                u = [b[i] - a[i] for i in range(3)]
                w = [c[i] - a[i] for i in range(3)]
                # (along, left, up) is right-handed, as the track's (x, y, z) is.
                n = (u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0])
                self.assertGreater(sum(n[i] * (middle[i] - hub[i]) for i in range(3)), 0.0, piece.name)

    def test_the_row_letters_nothing(self) -> None:
        self.assertFalse(any(t.name.startswith(f"{HANGARS}.") for t in build(self.layout).lettering))

    def test_it_refuses_a_layout_that_gives_no_length(self) -> None:
        short = replace(self.layout, decor=replace(self.layout.decor, props=[Prop("hangars", Spot(s=900.0, offset=33.0))]))
        with self.assertRaises(ValueError):
            build(short)

    def test_a_hangar_costs_about_a_dozen_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        count = len(self.pieces[f"{HANGARS}.carbon"].mesh[1])
        self.assertTrue(10 * count <= faces <= 16 * count, faces)


def clusters(values, gap: float) -> list[float]:
    """The middles of the runs of sorted `values` no more than `gap` apart."""
    runs: list[list[float]] = []
    for value in sorted(values):
        if runs and value - runs[-1][-1] <= gap:
            runs[-1].append(value)
        else:
            runs.append([value])
    return [(run[0] + run[-1]) / 2.0 for run in runs]


class HangarShell(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "hangar-shell"]
        self.geometry = build(self.layout)
        self.pieces = family(self.geometry, SHELL)
        self.straight = self.layout.path.segment("hangar")
        self.walls = self.layout.runoff_half_width + WALL_THICKNESS_M
        self.vertices = [self.frame(v) for piece in self.pieces.values() for v in piece.mesh[0]]

    def frame(self, v):
        """A vertex as (along the track from the shell's middle, left of the centreline, up from the road)."""
        along, left = across_track(self.layout, self.prop.at.s, v[0], v[1])
        return along, left, v[2]

    def faces(self, material: str):
        piece = self.pieces[f"{SHELL}.{material}"]
        return [[self.frame(piece.mesh[0][i]) for i in f] for f in piece.mesh[1]]

    def test_it_spans_the_middle_of_the_hangar_straight_well_short_of_stowe(self) -> None:
        self.assertEqual(self.straight.turn, 0)
        self.assertAlmostEqual(self.prop.at.s, self.straight.s_mid)
        self.assertEqual(self.prop.at.offset, 0.0)
        half = self.prop.length / 2.0
        self.assertAlmostEqual(min(a for a, _, _ in self.vertices), -half)
        self.assertAlmostEqual(max(a for a, _, _ in self.vertices), half)
        self.assertGreaterEqual(self.straight.s_end - (self.prop.at.s + half), 50.0, "Stowe's braking in the open")
        self.assertGreater(max(b for _, b, _ in self.vertices), self.walls)
        self.assertLess(min(b for _, b, _ in self.vertices), -self.walls)

    def test_everything_under_the_roof_stands_on_grass_beyond_the_walls(self) -> None:
        """Its sides, its frames' legs and the aircraft: nothing below the span rule's height
        stands over the road or the run-off."""
        for along, left, up in self.vertices:
            if up < GANTRY_TOP_M - GANTRY_BEAM_HEIGHT_M:
                self.assertGreater(abs(left), self.walls, (along, left, up))
        feet = [f for f in footprints(self.layout) if f.kind == "hangar-shell"]
        self.assertEqual(len(feet), 4, "two sides and two aircraft")
        for foot in feet:
            with self.subTest(foot=foot):
                self.assertEqual(foot.clearance, SPAN_FOOT_CLEARANCE_M)
                self.assertGreater(footprint_clearance(self.layout, foot), 0.0, "on grass")
        (row,) = [f for f in footprints(self.layout) if f.kind == "hangars"]
        for foot in feet:
            self.assertGreater(min(row.distance(x, y) for x, y in edge_of(foot)), 0.0, "clear of the hangars' row")

    def test_its_roof_keeps_the_span_rule_and_clears_a_boosted_chase_camera_by_2_m(self) -> None:
        asphalt = self.layout.asphalt_half_width
        over_the_road = [(b, c) for _, b, c in self.vertices if abs(b) <= self.walls]
        self.assertTrue(over_the_road, "the roof spans the road")
        for b, c in over_the_road:
            self.assertGreaterEqual(c, GANTRY_TOP_M if abs(b) <= asphalt else GANTRY_TOP_M - GANTRY_BEAM_HEIGHT_M, (b, c))
        # Off a boost pad the camera pulls back past its tuned metre and shakes on a hit; it
        # rides at its height over the flat road wherever the kart is between the walls.
        boosted = chase_camera_behind_m() + tuning_constant("BOOST_PAD_FACTOR") * tuning_constant("CAMERA_PULL_BACK_M")
        self.assertLess(boosted, self.prop.length / 4.0, "a boosted camera rides under the ribs most of the way")
        camera = CHASE_CAMERA_ABOVE_M + tuning_constant("CAMERA_SHAKE_AMPLITUDE_M")
        self.assertGreaterEqual(min(c for _, c in over_the_road) - camera, 2.0)

    def test_steel_ribs_cross_overhead_at_a_pitch_under_its_dark_underside(self) -> None:
        overhead = [f for f in self.faces("wall") if all(abs(b) <= self.walls and c > HANGAR_SHELL_EAVES_M / 2.0 for _, b, c in f)]
        middles = clusters((sum(a for a, _, _ in f) / len(f) for f in overhead), gap=1.0)
        self.assertGreaterEqual(len(middles), 8)
        gaps = [b - a for a, b in zip(middles, middles[1:])]
        self.assertTrue(all(abs(g - HANGAR_SHELL_FRAME_PITCH_M) < 0.5 for g in gaps), gaps)
        # The underside's facets, not the roof's edges in the end planes.
        underside = [f for f in self.faces("carbon-shade") if len({round(a, 6) for a, _, _ in f}) > 1]
        self.assertTrue(underside)
        for f in underside:
            (a0, b0, c0), (a1, b1, c1), (a2, b2, c2) = f[:3]
            u, w = (a1 - a0, b1 - b0, c1 - c0), (a2 - a0, b2 - b0, c2 - c0)
            self.assertLess(u[0] * w[1] - u[1] * w[0], 0.0, "faces down at the road")
        # Each rib hangs below the underside over it.
        lowest_rib = min(c for f in overhead for _, _, c in f)
        lowest_under = min(c for f in underside for _, b, c in f if abs(b) <= self.walls)
        self.assertLess(lowest_rib, lowest_under)

    def test_light_aircraft_park_low_on_the_floor_right_of_the_track(self) -> None:
        fins = self.faces("kerb-red") + self.faces("carbon")
        middles = clusters((sum(a for a, _, _ in f) / len(f) for f in fins), gap=5.0)
        self.assertEqual(len(middles), 2, "two aircraft")
        aircraft = [v for f in fins for v in f]
        for _, b, c in aircraft:
            self.assertLess(b, -self.walls)
            self.assertGreater(b, -HANGAR_SHELL_RIGHT_M)
            self.assertLessEqual(c + GRASS_DROP_M, LOW_PROP_M)
        # Nose to the track: each canopy nearer the wall than its tail's fin.
        canopy = sum(b for f in self.faces("carbon") for _, b, _ in f) / sum(len(f) for f in self.faces("carbon"))
        fin = sum(b for f in self.faces("kerb-red") for _, b, _ in f) / sum(len(f) for f in self.faces("kerb-red"))
        self.assertGreater(canopy, fin + 4.0)

    def test_its_exit_frames_stowe_from_inside(self) -> None:
        sample = self.layout.path.at(self.prop.at.s)
        cx, cy = camera_at(sample)
        camera = (cx, cy, CHASE_CAMERA_ABOVE_M)
        stowe = self.layout.path.segment("stowe")
        shell = triangles_of(self.geometry)
        shell = [t for t in shell if t[0].startswith(f"{SHELL}.")]
        half = half_field_of_view()
        for s in (stowe.s_start, stowe.s_start + 15.0):
            point = self.layout.path.at(s)
            target = (point.x, point.y, 1.0)
            with self.subTest(s=s):
                off = math.atan2(point.y - cy, point.x - cx) - sample.heading
                self.assertLess(abs((off + math.pi) % math.tau - math.pi), half)
                unhidden(self, shell, camera, target)

    def test_the_zones_mirror_of_its_ends_matches_the_client(self) -> None:
        source = PRESENTATION_ZONES_TS.read_text()
        for name, s in (("FROM", self.prop.at.s - self.prop.length / 2.0), ("TO", self.prop.at.s + self.prop.length / 2.0)):
            found = re.search(rf"const GOLDSTONE_SHELL_{name}_M = ([0-9.]+);", source)
            self.assertIsNotNone(found, name)
            self.assertAlmostEqual(float(found.group(1)), s, delta=0.01)

    def test_it_paints_from_the_palette_and_letters_nothing(self) -> None:
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())
        self.assertFalse(any(t.name.startswith(f"{SHELL}.") for t in self.geometry.lettering))

    def test_it_refuses_a_layout_that_gives_no_length(self) -> None:
        short = replace(self.layout, decor=replace(self.layout.decor, props=[Prop("hangar-shell", Spot(s=900.0))]))
        with self.assertRaises(ValueError):
            build(short)

    def test_it_costs_about_600_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        self.assertTrue(500 <= faces <= 700, faces)


class ControlTower(unittest.TestCase):
    """The Wellington Straight runs at the control tower, so its view ends on it."""

    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "control-tower"]
        self.geometry = build(self.layout)
        self.pieces = family(self.geometry, TOWER)
        self.x, self.y, _ = locate(self.layout, self.prop.at)
        ground = -GRASS_DROP_M
        # The cab's middle and its roof, where the straight looks.
        self.targets = [(self.x, self.y, ground + (TOWER_SHAFT_TOP_M + TOWER_CAB_TOP_M) / 2.0), (self.x, self.y, ground + TOWER_CAB_TOP_M)]
        segment = self.layout.path.segment("wellington")
        self.end = self.layout.path.at(segment.s_end)
        self.cameras = []
        for i in self.layout.path.indices_between(segment.s_start, segment.s_end)[::10]:
            sample = self.layout.path.samples[i]
            self.cameras.append((sample, (*camera_at(sample), CHASE_CAMERA_ABOVE_M)))

    def test_it_stands_on_the_straights_line_past_brooklands_and_faces_back_up_it(self) -> None:
        dx, dy = self.x - self.end.x, self.y - self.end.y
        along = dx * math.cos(self.end.heading) + dy * math.sin(self.end.heading)
        across = -dx * math.sin(self.end.heading) + dy * math.cos(self.end.heading)
        self.assertAlmostEqual(across, 0.0)
        self.assertTrue(60.0 <= along <= 120.0, along)
        self.assertAlmostEqual(math.cos(self.prop.at.heading - self.end.heading), -1.0)

    def test_it_is_in_every_chase_cameras_view_down_the_wellington_straight(self) -> None:
        half = half_field_of_view()
        for sample, (cx, cy, _) in self.cameras:
            off = math.atan2(self.y - cy, self.x - cx) - sample.heading
            with self.subTest(s=sample.s):
                self.assertLess(abs((off + math.pi) % math.tau - math.pi), half / 2.0)

    def test_nothing_stands_between_a_chase_camera_on_the_straight_and_its_cab(self) -> None:
        triangles = triangles_of(self.geometry, skip=f"{TOWER}.")
        for sample, camera in self.cameras:
            for target in self.targets:
                with self.subTest(s=sample.s, target=target[2]):
                    unhidden(self, triangles, camera, target)

    def test_a_glazed_cab_overhangs_a_narrower_shaft_under_a_banded_mast(self) -> None:
        def reach(material: str) -> float:
            piece = self.pieces[f"{TOWER}.{material}"]
            # In the tower's own frame, square to the way its cab faces.
            cos, sin = math.cos(self.prop.at.heading), math.sin(self.prop.at.heading)
            return max(
                max(abs((v[0] - self.x) * cos + (v[1] - self.y) * sin), abs(-(v[0] - self.x) * sin + (v[1] - self.y) * cos))
                for v in piece.mesh[0]
            )

        self.assertAlmostEqual(reach("stand-riser"), TOWER_SHAFT_M / 2.0)
        self.assertAlmostEqual(reach("carbon"), TOWER_CAB_M / 2.0)
        self.assertGreater(TOWER_CAB_M, TOWER_SHAFT_M + 2.0, "the cab overhangs the shaft")
        glass = [v[2] + GRASS_DROP_M for v in self.pieces[f"{TOWER}.carbon"].mesh[0]]
        self.assertGreaterEqual(min(glass), TOWER_SHAFT_TOP_M)
        self.assertLessEqual(max(glass), TOWER_CAB_TOP_M)
        top = max(v[2] for piece in self.pieces.values() for v in piece.mesh[0]) + GRASS_DROP_M
        self.assertGreater(top, TOWER_CAB_TOP_M + 3.0, "the mast over the cab")
        self.assertIn(f"{TOWER}.kerb-white", self.pieces)
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())
        self.assertFalse(any(t.name.startswith(f"{TOWER}.") for t in self.geometry.lettering))

    def test_it_costs_about_40_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        self.assertTrue(30 <= faces <= 60, faces)


class DisplayJets(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(NAME)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "jets"]
        self.geometry = build(self.layout)
        self.pieces = family(self.geometry, JETS)
        self.formation = self.pieces[f"{JETS}.formation"]
        self.pivot = self.formation.pivot

    def frame(self, v):
        """A vertex as (back from the leader's nose, left of it, up from it)."""
        dx, dy = v[0] - self.pivot.x, v[1] - self.pivot.y
        cos, sin = math.cos(self.pivot.heading), math.sin(self.pivot.heading)
        return -(dx * cos + dy * sin), -dx * sin + dy * cos, v[2] - self.pivot.z

    def test_it_flies_30_m_up_over_the_hangars_against_the_race(self) -> None:
        (hangars,) = [p for p in self.layout.decor.props if p.kind == "hangars"]
        self.assertEqual(self.prop.at, hangars.at)
        self.assertEqual((self.prop.length, self.prop.height), (600.0, 30.0))
        self.assertIsNone(self.pivot.parent)
        sample = self.layout.path.at(self.prop.at.s)
        nx, ny = sample.left_normal
        self.assertAlmostEqual(self.pivot.x, sample.x + self.prop.at.offset * nx)
        self.assertAlmostEqual(self.pivot.y, sample.y + self.prop.at.offset * ny)
        self.assertAlmostEqual(self.pivot.z, -GRASS_DROP_M + self.prop.height)
        # Against the race: the nose points back up the Hangar Straight, toward Chapel.
        race = (math.cos(sample.heading), math.sin(sample.heading))
        nose = (math.cos(self.pivot.heading), math.sin(self.pivot.heading))
        self.assertAlmostEqual(race[0] * nose[0] + race[1] * nose[1], -1.0)

    def test_the_formation_is_one_node_its_ribbons_part_of_it_and_letters_nothing(self) -> None:
        self.assertEqual(set(self.pieces), {f"{JETS}.formation"})
        self.assertFalse(any(t.name.startswith("Prop.jets.") for t in self.geometry.lettering))
        self.assertEqual(set(self.formation.materials), {"kerb-red", "wall", "carbon", "ink-blue"})
        self.assertTrue(set(self.formation.materials) <= build_materials())

    def test_three_jets_fly_an_arrowhead_each_trailing_its_own_smoke(self) -> None:
        leader, *wingmen = JETS_FORMATION
        self.assertEqual(leader, (0.0, 0.0, 0.0))
        self.assertEqual(len(wingmen), 2)
        self.assertEqual({math.copysign(1.0, b) for _, b, _ in wingmen}, {1.0, -1.0}, "one either side")
        self.assertTrue(all(s > JET_LENGTH_M for s, _, _ in wingmen), "behind the leader's tail")
        # Ink blue above, wall for the leader, kerb red below.
        by_height = [smoke for (_, _, c), smoke in sorted(zip(JETS_FORMATION, JETS_SMOKE), key=lambda pair: -pair[0][2])]
        self.assertEqual(by_height, ["ink-blue", "wall", "kerb-red"])
        vertices, faces = self.formation.mesh
        s_end, _ = JETS_RIBBON[-1]
        for (s0, b0, c0), smoke in zip(JETS_FORMATION, JETS_SMOKE):
            with self.subTest(smoke=smoke):
                local = [(material, [self.frame(vertices[i]) for i in face]) for material, face in zip(self.formation.materials, faces)]
                # Every face whose middle is behind this jet's tail and in its lane.
                behind = [
                    (material, face)
                    for material, face in local
                    if sum(s for s, _, _ in face) / len(face) > s0 + JET_LENGTH_M + 1e-6 and abs(sum(b for _, b, _ in face) / len(face) - b0) < 3.0
                ]
                self.assertEqual({material for material, _ in behind}, {smoke})
                # A tapered ribbon some 50 m long, from a point at the tail.
                reach = max(s for _, face in behind for s, _, _ in face) - (s0 + JET_LENGTH_M)
                self.assertAlmostEqual(reach, s_end - JET_LENGTH_M)
                self.assertTrue(45.0 <= reach <= 55.0, reach)
                widest = max(abs(c - c0) for _, face in behind for _, _, c in face)
                self.assertAlmostEqual(widest, max(half for _, half in JETS_RIBBON))

    def test_the_mirror_of_its_pass_matches_the_client(self) -> None:
        source = SCENERY_MOTION_TS.read_text()
        found = re.search(r"export const JETS_PASS_M = ([0-9_.]+);", source)
        self.assertIsNotNone(found)
        self.assertEqual(float(found.group(1).replace("_", "")), self.prop.length)

    def test_it_never_flies_under_20_m_over_any_road_its_pass_crosses_ribbons_included(self) -> None:
        lowest = min(v[2] for v in self.formation.mesh[0])
        self.assertLess(lowest, self.pivot.z - 4.5, "the lowest wingman's ribbon")
        # Anywhere along the pass, the formation's widest reach either side of its line.
        reach = max(math.hypot(*self.frame(v)[:2]) for v in self.formation.mesh[0])
        nose = (math.cos(self.pivot.heading), math.sin(self.pivot.heading))
        half = self.prop.length / 2.0

        def from_line(x: float, y: float) -> float:
            dx, dy = x - self.pivot.x, y - self.pivot.y
            along = max(-half, min(half, dx * nose[0] + dy * nose[1]))
            return math.hypot(dx - along * nose[0], dy - along * nose[1])

        under = [s for s in self.layout.path.samples if from_line(s.x, s.y) <= reach]
        self.assertGreater(len(under), 0, "it crosses the Hangar Straight's run-off")
        road = max(elevation(self.layout.elevation, s.s) for s in under)
        self.assertGreaterEqual(lowest - road, JETS_CLEARANCE_M)
        self.assertEqual(JETS_CLEARANCE_M, 20.0)

    def test_it_has_no_footprint_since_it_flies_over_the_track(self) -> None:
        self.assertNotIn("jets", {f.kind for f in footprints(self.layout)})

    def test_it_refuses_a_layout_that_gives_no_pass_or_no_height(self) -> None:
        for prop in (Prop("jets", World(190.0, -57.0), height=60.0), Prop("jets", World(190.0, -57.0), length=600.0)):
            with self.subTest(prop=prop), self.assertRaises(ValueError):
                build(replace(self.layout, decor=replace(self.layout.decor, props=[prop])))

    def test_the_formation_costs_about_120_faces(self) -> None:
        faces = len(self.formation.mesh[1])
        self.assertTrue(100 <= faces <= 140, faces)


if __name__ == "__main__":
    unittest.main()

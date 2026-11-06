"""Goldstone, the course after Silverstone, checked without Blender: the lap
docs/mockups/silverstone-course.html decided, flat and with no ramp; the
pit building drawn white inside the Hamilton Straight, facing the grid; and the row of airfield hangars outside the Hangar Straight, each a
barrel-roofed shed with its door toward the track, every footprint keeping
the 8 m rule; and the display jets' formation over the hangars, flying
against the race and keeping 30 m over every road its pass crosses.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from dataclasses import replace
from pathlib import Path

from tests.test_props import (
    across_track,
    build_materials,
    faces_up_and_toward_the_sun,
    footprint_clearance,
    materials_used,
)
from trackgen.layout import CROWD_KEYS, GRASS_DROP_M, PROP_WALL_CLEARANCE_M, Prop, Spot, World, build
from trackgen.layouts import load
from trackgen.props import (
    HANGAR_DEPTH_M,
    HANGAR_DOOR_PROUD_M,
    HANGAR_EAVES_M,
    HANGAR_PITCH_M,
    HANGAR_RIDGE_M,
    HANGAR_WIDTH_M,
    JET_LENGTH_M,
    JETS_CLEARANCE_M,
    JETS_FORMATION,
    JETS_RIBBON,
    JETS_SMOKE,
    PITS_DEPTH_M,
    PITS_DOOR_PROUD_M,
    footprints,
)
from trackgen.path import elevation

NAME = "goldstone"
PITS = "Prop.pits"
HANGARS = "Prop.hangars"
JETS = "Prop.jets.1"
SCENERY_MOTION_TS = Path(__file__).resolve().parents[3] / "apps" / "web" / "src" / "scenery-motion.ts"


def faces_right(face) -> bool:
    """Whether a face given as (along, left of, up) points right: toward the track from a
    prop that stands left of it."""
    (a0, _, c0), (a1, _, c1), (a2, _, c2) = face[:3]
    return (c1 - c0) * (a2 - a0) - (a1 - a0) * (c2 - c0) < 0.0


def faces_left(face) -> bool:
    """Whether a face given as (along, left of, up) points left: toward the track from a
    prop that stands right of it."""
    (a0, _, c0), (a1, _, c1), (a2, _, c2) = face[:3]
    return (c1 - c0) * (a2 - a0) - (a1 - a0) * (c2 - c0) > 0.0


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

    def test_it_lays_what_the_mockup_placed(self) -> None:
        g = self.geometry
        counts = (len(g.item_boxes), len(g.boost_pads), len(g.kerbs), len(g.checkpoints), len(g.spawn_grid))
        self.assertEqual(counts, (12, 2, 5, 15, 8))
        self.assertEqual(self.layout.asphalt_half_width, 6.0)
        self.assertEqual(self.layout.runoff_width, 6.0)

    def test_it_is_flat_with_no_ramp_or_bridge(self) -> None:
        self.assertEqual(self.layout.elevation, [])
        self.assertEqual(self.layout.decor.bridges, [])

    def test_every_footprint_keeps_8_m_of_grass_from_every_wall(self) -> None:
        found = footprints(self.layout)
        self.assertEqual({f.kind for f in found}, {"pits", "hangars"})
        for footprint in found:
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

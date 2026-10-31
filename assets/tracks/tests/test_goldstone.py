"""Goldstone, the course after Silverstone, checked without Blender: the lap
docs/mockups/silverstone-course.html decided, flat and with no ramp; the
pit building drawn long and white outside the Hamilton Straight, facing the
grid; and the row of airfield hangars outside the Hangar Straight, each a
barrel-roofed shed with its door toward the track, every footprint keeping
the 8 m rule.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest
from dataclasses import replace

from tests.test_props import (
    across_track,
    build_materials,
    faces_up_and_toward_the_sun,
    footprint_clearance,
    materials_used,
)
from trackgen.layout import CROWD_KEYS, GRASS_DROP_M, PROP_WALL_CLEARANCE_M, Prop, Spot, build
from trackgen.layouts import load
from trackgen.props import (
    HANGAR_DEPTH_M,
    HANGAR_EAVES_M,
    HANGAR_PITCH_M,
    HANGAR_RIDGE_M,
    HANGAR_WIDTH_M,
    PITS_DEPTH_M,
    PITS_DOOR_PROUD_M,
    footprints,
)

NAME = "goldstone"
PITS = "Prop.pits"
HANGARS = "Prop.hangars"


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

    def test_it_stands_outside_the_hamilton_straight_longer_than_bell_deers(self) -> None:
        self.assertEqual(self.prop.at.s, 0.0)
        self.assertGreater(self.prop.at.offset, self.layout.runoff_half_width, "left of the track, on the outside")
        (bell_deer,) = [p for p in load("bell-deer").decor.props if p.kind == "pits"]
        self.assertGreater(self.prop.length, bell_deer.length)
        vertices = [self.frame(v) for piece in self.pieces.values() for v in piece.mesh[0]]
        self.assertAlmostEqual(min(a for a, _, _ in vertices), -self.prop.length / 2.0)
        self.assertAlmostEqual(max(a for a, _, _ in vertices), self.prop.length / 2.0)
        self.assertGreaterEqual(min(b for _, b, _ in vertices), self.prop.at.offset - PITS_DEPTH_M / 2.0 - PITS_DOOR_PROUD_M - 1e-6)

    def test_its_walls_are_wall_white_over_beam_shade(self) -> None:
        names = set(self.pieces)
        self.assertIn(f"{PITS}.wall", names)
        self.assertIn(f"{PITS}.beam-shade", names)
        self.assertNotIn(f"{PITS}.stand-riser", names)
        shaded = self.pieces[f"{PITS}.beam-shade"]
        faces = [[shaded.mesh[0][i] for i in f] for f in shaded.mesh[1]]
        self.assertTrue(faces)
        self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in faces))

    def test_its_doors_face_the_track_and_run_forward_from_the_line(self) -> None:
        front = self.prop.at.offset - PITS_DEPTH_M / 2.0 - PITS_DOOR_PROUD_M
        centres = []
        for slot in range(len(CROWD_KEYS)):
            piece = self.pieces[f"{PITS}.crowd-{slot}"]
            (door,) = [f for f in piece.mesh[1] if min(piece.mesh[0][v][2] for v in f) - self.ground < 1e-6]
            panel = [piece.mesh[0][v] for v in door]
            self.assertEqual({round(self.frame(v)[1], 3) for v in panel}, {round(front, 3)})
            # Wound toward the track: its normal points right, toward the centreline.
            (a0, b0, _), (a1, b1, _), (a2, b2, _) = (self.frame(v) for v in panel[:3])
            c0, c1, c2 = (self.frame(v)[2] for v in panel[:3])
            normal_left = (c1 - c0) * (a2 - a0) - (a1 - a0) * (c2 - c0)
            self.assertLess(normal_left, 0.0, f"door {slot} faces the track")
            centres.append(sum(self.frame(v)[0] for v in panel) / len(panel))
        self.assertGreater(centres[0], 0.0)
        self.assertEqual(centres, sorted(centres))

    def test_its_flags_face_the_track(self) -> None:
        for slot in range(len(CROWD_KEYS)):
            piece = self.pieces[f"{PITS}.crowd-{slot}"]
            (flag,) = [f for f in piece.mesh[1] if min(piece.mesh[0][v][2] for v in f) - self.ground > 1.0]
            (a0, _, c0), (a1, _, c1), (a2, _, c2) = (self.frame(piece.mesh[0][v]) for v in flag)
            normal_left = (c1 - c0) * (a2 - a0) - (a1 - a0) * (c2 - c0)
            self.assertLess(normal_left, 0.0, f"flag {slot} faces the track")

    def test_bell_deers_pits_keep_their_stand_colours(self) -> None:
        names = set(family(build(load("bell-deer")), PITS))
        self.assertIn(f"{PITS}.stand-riser", names)
        self.assertNotIn(f"{PITS}.beam-shade", names)


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
        self.assertGreaterEqual(min(b for _, b, _ in vertices), self.prop.at.offset - HANGAR_DEPTH_M / 2.0 - PITS_DOOR_PROUD_M - 1e-6)
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
        front = self.prop.at.offset - HANGAR_DEPTH_M / 2.0 - PITS_DOOR_PROUD_M
        for f in doors.mesh[1]:
            panel = [self.frame(doors.mesh[0][v]) for v in f]
            self.assertEqual({round(b, 3) for _, b, _ in panel}, {round(front, 3)})
            self.assertAlmostEqual(min(c for _, _, c in panel), 0.0)
            self.assertLess(max(c for _, _, c in panel), HANGAR_EAVES_M + 1e-9)
            (a0, _, c0), (a1, _, c1), (a2, _, c2) = panel[:3]
            self.assertLess((c1 - c0) * (a2 - a0) - (a1 - a0) * (c2 - c0), 0.0)

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


if __name__ == "__main__":
    unittest.main()

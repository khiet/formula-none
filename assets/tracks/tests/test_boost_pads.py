"""Boost pads drawn where the sim boosts, checked without Blender: a plate
covering the sim's rectangle at each pad's placement and yaw, with three
chevrons on it pointing down the track.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from pathlib import Path

from trackgen.layout import BOOST_PAD_PLATE_MATERIALS, build
from trackgen.layouts import NAMES, load

TUNING_RS = Path(__file__).resolve().parents[3] / "crates" / "sim" / "src" / "tuning.rs"


def tuning_constant(name: str) -> float:
    match = re.search(rf"pub const {name}: f32 = ([0-9.]+);", TUNING_RS.read_text())
    if match is None:
        raise AssertionError(f"{name} is not an f32 constant in {TUNING_RS}")
    return float(match.group(1))


def in_pad_frame(pad, vertex) -> tuple[float, float]:
    """`vertex` as (metres ahead of the pad, metres right of it)."""
    dx, dy = vertex[0] - pad.x, vertex[1] - pad.y
    cos, sin = math.cos(pad.heading), math.sin(pad.heading)
    return dx * cos + dy * sin, dx * sin - dy * cos


def signed_area(points) -> float:
    """Shoelace area seen from above: positive when wound counter-clockwise, so facing up."""
    return sum(
        a[0] * b[1] - b[0] * a[1] for a, b in zip(points, points[1:] + points[:1])
    ) / 2.0


def faces_of(piece):
    vertices, faces = piece.mesh
    return [[vertices[v] for v in face] for face in faces]


def pieces_of(geometry, number: int):
    return [piece for piece in geometry.scenery if piece.name.startswith(f"BoostPad.{number:02d}.")]


class BoostPads(unittest.TestCase):
    def test_a_plate_covers_the_rectangle_the_sim_boosts_on_at_each_pads_placement_and_yaw(self) -> None:
        half_length = tuning_constant("BOOST_PAD_HALF_LENGTH_M")
        half_width = tuning_constant("BOOST_PAD_HALF_WIDTH_M")
        for name in NAMES:
            geometry = build(load(name))
            self.assertGreater(len(geometry.boost_pads), 0, name)
            for number, pad in enumerate(geometry.boost_pads):
                with self.subTest(track=name, pad=number):
                    plate = [p for p in pieces_of(geometry, number) if p.material in BOOST_PAD_PLATE_MATERIALS]
                    self.assertTrue(plate)
                    local = [in_pad_frame(pad, v) for piece in plate for v in piece.mesh[0]]
                    along = [a for a, _ in local]
                    across = [c for _, c in local]
                    self.assertAlmostEqual(min(along), -half_length, places=6)
                    self.assertAlmostEqual(max(along), half_length, places=6)
                    self.assertAlmostEqual(min(across), -half_width, places=6)
                    self.assertAlmostEqual(max(across), half_width, places=6)
                    # No gap and no overlap: the faces add up to the rectangle.
                    area = sum(signed_area(face) for piece in plate for face in faces_of(piece))
                    self.assertAlmostEqual(area, 4.0 * half_length * half_width, places=6)

    def test_three_chevrons_on_the_plate_point_down_the_track(self) -> None:
        for name in NAMES:
            geometry = build(load(name))
            for number, pad in enumerate(geometry.boost_pads):
                with self.subTest(track=name, pad=number):
                    pieces = pieces_of(geometry, number)
                    plate_z = max(v[2] for p in pieces if p.material in BOOST_PAD_PLATE_MATERIALS for v in p.mesh[0])
                    chevrons = [p for p in pieces if p.material not in BOOST_PAD_PLATE_MATERIALS]
                    self.assertTrue(chevrons)
                    # A chevron's tip is the one vertex on the centre line, ahead of its arms.
                    tips: list[float] = []
                    for piece in chevrons:
                        for vertex in piece.mesh[0]:
                            ahead, right = in_pad_frame(pad, vertex)
                            self.assertLessEqual(abs(ahead), tuning_constant("BOOST_PAD_HALF_LENGTH_M"))
                            self.assertLessEqual(abs(right), tuning_constant("BOOST_PAD_HALF_WIDTH_M"))
                            self.assertGreater(vertex[2], plate_z, "drawn over the plate")
                    for piece in chevrons:
                        if piece.material != "chevron":
                            continue
                        for face in faces_of(piece):
                            local = [in_pad_frame(pad, v) for v in face]
                            centred = [a for a, r in local if abs(r) < 1e-6]
                            if centred:
                                arms = [a for a, r in local if abs(r) > 1e-6]
                                self.assertGreater(max(centred), max(arms), "the tip leads its arms")
                                tips.append(round(max(centred), 6))
                    self.assertEqual(len(set(tips)), 3)

    def test_every_face_of_a_pad_faces_up(self) -> None:
        for name in NAMES:
            geometry = build(load(name))
            for number, _ in enumerate(geometry.boost_pads):
                for piece in pieces_of(geometry, number):
                    for face in faces_of(piece):
                        with self.subTest(track=name, piece=piece.name):
                            self.assertGreater(signed_area(face), 0.0)

    def test_the_pad_rests_on_the_asphalt_under_it(self) -> None:
        for name in NAMES:
            geometry = build(load(name))
            for number, pad in enumerate(geometry.boost_pads):
                with self.subTest(track=name, pad=number):
                    for piece in pieces_of(geometry, number):
                        for vertex in piece.mesh[0]:
                            self.assertGreater(vertex[2], pad.z)
                            self.assertLess(vertex[2], pad.z + 0.05)


if __name__ == "__main__":
    unittest.main()

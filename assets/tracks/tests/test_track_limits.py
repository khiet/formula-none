"""Track limits checked without Blender: a wall along each edge of the
run-off the whole way round, and evenly spaced checkpoints between them.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest

from trackgen.layout import GRASS_DROP_M, WALL, WALL_THICKNESS_M, build
from trackgen.layouts import NAMES, load
from trackgen.path import elevation


def wall_bases(layout) -> dict[tuple[float, float], float]:
    """Where the layout's walls stand: the height of the lowest wall vertex
    over each (x, y) a wall vertex has, to the millimetre."""
    bases: dict[tuple[float, float], float] = {}
    for surface in build(layout).surfaces:
        if surface.surface != WALL or surface.material != WALL:
            continue
        for x, y, z in surface.mesh[0]:
            key = (round(x, 3), round(y, 3))
            bases[key] = min(z, bases.get(key, z))
    return bases


def edge(sample, layout, side: int) -> tuple[float, float]:
    x, y = sample.offset(side * layout.runoff_half_width)
    return round(x, 3), round(y, 3)


def in_a_gap(layout, s: float) -> bool:
    """Whether `s` is in the stretch a pit lane sweeps across its wall's line, at the fork or the merge."""
    lane, lap = layout.pit_lane, layout.path.length
    along = (s - lane.s_fork) % lap
    fork = lane.s_entry - lane.s_fork
    merge_from = lane.s_exit - lane.s_fork
    merge_to = lane.s_merge - lane.s_fork
    return 0.0 < along < fork or merge_from < along < merge_to


class Walls(unittest.TestCase):
    def test_a_wall_stands_on_both_edges_of_the_run_off_at_every_centreline_sample(self) -> None:
        for name in NAMES:
            layout = load(name)
            bases = wall_bases(layout)
            with self.subTest(track=name):
                lane = layout.pit_lane
                for sample in layout.path.samples:
                    for side in (1, -1):
                        # A pit lane comes and goes by a gap in the wall on its
                        # side; test_pit_lane holds those shut behind its own wall.
                        if lane is not None and side == lane.side and in_a_gap(layout, sample.s):
                            continue
                        self.assertIn(edge(sample, layout, side), bases, f"{sample.s} m, side {side}")

    def test_the_walls_are_unbroken_loops_on_a_course_without_a_pit_lane(self) -> None:
        # Both faces and the top between every pair of samples, the last pair
        # joined back to the first, and no end anywhere a kart could get round.
        for name in NAMES:
            layout = load(name)
            if layout.pit_lane is not None:
                continue
            with self.subTest(track=name):
                walls = [
                    surface
                    for surface in build(layout).surfaces
                    if surface.surface == WALL and surface.material == WALL
                ]
                self.assertEqual(len(walls), 2)
                for wall in walls:
                    self.assertEqual(len(wall.mesh[1]), 3 * len(layout.path.samples), wall.name)

    def test_the_walls_climb_the_bridge_with_the_track(self) -> None:
        # A wall left at ground level would stand below the sand on the
        # deck, a cliff at the edge of the track instead of a parapet on it.
        layout = load("bell-deer")
        bases = wall_bases(layout)
        crest = layout.path.at(layout.decor.bridges[0].deck_s)
        crest_height = elevation(layout.elevation, crest.s)
        self.assertGreater(crest_height, 4.0, "the deck")
        for side in (1, -1):
            self.assertAlmostEqual(
                bases[edge(crest, layout, side)], crest_height - GRASS_DROP_M, places=3
            )


class Checkpoints(unittest.TestCase):
    def test_checkpoints_are_evenly_spaced_from_the_start_line_and_reach_both_walls_outer_faces(self) -> None:
        for name in NAMES:
            layout = load(name)
            path = layout.path
            with self.subTest(track=name):
                checkpoints = build(layout).checkpoints
                self.assertEqual(len(checkpoints), layout.checkpoint_count)
                spacing = path.length / layout.checkpoint_count
                for number, checkpoint in enumerate(checkpoints):
                    expected = path.at(number * spacing)
                    self.assertAlmostEqual(checkpoint.x, expected.x, places=6)
                    self.assertAlmostEqual(checkpoint.y, expected.y, places=6)
                    # At least: test_pit_lane holds those along a lane to reaching across it.
                    self.assertGreaterEqual(checkpoint.half_width, layout.runoff_half_width + WALL_THICKNESS_M)

    def test_the_oval_has_twelve_and_every_course_fifteen(self) -> None:
        for name, count in (("oval", 12), ("bell-deer", 15), ("goldstone", 15), ("monarch", 15)):
            with self.subTest(track=name):
                self.assertEqual(len(build(load(name)).checkpoints), count)


if __name__ == "__main__":
    unittest.main()

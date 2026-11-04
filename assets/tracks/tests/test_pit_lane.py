"""The pit lane checked without Blender (ADR 0081): Bell Deer's forks off the
start straight's right, past the pit wall and clear of the pit building and
the trees, and every checkpoint along it reaches across it. The oval has none.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from tests.test_tyre_walls import left_of_centre
from trackgen.layout import WALL_THICKNESS_M, build
from trackgen.layouts import load
from trackgen.pit_lane import LANE_WIDTH_M
from trackgen.props import PITS_DEPTH_M, trees

LANE_LENGTH_M = 140.0
"""ADR 0081's lane, line to line, which sets the cost of a stop."""


def bell_deer():
    layout = load("bell-deer")
    return layout, build(layout)


def nearest_sample(layout, x: float, y: float):
    """The closest centreline sample on the whole lap; nothing near the lane comes near another leg."""
    return min(layout.path.samples, key=lambda sample: math.dist((sample.x, sample.y), (x, y)))


def surface(geometry, name: str):
    (found,) = [mesh for mesh in geometry.surfaces if mesh.name == name]
    return found


class PitLaneOnBellDeer(unittest.TestCase):
    def setUp(self) -> None:
        self.layout, self.geometry = bell_deer()
        self.lane = self.layout.pit_lane
        self.pit_lane = self.geometry.pit_lane

    def test_the_lines_are_about_the_length_adr_0081_costs_a_stop_by(self) -> None:
        self.assertAlmostEqual(self.lane.s_exit - self.lane.s_entry, LANE_LENGTH_M, delta=5.0)

    def test_the_path_runs_from_the_fork_to_the_merge_through_both_lines(self) -> None:
        path = self.pit_lane.path
        for point, s in ((path[0], self.lane.s_fork), (path[-1], self.lane.s_merge)):
            self.assertLess(
                math.dist(point[:2], self.layout.path.at(s).offset(-9.0)),
                1.0,
                "the path starts and ends in the middle of the run-off",
            )
        for line in (self.pit_lane.entry, self.pit_lane.exit):
            nearest = min(math.dist(point[:2], (line.x, line.y)) for point in path)
            self.assertLess(nearest, 0.01)
            self.assertAlmostEqual(line.half_width, LANE_WIDTH_M / 2.0)

    def test_the_lane_lies_between_the_pit_wall_and_the_building_line_to_line(self) -> None:
        wall = self.layout.runoff_half_width + WALL_THICKNESS_M
        (pits,) = [prop for prop in self.layout.decor.props if prop.kind == "pits"]
        building_front = -pits.at.offset - PITS_DEPTH_M / 2.0
        for x, y, _ in self.pit_lane.path:
            s = nearest_sample(self.layout, x, y).s
            if (s - self.lane.s_entry) % self.layout.path.length > self.lane.s_exit - self.lane.s_entry:
                continue
            offset = left_of_centre(self.layout, x, y, s)
            self.assertAlmostEqual(offset, -(wall + LANE_WIDTH_M / 2.0), delta=0.05)
        self.assertLess(wall + LANE_WIDTH_M + WALL_THICKNESS_M, building_front)

    def test_the_lane_surface_stands_a_hair_over_what_it_crosses_and_flush_with_the_asphalt_edge(self) -> None:
        vertices = surface(self.geometry, "PitLane").mesh[0]
        heights = [z for _, _, z in vertices]
        self.assertLessEqual(max(heights), 0.0)
        self.assertGreater(min(heights), -0.05, "above the grass")
        self.assertEqual(surface(self.geometry, "PitLane").surface, "asphalt")

    def test_the_outer_wall_runs_on_from_the_wall_it_replaces(self) -> None:
        # Where the lane's wall meets the gap's edges there is no way round
        # it onto the grass: same offsets as the right wall at both ends.
        runoff = self.layout.runoff_half_width
        for s in (self.lane.s_fork, self.lane.s_merge):
            sample = self.layout.path.at(s)
            face = sample.offset(-runoff)
            for name in ("WallRight", "WallPitLane"):
                nearest = min(math.dist((x, y), face) for x, y, _ in surface(self.geometry, name).mesh[0])
                self.assertLess(nearest, 0.01, f"{name} reaches the gap's edge at s={s}")

    def test_the_right_wall_leaves_a_gap_at_the_fork_and_the_merge_and_the_pit_wall_stands_between(self) -> None:
        runoff = self.layout.runoff_half_width
        right = surface(self.geometry, "WallRight").mesh[0]
        pit_wall = surface(self.geometry, "WallPit").mesh[0]
        for gap_s in (
            (self.lane.s_fork + self.lane.s_entry) / 2.0,
            (self.lane.s_exit + self.lane.s_merge) / 2.0,
        ):
            middle = self.layout.path.at(gap_s).offset(-runoff)
            for wall in (right, pit_wall):
                self.assertGreater(min(math.dist((x, y), middle) for x, y, _ in wall), 5.0)
        beside_the_grid = self.layout.path.at(0.0).offset(-runoff)
        self.assertLess(min(math.dist((x, y), beside_the_grid) for x, y, _ in pit_wall), 1.0)
        self.assertGreater(min(math.dist((x, y), beside_the_grid) for x, y, _ in right), 5.0)

    def test_every_checkpoint_along_the_lane_reaches_across_it_and_no_other_grows(self) -> None:
        outer_face = self.layout.runoff_half_width + WALL_THICKNESS_M + LANE_WIDTH_M + WALL_THICKNESS_M
        lap = self.layout.path.length
        along_the_lane = 0
        for checkpoint in self.geometry.checkpoints:
            s = nearest_sample(self.layout, checkpoint.x, checkpoint.y).s
            if (s - self.lane.s_fork) % lap <= self.lane.s_merge - self.lane.s_fork:
                self.assertAlmostEqual(checkpoint.half_width, outer_face)
                along_the_lane += 1
            else:
                self.assertAlmostEqual(checkpoint.half_width, self.layout.runoff_half_width + WALL_THICKNESS_M)
        self.assertGreaterEqual(along_the_lane, 2, "the start gate and one past the building")

    def test_each_of_the_eight_pit_boxes_stands_on_the_apron_in_front_of_its_door(self) -> None:
        self.assertEqual(len(self.pit_lane.pit_boxes), 8)
        previous = None
        for box in self.pit_lane.pit_boxes:
            s = nearest_sample(self.layout, box.x, box.y).s
            offset = left_of_centre(self.layout, box.x, box.y, s)
            self.assertLess(offset, -(self.layout.runoff_half_width + WALL_THICKNESS_M + LANE_WIDTH_M / 2.0))
            self.assertGreater(offset, -(self.layout.runoff_half_width + WALL_THICKNESS_M + LANE_WIDTH_M))
            if previous is not None:
                self.assertAlmostEqual(s - previous, 5.5, delta=1.0)
            previous = s

    def test_no_tree_stands_on_the_lane(self) -> None:
        for tree in trees(self.layout):
            nearest = min(math.dist((tree.x, tree.y), point[:2]) for point in self.pit_lane.path)
            self.assertGreater(nearest, LANE_WIDTH_M)


class TheOval(unittest.TestCase):
    def test_it_has_no_lane_and_its_checkpoints_stop_at_the_walls(self) -> None:
        layout = load("oval")
        geometry = build(layout)
        self.assertIsNone(geometry.pit_lane)
        self.assertFalse([mesh for mesh in geometry.surfaces if "Pit" in mesh.name])
        wall = layout.runoff_half_width + WALL_THICKNESS_M
        for checkpoint in geometry.checkpoints:
            self.assertAlmostEqual(checkpoint.half_width, wall)


if __name__ == "__main__":
    unittest.main()

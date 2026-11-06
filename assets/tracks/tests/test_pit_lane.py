"""The pit lanes checked without Blender (ADR 0081): each forks off its start
straight on the right, the side the real circuits put them (ADRs 0088 and
0089), past the pit wall and clear of the pit building and the trees, with no
boost pad or coin on the road a kart called in crosses to reach it, and every
checkpoint along it reaches across it. The oval has none.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from tests.test_props import across_track
from tests.test_tyre_walls import left_of_centre
from trackgen.layout import GRASS_DROP_M, WALL_THICKNESS_M, build
from trackgen.layouts import load
from trackgen.pit_lane import LANE_WIDTH_M
from trackgen.props import PITS_DEPTH_M, PITS_DOOR_PITCH_M, trees

LANE_LENGTH_M = {"bell-deer": 130.0, "goldstone": 140.0, "monarch": 120.0}
"""ADR 0081's lanes, line to line along the path, which set the cost of a stop. Bell Deer's
140 m is along the centreline; its lane runs inside the first curve, so is shorter itself."""


def nearest_sample(layout, x: float, y: float):
    """The closest centreline sample on the whole lap; nothing near the lane comes near another leg."""
    return min(layout.path.samples, key=lambda sample: math.dist((sample.x, sample.y), (x, y)))


def surface(geometry, name: str):
    (found,) = [mesh for mesh in geometry.surfaces if mesh.name == name]
    return found


class ThePitLane:
    """What every course's lane promises; a subclass names the course."""

    NAME = ""
    SIDE = -1

    def setUp(self) -> None:
        self.layout = load(self.NAME)
        self.geometry = build(self.layout)
        self.lane = self.layout.pit_lane
        self.pit_lane = self.geometry.pit_lane
        self.side = self.lane.side
        self.wall_on_its_side = "WallRight" if self.SIDE < 0 else "WallLeft"

    def building(self):
        (pits,) = [prop for prop in self.layout.decor.props if prop.kind == "pits"]
        return pits

    def test_it_runs_on_its_side_of_the_start_straight(self) -> None:
        """The right, the real circuit's side (ADRs 0088 and 0089)."""
        self.assertEqual(self.side, self.SIDE)
        for x, y, _ in self.pit_lane.path:
            s = nearest_sample(self.layout, x, y).s
            self.assertGreater(self.SIDE * left_of_centre(self.layout, x, y, s), self.layout.asphalt_half_width)

    def test_no_corner_it_runs_inside_folds_its_outer_wall(self) -> None:
        """A wall built further out than a corner's radius passes the corner's centre and
        turns back on itself. So each point of the wall must be no nearer any bit of road
        than the offset it was built at."""
        far = self.layout.runoff_half_width + WALL_THICKNESS_M + LANE_WIDTH_M + WALL_THICKNESS_M
        indices = self.layout.path.indices_between(self.lane.s_fork, self.lane.s_merge)
        for (x, y, _), index in zip(surface(self.geometry, "WallPitLane").mesh[0], indices):
            sample = self.layout.path.samples[index]
            built = math.dist((x, y), (sample.x, sample.y))
            self.assertLessEqual(built, far + 1e-6)
            nearest = min(math.dist((x, y), (q.x, q.y)) for q in self.layout.path.samples)
            self.assertAlmostEqual(nearest, built, delta=0.05, msg=f"at s={sample.s}")

    def test_the_lines_are_about_the_length_adr_0081_costs_a_stop_by(self) -> None:
        path = self.pit_lane.path
        def index_at(line):
            return min(range(len(path)), key=lambda i: math.dist(path[i][:2], (line.x, line.y)))
        between = path[index_at(self.pit_lane.entry) : index_at(self.pit_lane.exit) + 1]
        length = sum(math.dist(a[:2], b[:2]) for a, b in zip(between, between[1:]))
        self.assertAlmostEqual(length, LANE_LENGTH_M[self.NAME], delta=5.0)

    def test_the_path_runs_from_the_fork_to_the_merge_through_both_lines(self) -> None:
        path = self.pit_lane.path
        for point, s in ((path[0], self.lane.s_fork), (path[-1], self.lane.s_merge)):
            self.assertLess(
                math.dist(point[:2], self.layout.path.at(s).offset(self.side * (self.layout.asphalt_half_width + self.layout.runoff_half_width) / 2.0)),
                1.0,
                "the path starts and ends in the middle of the run-off",
            )
        for line in (self.pit_lane.entry, self.pit_lane.exit):
            nearest = min(math.dist(point[:2], (line.x, line.y)) for point in path)
            self.assertLess(nearest, 0.01)
            self.assertAlmostEqual(line.half_width, LANE_WIDTH_M / 2.0)

    def test_the_lane_lies_between_the_pit_wall_and_the_building_line_to_line(self) -> None:
        wall = self.layout.runoff_half_width + WALL_THICKNESS_M
        building_front = self.side * self.building().at.offset - PITS_DEPTH_M / 2.0
        for x, y, _ in self.pit_lane.path:
            s = nearest_sample(self.layout, x, y).s
            if (s - self.lane.s_entry) % self.layout.path.length > self.lane.s_exit - self.lane.s_entry:
                continue
            offset = left_of_centre(self.layout, x, y, s)
            self.assertAlmostEqual(self.side * offset, wall + LANE_WIDTH_M / 2.0, delta=0.05)
        self.assertLess(wall + LANE_WIDTH_M + WALL_THICKNESS_M, building_front)

    def test_the_lane_surface_stands_a_hair_over_what_it_crosses_and_flush_with_the_asphalt_edge(self) -> None:
        vertices = surface(self.geometry, "PitLane").mesh[0]
        heights = [z for _, _, z in vertices]
        self.assertLessEqual(max(heights), 0.0, "no higher than the flat straight it runs beside")
        self.assertGreater(min(heights), -0.05, "above the grass")
        self.assertEqual(surface(self.geometry, "PitLane").surface, "asphalt")

    def test_no_grass_shows_between_the_run_off_and_the_lane_at_either_end_of_the_pit_wall(self) -> None:
        # A kart at the pit wall's end, off the road's middle, stands on asphalt or sand.
        (vertices, _) = surface(self.geometry, "PitLane").mesh
        for s in (self.lane.s_entry - 2.0, self.lane.s_entry + 2.0, self.lane.s_exit - 2.0, self.lane.s_exit + 2.0):
            sample = self.layout.path.at(s)
            near = min(
                (v for v in vertices if abs(across_track(self.layout, s, v[0], v[1])[0]) < 1.0),
                key=lambda v: abs(across_track(self.layout, s, v[0], v[1])[1]),
            )
            self.assertLessEqual(
                abs(across_track(self.layout, s, near[0], near[1])[1]),
                self.layout.runoff_half_width + 0.01,
                f"the lane reaches the run-off at s={s} ({sample.s})",
            )

    def test_the_outer_wall_runs_on_from_the_wall_it_replaces(self) -> None:
        # Where the lane's wall meets the gap's edges there is no way round
        # it onto the grass: same offsets as the right wall at both ends.
        runoff = self.layout.runoff_half_width
        for s in (self.lane.s_fork, self.lane.s_merge):
            sample = self.layout.path.at(s)
            face = sample.offset(self.side * runoff)
            for name in (self.wall_on_its_side, "WallPitLane"):
                nearest = min(math.dist((x, y), face) for x, y, _ in surface(self.geometry, name).mesh[0])
                self.assertLess(nearest, 0.01, f"{name} reaches the gap's edge at s={s}")

    def test_the_wall_on_its_side_leaves_a_gap_at_the_fork_and_the_merge_and_the_pit_wall_stands_between(self) -> None:
        runoff = self.layout.runoff_half_width
        wall_on_its_side = surface(self.geometry, self.wall_on_its_side).mesh[0]
        pit_wall = surface(self.geometry, "WallPit").mesh[0]
        for gap_s in (
            (self.lane.s_fork + self.lane.s_entry) / 2.0,
            (self.lane.s_exit + self.lane.s_merge) / 2.0,
        ):
            middle = self.layout.path.at(gap_s).offset(self.side * runoff)
            for wall in (wall_on_its_side, pit_wall):
                self.assertGreater(min(math.dist((x, y), middle) for x, y, _ in wall), 5.0)
        beside_the_grid = self.layout.path.at(0.0).offset(self.side * runoff)
        self.assertLess(min(math.dist((x, y), beside_the_grid) for x, y, _ in pit_wall), 1.0)
        self.assertGreater(min(math.dist((x, y), beside_the_grid) for x, y, _ in wall_on_its_side), 5.0)

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
        wall = self.layout.runoff_half_width + WALL_THICKNESS_M
        building = self.building()
        here = self.layout.path.at(building.at.s)
        centre = here.offset(self.side * abs(building.at.offset))
        forward = (here.left_normal[1], -here.left_normal[0])
        for slot, box in enumerate(self.pit_lane.pit_boxes):
            s = nearest_sample(self.layout, box.x, box.y).s
            offset = self.side * left_of_centre(self.layout, box.x, box.y, s)
            self.assertGreater(offset, wall + LANE_WIDTH_M / 2.0)
            self.assertLess(offset, wall + LANE_WIDTH_M)
            along_the_building = (box.x - centre[0]) * forward[0] + (box.y - centre[1]) * forward[1]
            door = self.lane.door_first + slot * PITS_DOOR_PITCH_M
            self.assertAlmostEqual(along_the_building, door, delta=0.5, msg=f"slot {slot}'s door")
            past_entry = (s - self.lane.s_entry) % self.layout.path.length
            self.assertLess(past_entry, self.lane.s_exit - self.lane.s_entry, f"slot {slot} between the lines")

    def test_each_door_of_the_building_stands_where_its_pit_box_lines_up(self) -> None:
        building = self.building()
        for slot in range(8):
            (piece,) = [p for p in self.geometry.scenery if p.name == f"Prop.pits.crowd-{slot}"]
            vertices, faces = piece.mesh
            # The colour's mesh holds a door and a flag; the door is the face that reaches the ground.
            (door,) = [f for f in faces if min(vertices[v][2] for v in f) < -GRASS_DROP_M + 1e-6]
            along = sum(across_track(self.layout, building.at.s, *vertices[v][:2])[0] for v in door) / len(door)
            self.assertAlmostEqual(along, self.lane.door_first + slot * PITS_DOOR_PITCH_M, places=3, msg=f"door {slot}")

    def test_no_boost_pad_or_coin_stands_on_a_called_kart_s_side_of_the_road(self) -> None:
        """A kart called in crosses the road to the lane's side between the fork and the
        entry line, where a boost would throw it at the pit wall."""
        lap = self.layout.path.length
        for kind, items in (("pad", self.geometry.boost_pads), ("coin", self.geometry.coins)):
            for item in items:
                s = nearest_sample(self.layout, item.x, item.y).s
                if (s - self.lane.s_fork) % lap <= self.lane.s_entry - self.lane.s_fork:
                    self.assertLessEqual(self.SIDE * left_of_centre(self.layout, item.x, item.y, s), 1e-6, f"{kind} at s={s}")

    def test_no_tree_stands_on_the_lane(self) -> None:
        for tree in trees(self.layout):
            nearest = min(math.dist((tree.x, tree.y), point[:2]) for point in self.pit_lane.path)
            self.assertGreater(nearest, LANE_WIDTH_M)


class PitLaneOnBellDeer(ThePitLane, unittest.TestCase):
    NAME = "bell-deer"


class PitLaneOnGoldstone(ThePitLane, unittest.TestCase):
    NAME = "goldstone"


class PitLaneOnMonarch(ThePitLane, unittest.TestCase):
    NAME = "monarch"

    def test_the_building_keeps_clear_of_every_other_road(self) -> None:
        building = self.building()
        here = self.layout.path.at(building.at.s)
        back = here.offset(self.side * (abs(building.at.offset) + PITS_DEPTH_M / 2.0))
        lap = self.layout.path.length
        for sample in self.layout.path.samples:
            if min((sample.s - building.at.s) % lap, (building.at.s - sample.s) % lap) > building.length:
                self.assertGreater(math.dist((sample.x, sample.y), back), self.layout.runoff_half_width + WALL_THICKNESS_M + 8.0)


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

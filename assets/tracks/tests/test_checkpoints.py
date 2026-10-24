"""Checkpoint placement and the shortcut, checked without Blender:

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest
from dataclasses import replace

from trackgen.layout import (
    CHECKPOINT_SHORTCUT_LIMIT_M,
    SHORTCUT_WIDTH_M,
    WALL,
    Shortcut,
    Wall,
    build,
    check_checkpoint_spacing,
    checkpoint_arc_lengths,
    shortcut_pair,
    shortcut_skipped_arc,
)
from trackgen.layouts import NAMES, load


def without_wall(layout, segment: str):
    """`layout` less the wall whose run includes `segment`."""
    return replace(layout, walls=[wall for wall in layout.walls if segment not in wall.segments])


class CheckpointSpacing(unittest.TestCase):
    def test_both_shipped_layouts_pass_the_check(self) -> None:
        for name in NAMES:
            with self.subTest(track=name):
                build(load(name))

    def test_a_pair_joined_by_a_line_30_m_shorter_than_the_centreline_across_open_grass_fails(
        self,
    ) -> None:
        # Brands Patch with hairpin one's inside wall taken away and no
        # shortcut declared: the checkpoints either side of the hairpin are
        # 36 m apart across the infield and 66 m apart round the centreline,
        # with nothing between. With the wall, the shipped layout passes
        # above.
        layout = replace(without_wall(load("brands-patch"), "hairpin-1"), shortcut=None)
        with self.assertRaises(ValueError) as refused:
            build(layout)
        message = str(refused.exception)
        self.assertIn("checkpoints 1 and 2", message)
        self.assertIn(f"{CHECKPOINT_SHORTCUT_LIMIT_M:.0f} m", message)

    def test_the_check_reads_the_pair_that_wraps_past_the_start_line(self) -> None:
        # The oval's walls stand outside its turns only. Without the two
        # checkpoints deep in the second turn, the one at its entry and the
        # start line are joined by a line across the turn's inside.
        layout = load("oval")
        arc_lengths = checkpoint_arc_lengths(layout)
        check_checkpoint_spacing(layout, arc_lengths, None)
        with self.assertRaises(ValueError) as refused:
            check_checkpoint_spacing(layout, arc_lengths[:-2], None)
        self.assertIn(f"checkpoints {len(arc_lengths) - 3} and 0", str(refused.exception))

    def test_only_the_declared_shortcut_pair_may_skip_a_corner_with_no_wall_across_it(self) -> None:
        # Hairpin two's wall cut back to the approach alone: the gap is still
        # covered, but the line from the chicane to the jump straight
        # crosses no wall, so only the exemption lets the pair through.
        layout = without_wall(load("brands-patch"), "hairpin-2")
        layout = replace(layout, walls=layout.walls + [Wall(("approach",), side=1)])
        arc_lengths = checkpoint_arc_lengths(layout)
        pair = shortcut_pair(layout, arc_lengths)
        self.assertEqual(pair, (6, 7))

        check_checkpoint_spacing(layout, arc_lengths, pair)
        with self.assertRaises(ValueError) as refused:
            check_checkpoint_spacing(layout, arc_lengths, None)
        self.assertIn("checkpoints 6 and 7", str(refused.exception))


class TheShortcut(unittest.TestCase):
    def test_brands_patch_declares_one_on_the_approach_and_has_eleven_checkpoints(self) -> None:
        layout = load("brands-patch")
        approach = layout.path.segment("approach")
        self.assertIsNotNone(layout.shortcut)
        self.assertTrue(approach.s_start < layout.shortcut.s < approach.s_end)
        self.assertEqual(layout.shortcut.width, SHORTCUT_WIDTH_M)
        self.assertEqual(layout.checkpoint_count, 12)

        arc_lengths = checkpoint_arc_lengths(layout)
        self.assertEqual(len(arc_lengths), 11)
        self.assertEqual(len(build(layout).checkpoints), 11)

    def test_the_skipped_arc_runs_from_the_gap_round_hairpin_two_to_the_jump_straight_opposite(
        self,
    ) -> None:
        layout = load("brands-patch")
        path = layout.path
        s_gap, s_rejoin = shortcut_skipped_arc(layout)
        self.assertEqual(s_gap, layout.shortcut.s)
        jump = path.segment("jump")
        self.assertTrue(jump.s_start < s_rejoin < jump.s_end)
        # Straight across the infield: the kart rejoins level with the gap.
        self.assertAlmostEqual(path.at(s_rejoin).x, path.at(s_gap).x, delta=1.0)

    def test_no_evenly_spaced_checkpoint_stands_inside_the_skipped_arc(self) -> None:
        layout = load("brands-patch")
        s_gap, s_rejoin = shortcut_skipped_arc(layout)
        even = [k * layout.path.length / layout.checkpoint_count for k in range(layout.checkpoint_count)]
        skipped = [s for s in even if s_gap < s < s_rejoin]
        self.assertEqual(len(skipped), 1, "hairpin two's apex checkpoint")
        arc_lengths = checkpoint_arc_lengths(layout)
        self.assertEqual(arc_lengths, [s for s in even if s not in skipped])

    def test_the_gap_is_the_shortcuts_width_centred_on_its_arc_length(self) -> None:
        layout = load("brands-patch")
        path = layout.path
        sample = path.at(layout.shortcut.s)
        # The gap is in the wall on the hairpin's inside, left of travel.
        gx, gy = sample.offset(layout.runoff_half_width)
        half = layout.shortcut.width / 2.0

        walls = [surface for surface in build(layout).surfaces if surface.surface == WALL]
        distances = [
            math.hypot(x - gx, y - gy) for surface in walls for x, y, _ in surface.mesh[0]
        ]
        self.assertGreaterEqual(min(distances), half - 1e-6)
        ends = [
            (x, y)
            for surface in walls
            for x, y, _ in surface.mesh[0]
            if abs(math.hypot(x - gx, y - gy) - half) < 1e-6
        ]
        # The approach runs west: one edge of the gap on each side of it.
        self.assertTrue(any(x < gx for x, _ in ends) and any(x > gx for x, _ in ends), ends)

    def test_a_shortcut_over_an_arc_length_no_wall_covers_fails(self) -> None:
        layout = load("brands-patch")
        on_the_hill = replace(layout, shortcut=Shortcut(layout.path.segment("hill").s_mid))
        with self.assertRaises(ValueError) as refused:
            build(on_the_hill)
        self.assertIn("no wall", str(refused.exception))

    def test_a_layout_without_a_shortcut_keeps_every_evenly_spaced_checkpoint(self) -> None:
        oval = load("oval")
        self.assertIsNone(oval.shortcut)
        self.assertEqual(len(checkpoint_arc_lengths(oval)), oval.checkpoint_count)
        self.assertIsNone(shortcut_pair(oval, checkpoint_arc_lengths(oval)))


if __name__ == "__main__":
    unittest.main()

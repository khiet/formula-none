"""Checkpoint placement, checked without Blender:

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest
from dataclasses import replace

from trackgen.layout import CHECKPOINT_SHORTCUT_LIMIT_M, build, check_checkpoint_spacing, checkpoint_arc_lengths
from trackgen.layouts import NAMES, load


class CheckpointSpacing(unittest.TestCase):
    def test_both_shipped_layouts_pass_the_check(self) -> None:
        for name in NAMES:
            with self.subTest(track=name):
                build(load(name))

    def test_a_pair_of_checkpoints_joined_by_a_line_30_m_shorter_than_the_centreline_fails(self) -> None:
        # Brands Patch with hairpin one's apex checkpoint taken away: the
        # checkpoints either side of the hairpin are 36 m apart across the
        # infield and 66 m apart round the centreline.
        layout = replace(load("brands-patch"), apex_checkpoints=())
        with self.assertRaises(ValueError) as refused:
            build(layout)
        message = str(refused.exception)
        self.assertIn("checkpoints 1 and 2", message)
        self.assertIn(f"{CHECKPOINT_SHORTCUT_LIMIT_M:.0f} m", message)

    def test_the_check_reads_the_pair_that_wraps_past_the_start_line(self) -> None:
        layout = load("brands-patch")
        arc_lengths = checkpoint_arc_lengths(layout)
        check_checkpoint_spacing(layout.path, arc_lengths)
        # Without the checkpoint on the final corner, the last one on the
        # jump straight and the start line are joined by a line across it.
        with self.assertRaises(ValueError) as refused:
            check_checkpoint_spacing(layout.path, arc_lengths[:-1])
        self.assertIn(f"checkpoints {len(arc_lengths) - 2} and 0", str(refused.exception))


class ApexCheckpoints(unittest.TestCase):
    def test_brands_patch_checkpoints_hairpin_one_at_its_apex_in_lap_order_with_the_run_off_width(self) -> None:
        layout = load("brands-patch")
        hairpin = layout.path.segment("hairpin-1")
        apex = layout.path.at(hairpin.s_mid)

        arc_lengths = checkpoint_arc_lengths(layout)
        self.assertEqual(len(arc_lengths), layout.checkpoint_count + 1)
        self.assertEqual(arc_lengths, sorted(arc_lengths))
        self.assertEqual(arc_lengths.index(hairpin.s_mid), 2)

        checkpoints = build(layout).checkpoints
        self.assertEqual(len(checkpoints), len(arc_lengths))
        at_apex = checkpoints[2]
        self.assertAlmostEqual(math.hypot(at_apex.x - apex.x, at_apex.y - apex.y), 0.0)
        self.assertEqual(
            {checkpoint.half_width for checkpoint in checkpoints}, {layout.runoff_half_width}
        )


if __name__ == "__main__":
    unittest.main()

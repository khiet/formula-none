"""Gate placement, checked without Blender:

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest
from dataclasses import replace

from trackgen.layout import GATE_SHORTCUT_LIMIT_M, build, check_gate_spacing, gate_arc_lengths
from trackgen.layouts import NAMES, load


class GateSpacing(unittest.TestCase):
    def test_both_shipped_layouts_pass_the_check(self) -> None:
        for name in NAMES:
            with self.subTest(track=name):
                build(load(name))

    def test_a_pair_of_gates_joined_by_a_line_30_m_shorter_than_the_centreline_fails(self) -> None:
        # Brands Patch with hairpin one's apex gate taken away: its gates
        # either side of the hairpin are 36 m apart across the infield and
        # 66 m apart round the centreline.
        layout = replace(load("brands-patch"), apex_gates=())
        with self.assertRaises(ValueError) as refused:
            build(layout)
        message = str(refused.exception)
        self.assertIn("gates 1 and 2", message)
        self.assertIn(f"{GATE_SHORTCUT_LIMIT_M:.0f} m", message)

    def test_the_check_reads_the_pair_that_wraps_past_the_start_line(self) -> None:
        layout = load("brands-patch")
        gates = gate_arc_lengths(layout)
        check_gate_spacing(layout.path, gates)
        # Without the gate on the final corner, the last gate on the jump
        # straight and the start line are joined by a line across the corner.
        with self.assertRaises(ValueError) as refused:
            check_gate_spacing(layout.path, gates[:-1])
        self.assertIn(f"gates {len(gates) - 2} and 0", str(refused.exception))


class ApexGates(unittest.TestCase):
    def test_brands_patch_gates_hairpin_one_at_its_midpoint_in_lap_order(self) -> None:
        layout = load("brands-patch")
        gates = gate_arc_lengths(layout)
        hairpin = layout.path.segment("hairpin-1")

        self.assertEqual(len(gates), layout.checkpoint_count + 1)
        self.assertEqual(gates, sorted(gates))
        self.assertIn(hairpin.s_mid, gates)
        self.assertEqual(gates.index(hairpin.s_mid), 2)

    def test_the_apex_gate_spans_the_run_off_like_every_other_gate(self) -> None:
        layout = load("brands-patch")
        geometry = build(layout)
        hairpin = layout.path.segment("hairpin-1")
        apex = layout.path.at(hairpin.s_mid)

        gate = geometry.checkpoints[2]
        self.assertEqual(len(geometry.checkpoints), layout.checkpoint_count + 1)
        self.assertAlmostEqual(math.hypot(gate.x - apex.x, gate.y - apex.y), 0.0)
        self.assertEqual(gate.half_width, layout.runoff_half_width)
        self.assertEqual({gate.half_width for gate in geometry.checkpoints}, {layout.runoff_half_width})


if __name__ == "__main__":
    unittest.main()

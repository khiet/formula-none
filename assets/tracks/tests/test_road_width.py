"""The road's width along the lap, checked without Blender: a layout's default asphalt half
width, narrowed over named segments and tapered linearly in and out so the edge never steps;
the asphalt, sand, walls, checkpoints and painted kerbs following the local width; every item
box, pad, coin, sausage kerb and wheel crossing inside it; and Monarch narrowed toward Monaco's
road, through Mirabeau and the Grand Hotel hairpin and at Rascasse, 12 m everywhere else.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import unittest
from dataclasses import replace

from tests.test_boost_pads import tuning_constant
from tests.test_tyre_walls import left_of_centre, nearest_sample
from trackgen.layout import (
    COIN_LINE_SPACING_M,
    HAZARD_HALF_DEPTH_M,
    HOARDING_SETBACK_M,
    HOARDING_SPACING_M,
    KERB_WIDTH_M,
    WALL_THICKNESS_M,
    Taper,
    along_outside,
    build,
)
from trackgen.layouts import NAMES, load

BOX_HALF_LENGTH_M = tuning_constant("ITEM_BOX_HALF_LENGTH_M")
BOX_HALF_WIDTH_M = tuning_constant("ITEM_BOX_HALF_WIDTH_M")
PAD_HALF_WIDTH_M = tuning_constant("BOOST_PAD_HALF_WIDTH_M")
COIN_HALF_WIDTH_M = tuning_constant("COIN_HALF_WIDTH_M")
KERB_HALF_WIDTH_M = tuning_constant("KERB_HALF_WIDTH_M")

# Monarch's narrowed corners: Mirabeau to Mirabeau Bas, the hairpin narrowest, and Rascasse.
# Mirabeau Bas is the bot gate's: at 9 m a boosted kart cleared its wall.
ZONE_A = {
    "mirabeau": 4.5,
    "mirabeau-kink": 4.5,
    "hairpin-approach": 4.5,
    "grand-hotel": 4.0,
    "hairpin-exit": 4.5,
    "mirabeau-bas": 5.0,
}
TAPER_M = 15.0
# The half metre the hairpin closes over its own ends, and Mirabeau Bas opens over its start.
HALF_METRE_TAPER_M = 5.0
# How far before the pit lane's fork the road is back to its full width at the latest.
FORK_MARGIN_M = 20.0
# Metres between the readings of a sweep along the lap.
SWEEP_STEP_M = 0.05


def sweep(start: float, end: float):
    steps = round((end - start) / SWEEP_STEP_M)
    return [start + k * (end - start) / steps for k in range(steps + 1)]


class Monarch(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load("monarch")
        self.seg = self.layout.path.segment
        self.width = self.layout.half_width_at

    def test_mirabeau_to_the_hairpin_exit_narrows_to_9_m_the_hairpin_to_8_m_and_mirabeau_bas_to_10_m(self) -> None:
        tapered = {"grand-hotel": (HALF_METRE_TAPER_M, HALF_METRE_TAPER_M), "mirabeau-bas": (HALF_METRE_TAPER_M, 0.0)}
        for name, half in ZONE_A.items():
            segment = self.seg(name)
            start, end = tapered.get(name, (0.0, 0.0))
            with self.subTest(name):
                for s in sweep(segment.s_start + start, segment.s_end - end):
                    self.assertAlmostEqual(self.width(s), half, places=9, msg=round(s, 2))
        # The half metres close and open from and to the 9 m either side.
        hairpin, bas = self.seg("grand-hotel"), self.seg("mirabeau-bas")
        self.assertAlmostEqual(self.width(hairpin.s_start), 4.5, places=9)
        self.assertAlmostEqual(self.width(hairpin.s_end), 4.5, places=9)
        self.assertAlmostEqual(self.width(bas.s_start), 4.5, places=9)

    def test_rascasse_narrows_to_9_m(self) -> None:
        segment = self.seg("rascasse")
        for s in sweep(segment.s_start, segment.s_end):
            self.assertAlmostEqual(self.width(s), 4.5, places=9, msg=round(s, 2))

    def test_the_road_narrows_over_the_last_15_m_of_the_descent_past_the_wheels_hazard_zone(self) -> None:
        descent = self.seg("casino-descent")
        start = descent.s_end - TAPER_M
        (crossing,) = self.layout.wheel_crossings
        self.assertLess(crossing.s + HAZARD_HALF_DEPTH_M, start, "the wheel crosses full width")
        for s in sweep(descent.s_start, start):
            self.assertEqual(self.width(s), 6.0)
        self.assertAlmostEqual(self.width(start + TAPER_M / 2.0), 5.25, places=6)
        self.assertAlmostEqual(self.width(descent.s_end), 4.5, places=9)

    def test_the_road_widens_over_portier_to_12_m_at_the_tunnel_entry_and_its_box_row(self) -> None:
        start, end = self.seg("portier-approach").s_start, self.seg("tunnel-entry").s_start
        self.assertAlmostEqual(self.width(start), 5.0, places=9)
        self.assertAlmostEqual(self.width((start + end) / 2.0), 5.5, places=6)
        entry = self.seg("tunnel-entry")
        for s in sweep(end, entry.s_end):
            self.assertAlmostEqual(self.width(s), 6.0, places=9)
        (row,) = [r for r in self.layout.item_rows if entry.s_start <= r.s < entry.s_end]
        self.assertAlmostEqual(self.width(row.s - BOX_HALF_LENGTH_M), 6.0, places=9)

    def test_the_road_narrows_over_the_last_15_m_of_the_quay_past_its_box_row(self) -> None:
        quay = self.seg("quay")
        start = quay.s_end - TAPER_M
        (row,) = [r for r in self.layout.item_rows if quay.s_start <= r.s < quay.s_end]
        self.assertLess(row.s + BOX_HALF_LENGTH_M, start)
        for s in sweep(quay.s_start, start):
            self.assertEqual(self.width(s), 6.0)
        self.assertAlmostEqual(self.width(quay.s_end), 4.5, places=9)

    def test_the_road_is_back_to_12_m_well_before_the_pit_lanes_fork(self) -> None:
        lap = self.layout.path.length
        fork = self.layout.pit_lane.s_fork % lap
        back = self.seg("rascasse").s_end
        widened = next(s for s in sweep(back, fork) if self.width(s) == 6.0)
        self.assertLessEqual(widened, fork - FORK_MARGIN_M)
        for s in sweep(widened, fork):
            self.assertEqual(self.width(s), 6.0)

    def test_everywhere_else_is_12_m(self) -> None:
        narrowed = [
            (self.seg("casino-descent").s_end - TAPER_M, self.seg("tunnel-entry").s_start),
            (self.seg("quay").s_end - TAPER_M, self.seg("rascasse-exit").s_end),
        ]
        for sample in self.layout.path.samples:
            if any(a < sample.s < b for a, b in narrowed):
                continue
            self.assertEqual(self.width(sample.s), 6.0, round(sample.s, 2))

    def test_the_edge_never_steps_anywhere_round_the_lap(self) -> None:
        readings = [self.width(s) for s in sweep(0.0, self.layout.path.length)]
        # The steepest taper closes 1.5 m over 15 m; a half metre over 5 m is as steep.
        steepest = 1.5 / TAPER_M * SWEEP_STEP_M
        self.assertLessEqual(max(abs(b - a) for a, b in zip(readings, readings[1:])), steepest + 1e-9)


class TheLocalWidth(unittest.TestCase):
    """What every course's export hangs on the width at each sample, and what it must keep inside it."""

    def test_the_asphalt_runs_edge_to_edge_at_the_local_width(self) -> None:
        for name in NAMES:
            layout = load(name)
            (asphalt,) = [s for s in build(layout).surfaces if s.name == "Asphalt"]
            vertices = asphalt.mesh[0]
            count = len(layout.path.samples)
            with self.subTest(name):
                # A ramp rebuilds part of the ribbon but keeps its rails, the first two runs of vertices.
                for index, sample in enumerate(layout.path.samples):
                    half = layout.half_width_at(sample.s)
                    for side, vertex in ((1, vertices[index]), (-1, vertices[count + index])):
                        self.assertAlmostEqual(vertex[0], sample.offset(side * half)[0], places=9)
                        self.assertAlmostEqual(vertex[1], sample.offset(side * half)[1], places=9)

    def test_the_checkpoints_reach_the_local_walls_outer_faces(self) -> None:
        for name in NAMES:
            layout = load(name)
            spacing = layout.path.length / layout.checkpoint_count
            for number, checkpoint in enumerate(build(layout).checkpoints):
                with self.subTest(name, checkpoint=number):
                    s = layout.path.at(number * spacing).s
                    self.assertGreaterEqual(checkpoint.half_width, layout.runoff_half_width_at(s) + WALL_THICKNESS_M)

    def test_the_painted_kerbs_line_the_local_edge(self) -> None:
        layout = load("monarch")
        hairpin = layout.path.segment("grand-hotel")
        stripes = [p for p in build(layout).scenery if p.name.startswith("Kerb.grand-hotel.L.")]
        self.assertTrue(stripes)
        for x, y, _ in (v for p in stripes for v in p.mesh[0]):
            half = layout.half_width_at(nearest_sample(layout, x, y, hairpin.s_mid).s)
            left = left_of_centre(layout, x, y, hairpin.s_mid)
            self.assertGreaterEqual(left, half - KERB_WIDTH_M - 1e-6)
            self.assertLessEqual(left, half + 1e-6)
        self.assertAlmostEqual(min(left_of_centre(layout, x, y, hairpin.s_mid) for p in stripes for x, y, _ in p.mesh[0]), 4.0 - KERB_WIDTH_M, places=6)

    def test_the_hoardings_at_portier_and_rascasse_stand_their_setback_behind_the_local_wall(self) -> None:
        layout = load("monarch")
        for run in layout.decor.hoardings:
            for spot in along_outside(layout, run.segments, run.side, HOARDING_SETBACK_M, HOARDING_SPACING_M):
                sample = nearest_sample(layout, spot.x, spot.y, layout.path.segment(run.segments[0]).s_mid)
                with self.subTest(run.segments[0], s=round(sample.s)):
                    expected = layout.runoff_half_width_at(sample.s) + WALL_THICKNESS_M + HOARDING_SETBACK_M
                    self.assertAlmostEqual(abs(left_of_centre(layout, spot.x, spot.y, sample.s)), expected, places=6)
        narrowed = [run for run in layout.decor.hoardings if run.segments in (("portier",), ("rascasse",))]
        self.assertEqual(len(narrowed), 2, "both stand where the road narrows")

    def test_every_box_pad_coin_and_sausage_kerb_lies_inside_the_local_width(self) -> None:
        for name in NAMES:
            layout = load(name)
            placed = [(row.s, offset, BOX_HALF_WIDTH_M) for row in layout.item_rows for offset in row.offsets]
            placed += [(pad.s, pad.offset, PAD_HALF_WIDTH_M) for pad in layout.boost_pads]
            placed += [(line.s + COIN_LINE_SPACING_M * k, line.offset, COIN_HALF_WIDTH_M) for line in layout.coin_lines for k in range(line.count)]
            placed += [(kerb.s, kerb.offset, KERB_HALF_WIDTH_M) for kerb in layout.kerbs]
            for s, offset, half_width in placed:
                with self.subTest(name, s=round(s), offset=offset):
                    self.assertLessEqual(abs(offset) + half_width, layout.half_width_at(s) + 1e-9)

    def test_every_wheel_crossing_reaches_the_local_walls_and_crosses_where_the_width_is_constant(self) -> None:
        for name in NAMES:
            layout = load(name)
            for crossing, placed in zip(layout.wheel_crossings, build(layout).wheel_crossings, strict=True):
                with self.subTest(name, s=round(crossing.s)):
                    self.assertAlmostEqual(placed.half_width, layout.runoff_half_width_at(crossing.s))
                    reach = sweep(crossing.s - HAZARD_HALF_DEPTH_M, crossing.s + HAZARD_HALF_DEPTH_M)
                    self.assertEqual({layout.half_width_at(s) for s in reach}, {layout.half_width_at(crossing.s)})


class Overrides(unittest.TestCase):
    def test_a_course_without_overrides_is_its_default_width_everywhere(self) -> None:
        layout = load("oval")
        self.assertEqual(layout.half_widths, {})
        self.assertEqual({layout.half_width_at(s.s) for s in layout.path.samples}, {layout.asphalt_half_width})

    def test_an_override_names_a_segment_of_the_path(self) -> None:
        with self.assertRaises(KeyError):
            replace(load("oval"), half_widths={"no-such-corner": 4.0})

    def test_a_taper_spans_from_the_width_before_it_to_the_width_after_it(self) -> None:
        layout = load("monarch")
        quay = layout.path.segment("quay")
        # Taper the whole quay instead: from 12 m at its start to Rascasse's 9 m at its end.
        tapered = replace(layout, tapers=[Taper(quay.s_start, quay.s_end)])
        self.assertAlmostEqual(tapered.half_width_at(quay.s_start), 6.0)
        self.assertAlmostEqual(tapered.half_width_at(quay.s_mid), 5.25)
        self.assertAlmostEqual(tapered.half_width_at(quay.s_end), 4.5)

    def test_a_tunnel_or_a_pit_lane_where_the_width_changes_is_refused(self) -> None:
        layout = load("monarch")
        hairpin = layout.path.segment("grand-hotel")
        tunnel = replace(layout.decor.tunnels[0], s_start=hairpin.s_start - 30.0)
        lane = replace(layout.pit_lane, s_fork=layout.path.segment("rascasse").s_start - layout.path.length)
        for what, broken in (
            ("tunnel", replace(layout, decor=replace(layout.decor, tunnels=[tunnel]))),
            ("pit lane", replace(layout, pit_lane=lane)),
        ):
            with self.subTest(what), self.assertRaises(ValueError):
                build(broken)


if __name__ == "__main__":
    unittest.main()

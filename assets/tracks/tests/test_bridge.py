"""The bridge where Bell Deer crosses itself, checked without Blender:
nothing the sim reads sits near the other leg, the deck clears the underpass,
the scenery stands the deck on an embankment with a span over the road
below, its underside dark and ribbed with girders that still clear the chase
camera, and the Circuit Wheel stands in view, unhidden, from the whole back
straight over it.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import unittest

from tests.test_monarch import camera_at, half_field_of_view
from trackgen.layout import (
    BRIDGE_GIRDER_WIDTH_M,
    BRIDGE_GIRDERS,
    CHASE_CAMERA_ABOVE_M,
    GRASS_DROP_M,
    GRID_FRONT_ROW_BEHIND_LINE_M,
    WALL_THICKNESS_M,
    build,
    locate,
)
from trackgen.layouts import load
from trackgen.path import elevation
from trackgen.props import WHEEL_HUB_HEIGHT_M, WHEEL_RADIUS_M

# Gates and placements are height-blind in the sim, so each stays this far,
# in plan, from the other leg's centreline.
CLEARANCE_M = 20.0
# Metres of arc length either side of the crossing that count as a leg at
# the bridge: a little past the embankment's 55 m.
LEG_REACH_M = 60.0
# The span leaves at least this much open ground under the deck, wide enough
# that both of the underpass's walls stand clear of the abutments.
MIN_SPAN_M = 28.0


def face_normal(vertices, face) -> tuple[float, float, float]:
    """The face's unnormalised normal, the way its winding points."""
    a, b, c = (vertices[i] for i in face[:3])
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    return u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]


def bell_deer():
    layout = load("bell-deer")
    return layout, build(layout), layout.decor.bridges[0]


def wrapped(layout, a: float, b: float) -> float:
    """Metres of arc length between `a` and `b`, the short way round the loop."""
    gap = abs(a - b) % layout.path.length
    return min(gap, layout.path.length - gap)


def leg(layout, s: float):
    path = layout.path
    return [path.samples[i] for i in path.indices_between(s - LEG_REACH_M, s + LEG_REACH_M)]


def placements(layout, geometry):
    """Everything the sim reads off the sidecar, as (what, arc length, points in plan)."""
    path = layout.path
    for number, checkpoint in enumerate(geometry.checkpoints):
        across = (-math.sin(checkpoint.heading), math.cos(checkpoint.heading))
        ends = [
            (checkpoint.x + side * checkpoint.half_width * across[0], checkpoint.y + side * checkpoint.half_width * across[1])
            for side in (1, -1)
        ]
        yield f"checkpoint {number}", number * path.length / layout.checkpoint_count, [(checkpoint.x, checkpoint.y), *ends]
    boxes = iter(geometry.item_boxes)
    for row in layout.item_rows:
        for offset in row.offsets:
            box = next(boxes)
            yield f"item box at {row.s:.0f} m, {offset} m", row.s, [(box.x, box.y)]
    for pad, placed in zip(layout.boost_pads, geometry.boost_pads, strict=True):
        yield f"boost pad at {pad.s:.0f} m", pad.s, [(placed.x, placed.y)]
    for kerb, placed in zip(layout.kerbs, geometry.kerbs, strict=True):
        yield f"kerb at {kerb.s:.0f} m", kerb.s, [(placed.x, placed.y)]
    for crossing, placed in zip(layout.wheel_crossings, geometry.wheel_crossings, strict=True):
        # The wheel's path, wall to wall; its hazard zone is paint the sim never reads.
        across = (-math.sin(placed.heading), math.cos(placed.heading))
        path_ends = [(placed.x + d * across[0], placed.y + d * across[1]) for d in (-placed.half_width, placed.half_width)]
        yield f"wheel crossing at {crossing.s:.0f} m", crossing.s, [(placed.x, placed.y), *path_ends]
    for wall in layout.tyre_walls:
        x, y = path.at(wall.s).offset(wall.offset)
        yield f"tyre wall at {wall.s:.0f} m", wall.s, [(x, y)]
    for slot, spot in enumerate(geometry.spawn_grid):
        yield f"grid slot {slot}", -GRID_FRONT_ROW_BEHIND_LINE_M, [(spot.x, spot.y)]


class Clearance(unittest.TestCase):
    def test_nothing_the_sim_reads_sits_within_20_m_in_plan_of_the_other_leg_at_the_bridge(self) -> None:
        layout, geometry, bridge = bell_deer()
        for own, other in ((bridge.deck_s, bridge.underpass_s), (bridge.underpass_s, bridge.deck_s)):
            other_leg = leg(layout, other)
            for what, s, points in placements(layout, geometry):
                if wrapped(layout, s, own) > LEG_REACH_M:
                    continue
                nearest = min(math.dist(p, (q.x, q.y)) for p in points for q in other_leg)
                with self.subTest(what):
                    self.assertGreaterEqual(nearest, CLEARANCE_M)

    def test_the_legs_cross_where_the_bridge_says(self) -> None:
        layout, _, bridge = bell_deer()
        deck = layout.path.at(bridge.deck_s)
        under = layout.path.at(bridge.underpass_s)
        self.assertLess(math.dist((deck.x, deck.y), (under.x, under.y)), 1.0)


class Deck(unittest.TestCase):
    def test_the_deck_stands_5_m_over_the_underpass(self) -> None:
        layout, _, bridge = bell_deer()
        self.assertAlmostEqual(elevation(layout.elevation, bridge.deck_s), 5.0, delta=0.01)
        self.assertEqual(elevation(layout.elevation, bridge.underpass_s), 0.0)

    def test_the_span_leaves_28_m_open_under_the_deck_with_both_underpass_walls_clear(self) -> None:
        layout, geometry, bridge = bell_deer()
        under = layout.path.at(bridge.underpass_s)
        across = under.left_normal
        # The abutments' faces, as metres left of the underpass's centreline.
        abutments = [
            (x - under.x) * across[0] + (y - under.y) * across[1]
            for piece in geometry.scenery
            if piece.name == "Bridge.wall"
            for x, y, z in piece.mesh[0]
            if z < 0.0
        ]
        near = max(a for a in abutments if a < 0.0)
        far = min(a for a in abutments if a > 0.0)
        self.assertGreaterEqual(far - near, MIN_SPAN_M)
        walls = layout.runoff_half_width + WALL_THICKNESS_M
        self.assertGreater(-near, walls)
        self.assertGreater(far, walls)

    def test_the_underside_is_the_tunnels_dark(self) -> None:
        _, geometry, _ = bell_deer()
        names = {p.name for p in geometry.scenery if p.name.startswith("Bridge.")}
        self.assertIn("Bridge.carbon", names)
        self.assertIn("Bridge.carbon-shade", names)

    def test_girders_hang_below_a_recessed_soffit_and_clear_the_chase_camera_on_the_underpass(self) -> None:
        layout, geometry, bridge = bell_deer()
        road = elevation(layout.elevation, bridge.underpass_s)
        (soffit,) = [p for p in geometry.scenery if p.name == "Bridge.carbon-shade"]
        (girders,) = [p for p in geometry.scenery if p.name == "Bridge.carbon"]
        under = layout.path.at(bridge.underpass_s)
        walls = layout.runoff_half_width + WALL_THICKNESS_M

        def over_the_road(piece):
            """The piece's heights within the underpass's walls."""
            return [z for x, y, z in piece.mesh[0] if abs((x - under.x) * under.left_normal[0] + (y - under.y) * under.left_normal[1]) <= walls]

        lowest = min(over_the_road(girders))
        self.assertGreater(lowest - road, CHASE_CAMERA_ABOVE_M)
        self.assertGreater(min(over_the_road(soffit)), lowest + 0.3)
        self.assertLess(max(z for _, _, z in soffit.mesh[0]), 5.0)

    def test_every_underside_face_looks_down_or_out_of_its_girder(self) -> None:
        layout, geometry, bridge = bell_deer()
        path = layout.path
        edge = layout.runoff_half_width + WALL_THICKNESS_M
        width = BRIDGE_GIRDER_WIDTH_M
        pitch = (2.0 * edge - width) / (BRIDGE_GIRDERS - 1)
        middles = [-edge + width / 2.0 + k * pitch for k in range(BRIDGE_GIRDERS)]
        near = [path.samples[i] for i in path.indices_between(bridge.s_start, bridge.s_end)]
        for name in ("Bridge.carbon", "Bridge.carbon-shade"):
            (piece,) = [p for p in geometry.scenery if p.name == name]
            vertices, faces = piece.mesh
            for face in faces:
                normal = face_normal(vertices, face)
                with self.subTest(name=name, face=face):
                    if abs(normal[2]) > abs(normal[0]) + abs(normal[1]):
                        self.assertLess(normal[2], 0.0)
                        continue
                    x = sum(vertices[i][0] for i in face) / len(face)
                    y = sum(vertices[i][1] for i in face) / len(face)
                    sample = min(near, key=lambda q: math.dist((q.x, q.y), (x, y)))
                    across = (x - sample.x) * sample.left_normal[0] + (y - sample.y) * sample.left_normal[1]
                    girder = min(middles, key=lambda m: abs(m - across))
                    outward = normal[0] * sample.left_normal[0] + normal[1] * sample.left_normal[1]
                    self.assertEqual(outward > 0.0, across > girder)

    def test_at_least_four_girders_cross_over_the_underpass(self) -> None:
        layout, geometry, bridge = bell_deer()
        (girders,) = [p for p in geometry.scenery if p.name == "Bridge.carbon"]
        under = layout.path.at(bridge.underpass_s)
        vertices, faces = girders.mesh
        # Where each downward face, a girder's foot, lies along the underpass, to the metre.
        feet = set()
        for face in faces:
            normal = face_normal(vertices, face)
            if normal[2] >= -(abs(normal[0]) + abs(normal[1])):
                continue
            x = sum(vertices[i][0] for i in face) / len(face)
            y = sum(vertices[i][1] for i in face) / len(face)
            feet.add(round((x - under.x) * math.cos(under.heading) + (y - under.y) * math.sin(under.heading)))
        self.assertGreaterEqual(len(feet), 4, sorted(feet))

    def test_the_embankment_runs_from_the_road_down_to_the_grass(self) -> None:
        _, geometry, _ = bell_deer()
        (bank,) = [p for p in geometry.scenery if p.name == "Bridge.grass"]
        heights = [z for _, _, z in bank.mesh[0]]
        self.assertAlmostEqual(min(heights), -GRASS_DROP_M, places=6)
        self.assertGreater(max(heights), 3.0)


def ray_hits_triangle(origin, target, a, b, c) -> bool:
    """Whether the segment from `origin` to `target` passes through the triangle, short of both ends."""
    d = [target[i] - origin[i] for i in range(3)]
    e1 = [b[i] - a[i] for i in range(3)]
    e2 = [c[i] - a[i] for i in range(3)]
    p = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]]
    det = sum(e1[i] * p[i] for i in range(3))
    if abs(det) < 1e-12:
        return False
    t0 = [origin[i] - a[i] for i in range(3)]
    u = sum(t0[i] * p[i] for i in range(3)) / det
    if not 0.0 <= u <= 1.0:
        return False
    q = [t0[1] * e1[2] - t0[2] * e1[1], t0[2] * e1[0] - t0[0] * e1[2], t0[0] * e1[1] - t0[1] * e1[0]]
    v = sum(d[i] * q[i] for i in range(3)) / det
    if v < 0.0 or u + v > 1.0:
        return False
    return 1e-4 < sum(e2[i] * q[i] for i in range(3)) / det < 1.0 - 1e-3


class Landmark(unittest.TestCase):
    """The back straight runs at the Circuit Wheel the lap passed after the start, so the
    deck's view ends on it."""

    def setUp(self) -> None:
        self.layout = load("bell-deer")
        self.geometry = build(self.layout)
        (wheel,) = [p for p in self.layout.decor.props if p.kind == "circuit-wheel"]
        x, y, _ = locate(self.layout, wheel.at)
        ground = -GRASS_DROP_M
        # The hub and the rim's top, on the wheel's plane.
        self.targets = [(x, y, ground + WHEEL_HUB_HEIGHT_M), (x, y, ground + WHEEL_HUB_HEIGHT_M + WHEEL_RADIUS_M)]
        segment = self.layout.path.segment("back-straight")
        self.cameras = []
        for i in self.layout.path.indices_between(segment.s_start, segment.s_end)[::10]:
            sample = self.layout.path.samples[i]
            cx, cy = camera_at(sample)
            self.cameras.append((sample, (cx, cy, elevation(self.layout.elevation, sample.s) + CHASE_CAMERA_ABOVE_M)))

    def test_the_wheel_is_in_every_chase_cameras_view_down_the_back_straight(self) -> None:
        half = half_field_of_view()
        x, y, _ = self.targets[0]
        for sample, (cx, cy, _) in self.cameras:
            off = math.atan2(y - cy, x - cx) - sample.heading
            with self.subTest(s=sample.s):
                self.assertLess(abs((off + math.pi) % math.tau - math.pi), half / 2.0)

    def test_nothing_stands_between_a_chase_camera_on_the_back_straight_and_the_wheel(self) -> None:
        # Pivoted pieces are in their own frames, so skip them: the wheel's rim and gondolas, and the
        # airship, which flies 45 m up, above every sight line here.
        triangles = []
        for piece in self.geometry.scenery:
            if piece.pivot is not None or piece.name.startswith("Prop.circuit-wheel"):
                continue
            vertices, faces = piece.mesh
            for face in faces:
                for k in range(1, len(face) - 1):
                    triangles.append((piece.name, vertices[face[0]], vertices[face[k]], vertices[face[k + 1]]))
        for sample, camera in self.cameras:
            for target in self.targets:
                low_x, high_x = sorted((camera[0], target[0]))
                low_y, high_y = sorted((camera[1], target[1]))
                for name, a, b, c in triangles:
                    if max(a[0], b[0], c[0]) < low_x or min(a[0], b[0], c[0]) > high_x:
                        continue
                    if max(a[1], b[1], c[1]) < low_y or min(a[1], b[1], c[1]) > high_y:
                        continue
                    with self.subTest(s=sample.s, target=target[2]):
                        self.assertFalse(ray_hits_triangle(camera, target, a, b, c), name)


if __name__ == "__main__":
    unittest.main()

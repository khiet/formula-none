"""The props a layout declares and the builder stands beyond the walls,
checked without Blender: every footprint keeps its clearance from the walls
and from the other leg at the bridge, no tree stands inside one, the sidecar
never learns of one, and the Circuit Wheel is the style guide's card in
three dimensions, with the named nodes the View turns.

    python3 -I -m unittest discover -s assets/tracks/tests -t assets/tracks
"""

from __future__ import annotations

import math
import re
import unittest
from dataclasses import replace
from pathlib import Path

from trackgen import mesh
from trackgen.layout import (
    CROWD_KEYS,
    GANTRY_TOP_M,
    GRANDSTAND_CANOPY_BOTTOM_M,
    GRASS_DROP_M,
    PROP_WALL_CLEARANCE_M,
    WALL_THICKNESS_M,
    Disc,
    Prop,
    Spot,
    Wood,
    World,
    build,
    locate,
)
from trackgen.layouts import load
from trackgen.props import (
    CANOPY_RADIUS_M,
    SUN_AZIMUTH,
    WHEEL_FOOTPRINT_M,
    WHEEL_GONDOLAS,
    WHEEL_HUB_HEIGHT_M,
    WHEEL_RADIUS_M,
    WHEEL_SPOKE_OFFSET_M,
    footprints,
    scatter,
)

BUILD_PY = Path(__file__).resolve().parents[1] / "build.py"
# The only shipped track with props; the oval stays bare.
DRESSED = "brands-patch"
WHEEL = "Prop.circuit-wheel.1"
# Gates and placements are height-blind in the sim, so a prop near the
# bridge stays this far, in plan, from the other leg's centreline (ADR 0047).
BRIDGE_CLEARANCE_M = 20.0
LEG_REACH_M = 60.0


def wall_clearance(layout, x: float, y: float) -> float:
    """Metres of grass from the point to the outer face of the nearest wall."""
    nearest = min(math.dist((x, y), (s.x, s.y)) for s in layout.path.samples)
    return nearest - layout.runoff_half_width - WALL_THICKNESS_M


def faces_up_and_toward_the_sun(face: list[mesh.Vertex]) -> bool:
    a, b, c = face[0], face[1], face[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    return n[2] >= 0.0 and n[0] * SUN_AZIMUTH[0] + n[1] * SUN_AZIMUTH[1] >= 0.0


def wheel_pieces():
    """The Circuit Wheel's scenery on the dressed track, by name."""
    geometry = build(load(DRESSED))
    found = {p.name: p for p in geometry.scenery if p.name.startswith("Prop.circuit-wheel.")}
    if not found:
        raise AssertionError(f"{DRESSED} draws no Circuit Wheel")
    return found


def faces_by_material(piece) -> dict[str, list[list[mesh.Vertex]]]:
    vertices, faces = piece.mesh
    materials = piece.materials or (piece.material,) * len(faces)
    out: dict[str, list[list[mesh.Vertex]]] = {}
    for material, face in zip(materials, faces, strict=True):
        out.setdefault(material, []).append([vertices[v] for v in face])
    return out


class Placing(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(DRESSED)

    def test_the_circuit_wheel_stands_outside_the_first_curve(self) -> None:
        (prop,) = self.layout.decor.props
        self.assertEqual(prop.kind, "circuit-wheel")
        curve = self.layout.path.segment("first-curve")
        self.assertEqual(curve.turn, -1, "the first curve turns right, so its outside is the left")
        self.assertTrue(curve.s_start <= prop.at.s <= curve.s_end)
        self.assertGreater(prop.at.offset, self.layout.runoff_half_width)

    def test_a_spot_hangs_on_the_lap_and_a_world_place_stands_where_it_says(self) -> None:
        sample = self.layout.path.at(86.0)
        x, y, heading = locate(self.layout, Spot(s=86.0, offset=34.0))
        self.assertAlmostEqual(math.dist((x, y), (sample.x, sample.y)), 34.0)
        self.assertAlmostEqual(heading, sample.heading)
        # Left of the centreline: the cross product of the heading and the offset points up.
        along = (math.cos(sample.heading), math.sin(sample.heading))
        self.assertGreater(along[0] * (y - sample.y) - along[1] * (x - sample.x), 0.0)
        self.assertEqual(locate(self.layout, World(-60.0, -40.0, 1.5)), (-60.0, -40.0, 1.5))

    def test_every_props_footprint_keeps_8_m_of_grass_from_every_wall(self) -> None:
        for prop, (x, y, radius) in zip(self.layout.decor.props, footprints(self.layout), strict=True):
            with self.subTest(prop=prop):
                self.assertGreaterEqual(wall_clearance(self.layout, x, y) - radius, PROP_WALL_CLEARANCE_M - 1e-9)

    def test_no_prop_sits_within_20_m_in_plan_of_the_other_leg_at_the_bridge(self) -> None:
        path = self.layout.path
        (bridge,) = self.layout.decor.bridges
        for own, other in ((bridge.deck_s, bridge.underpass_s), (bridge.underpass_s, bridge.deck_s)):
            other_leg = [path.samples[i] for i in path.indices_between(other - LEG_REACH_M, other + LEG_REACH_M)]
            own_leg = [path.samples[i] for i in path.indices_between(own - LEG_REACH_M, own + LEG_REACH_M)]
            for prop, (x, y, radius) in zip(self.layout.decor.props, footprints(self.layout), strict=True):
                # Only a prop beside this leg could reach the other; the wheel stands far from both today.
                if min(math.dist((x, y), (q.x, q.y)) for q in own_leg) - radius > PROP_WALL_CLEARANCE_M + BRIDGE_CLEARANCE_M:
                    continue
                nearest = min(math.dist((x, y), (q.x, q.y)) for q in other_leg) - radius
                with self.subTest(prop=prop, leg=own):
                    self.assertGreaterEqual(nearest, BRIDGE_CLEARANCE_M)

    def test_a_tree_never_stands_inside_a_props_footprint(self) -> None:
        ((x, y, radius),) = footprints(self.layout)
        over = Wood("round", Disc(x, y, radius + 12.0), count=60, seed=11)
        trees = scatter(self.layout, over)
        self.assertGreater(len(trees), 5, "the wood still grows round the footprint")
        bare = replace(self.layout, decor=replace(self.layout.decor, props=[]))
        self.assertGreater(len(scatter(bare, over)), len(trees), "the footprint is what keeps trees out")
        for tree in trees:
            with self.subTest(tree=tree):
                self.assertGreaterEqual(math.dist((tree.x, tree.y), (x, y)), radius + CANOPY_RADIUS_M[tree.kind] - 1e-9)

    def test_a_prop_never_reaches_the_sidecar(self) -> None:
        with_props = build(self.layout)
        without = build(replace(self.layout, decor=replace(self.layout.decor, props=[])))
        for what in ("centreline", "surfaces", "checkpoints", "item_boxes", "boost_pads", "spawn_grid", "kerbs", "wheel_crossings"):
            with self.subTest(what):
                self.assertEqual(getattr(with_props, what), getattr(without, what))

    def test_the_oval_has_no_props(self) -> None:
        oval = load("oval")
        self.assertEqual(oval.decor.props, [])
        self.assertFalse(any(p.name.startswith("Prop.") for p in build(oval).scenery))


class CircuitWheel(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(DRESSED)
        self.pieces = wheel_pieces()
        self.rim = self.pieces[f"{WHEEL}.rim"]
        self.gondolas = [self.pieces[f"{WHEEL}.gondola.{k}"] for k in range(1, WHEEL_GONDOLAS + 1)]

    def test_the_rim_is_a_node_of_its_own_with_its_origin_at_the_hub(self) -> None:
        (prop,) = self.layout.decor.props
        x, y, heading = locate(self.layout, prop.at)
        self.assertIsNotNone(self.rim.pivot)
        self.assertAlmostEqual(self.rim.pivot.x, x)
        self.assertAlmostEqual(self.rim.pivot.y, y)
        self.assertAlmostEqual(self.rim.pivot.z, -GRASS_DROP_M + WHEEL_HUB_HEIGHT_M)
        self.assertAlmostEqual(self.rim.pivot.heading, heading)
        self.assertIsNone(self.rim.pivot.parent)
        self.assertEqual(set(self.rim.materials), {"wall", "beam-shade"})

    def test_sixteen_gondolas_hang_from_the_rim_each_at_its_own_hanger(self) -> None:
        hub = (self.rim.pivot.x, self.rim.pivot.y, self.rim.pivot.z)
        axle = (-math.sin(self.rim.pivot.heading), math.cos(self.rim.pivot.heading), 0.0)
        hangers = set()
        for gondola in self.gondolas:
            with self.subTest(gondola=gondola.name):
                pivot = gondola.pivot
                self.assertEqual(pivot.parent, self.rim.name)
                self.assertAlmostEqual(pivot.heading, self.rim.pivot.heading)
                from_hub = (pivot.x - hub[0], pivot.y - hub[1], pivot.z - hub[2])
                self.assertAlmostEqual(math.sqrt(sum(c * c for c in from_hub)), WHEEL_RADIUS_M)
                self.assertAlmostEqual(sum(a * b for a, b in zip(from_hub, axle)), 0.0, places=9, msg="in the wheel's plane")
                # Hung below its hanger, in the wheel's plane, never through the ground.
                vertices = gondola.mesh[0]
                self.assertLessEqual(max(v[2] for v in vertices), pivot.z + 1e-9)
                self.assertGreater(min(v[2] for v in vertices), -GRASS_DROP_M)
                hangers.add((round(pivot.x, 3), round(pivot.y, 3), round(pivot.z, 3)))
        self.assertEqual(len(hangers), WHEEL_GONDOLAS)

    def test_the_gondolas_wear_the_crowds_eight_colours_two_apiece(self) -> None:
        worn: dict[str, int] = {}
        for gondola in self.gondolas:
            (colour,) = {m for m in gondola.materials if m.startswith("crowd-")}
            worn[colour] = worn.get(colour, 0) + 1
            self.assertEqual(set(gondola.materials), {colour, "carbon"}, "a body in its colour on a carbon hanger")
        self.assertEqual(worn, {f"crowd-{slot}": 2 for slot in range(len(CROWD_KEYS))})

    def test_the_wheel_is_24_m_across_with_its_hub_14_m_up_and_tops_everything_else(self) -> None:
        ground = -GRASS_DROP_M
        rim_z = [v[2] for v in self.rim.mesh[0]]
        self.assertAlmostEqual((max(rim_z) + min(rim_z)) / 2.0, ground + WHEEL_HUB_HEIGHT_M, places=6)
        self.assertGreaterEqual(max(rim_z), ground + WHEEL_HUB_HEIGHT_M + WHEEL_RADIUS_M)
        self.assertEqual(WHEEL_RADIUS_M * 2.0, 24.0)
        self.assertGreater(max(rim_z), max(GANTRY_TOP_M, GRANDSTAND_CANOPY_BOTTOM_M) + 10.0)
        # The footprint holds the lot, legs and gondolas included.
        ((x, y, radius),) = footprints(self.layout)
        for piece in self.pieces.values():
            for v in piece.mesh[0]:
                self.assertLessEqual(math.dist((v[0], v[1]), (x, y)), radius + 1e-9, piece.name)
        self.assertEqual(radius, WHEEL_FOOTPRINT_M)

    def test_the_legs_and_hub_are_carbon_standing_on_the_grass(self) -> None:
        still = {name: piece for name, piece in self.pieces.items() if piece.pivot is None}
        self.assertEqual(set(still), {"Prop.circuit-wheel.carbon", "Prop.circuit-wheel.carbon-shade"})
        lowest = min(v[2] for piece in still.values() for v in piece.mesh[0])
        self.assertAlmostEqual(lowest, -GRASS_DROP_M, places=6)
        materials = set(re.findall(r'^\s+"([\w-]+)": \(', BUILD_PY.read_text(), re.M)) | {f"crowd-{s}" for s in range(8)}
        used = {m for piece in self.pieces.values() for m in (piece.materials or (piece.material,))}
        self.assertTrue(used <= materials, used - materials)

    def test_the_rim_and_spokes_are_wall_white_where_the_sun_reaches_and_beam_shade_elsewhere(self) -> None:
        faces = faces_by_material(self.rim)
        self.assertTrue(all(faces_up_and_toward_the_sun(f) for f in faces["wall"]))
        self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in faces["beam-shade"]))

    def test_every_rim_and_spoke_face_points_out_of_its_bar(self) -> None:
        """A face wound inward would take the tone of the side it does not show."""
        hub = (self.rim.pivot.x, self.rim.pivot.y, self.rim.pivot.z)
        axle = (-math.sin(self.rim.pivot.heading), math.cos(self.rim.pivot.heading), 0.0)
        along = (math.cos(self.rim.pivot.heading), math.sin(self.rim.pivot.heading), 0.0)
        for face in [[self.rim.mesh[0][v] for v in f] for f in self.rim.mesh[1]]:
            centre = tuple(sum(v[i] for v in face) / len(face) for i in range(3))
            from_hub = tuple(centre[i] - hub[i] for i in range(3))
            b = sum(from_hub[i] * axle[i] for i in range(3))
            # The corner nearest the face's angle round the wheel is the bar it belongs to.
            angle = math.atan2(from_hub[2], sum(from_hub[i] * along[i] for i in range(3)))
            angle = round(angle / (math.tau / WHEEL_GONDOLAS)) * (math.tau / WHEEL_GONDOLAS)
            radial = tuple(math.cos(angle) * along[i] + math.sin(angle) * (0.0, 0.0, 1.0)[i] for i in range(3))
            reach = math.sqrt(sum(c * c for c in from_hub) - b * b)
            if reach < WHEEL_RADIUS_M - 1.0:
                # A spoke face: its bar runs from the hub along `radial`, either side of the plane.
                b_bar = math.copysign(WHEEL_SPOKE_OFFSET_M, b)
                out = sum(from_hub[i] * radial[i] for i in range(3))
                spine = tuple(hub[i] + b_bar * axle[i] + out * radial[i] for i in range(3))
            else:
                # A rim face: its section's middle is on the ring, at the same angle.
                spine = tuple(hub[i] + WHEEL_RADIUS_M * radial[i] for i in range(3))
            u = tuple(face[1][i] - face[0][i] for i in range(3))
            v = tuple(face[2][i] - face[0][i] for i in range(3))
            n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
            away = sum(n[i] * (centre[i] - spine[i]) for i in range(3))
            self.assertGreater(away, 0.0, (face, spine))

    def test_the_wheel_costs_about_420_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        self.assertTrue(400 <= faces <= 440, faces)

    def test_grass_lies_under_the_whole_footprint(self) -> None:
        geometry = build(self.layout)
        grass = [s.mesh for s in geometry.surfaces if s.surface == "grass"]
        grass += [p.mesh for p in geometry.scenery if p.material == "grass" and p.pivot is None]
        ((x, y, radius),) = footprints(self.layout)
        covered = any(
            min(v[0] for v in g[0]) <= x - radius
            and max(v[0] for v in g[0]) >= x + radius
            and min(v[1] for v in g[0]) <= y - radius
            and max(v[1] for v in g[0]) >= y + radius
            for g in grass
        )
        self.assertTrue(covered)

    def test_the_same_layout_builds_the_same_wheel_twice(self) -> None:
        first = {p.name: (p.pivot, p.materials, p.mesh) for p in build(self.layout).scenery if p.name.startswith("Prop.")}
        second = {p.name: (p.pivot, p.materials, p.mesh) for p in build(self.layout).scenery if p.name.startswith("Prop.")}
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()

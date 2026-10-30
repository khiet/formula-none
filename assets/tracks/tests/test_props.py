"""The props a layout declares and the builder stands beyond the walls,
checked without Blender: every footprint keeps its clearance from the walls
and from the other leg at the bridge, no tree stands inside one, the sidecar
never learns of one, the Circuit Wheel is the style guide's card in three
dimensions with the named nodes the View turns, the arch spans Dunlop under
the span rule with its sponsor on both faces, and the pit building faces
the grid with a door and a flag in each key colour.

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
    GANTRY_BEAM_HEIGHT_M,
    GANTRY_TOP_M,
    GRANDSTAND_CANOPY_BOTTOM_M,
    GRASS_DROP_M,
    HOARDING_TEXT_PROUD_M,
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
    ARCH_BELT_RADIUS_M,
    ARCH_DEPTH_M,
    ARCH_RADIUS_M,
    ARCH_RING_M,
    ARCH_TEXT_SIZE_M,
    CANOPY_RADIUS_M,
    PITS_DEPTH_M,
    PITS_DOOR_HEIGHT_M,
    PITS_DOOR_PROUD_M,
    PITS_HEIGHT_M,
    SUN_AZIMUTH,
    WHEEL_FOOTPRINT_M,
    WHEEL_GONDOLAS,
    WHEEL_HUB_HEIGHT_M,
    WHEEL_RADIUS_M,
    WHEEL_SPOKE_OFFSET_M,
    Footprint,
    footprints,
    scatter,
)
from trackgen.sponsors import ARCH_SPONSOR, SPONSORS

BUILD_PY = Path(__file__).resolve().parents[1] / "build.py"
# The only shipped track with props; the oval stays bare.
DRESSED = "brands-patch"
WHEEL = "Prop.circuit-wheel.1"
ARCH = "Prop.arch"
PITS = "Prop.pits"
# How finely a footprint's edge is walked when its clearance is measured.
EDGE_STEP_M = 0.5
# Gates and placements are height-blind in the sim, so a prop near the
# bridge stays this far, in plan, from the other leg's centreline (ADR 0047).
BRIDGE_CLEARANCE_M = 20.0
LEG_REACH_M = 60.0


def wall_clearance(layout, x: float, y: float) -> float:
    """Metres of grass from the point to the outer face of the nearest wall."""
    nearest = min(math.dist((x, y), (s.x, s.y)) for s in layout.path.samples)
    return nearest - layout.runoff_half_width - WALL_THICKNESS_M


def edge_of(footprint: Footprint) -> list[tuple[float, float]]:
    """Points round the footprint's edge, every `EDGE_STEP_M` or so."""
    if footprint.radius:
        count = max(8, math.ceil(math.tau * footprint.radius / EDGE_STEP_M))
        return [
            (footprint.x + footprint.radius * math.cos(math.tau * k / count), footprint.y + footprint.radius * math.sin(math.tau * k / count))
            for k in range(count)
        ]
    cos, sin = math.cos(footprint.heading), math.sin(footprint.heading)
    points = []
    along_steps = max(1, math.ceil(2.0 * footprint.half_along / EDGE_STEP_M))
    across_steps = max(1, math.ceil(2.0 * footprint.half_across / EDGE_STEP_M))
    for k in range(along_steps + 1):
        a = -footprint.half_along + 2.0 * footprint.half_along * k / along_steps
        for b in (-footprint.half_across, footprint.half_across):
            points.append((footprint.x + a * cos - b * sin, footprint.y + a * sin + b * cos))
    for k in range(across_steps + 1):
        b = -footprint.half_across + 2.0 * footprint.half_across * k / across_steps
        for a in (-footprint.half_along, footprint.half_along):
            points.append((footprint.x + a * cos - b * sin, footprint.y + a * sin + b * cos))
    return points


def footprint_clearance(layout, footprint: Footprint) -> float:
    """The least grass between the footprint's edge and a wall."""
    return min(wall_clearance(layout, x, y) for x, y in edge_of(footprint))


def pieces_of_family(family: str):
    """A prop family's scenery on the dressed track, by name."""
    geometry = build(load(DRESSED))
    found = {p.name: p for p in geometry.scenery if p.name.startswith(f"{family}.")}
    if not found:
        raise AssertionError(f"{DRESSED} draws no {family}")
    return found


def across_track(layout, s: float, x: float, y: float) -> tuple[float, float]:
    """A point as (along, left of) the centreline's frame at `s`, in metres."""
    sample = layout.path.at(s % layout.path.length)
    dx, dy = x - sample.x, y - sample.y
    cos, sin = math.cos(sample.heading), math.sin(sample.heading)
    return dx * cos + dy * sin, -dx * sin + dy * cos


def faces_up_and_toward_the_sun(face: list[mesh.Vertex]) -> bool:
    a, b, c = face[0], face[1], face[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    return n[2] >= 0.0 and n[0] * SUN_AZIMUTH[0] + n[1] * SUN_AZIMUTH[1] >= 0.0


def wheel_pieces():
    """The Circuit Wheel's scenery on the dressed track, by name."""
    return pieces_of_family("Prop.circuit-wheel")


def wheel_footprint(layout) -> Footprint:
    (wheel,) = [f for f in footprints(layout) if f.kind == "circuit-wheel"]
    return wheel


def build_materials() -> set[str]:
    """Every material `build.py` can paint: the literal palette and the crowd's keys with their lit tones."""
    literal = set(re.findall(r'^\s+"([\w-]+)": \(', BUILD_PY.read_text(), re.M))
    return literal | {f"crowd-{s}" for s in range(8)} | {f"crowd-{s}-lit" for s in range(8)}


def materials_used(pieces) -> set[str]:
    return {m for piece in pieces for m in (piece.materials or (piece.material,))}


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
        (prop,) = [p for p in self.layout.decor.props if p.kind == "circuit-wheel"]
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

    def test_every_tall_props_footprint_keeps_8_m_of_grass_from_every_wall(self) -> None:
        """A span is held to the span rule instead, in `Arch`: its feet only have to stand on grass."""
        held = [f for f in footprints(self.layout) if f.kind != "arch"]
        self.assertEqual({f.kind for f in held}, {"circuit-wheel", "pits"})
        for footprint in held:
            with self.subTest(footprint=footprint):
                self.assertEqual(footprint.clearance, PROP_WALL_CLEARANCE_M)
                self.assertGreaterEqual(footprint_clearance(self.layout, footprint), PROP_WALL_CLEARANCE_M - 1e-9)

    def test_every_footprint_keeps_the_grass_its_rule_asks_for(self) -> None:
        for footprint in footprints(self.layout):
            with self.subTest(footprint=footprint):
                self.assertGreaterEqual(footprint_clearance(self.layout, footprint), footprint.clearance - 1e-9)

    def test_a_footprint_measures_the_way_out_of_itself(self) -> None:
        disc = Footprint("x", 10.0, 0.0, clearance=0.0, radius=3.0)
        self.assertAlmostEqual(disc.distance(10.0, 5.0), 2.0)
        self.assertAlmostEqual(disc.distance(10.0, 0.0), -3.0)
        self.assertEqual(disc.bounds(), (7.0, -3.0, 13.0, 3.0))
        box = Footprint("x", 0.0, 0.0, clearance=0.0, heading=math.pi / 2.0, half_along=4.0, half_across=1.0)
        # Its length runs along +y, so it reaches 4 m up and 1 m across.
        self.assertAlmostEqual(box.distance(0.0, 6.0), 2.0)
        self.assertAlmostEqual(box.distance(3.0, 0.0), 2.0)
        self.assertAlmostEqual(box.distance(4.0, 7.0), math.hypot(3.0, 3.0))
        self.assertAlmostEqual(box.distance(0.0, 0.0), -1.0)
        min_x, min_y, max_x, max_y = box.bounds()
        self.assertAlmostEqual(min_x, -1.0)
        self.assertAlmostEqual(min_y, -4.0)
        self.assertAlmostEqual(max_x, 1.0)
        self.assertAlmostEqual(max_y, 4.0)

    def test_no_prop_sits_within_20_m_in_plan_of_the_other_leg_at_the_bridge(self) -> None:
        path = self.layout.path
        (bridge,) = self.layout.decor.bridges
        for own, other in ((bridge.deck_s, bridge.underpass_s), (bridge.underpass_s, bridge.deck_s)):
            other_leg = [path.samples[i] for i in path.indices_between(other - LEG_REACH_M, other + LEG_REACH_M)]
            own_leg = [path.samples[i] for i in path.indices_between(own - LEG_REACH_M, own + LEG_REACH_M)]
            for footprint in footprints(self.layout):
                # Only a prop beside this leg could reach the other; every prop stands far from both today.
                if min(footprint.distance(q.x, q.y) for q in own_leg) > PROP_WALL_CLEARANCE_M + BRIDGE_CLEARANCE_M:
                    continue
                nearest = min(footprint.distance(q.x, q.y) for q in other_leg)
                with self.subTest(footprint=footprint, leg=own):
                    self.assertGreaterEqual(nearest, BRIDGE_CLEARANCE_M)

    def test_a_tree_never_stands_inside_a_props_footprint(self) -> None:
        bare = replace(self.layout, decor=replace(self.layout.decor, props=[]))
        canopy = CANOPY_RADIUS_M["round"]
        for footprint in footprints(self.layout):
            with self.subTest(footprint=footprint):
                min_x, min_y, max_x, max_y = footprint.bounds()
                reach = math.dist((min_x, min_y), (max_x, max_y)) / 2.0 + 12.0
                patch = Disc((min_x + max_x) / 2.0, (min_y + max_y) / 2.0, reach)
                # A seed that, with no prop there, would grow a tree into the footprint.
                seed = next(
                    seed
                    for seed in range(11, 60)
                    if any(footprint.distance(t.x, t.y) < canopy for t in scatter(bare, Wood("round", patch, count=80, seed=seed)))
                )
                trees = scatter(self.layout, Wood("round", patch, count=80, seed=seed))
                self.assertGreater(len(trees), 5, "the wood still grows round the footprint")
                for tree in trees:
                    self.assertGreaterEqual(footprint.distance(tree.x, tree.y), canopy - 1e-9, tree)

    def test_a_prop_never_reaches_the_sidecar(self) -> None:
        with_props = build(self.layout)
        without = build(replace(self.layout, decor=replace(self.layout.decor, props=[])))
        for what in ("centreline", "surfaces", "checkpoints", "item_boxes", "boost_pads", "spawn_grid", "kerbs", "wheel_crossings"):
            with self.subTest(what):
                self.assertEqual(getattr(with_props, what), getattr(without, what))
        self.assertGreater(len(with_props.lettering), len(without.lettering), "the arch letters its belt")
        self.assertEqual(
            [t for t in with_props.lettering if not t.name.startswith("Prop.")], without.lettering, "and touches no hoarding"
        )

    def test_props_are_numbered_from_one_among_their_kind(self) -> None:
        names = {p.name for p in build(self.layout).scenery} | {t.name for t in build(self.layout).lettering}
        self.assertIn(f"{WHEEL}.rim", names)
        self.assertIn(f"{ARCH}.1.belt.front", names)
        self.assertFalse(any(name.startswith(f"{ARCH}.2.") for name in names))

    def test_the_pits_refuse_a_layout_that_gives_no_length(self) -> None:
        short = replace(self.layout, decor=replace(self.layout.decor, props=[Prop("pits", Spot(s=0.0, offset=-26.0))]))
        with self.assertRaises(ValueError):
            footprints(short)
        with self.assertRaises(ValueError):
            build(short)

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
        (prop,) = [p for p in self.layout.decor.props if p.kind == "circuit-wheel"]
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
        footprint = wheel_footprint(self.layout)
        for piece in self.pieces.values():
            for v in piece.mesh[0]:
                self.assertLessEqual(footprint.distance(v[0], v[1]), 1e-9, piece.name)
        self.assertEqual(footprint.radius, WHEEL_FOOTPRINT_M)

    def test_the_legs_and_hub_are_carbon_standing_on_the_grass(self) -> None:
        still = {name: piece for name, piece in self.pieces.items() if piece.pivot is None}
        self.assertEqual(set(still), {"Prop.circuit-wheel.carbon", "Prop.circuit-wheel.carbon-shade"})
        lowest = min(v[2] for piece in still.values() for v in piece.mesh[0])
        self.assertAlmostEqual(lowest, -GRASS_DROP_M, places=6)
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())

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

    def test_grass_lies_under_every_footprint(self) -> None:
        geometry = build(self.layout)
        grass = [s.mesh for s in geometry.surfaces if s.surface == "grass"]
        grass += [p.mesh for p in geometry.scenery if p.material == "grass" and p.pivot is None]
        for footprint in footprints(self.layout):
            min_x, min_y, max_x, max_y = footprint.bounds()
            covered = any(
                min(v[0] for v in g[0]) <= min_x
                and max(v[0] for v in g[0]) >= max_x
                and min(v[1] for v in g[0]) <= min_y
                and max(v[1] for v in g[0]) >= max_y
                for g in grass
            )
            self.assertTrue(covered, footprint)

    def test_the_same_layout_builds_the_same_wheel_twice(self) -> None:
        first = {p.name: (p.pivot, p.materials, p.mesh) for p in build(self.layout).scenery if p.name.startswith("Prop.")}
        second = {p.name: (p.pivot, p.materials, p.mesh) for p in build(self.layout).scenery if p.name.startswith("Prop.")}
        self.assertEqual(first, second)


class Arch(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(DRESSED)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "arch"]
        self.pieces = pieces_of_family(ARCH)
        self.geometry = build(self.layout)
        self.lettering = [t for t in self.geometry.lettering if t.name.startswith(f"{ARCH}.")]
        self.x, self.y, self.heading = locate(self.layout, self.prop.at)
        self.ground = -GRASS_DROP_M

    def frame(self, v: mesh.Vertex) -> tuple[float, float, float]:
        """A vertex as (along the track, left of the centre, up from the grass) at the arch."""
        dx, dy = v[0] - self.x, v[1] - self.y
        cos, sin = math.cos(self.heading), math.sin(self.heading)
        return dx * cos + dy * sin, -dx * sin + dy * cos, v[2] - self.ground

    def test_the_arch_spans_dunlop_at_its_middle(self) -> None:
        dunlop = self.layout.path.segment("dunlop")
        self.assertAlmostEqual(self.prop.at.s, dunlop.s_mid)
        self.assertEqual(self.prop.at.offset, 0.0)

    def test_the_ring_is_32_m_wide_and_16_m_to_the_crown_2_m_deep_and_2_5_m_thick(self) -> None:
        ring = [self.frame(v) for name, piece in self.pieces.items() if name.split(".")[2] in ("tyre", "tyre-shade", "tyre-face", "kerb-white") for v in piece.mesh[0]]
        self.assertAlmostEqual(max(b for _, b, _ in ring), ARCH_RADIUS_M)
        self.assertAlmostEqual(min(b for _, b, _ in ring), -ARCH_RADIUS_M)
        self.assertAlmostEqual(max(c for _, _, c in ring), ARCH_RADIUS_M)
        self.assertEqual(ARCH_RADIUS_M * 2.0, 32.0)
        self.assertAlmostEqual(max(a for a, _, _ in ring), ARCH_DEPTH_M / 2.0)
        self.assertAlmostEqual(min(a for a, _, _ in ring), -ARCH_DEPTH_M / 2.0)
        self.assertEqual(ARCH_DEPTH_M, 2.0)
        self.assertEqual(ARCH_RING_M, 2.5)
        radii = sorted({round(math.hypot(b, c), 6) for _, b, c in ring})
        self.assertEqual(radii[0], ARCH_RADIUS_M - ARCH_RING_M)
        self.assertEqual(radii[-1], ARCH_RADIUS_M)
        # Nothing below the grass: the ring is cut at the ground and the abutments hide its feet.
        self.assertGreaterEqual(min(c for _, _, c in ring), -1e-9)

    def test_the_span_rule_holds_the_underside_over_the_track_and_the_feet_on_grass(self) -> None:
        """7 m, the gantry's top, over every point of asphalt; the gantry's beam clears the
        run-off; the feet keep some grass from the wall and never the 8 m a tall prop does."""
        inner = ARCH_RADIUS_M - ARCH_RING_M
        lining = [
            [self.frame(v) for v in face]
            for piece in self.pieces.values()
            for face in faces_by_material(piece).get("tyre", []) + faces_by_material(piece).get("tyre-shade", [])
            if all(abs(math.hypot(self.frame(v)[1], self.frame(v)[2]) - inner) < 1e-6 for v in face)
        ]
        self.assertGreater(len(lining), 8, "the ring's underside")
        asphalt = self.layout.asphalt_half_width
        runoff = self.layout.runoff_half_width
        for face in lining:
            for _, b, c in face:
                if abs(b) <= asphalt:
                    self.assertGreaterEqual(c, GANTRY_TOP_M, (b, c))
                elif abs(b) <= runoff:
                    self.assertGreaterEqual(c, GANTRY_TOP_M - GANTRY_BEAM_HEIGHT_M, (b, c))
        feet = [f for f in footprints(self.layout) if f.kind == "arch"]
        self.assertEqual(len(feet), 2, "a foot either side")
        for foot in feet:
            clearance = footprint_clearance(self.layout, foot)
            self.assertGreater(clearance, 0.0, "on grass")
            self.assertLess(clearance, PROP_WALL_CLEARANCE_M, "a span, not a tall prop beside the track")
            self.assertEqual(foot.clearance, 0.0)
        # The abutments stand on the feet and nowhere else.
        for material in ("wall", "stand-plinth"):
            for v in self.pieces[f"{ARCH}.{material}"].mesh[0]:
                self.assertTrue(any(foot.distance(v[0], v[1]) <= 1e-6 for foot in feet), (material, v))

    def test_the_ring_is_tyre_black_with_a_lit_edge_and_a_kerb_white_belt_on_wall_white_abutments(self) -> None:
        self.assertEqual(
            set(self.pieces),
            {f"{ARCH}.{m}" for m in ("tyre", "tyre-shade", "tyre-face", "kerb-white", "wall", "stand-plinth")},
        )
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())
        for name in ("tyre", "tyre-shade"):
            piece = self.pieces[f"{ARCH}.{name}"]
            faces = [[piece.mesh[0][v] for v in f] for f in piece.mesh[1]]
            if name == "tyre":
                # The tread and the lining take the sun; the flat faces are tyre black either way.
                self.assertTrue(any(faces_up_and_toward_the_sun(f) for f in faces))
            else:
                self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in faces))
        # The belt runs round both flat faces at its radius, between the edge and the lining.
        belt = [self.frame(v) for v in self.pieces[f"{ARCH}.kerb-white"].mesh[0]]
        self.assertEqual({round(abs(a), 6) for a, _, _ in belt}, {ARCH_DEPTH_M / 2.0})
        radii = {round(math.hypot(b, c), 6) for _, b, c in belt}
        self.assertEqual(len(radii), 2)
        self.assertAlmostEqual(sum(radii) / 2.0, ARCH_BELT_RADIUS_M)
        self.assertLess(max(radii), ARCH_RADIUS_M)
        self.assertGreater(min(radii), ARCH_RADIUS_M - ARCH_RING_M)
        edge = [self.frame(v) for v in self.pieces[f"{ARCH}.tyre-face"].mesh[0]]
        self.assertEqual({round(abs(a), 6) for a, _, _ in edge}, {ARCH_DEPTH_M / 2.0})
        self.assertAlmostEqual(max(math.hypot(b, c) for _, b, c in edge), ARCH_RADIUS_M)

    def test_the_sponsor_from_sponsors_py_reads_round_the_belt_on_both_faces(self) -> None:
        self.assertIn(ARCH_SPONSOR, SPONSORS)
        name, ink = ARCH_SPONSOR
        self.assertIn("TYRES", name)
        self.assertEqual(ink, "ink-black")
        self.assertEqual({t.name for t in self.lettering}, {f"{ARCH}.1.belt.front", f"{ARCH}.1.belt.back"})
        facings = set()
        for text in self.lettering:
            with self.subTest(text=text.name):
                self.assertEqual(text.text, name)
                self.assertEqual(text.material, ink)
                self.assertEqual(text.size, ARCH_TEXT_SIZE_M)
                self.assertEqual(text.arc, ARCH_BELT_RADIUS_M)
                # Bent about the ring's centre, just proud of its face, read from the track.
                along, left, up = self.frame((text.at.x, text.at.y, text.at.z))
                self.assertAlmostEqual(left, 0.0)
                self.assertAlmostEqual(up, 0.0)
                self.assertAlmostEqual(abs(along), ARCH_DEPTH_M / 2.0 + HOARDING_TEXT_PROUD_M)
                away = math.cos(text.at.heading - self.heading)
                self.assertAlmostEqual(away, math.copysign(1.0, along), msg="faces away from the ring")
                facings.add(round(along, 3))
        self.assertEqual(len(facings), 2)

    def test_the_arch_costs_about_260_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        self.assertTrue(230 <= faces <= 290, faces)


class PitBuilding(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(DRESSED)
        (self.prop,) = [p for p in self.layout.decor.props if p.kind == "pits"]
        self.pieces = pieces_of_family(PITS)
        self.x, self.y, self.heading = locate(self.layout, self.prop.at)
        self.ground = -GRASS_DROP_M

    def frame(self, v: mesh.Vertex) -> tuple[float, float, float]:
        """A vertex as (along the track from the line, left of the centreline, up from the grass)."""
        along, left = across_track(self.layout, self.prop.at.s, v[0], v[1])
        return along, left, v[2] - self.ground

    def test_the_building_stands_inside_the_start_straight_either_side_of_the_line(self) -> None:
        self.assertEqual(self.prop.at.s, 0.0)
        self.assertEqual(self.prop.at.offset, -26.0)
        self.assertEqual(self.prop.length, 90.0)
        vertices = [self.frame(v) for piece in self.pieces.values() for v in piece.mesh[0]]
        self.assertAlmostEqual(min(a for a, _, _ in vertices), -self.prop.length / 2.0)
        self.assertAlmostEqual(max(a for a, _, _ in vertices), self.prop.length / 2.0)
        # The front face 21 m right of the centreline, the back 10 m further; nothing nearer the track.
        self.assertAlmostEqual(min(b for _, b, _ in vertices), self.prop.at.offset - PITS_DEPTH_M / 2.0)
        self.assertLessEqual(max(b for _, b, _ in vertices), self.prop.at.offset + PITS_DEPTH_M / 2.0 + 0.1)
        self.assertGreaterEqual(min(c for _, _, c in vertices), -1e-9)
        walls = [self.frame(v) for name in ("stand-riser", "stand-plinth", "wall") for v in self.pieces[f"{PITS}.{name}"].mesh[0]]
        self.assertAlmostEqual(max(c for _, _, c in walls), PITS_HEIGHT_M)
        self.assertEqual(PITS_HEIGHT_M, 6.0)
        (footprint,) = [f for f in footprints(self.layout) if f.kind == "pits"]
        self.assertEqual(footprint.clearance, PROP_WALL_CLEARANCE_M)
        # Everything stands on the footprint; only the doors stand a hair proud of it.
        for piece in self.pieces.values():
            for v in piece.mesh[0]:
                self.assertLessEqual(footprint.distance(v[0], v[1]), PITS_DOOR_PROUD_M + 1e-6, piece.name)

    def test_eight_doors_in_the_crowds_order_run_forward_from_the_line(self) -> None:
        """The grid's chase cameras look along the track, so the doors stand ahead of the line."""
        centres = []
        for slot in range(len(CROWD_KEYS)):
            coloured = self.pieces[f"{PITS}.crowd-{slot}"]
            band = self.pieces[f"{PITS}.crowd-{slot}-lit"]
            # The colour's mesh holds a door and a flag; the door is the face that reaches the ground.
            doors = [f for f in coloured.mesh[1] if min(coloured.mesh[0][v][2] for v in f) - self.ground < 1e-6]
            self.assertEqual(len(doors), 1, "one door")
            self.assertEqual(len(band.mesh[1]), 1, "one lit band")
            panel = [self.frame(coloured.mesh[0][v]) for v in doors[0]]
            lit = [self.frame(v) for v in band.mesh[0]]
            # On the front face, from the ground to the door's top, the band along its top edge.
            self.assertEqual({round(b, 3) for _, b, _ in panel + lit}, {round(self.prop.at.offset + PITS_DEPTH_M / 2.0 + 0.05, 3)})
            self.assertAlmostEqual(min(c for _, _, c in panel), 0.0)
            self.assertAlmostEqual(max(c for _, _, c in lit), PITS_DOOR_HEIGHT_M)
            self.assertAlmostEqual(max(c for _, _, c in panel), min(c for _, _, c in lit))
            self.assertEqual({round(a, 3) for a, _, _ in panel}, {round(a, 3) for a, _, _ in lit})
            centres.append(sum(a for a, _, _ in panel) / len(panel))
        self.assertGreater(centres[0], 0.0, "the first door is just past the line")
        self.assertEqual(centres, sorted(centres), "each door further on than the last")
        self.assertLess(centres[7], self.prop.length / 2.0)

    def test_eight_still_flags_in_the_eight_colours_on_carbon_poles_over_the_roof(self) -> None:
        poles = [self.frame(v) for v in self.pieces[f"{PITS}.carbon"].mesh[0]]
        self.assertGreaterEqual(min(c for _, _, c in poles), PITS_HEIGHT_M - 1e-9, "from the roof up")
        self.assertGreater(max(c for _, _, c in poles), PITS_HEIGHT_M + 2.0)
        flags = {}
        for slot in range(len(CROWD_KEYS)):
            piece = self.pieces[f"{PITS}.crowd-{slot}"]
            flag_faces = [f for f in piece.mesh[1] if all(piece.mesh[0][v][2] - self.ground > PITS_HEIGHT_M for v in f)]
            self.assertEqual(len(flag_faces), 1, f"one flag in crowd-{slot}")
            flags[slot] = [self.frame(piece.mesh[0][v]) for v in flag_faces[0]]
        tops = {round(max(c for _, _, c in f), 3) for f in flags.values()}
        self.assertEqual(len(tops), 1, "every flag at the same height")
        # Each hangs from the pole over a door, in the plane along the track.
        for slot, face in flags.items():
            self.assertEqual(len({round(b, 3) for _, b, _ in face}), 1, "flat along the track")
        self.assertEqual(len({round(min(a for a, _, _ in f), 3) for f in flags.values()}), 8, "one per pole")

    def test_the_walls_are_stand_riser_the_roof_lit_and_the_parapet_stand_plinth(self) -> None:
        self.assertEqual(
            set(self.pieces),
            {f"{PITS}.{m}" for m in ("stand-riser", "stand-plinth", "wall", "carbon")}
            | {f"{PITS}.crowd-{s}" for s in range(8)}
            | {f"{PITS}.crowd-{s}-lit" for s in range(8)},
        )
        self.assertTrue(materials_used(self.pieces.values()) <= build_materials())
        roof = [self.frame(v) for v in self.pieces[f"{PITS}.wall"].mesh[0]]
        self.assertEqual({round(c, 6) for _, _, c in roof}, {PITS_HEIGHT_M})
        # Stand plinth is the parapet, the top of every wall, and the shade tone of the walls below it.
        plinth = self.pieces[f"{PITS}.stand-plinth"]
        world = [[plinth.mesh[0][v] for v in f] for f in plinth.mesh[1]]
        parapet = [f for f in world if min(v[2] for v in f) - self.ground > PITS_HEIGHT_M - 1.5]
        self.assertEqual(len(parapet), 4, "the parapet runs round all four walls")
        self.assertEqual({round(max(v[2] for v in f) - self.ground, 6) for f in parapet}, {PITS_HEIGHT_M})
        shaded = [f for f in world if f not in parapet]
        self.assertTrue(shaded, "a wall away from the sun")
        self.assertFalse(any(faces_up_and_toward_the_sun(f) for f in shaded))
        riser = [self.frame(v) for v in self.pieces[f"{PITS}.stand-riser"].mesh[0]]
        self.assertAlmostEqual(min(c for _, _, c in riser), 0.0)
        self.assertAlmostEqual(max(c for _, _, c in riser), min(v[2] for f in parapet for v in f) - self.ground)

    def test_the_doors_carry_no_number(self) -> None:
        """ADR 0035: the doors are colours only, so the building letters nothing."""
        self.assertFalse(any(t.name.startswith(f"{PITS}.") for t in build(self.layout).lettering))

    def test_the_building_costs_about_90_faces(self) -> None:
        faces = sum(len(piece.mesh[1]) for piece in self.pieces.values())
        self.assertTrue(70 <= faces <= 130, faces)


if __name__ == "__main__":
    unittest.main()

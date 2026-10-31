"""The woods a layout declares and the builder scatters, checked without
Blender: every tree keeps its distance from the walls and from every other
tree, each wood scatters the same way twice, and the trees are the style
guide's Woods card in three dimensions.

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
    GRASS_DROP_M,
    TREE_SPACING_M,
    TREE_WALL_CLEARANCE_M,
    WALL_THICKNESS_M,
    build,
)
from trackgen.layouts import load
from trackgen.props import CANOPY_RADIUS_M, SUN_AZIMUTH, pieces_of, scatter, trees

BUILD_PY = Path(__file__).resolve().parents[1] / "build.py"
RACE_VIEW_TS = Path(__file__).resolve().parents[3] / "apps" / "web" / "src" / "race-view.ts"
# The only shipped track with woods; the oval stays bare.
WOODED = "bell-deer"
TREE_MATERIALS = {
    "canopy-pine",
    "canopy-pine-shade",
    "canopy",
    "canopy-shade",
    "blossom",
    "blossom-shade",
    "trunk",
}
# The Woods card's sizes, by kind and whether the tree is young.
HEIGHT_M = {("pine", False): 7.0, ("round", False): 5.0, ("round", True): 3.0, ("cherry", False): 5.0}


def normal(face: list[mesh.Vertex]) -> mesh.Vertex:
    a, b, c = face[0], face[1], face[2]
    u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    length = math.sqrt(sum(c * c for c in n))
    return n[0] / length, n[1] / length, n[2] / length


def faces_of(data: mesh.Mesh) -> list[list[mesh.Vertex]]:
    vertices, faces = data
    return [[vertices[v] for v in face] for face in faces]


def wall_clearance(layout, x: float, y: float) -> float:
    """Metres of grass from the point to the outer face of the nearest wall."""
    nearest = min(math.dist((x, y), (s.x, s.y)) for s in layout.path.samples)
    return nearest - layout.runoff_half_width - WALL_THICKNESS_M


def woods_pieces(name: str) -> dict[str, mesh.Mesh]:
    geometry = build(load(name))
    return {p.material: p.mesh for p in geometry.scenery if p.name.startswith("Woods.")}


def faces_up_and_toward_the_sun(n: mesh.Vertex) -> bool:
    return n[2] >= 0.0 and n[0] * SUN_AZIMUTH[0] + n[1] * SUN_AZIMUTH[1] >= 0.0


class Scatter(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(WOODED)
        self.trees = trees(self.layout)

    def test_the_layout_declares_seven_woods_and_the_builder_fills_each(self) -> None:
        self.assertEqual(len(self.layout.decor.woods), 7)
        for wood in self.layout.decor.woods:
            with self.subTest(wood=wood):
                placed = scatter(self.layout, wood)
                self.assertTrue(placed, "a wood with no trees is a patch in the wrong place")
                self.assertLessEqual(len(placed), wood.count, "the count is a cap")
                self.assertTrue(all(tree.kind == wood.kind for tree in placed))

    def test_every_trees_canopy_keeps_its_clearance_from_every_wall(self) -> None:
        for tree in self.trees:
            with self.subTest(tree=tree):
                clearance = wall_clearance(self.layout, tree.x, tree.y) - CANOPY_RADIUS_M[tree.kind]
                self.assertGreaterEqual(clearance, TREE_WALL_CLEARANCE_M - 1e-9)

    def test_every_tree_keeps_its_spacing_from_every_other(self) -> None:
        for i, a in enumerate(self.trees):
            for b in self.trees[i + 1 :]:
                self.assertGreaterEqual(math.dist((a.x, a.y), (b.x, b.y)), TREE_SPACING_M - 1e-9, (a, b))

    def test_a_wood_keeps_its_spacing_from_the_woods_before_it(self) -> None:
        first = self.layout.decor.woods[0]
        # A second wood over the same ground, with a stream of its own.
        second = replace(first, seed=first.seed + 199)
        overlapping = replace(self.layout, decor=replace(self.layout.decor, woods=[first, second]))
        placed = trees(overlapping)
        self.assertGreater(len(placed), len(scatter(overlapping, first)), "the second wood adds trees")
        for i, a in enumerate(placed):
            for b in placed[i + 1 :]:
                self.assertGreaterEqual(math.dist((a.x, a.y), (b.x, b.y)), TREE_SPACING_M - 1e-9, (a, b))

    def test_a_wood_scatters_to_the_same_trees_twice(self) -> None:
        for wood in self.layout.decor.woods:
            with self.subTest(wood=wood):
                self.assertEqual(scatter(self.layout, wood), scatter(self.layout, wood))
        first = build(self.layout).scenery
        second = build(self.layout).scenery
        self.assertEqual(
            [(p.name, p.mesh) for p in first if p.name.startswith("Woods.")],
            [(p.name, p.mesh) for p in second if p.name.startswith("Woods.")],
        )

    def test_a_different_seed_scatters_different_trees(self) -> None:
        wood = self.layout.decor.woods[0]
        self.assertNotEqual(scatter(self.layout, wood), scatter(self.layout, replace(wood, seed=wood.seed + 1)))

    def test_the_cherry_row_runs_along_its_line(self) -> None:
        (row,) = [wood for wood in self.layout.decor.woods if wood.kind == "cherry"]
        (ax, ay), (bx, by) = row.patch.a, row.patch.b
        length = math.dist((ax, ay), (bx, by))
        along = ((bx - ax) / length, (by - ay) / length)
        for tree in scatter(self.layout, row):
            dx, dy = tree.x - ax, tree.y - ay
            across = abs(-along[1] * dx + along[0] * dy)
            self.assertLess(across, 2.0, "a cherry stands close to the row's line")
            self.assertGreaterEqual(dx * along[0] + dy * along[1], -2.0)
            self.assertLessEqual(dx * along[0] + dy * along[1], length + 2.0)


class Trees(unittest.TestCase):
    def test_the_woods_are_one_mesh_per_tree_material(self) -> None:
        pieces = woods_pieces(WOODED)
        self.assertEqual(set(pieces) - {"grass"}, TREE_MATERIALS)
        materials = set(re.findall(r'^\s+"([\w-]+)": \(', BUILD_PY.read_text(), re.M))
        self.assertTrue(TREE_MATERIALS <= materials, TREE_MATERIALS - materials)

    def test_every_tree_stands_flat_on_grass(self) -> None:
        layout = load(WOODED)
        geometry = build(layout)
        grass = [s.mesh for s in geometry.surfaces if s.surface == "grass"]
        grass += [p.mesh for p in geometry.scenery if p.name == "Woods.grass"]
        self.assertAlmostEqual(min(v[2] for v in woods_pieces(WOODED)["trunk"][0]), -GRASS_DROP_M, places=6)
        for tree in trees(layout):
            with self.subTest(tree=tree):
                reach = CANOPY_RADIUS_M[tree.kind]
                covered = any(
                    min(v[0] for v in g[0]) <= tree.x - reach
                    and max(v[0] for v in g[0]) >= tree.x + reach
                    and min(v[1] for v in g[0]) <= tree.y - reach
                    and max(v[1] for v in g[0]) >= tree.y + reach
                    for g in grass
                )
                self.assertTrue(covered, "the tree and its canopy stand over grass")
                top = max(v[2] for _, piece in pieces_of(tree) for v in piece[0])
                self.assertAlmostEqual(top, -GRASS_DROP_M + HEIGHT_M[tree.kind, tree.young], places=6)

    def test_grass_drawn_under_a_wood_lies_under_the_slab_and_never_in_the_sidecar(self) -> None:
        geometry = build(load(WOODED))
        (slab,) = [s.mesh for s in geometry.surfaces if s.surface == "grass"]
        (beyond,) = [p.mesh for p in geometry.scenery if p.name == "Woods.grass"]
        self.assertLess(max(v[2] for v in beyond[0]), min(v[2] for v in slab[0]))
        self.assertFalse(any(s.name.startswith("Woods") for s in geometry.surfaces))

    def test_each_kind_is_the_cards_shape(self) -> None:
        layout = load(WOODED)
        by_kind: dict[tuple[str, bool], dict[str, int]] = {}
        for tree in trees(layout):
            if (tree.kind, tree.young) in by_kind:
                continue
            faces = by_kind.setdefault((tree.kind, tree.young), {})
            for material, piece in pieces_of(tree):
                faces[material] = faces.get(material, 0) + len(piece[1])
        # A pine: two six-sided cones with their bases, on a trunk of four sides and a top.
        self.assertEqual(sum(by_kind["pine", False].values()), 2 * (6 + 1) + 5)
        # A round tree, full-grown or young: one eight-facet lump, sixteen faces, on the same trunk.
        self.assertEqual(sum(by_kind["round", False].values()), 16 + 5)
        self.assertEqual(sum(by_kind["round", True].values()), 16 + 5)
        # A cherry: three lumps.
        self.assertEqual(sum(by_kind["cherry", False].values()), 3 * 16 + 5)
        for (kind, _), faces in by_kind.items():
            with self.subTest(kind=kind):
                canopy = {"pine": "canopy-pine", "round": "canopy", "cherry": "blossom"}[kind]
                self.assertEqual(set(faces), {canopy, f"{canopy}-shade", "trunk"})

    def test_a_facet_is_lit_when_it_faces_up_and_toward_the_views_sun(self) -> None:
        # The sun the View hangs over the track, in its glTF frame (x, up, -forward).
        match = re.search(r"sun\.position\.set\(([-\d.]+), ([-\d.]+), ([-\d.]+)\);", RACE_VIEW_TS.read_text())
        self.assertIsNotNone(match, "race-view.ts places the sun")
        x, _, z = (float(v) for v in match.groups())
        self.assertAlmostEqual(SUN_AZIMUTH[0], x / math.hypot(x, z), places=9)
        self.assertAlmostEqual(SUN_AZIMUTH[1], -z / math.hypot(x, z), places=9)
        pieces = woods_pieces(WOODED)
        for canopy in ("canopy-pine", "canopy", "blossom"):
            lit = [normal(face) for face in faces_of(pieces[canopy])]
            shade = [normal(face) for face in faces_of(pieces[f"{canopy}-shade"])]
            self.assertTrue(lit and shade, canopy)
            self.assertTrue(all(faces_up_and_toward_the_sun(n) for n in lit), f"{canopy}: every lit facet")
            self.assertFalse(any(faces_up_and_toward_the_sun(n) for n in shade), f"{canopy}: no shaded facet")

    def test_the_stands_keep_their_red_canopy_shade(self) -> None:
        geometry = build(load(WOODED))
        stands = {p.material for p in geometry.scenery if p.name.startswith("Grandstand.")}
        self.assertIn("stand-canopy-shade", stands)
        self.assertNotIn("canopy-shade", stands, "the trees' shade is not the stands'")

    def test_the_oval_has_no_woods(self) -> None:
        self.assertEqual(woods_pieces("oval"), {})
        self.assertEqual(load("oval").decor.woods, [])


if __name__ == "__main__":
    unittest.main()

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
from trackgen.props import CANOPY_RADIUS_M, TREE_HEIGHT_M, pieces_of, scatter

BUILD_PY = Path(__file__).resolve().parents[1] / "build.py"
# The only shipped track with woods; the oval stays bare.
WOODED = "brands-patch"
TREE_MATERIALS = {
    "canopy-pine",
    "canopy-pine-shade",
    "canopy",
    "canopy-shade",
    "blossom",
    "blossom-shade",
    "trunk",
}


def every_tree(layout):
    return [tree for wood in layout.decor.woods for tree in scatter(layout, wood)]


def wall_clearance(layout, x: float, y: float) -> float:
    """Metres of grass from the point to the outer face of the nearest wall."""
    nearest = min(math.dist((x, y), (s.x, s.y)) for s in layout.path.samples)
    return nearest - layout.runoff_half_width - WALL_THICKNESS_M


def woods_pieces(name: str) -> dict[str, mesh.Mesh]:
    geometry = build(load(name))
    return {p.material: p.mesh for p in geometry.scenery if p.name.startswith("Woods.")}


class Scatter(unittest.TestCase):
    def setUp(self) -> None:
        self.layout = load(WOODED)
        self.trees = every_tree(self.layout)

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
        reseeded = type(wood)(wood.kind, wood.patch, wood.count, wood.seed + 1)
        self.assertNotEqual(scatter(self.layout, wood), scatter(self.layout, reseeded))

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
        self.assertEqual(set(pieces), TREE_MATERIALS)
        materials = set(re.findall(r'^\s+"([\w-]+)": \(', BUILD_PY.read_text(), re.M))
        self.assertTrue(TREE_MATERIALS <= materials, TREE_MATERIALS - materials)

    def test_every_tree_stands_flat_on_the_grass(self) -> None:
        layout = load(WOODED)
        pieces = woods_pieces(WOODED)
        self.assertAlmostEqual(min(v[2] for v in pieces["trunk"][0]), -GRASS_DROP_M, places=6)
        for tree in every_tree(layout):
            with self.subTest(tree=tree):
                top = max(v[2] for _, piece in pieces_of(tree) for v in piece[0])
                self.assertAlmostEqual(top, -GRASS_DROP_M + TREE_HEIGHT_M[tree.kind, tree.young], places=6)

    def test_each_kind_is_the_cards_shape(self) -> None:
        layout = load(WOODED)
        by_kind: dict[tuple[str, bool], dict[str, int]] = {}
        for tree in every_tree(layout):
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

    def test_the_oval_has_no_woods(self) -> None:
        self.assertEqual(woods_pieces("oval"), {})
        self.assertEqual(load("oval").decor.woods, [])


if __name__ == "__main__":
    unittest.main()

"""Build the car the View draws from primitives and export it.

    blender --background --python assets/karts/build.py

A chibi low-poly open-wheel F1 car (ADR 0034): short wheelbase, oversized
octagonal wheels, wings, sidepods, a halo and an oversized helmet, every
face flat-shaded in a flat material. It is built in metres to the box the
sim collides with (`KART_HALF_*_M` in crates/sim/src/tuning.rs), wheels on
the floor, front along the sim's -Z, so the View's fit is close to a unit
scale. Four materials, `Primary`, `Secondary`, `Accent` and `Helmet`, are
the livery the View recolours per slot (apps/web/src/kart-model.ts,
ADR 0035); the rest keep the colours set here. The result, `kart.glb`,
carries no texture and is committed; CI has no Blender.
"""

from __future__ import annotations

import math
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
OUTPUT = HERE / "kart.glb"

# The sim's box: 1.2 m wide, 1.8 m long. The helmet may stand above its 0.6 m.
HALF_WIDTH = 0.6
HALF_LENGTH = 0.9

# A neutral grey until the View paints them, so an unpainted car is a visible bug.
UNPAINTED = (0.5, 0.5, 0.5)
LIVERY = ("Primary", "Secondary", "Accent", "Helmet")
FIXED = {
    "Carbon": (0.17, 0.18, 0.2),
    "Tyre": (0.11, 0.11, 0.12),
    "Rim": (0.23, 0.24, 0.26),
    "Visor": (0.11, 0.16, 0.22),
}

WHEEL_SEGMENTS = 8
# Oversized wheels, the rears bigger, on a short wheelbase.
FRONT_WHEEL = {"radius": 0.27, "width": 0.22, "x": HALF_WIDTH - 0.11, "y": 0.52}
REAR_WHEEL = {"radius": 0.3, "width": 0.24, "x": HALF_WIDTH - 0.12, "y": -0.52}


def axle_height(spec: dict[str, float]) -> float:
    """The octagon rests on a flat, so the axle stands its inradius off the ground."""
    return spec["radius"] * math.cos(math.pi / WHEEL_SEGMENTS)


def srgb_to_linear(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def flat_material(name: str, srgb: tuple[float, float, float]) -> bpy.types.Material:
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*(srgb_to_linear(c) for c in srgb), 1.0)
    bsdf.inputs["Roughness"].default_value = 0.8
    mat.diffuse_color = (*srgb, 1.0)
    return mat


class Part:
    """One mesh under construction: primitives dropped into a bmesh, each face in a named material."""

    def __init__(self, materials: list[str]) -> None:
        self.bm = bmesh.new()
        self.materials = materials

    def _paint(self, verts: list[bmesh.types.BMVert], material: str) -> list[bmesh.types.BMVert]:
        index = self.materials.index(material)
        for face in {face for vert in verts for face in vert.link_faces}:
            face.material_index = index
        return verts

    def box(
        self,
        material: str,
        centre: tuple[float, float, float],
        size: tuple[float, float, float],
        front_scale: tuple[float, float] = (1.0, 1.0),
        front_drop: float = 0.0,
    ) -> None:
        """A box, its front end (+Y) scaled in width and height by `front_scale` and lowered by `front_drop`."""
        verts = bmesh.ops.create_cube(self.bm, size=1.0)["verts"]
        for vert in verts:
            vert.co.x *= size[0]
            vert.co.y *= size[1]
            vert.co.z *= size[2]
            if vert.co.y > 0:
                vert.co.x *= front_scale[0]
                vert.co.z = vert.co.z * front_scale[1] - front_drop
            vert.co += Vector(centre)
        self._paint(verts, material)

    def cylinder(
        self, material: str, centre: tuple[float, float, float], axis: Vector, radius: float, depth: float, segments: int
    ) -> None:
        turn = Vector((0, 0, 1)).rotation_difference(axis.normalized()).to_matrix().to_4x4()
        verts = bmesh.ops.create_cone(
            self.bm,
            cap_ends=True,
            segments=segments,
            radius1=radius,
            radius2=radius,
            depth=depth,
            # A flat on the ground, not a corner: the octagon rests on its lowest face.
            matrix=Matrix.Translation(centre) @ turn @ Matrix.Rotation(math.pi / segments, 4, "Z"),
        )["verts"]
        self._paint(verts, material)

    def bar(self, material: str, a: tuple[float, float, float], b: tuple[float, float, float], radius: float) -> None:
        start, end = Vector(a), Vector(b)
        self.cylinder(material, tuple((start + end) / 2), end - start, radius, (end - start).length, 6)

    def ball(self, material: str, centre: tuple[float, float, float], radius: float) -> None:
        # Octagonal like the wheels, so the helmet is a polygon, not a smooth dome.
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=8, v_segments=6, radius=radius)["verts"]
        for vert in verts:
            vert.co += Vector(centre)
        self._paint(verts, material)

    def finish(self, name: str) -> bpy.types.Object:
        mesh = bpy.data.meshes.new(name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for material in self.materials:
            mesh.materials.append(bpy.data.materials[material])
        # Faceted: every face lit as one plane, the low-poly look.
        for poly in mesh.polygons:
            poly.use_smooth = False
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        return obj


def body() -> bpy.types.Object:
    # Blender is Z up with the nose along +Y; the glTF exporter turns that
    # into Y up with the nose along -Z, the way the sim's kart drives.
    part = Part([*LIVERY, "Carbon", "Visor"])
    # The floor every panel sits on.
    part.box("Secondary", (0, -0.2, 0.1), (0.76, 1.1, 0.06))
    # The tub, the cockpit's sides, and the nose tapering down to the front wing.
    part.box("Primary", (0, -0.04, 0.32), (0.42, 0.66, 0.4))
    part.box("Primary", (0, 0.52, 0.27), (0.26, 0.46, 0.26), front_scale=(0.55, 0.55), front_drop=0.07)
    part.box("Accent", (0, 0.81, 0.14), (0.13, 0.1, 0.09), front_scale=(0.7, 0.8))
    # Sidepods, undercut at the front, tapering into the engine cover.
    for side in (-1, 1):
        part.box("Secondary", (side * 0.29, -0.18, 0.28), (0.2, 0.52, 0.3), front_scale=(0.9, 0.75), front_drop=0.03)
    # Engine cover running back from the airbox to the rear wing, with its fin.
    part.box("Primary", (0, -0.52, 0.34), (0.32, 0.44, 0.42), front_scale=(1.15, 1.1))
    part.box("Accent", (0, -0.6, 0.6), (0.03, 0.34, 0.16), front_scale=(1.0, 0.5))
    part.box("Secondary", (0, -0.36, 0.62), (0.18, 0.14, 0.16))
    # Front wing: a low plane the full width between the endplates.
    part.box("Secondary", (0, 0.82, 0.07), (1.1, 0.14, 0.04))
    part.box("Primary", (0, 0.77, 0.12), (1.0, 0.06, 0.04))
    for side in (-1, 1):
        part.box("Accent", (side * 0.56, 0.82, 0.11), (0.03, 0.16, 0.13))
    # Rear wing on its pillar, with tall endplates.
    part.box("Primary", (0, -0.82, 0.8), (0.82, 0.14, 0.06))
    part.box("Accent", (0, -0.86, 0.87), (0.82, 0.08, 0.04))
    part.box("Carbon", (0, -0.78, 0.6), (0.05, 0.06, 0.36))
    for side in (-1, 1):
        part.box("Accent", (side * 0.42, -0.83, 0.73), (0.03, 0.18, 0.34))
    # Suspension wishbones from the tub to each upright.
    for wheel in (FRONT_WHEEL, REAR_WHEEL):
        for side in (-1, 1):
            hub = (side * (wheel["x"] - wheel["width"] / 2), wheel["y"], axle_height(wheel))
            for inner_y in (-0.06, 0.06):
                part.bar("Carbon", (side * 0.17, wheel["y"] + inner_y, axle_height(wheel) - 0.03), hub, 0.014)
    # The driver, chibi: shoulders in the team's colour proud of the cockpit
    # and an oversized helmet with its visor, the car's highest point.
    part.box("Primary", (0, -0.12, 0.62), (0.46, 0.24, 0.2), front_scale=(0.85, 1.0))
    helmet = (0, -0.1, 1.0)
    radius = 0.34
    part.ball("Helmet", helmet, radius)
    part.box("Visor", (0, helmet[1] + 0.27, helmet[2] + 0.02), (0.46, 0.14, 0.13), front_scale=(0.8, 1.0))
    # The halo: a central pillar up from the nose, a hoop round the front of
    # the helmet below its widest, and a leg down to the tub at each side.
    hoop_z = helmet[2] - 0.14
    hoop_x, hoop_y = radius + 0.03, radius + 0.08
    pillar = (0, helmet[1] + hoop_y, hoop_z)
    part.bar("Carbon", (0, helmet[1] + hoop_y + 0.08, 0.52), pillar, 0.025)
    left = [
        (hoop_x * math.cos(math.pi / 2 + k * math.pi / 8), helmet[1] + hoop_y * math.sin(math.pi / 2 + k * math.pi / 8), hoop_z)
        for k in range(5)
    ]
    hoop = [*reversed([(-x, hy, z) for (x, hy, z) in left[1:]]), *left]
    for a, b in zip(hoop, hoop[1:]):
        part.bar("Carbon", a, b, 0.022)
    for end in (hoop[0], hoop[-1]):
        part.bar("Carbon", end, (end[0] * 0.75, end[1] - 0.08, 0.52), 0.022)
    return part.finish("kart")


def wheel(name: str, spec: dict[str, float], side: int, parent: bpy.types.Object) -> None:
    """A wheel built about its axle and placed there, unrotated, for the tells to find by name."""
    part = Part(["Tyre", "Rim"])
    axis = Vector((1, 0, 0))
    part.cylinder("Tyre", (0, 0, 0), axis, spec["radius"], spec["width"], WHEEL_SEGMENTS)
    # The rim, a smaller octagon proud of the tyre's outer wall.
    part.cylinder("Rim", (side * spec["width"] / 2, 0, 0), axis, spec["radius"] * 0.58, 0.02, WHEEL_SEGMENTS)
    obj = part.finish(name)
    obj.location = (side * spec["x"], spec["y"], axle_height(spec))
    obj.parent = parent


def build() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for name in LIVERY:
        flat_material(name, UNPAINTED)
    for name, srgb in FIXED.items():
        flat_material(name, srgb)
    root = body()
    for corner, spec, side in (
        ("front-left", FRONT_WHEEL, -1),
        ("front-right", FRONT_WHEEL, 1),
        ("back-left", REAR_WHEEL, -1),
        ("back-right", REAR_WHEEL, 1),
    ):
        wheel(f"wheel-{corner}", spec, side, root)
    bpy.ops.export_scene.gltf(
        filepath=str(OUTPUT),
        export_format="GLB",
        export_yup=True,
        export_texcoords=False,
        export_tangents=False,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
        export_animations=False,
    )
    print(f"exported {OUTPUT}: {[m.name for m in bpy.data.materials]}")


if __name__ == "__main__":
    build()

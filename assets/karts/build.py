"""Build the kart the View draws from Kenney's CC0 kart and export it.

    blender --background --python assets/karts/build.py

Kenney's Car Kit paints every model from one texture atlas, `colormap.png`,
a sheet of shaded colour strips each face samples a point of. The View
needs one swappable paint instead, so this flattens the atlas: every face
is sampled at its centre, faces of a colour family share a flat material in
that family's mean colour, and the body's and the driver's dominant
families become one material named `Livery`, which the View recolours per
slot (apps/web/src/kart-model.ts). The result, `kart.glb`, carries no
texture and is committed; CI has no Blender.
"""

from __future__ import annotations

import colorsys
import math
from collections import defaultdict
from pathlib import Path

import bpy
from mathutils import Matrix

HERE = Path(__file__).resolve().parent
SOURCE = HERE / "kenney" / "kart-oobi.glb"
OUTPUT = HERE / "kart.glb"
LIVERY_MATERIAL = "Livery"
# The body and the driver's suit are the paint; the rest keeps Kenney's colours.
PAINTED_OBJECTS = ("kart-oobi", "character")
# Below this saturation a sample is a grey, whatever its hue says.
GREY_SATURATION = 0.2
HUE_BUCKETS = 12
LIGHTNESS_BUCKETS = 4

Colour = tuple[float, float, float]
Family = tuple[str | int, float]


def family_of(rgb: Colour) -> Family:
    """The colour family a sample belongs to: a hue bucket, or grey, by lightness quarter."""
    hue, lightness, saturation = colorsys.rgb_to_hls(*rgb)
    tone = "grey" if saturation < GREY_SATURATION else int(hue * HUE_BUCKETS) * (360 // HUE_BUCKETS)
    return (tone, round(lightness * LIGHTNESS_BUCKETS) / LIGHTNESS_BUCKETS)


def srgb_to_linear(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


class Atlas:
    def __init__(self, image: bpy.types.Image) -> None:
        self.width, self.height = image.size
        # Blender hands a byte image's pixels back as the sRGB values it stores.
        self.pixels = image.pixels[:]

    def sample(self, u: float, v: float) -> Colour:
        x = min(max(int(u * self.width), 0), self.width - 1)
        y = min(max(int(v * self.height), 0), self.height - 1)
        i = (y * self.width + x) * 4
        return tuple(self.pixels[i : i + 3])


def flat_material(name: str, srgb: Colour) -> bpy.types.Material:
    existing = bpy.data.materials.get(name)
    if existing is not None:
        return existing
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*(srgb_to_linear(c) for c in srgb), 1.0)
    bsdf.inputs["Roughness"].default_value = 0.8
    mat.diffuse_color = (*srgb, 1.0)
    return mat


def flatten(obj: bpy.types.Object, atlas: Atlas, painted: bool) -> None:
    """Replace the object's atlas material with one flat material per colour family."""
    mesh = obj.data
    uv = mesh.uv_layers.active.data
    faces_by_family: dict[Family, list[int]] = defaultdict(list)
    samples: dict[Family, list[Colour]] = defaultdict(list)
    for poly in mesh.polygons:
        corners = [uv[loop].uv for loop in poly.loop_indices]
        colour = atlas.sample(
            sum(c.x for c in corners) / len(corners), sum(c.y for c in corners) / len(corners)
        )
        family = family_of(colour)
        faces_by_family[family].append(poly.index)
        samples[family].append(colour)

    dominant = max(faces_by_family, key=lambda family: len(faces_by_family[family]))
    mesh.materials.clear()
    for family, faces in faces_by_family.items():
        mean = tuple(sum(c[i] for c in samples[family]) / len(faces) for i in range(3))
        if painted and family == dominant:
            name = LIVERY_MATERIAL
            # A neutral grey until the View paints it, so an unpainted kart is a visible bug, not a Kenney blue.
            mean = (0.5, 0.5, 0.5)
        else:
            tone, lightness = family
            name = f"Kenney {tone} {lightness:.2f}"
        mesh.materials.append(flat_material(name, mean))
        slot = len(mesh.materials) - 1
        for face in faces:
            mesh.polygons[face].material_index = slot
    mesh.uv_layers.remove(mesh.uv_layers.active)


def build() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(SOURCE))
    atlas = Atlas(bpy.data.images["colormap"])
    for obj in bpy.data.objects:
        if obj.type == "MESH":
            flatten(obj, atlas, painted=obj.name in PAINTED_OBJECTS)
    root = bpy.data.objects[PAINTED_OBJECTS[0]]
    root.name = "kart"
    # Kenney's kart drives along glTF +Z; the sim's drives along -Z. Turn the
    # meshes and the wheel positions about, so the file needs no transform.
    about_face = Matrix.Rotation(math.pi, 4, "Z")
    for obj in bpy.data.objects:
        if obj.type == "MESH":
            obj.data.transform(about_face)
            obj.data.update()
        if obj.parent is not None:
            obj.location = about_face @ obj.location
    # Nothing references the atlas now; dropping it keeps it out of the export.
    bpy.data.materials.remove(bpy.data.materials["colormap"])
    bpy.data.images.remove(bpy.data.images["colormap"])
    bpy.ops.export_scene.gltf(
        filepath=str(OUTPUT),
        export_format="GLB",
        export_apply=True,
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

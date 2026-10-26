"""Build a track in Blender from its layout and export it.

    blender --background --python assets/tracks/build.py -- <name>...

`<name>` is a track in `trackgen/layouts` or `all`. Each track is written
to `assets/tracks/<name>/` as `<name>.glb`, `<name>.track.json` and, for
inspection in Blender, `<name>.blend` (ignored by git).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import bpy

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import export  # noqa: E402
from trackgen.layout import CROWD_KEYS, Placement, TextProp, TrackGeometry, build  # noqa: E402
from trackgen.layouts import NAMES, load  # noqa: E402

# Flat colours in the 16-bit kart racer's key: one saturated tone per
# surface, red-and-white kerbs, white boards with bold lettering. sRGB, as
# picked; converted to linear for the shader.
MATERIALS = {
    "asphalt": (0.42, 0.42, 0.47),
    "sand": (0.91, 0.78, 0.45),
    "grass": (0.36, 0.69, 0.29),
    "wall": (0.94, 0.94, 0.94),
    "pipe": (0.25, 0.64, 0.30),
    "kerb-red": (0.84, 0.23, 0.18),
    "kerb-white": (0.96, 0.96, 0.96),
    "checker-dark": (0.13, 0.13, 0.13),
    "checker-light": (0.96, 0.96, 0.96),
    "board": (0.98, 0.98, 0.98),
    # Carbon like the Instrument panel's plates, for the boost pad, the
    # gantry's pillars and the stands' posts; the pad's chevrons in the
    # mini-turbo orange of `sparks.ts`.
    "carbon": (0.169, 0.180, 0.200),
    "carbon-lit": (0.227, 0.243, 0.271),
    "carbon-shade": (0.110, 0.110, 0.118),
    "chevron": (1.0, 0.533, 0.0),
    "chevron-shade": (0.769, 0.373, 0.0),
    # The asset catalog's Grandstand and Start gantry cards.
    "stand-riser": (0.843, 0.859, 0.878),
    "stand-plinth": (0.706, 0.729, 0.761),
    "canopy-shade": (0.647, 0.165, 0.125),
    "beam-top": (0.788, 0.808, 0.835),
    "beam-shade": (0.604, 0.627, 0.659),
    "ink-blue": (0.12, 0.33, 0.78),
    "ink-green": (0.13, 0.55, 0.25),
    "ink-red": (0.80, 0.16, 0.16),
    "ink-black": (0.12, 0.12, 0.12),
    "ink-brown": (0.45, 0.28, 0.14),
    "ink-orange": (0.93, 0.47, 0.08),
    "ink-purple": (0.50, 0.22, 0.70),
}
# The crowd in the stands, one material per livery's key colour.
MATERIALS.update(
    {
        f"crowd-{slot}": tuple(((key >> shift) & 0xFF) / 255.0 for shift in (16, 8, 0))
        for slot, key in enumerate(CROWD_KEYS)
    }
)
# Deep enough for the lettering to read as a letter from the track and no
# deeper: every millimetre is more faces on every board.
TEXT_EXTRUDE_M = 0.03


def reset_scene() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)


def srgb_to_linear(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name: str) -> bpy.types.Material:
    existing = bpy.data.materials.get(name)
    if existing is not None:
        return existing
    srgb = MATERIALS[name]
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*(srgb_to_linear(c) for c in srgb), 1.0)
    bsdf.inputs["Roughness"].default_value = 0.9
    mat.diffuse_color = (*srgb, 1.0)
    # The glTF exporter writes the material double-sided when culling is off,
    # so a ribbon reads from either side.
    mat.use_backface_culling = False
    return mat


def collection(name: str) -> bpy.types.Collection:
    col = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(col)
    return col


def add_empty(col: bpy.types.Collection, name: str, placement: Placement) -> bpy.types.Object:
    obj = bpy.data.objects.new(name, None)
    obj.empty_display_type = "SINGLE_ARROW"
    obj.location = (placement.x, placement.y, placement.z)
    # An empty faces along its local +Y, which the exporter reads as yaw.
    obj.rotation_euler = (0.0, 0.0, placement.heading - math.pi / 2.0)
    if placement.half_width:
        obj["half_width"] = placement.half_width
    col.objects.link(obj)
    return obj


def mesh_object(
    name: str,
    data: tuple[list, list],
    material_name: str,
    uvs: list[tuple[float, float]] | None = None,
) -> bpy.types.Object:
    vertices, faces = data
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(vertices, [], faces)
    mesh.validate()
    mesh.update()
    if uvs is not None:
        layer = mesh.uv_layers.new(name="UVMap")
        for loop in mesh.loops:
            layer.data[loop.index].uv = uvs[loop.vertex_index]
    mesh.materials.append(material(material_name))
    return bpy.data.objects.new(name, mesh)


def yaw_facing(facing: float) -> float:
    """The Z rotation that turns something facing Blender -Y to face `facing` (radians from +X)."""
    return math.atan2(math.cos(facing), -math.sin(facing))


def text_object(prop: TextProp) -> bpy.types.Object:
    """Lettering standing upright, read from the way its placement faces, centred on it."""
    curve = bpy.data.curves.new(prop.name, "FONT")
    curve.body = prop.text
    curve.size = prop.size
    curve.extrude = TEXT_EXTRUDE_M
    curve.align_x = "CENTER"
    curve.align_y = "CENTER"
    curve.resolution_u = 2
    curve.materials.append(material(prop.material))
    obj = bpy.data.objects.new(prop.name, curve)
    obj.location = (prop.at.x, prop.at.y, prop.at.z)
    # Text lies in its local XY plane reading along +X; stood up on its
    # bottom edge it faces -Y.
    obj.rotation_euler = (math.pi / 2.0, 0.0, yaw_facing(prop.at.heading))
    return obj


def populate(geometry: TrackGeometry) -> None:
    track = collection(export.TRACK_COLLECTION)
    for surface in geometry.surfaces:
        obj = mesh_object(surface.name, surface.mesh, surface.material, surface.uvs)
        obj["surface"] = surface.surface
        track.objects.link(obj)

    scenery = collection(export.SCENERY_COLLECTION)
    for piece in geometry.scenery:
        scenery.objects.link(mesh_object(piece.name, piece.mesh, piece.material))
    for prop in geometry.props:
        scenery.objects.link(text_object(prop))

    curve = bpy.data.curves.new(export.CENTRELINE_OBJECT, "CURVE")
    curve.dimensions = "3D"
    spline = curve.splines.new("POLY")
    spline.points.add(len(geometry.centreline) - 1)
    for point, (x, y, z) in zip(spline.points, geometry.centreline):
        point.co = (x, y, z, 1.0)
    spline.use_cyclic_u = True
    track_data = collection("Data")
    track_data.objects.link(bpy.data.objects.new(export.CENTRELINE_OBJECT, curve))

    groups = {
        "Checkpoints": ("Checkpoint", geometry.checkpoints),
        "ItemBoxes": ("ItemBox", geometry.item_boxes),
        "BoostPads": ("BoostPad", geometry.boost_pads),
        "SpawnGrid": ("Spawn", geometry.spawn_grid),
    }
    for collection_name, (prefix, items) in groups.items():
        col = collection(collection_name)
        for number, placement in enumerate(items):
            add_empty(col, f"{prefix}.{number:02d}", placement)


def build_track(name: str) -> None:
    reset_scene()
    populate(build(load(name)))
    out_dir = HERE / name
    export.export_scene(name, out_dir)
    bpy.ops.wm.save_as_mainfile(filepath=str(out_dir / f"{name}.blend"))
    print(f"exported {name} to {out_dir}")


def main(argv: list[str]) -> None:
    if not argv:
        raise SystemExit("usage: blender --background --python build.py -- <name>... | all")
    names = NAMES if argv == ["all"] else argv
    for name in names:
        build_track(name)


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])

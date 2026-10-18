"""Build a greybox track in Blender from its layout and export it.

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
from trackgen.layout import Placement, TrackGeometry, build  # noqa: E402
from trackgen.layouts import NAMES, load  # noqa: E402

# Placeholder colours only (ADR 0007): one flat tone per surface so the
# greybox reads at a glance.
MATERIALS = {
    "asphalt": (0.23, 0.23, 0.25, 1.0),
    "sand": (0.85, 0.76, 0.48, 1.0),
    "grass": (0.29, 0.48, 0.23, 1.0),
    "wall": (0.77, 0.23, 0.23, 1.0),
    "pipe": (0.18, 0.62, 0.29, 1.0),
}


def reset_scene() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name: str) -> bpy.types.Material:
    existing = bpy.data.materials.get(name)
    if existing is not None:
        return existing
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = MATERIALS[name]
    mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.9
    mat.diffuse_color = MATERIALS[name]
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


def populate(geometry: TrackGeometry) -> None:
    track = collection(export.TRACK_COLLECTION)
    for surface in geometry.surfaces:
        vertices, faces = surface.mesh
        mesh = bpy.data.meshes.new(surface.name)
        mesh.from_pydata(vertices, [], faces)
        mesh.validate()
        mesh.update()
        obj = bpy.data.objects.new(surface.name, mesh)
        obj["surface"] = surface.surface
        mesh.materials.append(material(surface.material))
        track.objects.link(obj)

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

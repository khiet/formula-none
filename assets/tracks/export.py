"""Export the open Blender scene as glTF plus the track sidecar the sim reads.

Run on a saved track:

    blender <track>.blend --background --python assets/tracks/export.py -- <name> <out-dir>

`build.py` calls `export_scene` on the scenes it generates. Either way the
scene follows these conventions:

- Collection `Track`: mesh objects with a `surface` custom property, one of
  asphalt, sand, grass, wall or tyres. These go into the glTF and, triangulated,
  into the sidecar as collision geometry.
- Collection `Scenery`, optional: anything drawn and never collided with,
  such as kerbs, hoardings and stands. It goes into the glTF only, so a
  change to it leaves the sidecar, and the sim, untouched.
- A curve object named `Centreline`, one closed POLY or BEZIER spline, run
  in the direction of travel starting at the start line.
- Collections `Checkpoints`, `ItemBoxes`, `BoostPads`, `Coins`, `Kerbs`,
  `WheelCrossings` and `SpawnGrid` hold empties ordered by name. An empty
  faces along its local +Y; checkpoints and wheel crossings carry a
  `half_width` custom property, and wheel crossings a `hazard_half_depth_m`.

Blender is Z up; the sidecar is written in the glTF frame (Y up, -Z
forward) so it matches the mesh.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.geometry import interpolate_bezier

SIDECAR_VERSION = 1
SURFACE_TAGS = ("asphalt", "sand", "grass", "wall", "tyres")
TRACK_COLLECTION = "Track"
SCENERY_COLLECTION = "Scenery"
CENTRELINE_OBJECT = "Centreline"
PLACEMENT_COLLECTIONS = {
    "checkpoints": "Checkpoints",
    "item_boxes": "ItemBoxes",
    "boost_pads": "BoostPads",
    "coins": "Coins",
    "kerbs": "Kerbs",
    "wheel_crossings": "WheelCrossings",
    "spawn_grid": "SpawnGrid",
}
# The placements that reach to either side of their centre.
HALF_WIDTH_COLLECTIONS = {"checkpoints", "wheel_crossings"}
# The placements whose hazard zone reaches along the track either side of them.
HAZARD_COLLECTIONS = {"wheel_crossings"}
# Millimetres are plenty for a greybox and keep the sidecar diffable.
DECIMALS = 3


def tidy(value: float, decimals: int = DECIMALS) -> float:
    """Round, and fold -0.0 into 0.0 so the sidecar does not churn on sign noise."""
    rounded = round(value, decimals)
    return 0.0 if rounded == 0.0 else rounded


def to_gltf(v: Vector) -> list[float]:
    return [tidy(v.x), tidy(v.z), tidy(-v.y)]


def yaw_of(obj: bpy.types.Object) -> float:
    """Rotation about up, counter-clockwise seen from above, zero facing glTF -Z (Blender +Y)."""
    z = obj.matrix_world.to_euler("XYZ").z
    return tidy((z + math.pi) % math.tau - math.pi, 6)


def objects_in(collection_name: str) -> list[bpy.types.Object]:
    collection = bpy.data.collections.get(collection_name)
    if collection is None:
        raise RuntimeError(f"the scene has no collection named {collection_name!r}")
    return sorted(collection.all_objects, key=lambda obj: obj.name)


def surface_meshes(depsgraph: bpy.types.Depsgraph) -> list[dict]:
    surfaces = []
    for obj in objects_in(TRACK_COLLECTION):
        if obj.type != "MESH":
            continue
        tag = obj.get("surface")
        if tag not in SURFACE_TAGS:
            raise RuntimeError(f"{obj.name}: surface must be one of {SURFACE_TAGS}, got {tag!r}")
        evaluated = obj.evaluated_get(depsgraph)
        mesh = evaluated.to_mesh()
        try:
            matrix = evaluated.matrix_world
            positions = [coord for v in mesh.vertices for coord in to_gltf(matrix @ v.co)]
            indices = [index for tri in mesh.loop_triangles for index in tri.vertices]
        finally:
            evaluated.to_mesh_clear()
        surfaces.append(
            {"name": obj.name, "surface": tag, "positions": positions, "indices": indices}
        )
    return surfaces


def centreline() -> list[list[float]]:
    obj = bpy.data.objects.get(CENTRELINE_OBJECT)
    if obj is None or obj.type != "CURVE":
        raise RuntimeError(f"the scene needs a curve object named {CENTRELINE_OBJECT!r}")
    spline = obj.data.splines[0]
    if not spline.use_cyclic_u:
        raise RuntimeError("the centreline must be a closed (cyclic) spline")
    if spline.type == "POLY":
        points = [Vector(p.co[:3]) for p in spline.points]
    elif spline.type == "BEZIER":
        knots = spline.bezier_points
        points = []
        for a, b in zip(knots, knots[1:] + knots[:1]):
            segment = interpolate_bezier(a.co, a.handle_right, b.handle_left, b.co, spline.resolution_u + 1)
            points.extend(segment[:-1])
    else:
        raise RuntimeError(f"unsupported centreline spline type {spline.type}")
    return [to_gltf(obj.matrix_world @ p) for p in points]


def placements(collection_name: str, with_half_width: bool, with_hazard: bool) -> list[dict]:
    result = []
    for obj in objects_in(collection_name):
        placement = {"position": to_gltf(obj.matrix_world.translation), "yaw": yaw_of(obj)}
        if with_half_width:
            half_width = obj.get("half_width")
            if half_width is None:
                raise RuntimeError(f"{obj.name}: needs a half_width custom property")
            placement["half_width"] = tidy(float(half_width))
        if with_hazard:
            hazard_half_depth = obj.get("hazard_half_depth_m")
            if hazard_half_depth is None:
                raise RuntimeError(f"{obj.name}: needs a hazard_half_depth_m custom property")
            placement["hazard_half_depth_m"] = tidy(float(hazard_half_depth))
        result.append(placement)
    return result


def sidecar(name: str) -> dict:
    depsgraph = bpy.context.evaluated_depsgraph_get()
    data = {"version": SIDECAR_VERSION, "name": name, "centreline": centreline()}
    for key, collection_name in PLACEMENT_COLLECTIONS.items():
        data[key] = placements(
            collection_name,
            with_half_width=key in HALF_WIDTH_COLLECTIONS,
            with_hazard=key in HAZARD_COLLECTIONS,
        )
    data["surfaces"] = surface_meshes(depsgraph)
    return data


def write_sidecar(data: dict, path: Path) -> None:
    """One top-level key per line, and one surface per line, so diffs stay readable."""
    compact = {"separators": (",", ":")}
    lines = ["{"]
    keys = list(data)
    for i, key in enumerate(keys):
        comma = "," if i < len(keys) - 1 else ""
        if key == "surfaces":
            lines.append(f'"{key}":[')
            for j, surface in enumerate(data[key]):
                lines.append(json.dumps(surface, **compact) + ("," if j < len(data[key]) - 1 else ""))
            lines.append(f"]{comma}")
        else:
            lines.append(f'"{key}":{json.dumps(data[key], **compact)}{comma}')
    lines.append("}")
    path.write_text("\n".join(lines) + "\n")


def export_glb(path: Path) -> None:
    """The `Track` and `Scenery` collections go into the glTF; the data collections stay in the sidecar."""
    bpy.context.view_layer.update()
    for obj in bpy.context.scene.objects:
        obj.select_set(False)
    for name in (TRACK_COLLECTION, SCENERY_COLLECTION):
        collection = bpy.data.collections.get(name)
        for obj in collection.all_objects if collection is not None else ():
            obj.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
        export_animations=False,
    )


def export_scene(name: str, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    export_glb(out_dir / f"{name}.glb")
    write_sidecar(sidecar(name), out_dir / f"{name}.track.json")


def main(argv: list[str]) -> None:
    if len(argv) != 2:
        raise SystemExit("usage: blender <track>.blend --background --python export.py -- <name> <out-dir>")
    export_scene(argv[0], Path(argv[1]))


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])

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
from mathutils import Euler, Matrix, Vector

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import export  # noqa: E402
from trackgen.layout import (  # noqa: E402
    CROWD_KEYS,
    HAZARD_TINT,
    Lettering,
    Pivot,
    Placement,
    SceneryMesh,
    TrackGeometry,
    build,
)
from trackgen.layouts import NAMES, load  # noqa: E402
from trackgen.pit_lane import PitLaneData  # noqa: E402

# Flat colours in the 16-bit kart racer's key: one saturated tone per
# surface, red-and-white kerbs, white boards with bold lettering. sRGB, as
# picked; converted to linear for the shader.
MATERIALS = {
    "asphalt": (0.42, 0.42, 0.47),
    "sand": (0.91, 0.78, 0.45),
    "grass": (0.36, 0.69, 0.29),
    "wall": (0.94, 0.94, 0.94),
    # The tyre wall: its prism in shadow behind the tyres, their treads and
    # the faces that read as rings from the track; the belts are kerb red
    # and white.
    "tyre-shade": (0.071, 0.071, 0.078),
    "tyre": (0.102, 0.102, 0.114),
    "tyre-face": (0.180, 0.180, 0.196),
    "kerb-red": (0.84, 0.23, 0.18),
    "kerb-white": (0.96, 0.96, 0.96),
    # The sausage kerb: gold striped black, as the style guide draws it.
    "kerb-yellow": (1.0, 0.824, 0.247),
    "kerb-black": (0.067, 0.067, 0.067),
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
    # The marshal post's band, in its lamp's orange; a material of its own
    # so the View tells the lamp's face to the track from the band round it.
    "post-band": (1.0, 0.533, 0.0),
    # The tunnel's lamps, warm and glowing: the one material that emits.
    "lamp": (1.0, 0.851, 0.541),
    # The style guide's Grandstand and Start gantry entries.
    "stand-riser": (0.843, 0.859, 0.878),
    "stand-plinth": (0.706, 0.729, 0.761),
    "stand-canopy-shade": (0.647, 0.165, 0.125),
    "beam-top": (0.788, 0.808, 0.835),
    "beam-shade": (0.604, 0.627, 0.659),
    "ink-blue": (0.12, 0.33, 0.78),
    "ink-green": (0.13, 0.55, 0.25),
    "ink-red": (0.80, 0.16, 0.16),
    "ink-black": (0.12, 0.12, 0.12),
    "ink-brown": (0.45, 0.28, 0.14),
    "ink-orange": (0.93, 0.47, 0.08),
    "ink-purple": (0.50, 0.22, 0.70),
    # The woods: each canopy colour in a lit tone and, at two thirds of it, a
    # shade tone, as the style guide's Woods card draws them, and the trunk.
    "canopy-pine": (0.184, 0.478, 0.227),
    "canopy-pine-shade": (0.122, 0.316, 0.150),
    "canopy": (0.361, 0.690, 0.290),
    "canopy-shade": (0.238, 0.455, 0.191),
    "blossom": (0.949, 0.639, 0.753),
    "blossom-shade": (0.626, 0.422, 0.497),
    "trunk": (0.353, 0.227, 0.118),
    # Monarch's harbour: the sea in the basin.
    "water": (0.165, 0.435, 0.690),
    # Monarch's casino: cream stucco and verdigris copper, each with its shade
    # tone at two thirds of it, as the woods' canopies have.
    "stucco": (0.957, 0.902, 0.776),
    "stucco-shade": (0.638, 0.601, 0.517),
    "copper": (0.388, 0.682, 0.592),
    "copper-shade": (0.259, 0.455, 0.395),
    # Monarch's quay spray: a cold white, lit from within, since the lap only
    # ever sees the side of it away from the sun.
    "spray": (0.92, 0.96, 1.0),
}
# The crowd in the stands, one material per livery's key colour, and the lit
# tone of each, the key mixed this far toward white, for the band across the
# top of the pit building's doors.
CROWD_LIT = 0.28
MATERIALS.update(
    {
        f"crowd-{slot}": tuple(((key >> shift) & 0xFF) / 255.0 for shift in (16, 8, 0))
        for slot, key in enumerate(CROWD_KEYS)
    }
)
MATERIALS.update(
    {f"crowd-{slot}-lit": tuple(c + (1.0 - c) * CROWD_LIT for c in MATERIALS[f"crowd-{slot}"]) for slot in range(len(CROWD_KEYS))}
)
# The hazard zone's tint: the sausage kerb's gold over the ground it lies on.
MATERIALS.update(
    {
        f"hazard-{ground}": tuple(
            g + (y - g) * HAZARD_TINT for g, y in zip(MATERIALS[ground], MATERIALS["kerb-yellow"])
        )
        for ground in ("asphalt", "sand")
    }
)
EMISSIVE = frozenset({"lamp", "spray"})
# Deep enough for the lettering to read as a letter from the track and no
# deeper: every millimetre is more faces on every board.
TEXT_EXTRUDE_M = 0.03
# Lettering bent round an arc follows a polyline with a point about this far
# apart, so a glyph bends smoothly and the arc costs no faces.
TEXT_ARC_STEP_M = 0.25


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
    if name in EMISSIVE:
        # The glTF exporter writes this as the material's emissive colour, so the
        # lamp reads lit however little of the sun reaches under the roof, and
        # the spray white from its shaded side.
        bsdf.inputs["Emission Color"].default_value = bsdf.inputs["Base Color"].default_value
        bsdf.inputs["Emission Strength"].default_value = 1.0
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
    if placement.hazard_half_depth:
        obj["hazard_half_depth_m"] = placement.hazard_half_depth
    col.objects.link(obj)
    return obj


def mesh_object(
    name: str,
    data: tuple[list, list],
    material_name: str,
    uvs: list[tuple[float, float]] | None = None,
    materials: tuple[str, ...] = (),
) -> bpy.types.Object:
    """A mesh object in `material_name`, or with `materials` one slot per distinct name and
    each face in its own, which the exporter writes as a primitive per slot."""
    vertices, faces = data
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(vertices, [], faces)
    slots = list(dict.fromkeys(materials or (material_name,)))
    for slot in slots:
        mesh.materials.append(material(slot))
    if materials:
        for polygon, slot in zip(mesh.polygons, materials, strict=True):
            polygon.material_index = slots.index(slot)
    mesh.validate()
    mesh.update()
    if uvs is not None:
        layer = mesh.uv_layers.new(name="UVMap")
        for loop in mesh.loops:
            layer.data[loop.index].uv = uvs[loop.vertex_index]
    return bpy.data.objects.new(name, mesh)


def frame_of(pivot: Pivot) -> Matrix:
    """The world matrix of a pivot's node: its origin at the pivot, local x along its heading."""
    return Matrix.Translation((pivot.x, pivot.y, pivot.z)) @ Matrix.Rotation(pivot.heading, 4, "Z")


def pivoted_object(
    piece: SceneryMesh, nodes: dict[str, tuple[bpy.types.Object, Matrix]]
) -> bpy.types.Object:
    """A scenery mesh the View moves as a node of its own: its vertices rebased on its pivot's
    frame so the object's origin is there, and hung under its parent in `nodes` when it names
    one, placed relative to that parent's frame. Parents come before children in the scenery."""
    pivot = piece.pivot
    frame = frame_of(pivot)
    into_local = frame.inverted()
    vertices, faces = piece.mesh
    local = [tuple(into_local @ Vector(v)) for v in vertices]
    obj = mesh_object(piece.name, (local, faces), piece.material, materials=piece.materials)
    if pivot.parent is None:
        obj.matrix_world = frame
    else:
        parent, parent_frame = nodes[pivot.parent]
        obj.parent = parent
        obj.matrix_local = parent_frame.inverted() @ frame
    nodes[piece.name] = (obj, frame)
    return obj


def yaw_facing(facing: float) -> float:
    """The Z rotation that turns something facing Blender -Y to face `facing` (radians from +X)."""
    return math.atan2(math.cos(facing), -math.sin(facing))


def text_object(prop: Lettering, nodes: dict[str, tuple[bpy.types.Object, Matrix]]) -> bpy.types.Object:
    """Lettering standing upright, read from the way its placement faces, centred on it, or
    with an `arc` bent round a circle of that radius about it, centred on the circle's top.
    Hung under its `parent` in `nodes` when it names one, placed relative to that node's
    frame, so it rides wherever the View moves the node."""
    curve = bpy.data.curves.new(prop.name, "FONT")
    curve.body = prop.text
    curve.size = prop.size
    curve.extrude = TEXT_EXTRUDE_M
    curve.align_x = "CENTER"
    curve.align_y = "CENTER"
    curve.resolution_u = 2
    curve.materials.append(material(prop.material))
    if prop.arc is not None:
        curve.follow_curve = arc_object(f"{prop.name}.arc", prop.arc)
    obj = bpy.data.objects.new(prop.name, curve)
    # Text lies in its local XY plane reading along +X; stood up on its
    # bottom edge it faces -Y.
    frame = Matrix.Translation((prop.at.x, prop.at.y, prop.at.z)) @ Euler(
        (math.pi / 2.0, 0.0, yaw_facing(prop.at.heading))
    ).to_matrix().to_4x4()
    if prop.parent is None:
        obj.matrix_world = frame
    else:
        parent, parent_frame = nodes[prop.parent]
        obj.parent = parent
        obj.matrix_local = parent_frame.inverted() @ frame
    return obj


def arc_object(name: str, radius: float) -> bpy.types.Object:
    """The half circle text bends round: a polyline of `radius` about the origin in the XY
    plane, run from -X over +Y to +X so the text reads left to right over the top. Blender
    bends text in the text's own frame and ignores the curve's transform, so the curve
    stays at the origin, in the data collection the exporter leaves out."""
    curve = bpy.data.curves.new(name, "CURVE")
    curve.dimensions = "2D"
    spline = curve.splines.new("POLY")
    count = max(8, math.ceil(math.pi * radius / TEXT_ARC_STEP_M))
    spline.points.add(count)
    for k, point in enumerate(spline.points):
        angle = math.pi - math.pi * k / count
        point.co = (radius * math.cos(angle), radius * math.sin(angle), 0.0, 1.0)
    obj = bpy.data.objects.new(name, curve)
    collection("Data").objects.link(obj)
    return obj


def populate(geometry: TrackGeometry) -> None:
    track = collection(export.TRACK_COLLECTION)
    for surface in geometry.surfaces:
        obj = mesh_object(surface.name, surface.mesh, surface.material, surface.uvs)
        obj["surface"] = surface.surface
        track.objects.link(obj)

    scenery = collection(export.SCENERY_COLLECTION)
    nodes: dict[str, tuple[bpy.types.Object, Matrix]] = {}
    for piece in geometry.scenery:
        if piece.pivot is None:
            scenery.objects.link(mesh_object(piece.name, piece.mesh, piece.material))
        else:
            scenery.objects.link(pivoted_object(piece, nodes))
    for prop in geometry.lettering:
        scenery.objects.link(text_object(prop, nodes))

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
        "Coins": ("Coin", geometry.coins),
        "Kerbs": ("Kerb", geometry.kerbs),
        "WheelCrossings": ("WheelCrossing", geometry.wheel_crossings),
        "SpawnGrid": ("Spawn", geometry.spawn_grid),
    }
    for collection_name, (prefix, items) in groups.items():
        col = collection(collection_name)
        for number, placement in enumerate(items):
            add_empty(col, f"{prefix}.{number:02d}", placement)

    if geometry.pit_lane is not None:
        populate_pit_lane(geometry.pit_lane)


def populate_pit_lane(lane: PitLaneData) -> None:
    curve = bpy.data.curves.new(export.PIT_LANE_OBJECT, "CURVE")
    curve.dimensions = "3D"
    spline = curve.splines.new("POLY")
    spline.points.add(len(lane.path) - 1)
    for point, (x, y, z) in zip(spline.points, lane.path):
        point.co = (x, y, z, 1.0)
    obj = bpy.data.objects.new(export.PIT_LANE_OBJECT, curve)
    obj["half_width"] = lane.half_width
    collection("Data").objects.link(obj)
    lines = collection(export.PIT_LINES_COLLECTION)
    add_empty(lines, "Entry", lane.entry)
    add_empty(lines, "Exit", lane.exit)
    boxes = collection(export.PIT_BOXES_COLLECTION)
    for number, box in enumerate(lane.pit_boxes):
        add_empty(boxes, f"PitBox.{number:02d}", box)


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

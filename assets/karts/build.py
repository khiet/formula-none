"""Build the car the View draws from primitives and export it.

    blender --background --python assets/karts/build.py

A chibi low-poly open-wheel F1 car (ADR 0034): short wheelbase, oversized
octagonal wheels, wings, sidepods, a halo and a big bare-headed driver
(ADR 0036), every face flat-shaded in a flat material. It is built in metres
to the box the sim collides with (`KART_HALF_*_M` in
crates/sim/src/tuning.rs), wheels on the floor, front along the sim's -Z, so
the View's fit is close to a unit scale. Three materials, `Primary`,
`Secondary` and `Accent`, are the livery the View recolours per slot
(apps/web/src/kart-model.ts, ADR 0035), and two, `Skin` and `Hair`, take the
slot's driver; the rest keep the colours set here. Every hair style and
facial hair in apps/web/src/livery.ts is a part of its own, `hair-<style>`
and `facial-hair-<kind>`, beside `earrings`, and so is every brow, nose and
mouth of a face, `brows-<kind>`, `nose-<kind>` and `mouth-<kind>`, with
`glasses` and the crew's `headset`, for the View to show the face's and hide
the rest. They are children of `head`, the driver's head pivoted at the
neck, which the View rolls for the head sway; the pit crew wears the same
head (apps/web/src/pit-box-meshes.ts). The result, `kart.glb`, carries no texture and
is committed; CI has no Blender.
"""

from __future__ import annotations

import math
from collections.abc import Callable
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
OUTPUT = HERE / "kart.glb"

# The sim's box: 1.2 m wide, 1.8 m long. The driver may stand above its 0.6 m.
HALF_WIDTH = 0.6
HALF_LENGTH = 0.9

# A neutral grey until the View paints them, so an unpainted car is a visible bug.
UNPAINTED = (0.5, 0.5, 0.5)
PAINTED = ("Primary", "Secondary", "Accent", "Skin", "Hair")
FIXED = {
    "Carbon": (0.17, 0.18, 0.2),
    "Tyre": (0.11, 0.11, 0.12),
    "Rim": (0.23, 0.24, 0.26),
    "Whites": (0.97, 0.97, 0.95),
    "Irises": (0.13, 0.09, 0.07),
    "Gold": (0.86, 0.66, 0.2),
    "Grey": (0.66, 0.66, 0.68),
    "Mouth": (0.416, 0.118, 0.133),
}

# The one shared head: about 0.6 m across, its centre low enough that the
# tallest hair stays under 1.34 m, the height the chase camera and the kart
# labels are placed to clear.
HEAD = Vector((0, -0.1, 0.97))
HEAD_RADIUS = 0.3
# A short jaw: below its middle the head narrows and shortens by these.
JAW_WIDTH = 0.86
JAW_HEIGHT = 0.78
# The head's pivot, under the chin on the neck: the head and everything it
# wears roll about it.
NECK = Vector((0, HEAD.y, HEAD.z - HEAD_RADIUS * JAW_HEIGHT))

WHEEL_SEGMENTS = 8
# Three, so the View's per-frame spin cap stays under half their 120 degree repeat.
WHEEL_SPOKES = 3
SPOKE_RADIUS = 0.03
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
        turn: float = 0.0,
    ) -> None:
        """
        A box, its front end (+Y) scaled in width and height by `front_scale`
        and lowered by `front_drop`, then turned `turn` radians about the
        forward axis through its centre: a positive turn lowers its +X end.
        """
        verts = bmesh.ops.create_cube(self.bm, size=1.0)["verts"]
        spin = Matrix.Rotation(turn, 3, "Y")
        for vert in verts:
            vert.co.x *= size[0]
            vert.co.y *= size[1]
            vert.co.z *= size[2]
            if vert.co.y > 0:
                vert.co.x *= front_scale[0]
                vert.co.z = vert.co.z * front_scale[1] - front_drop
            vert.co = spin @ vert.co + Vector(centre)
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

    def ring(
        self, material: str, centre: tuple[float, float, float], radius: float, thickness: float, arc: float, axis: str
    ) -> None:
        """An octagonal hoop of bars round `axis` through `centre`, from straight up through `arc` radians."""
        segments = max(2, round(8 * arc / (2 * math.pi)))
        points = []
        for k in range(segments + 1):
            angle = arc * k / segments - (arc / 2 if arc < 2 * math.pi else 0)
            up, across = radius * math.cos(angle), radius * math.sin(angle)
            offset = (across, 0, up) if axis == "Y" else (0, across, up)
            points.append(tuple(c + o for c, o in zip(centre, offset)))
        for a, b in zip(points, points[1:]):
            self.bar(material, a, b, thickness)

    def ball(self, material: str, centre: tuple[float, float, float], radius: float) -> None:
        # Octagonal like the wheels, so a ball is a polygon, not a smooth dome.
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=8, v_segments=6, radius=radius)["verts"]
        for vert in verts:
            vert.co += Vector(centre)
        self._paint(verts, material)

    def head_shell(self, material: str, radius: float, keep: Callable[[Vector], bool]) -> None:
        """
        The head's shape at `radius`, kept only where `keep` holds of a point
        before the jaw is applied (relative to the head's centre, on the
        unit sphere): the head itself, or hair and stubble lying on it.
        A flat faces forward, so the eyes sit on a plane.
        """
        verts = bmesh.ops.create_uvsphere(
            self.bm, u_segments=8, v_segments=6, radius=radius, matrix=Matrix.Rotation(math.pi / 8, 4, "Z")
        )["verts"]
        bmesh.ops.delete(self.bm, geom=[v for v in verts if not keep(v.co / radius)], context="VERTS")
        kept = [v for v in verts if v.is_valid]
        for vert in kept:
            if vert.co.z < 0:
                vert.co.x *= JAW_WIDTH
                vert.co.y *= JAW_WIDTH
                vert.co.z *= JAW_HEIGHT
            vert.co += HEAD
        self._paint(kept, material)

    def finish(
        self, name: str, parent: bpy.types.Object | None = None, pivot: Vector | None = None
    ) -> bpy.types.Object:
        """
        The part as an object under `parent`. Built in the kart's frame, it is
        moved so its origin is `pivot`, where it turns; every parent sits
        unrotated, so its location is the pivot less the parents' locations.
        """
        if pivot is not None:
            bmesh.ops.translate(self.bm, vec=-pivot, verts=self.bm.verts)
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
        obj.parent = parent
        if pivot is not None:
            origin = pivot.copy()
            while parent is not None:
                origin -= parent.location
                parent = parent.parent
            obj.location = origin
        return obj


def body() -> bpy.types.Object:
    # Blender is Z up with the nose along +Y; the glTF exporter turns that
    # into Y up with the nose along -Z, the way the sim's kart drives.
    part = Part(["Primary", "Secondary", "Accent", "Skin", "Carbon"])
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
    # with an accent collar, and a big bare head, the car's highest point.
    part.box("Primary", (0, -0.12, 0.62), (0.46, 0.24, 0.2), front_scale=(0.85, 1.0))
    part.box("Accent", (0, -0.1, 0.735), (0.24, 0.18, 0.05))
    part.box("Skin", (0, -0.1, 0.77), (0.13, 0.12, 0.08))
    # The halo: a central pillar up from the nose, a hoop at chin height
    # framing the face, and a leg down to the tub at each side.
    hoop_z = HEAD.z - HEAD_RADIUS * JAW_HEIGHT + 0.02
    hoop_x, hoop_y = 0.4, 0.44
    pillar = (0, HEAD.y + hoop_y, hoop_z)
    part.bar("Carbon", (0, HEAD.y + hoop_y + 0.06, 0.52), pillar, 0.025)
    left = [
        (hoop_x * math.cos(math.pi / 2 + k * math.pi / 8), HEAD.y + hoop_y * math.sin(math.pi / 2 + k * math.pi / 8), hoop_z)
        for k in range(5)
    ]
    hoop = [*reversed([(-x, hy, z) for (x, hy, z) in left[1:]]), *left]
    for a, b in zip(hoop, hoop[1:]):
        part.bar("Carbon", a, b, 0.022)
    for end in (hoop[0], hoop[-1]):
        part.bar("Carbon", end, (end[0] * 0.75, end[1] - 0.08, 0.52), 0.022)
    return part.finish("kart")


def head(root: bpy.types.Object) -> bpy.types.Object:
    """The driver's bare head with its ears and eyes, on its own node so the View can roll it at the neck."""
    part = Part(["Skin", "Whites", "Irises"])
    part.head_shell("Skin", HEAD_RADIUS, lambda _: True)
    for side in (-1, 1):
        part.box("Skin", (side * 0.29, HEAD.y - 0.02, HEAD.z - 0.02), (0.05, 0.08, 0.1))
        # Big eyes on the forward flat: whites, and irises proud of them looking ahead.
        part.box("Whites", (side * 0.09, HEAD.y + 0.272, HEAD.z + 0.04), (0.1, 0.03, 0.13))
        part.box("Irises", (side * 0.085, HEAD.y + 0.29, HEAD.z + 0.03), (0.06, 0.02, 0.085))
    return part.finish("head", root, NECK)


def cap(part: Part, radius: float = 0.32) -> None:
    """Hair over the crown and the back of the head, clear of the forehead."""
    part.head_shell("Hair", radius, lambda p: p.z > -0.1 and not (p.y > 0.5 and p.z < 0.3))


def short_curls(part: Part) -> None:
    cap(part, 0.31)
    for k in range(8):
        angle = k * math.pi / 4
        part.ball("Hair", (0.2 * math.cos(angle), HEAD.y - 0.03 + 0.2 * math.sin(angle), HEAD.z + 0.22), 0.08)
    part.ball("Hair", (0, HEAD.y - 0.03, HEAD.z + 0.28), 0.08)


def curly_fringe(part: Part) -> None:
    cap(part)
    for x in (-0.15, -0.05, 0.05, 0.15):
        part.ball("Hair", (x, HEAD.y + 0.22, HEAD.z + 0.18), 0.075)


def swept_side_part(part: Part) -> None:
    cap(part)
    # A sweep from the parting over to the other side of the forehead.
    part.box("Hair", (0.06, HEAD.y + 0.14, HEAD.z + 0.24), (0.26, 0.24, 0.07), front_scale=(1.2, 0.7), front_drop=0.04)


def swept_back(part: Part) -> None:
    cap(part)
    # Swept up off the forehead and back over the crown.
    part.box("Hair", (0, HEAD.y - 0.04, HEAD.z + 0.3), (0.26, 0.5, 0.08), front_scale=(1.1, 1.5))


def short_greying_temples(part: Part) -> None:
    cap(part)
    for side in (-1, 1):
        part.box("Grey", (side * 0.3, HEAD.y + 0.02, HEAD.z + 0.1), (0.04, 0.14, 0.12))


def braids_tied_back(part: Part) -> None:
    cap(part, 0.31)
    # Braids gathered at the crown and tied back down the neck.
    for k, z in enumerate((0.24, 0.12, 0.0, -0.12)):
        part.ball("Hair", (0, HEAD.y - 0.3 - 0.03 * k, HEAD.z + z), 0.075)


def buzz(part: Part) -> None:
    cap(part, 0.305)


def ponytail(part: Part) -> None:
    cap(part)
    for k in range(3):
        part.ball("Hair", (0, HEAD.y - 0.3, HEAD.z - 0.02 - 0.11 * k), 0.07)


def bun(part: Part) -> None:
    cap(part)
    part.ball("Hair", (0, HEAD.y - 0.2, HEAD.z + 0.24), 0.1)


def bald(part: Part) -> None:
    """A fringe of hair round the back and the sides, the crown bare."""
    part.head_shell("Hair", 0.315, lambda p: -0.6 < p.z < 0.6 and p.y < 0.2)


# Every hair style in livery.ts, the materials its part wears, and how it is built.
HAIR_STYLES: dict[str, tuple[list[str], Callable[[Part], None]]] = {
    "braids-tied-back": (["Hair"], braids_tied_back),
    "short": (["Hair"], cap),
    "swept-side-part": (["Hair"], swept_side_part),
    "curly-fringe": (["Hair"], curly_fringe),
    # The grey is fixed, so a driver carries one hair colour.
    "short-greying-temples": (["Hair", "Grey"], short_greying_temples),
    "swept-back": (["Hair"], swept_back),
    "short-curls": (["Hair"], short_curls),
    "buzz": (["Hair"], buzz),
    "ponytail": (["Hair"], ponytail),
    "bun": (["Hair"], bun),
    "bald": (["Hair"], bald),
}


def hair_parts(head: bpy.types.Object) -> None:
    for name, (materials, build) in HAIR_STYLES.items():
        part = Part(materials)
        build(part)
        part.finish(f"hair-{name}", head, NECK)


def facial_hair_parts(head: bpy.types.Object) -> None:
    """One part per facial hair in livery.ts but none, lying on the jaw below the eyes."""
    jaw = {
        "light-stubble": (0.303, lambda p: p.z < -0.4 and p.y > 0.3),
        "stubble": (0.305, lambda p: p.z < -0.4 and p.y > -0.3),
        "heavy-stubble": (0.308, lambda p: p.z < -0.4 and p.y > -0.6),
        "short-beard": (0.318, lambda p: (p.z < -0.4 or (p.z < 0.1 and abs(p.x) > 0.7)) and p.y > -0.6),
    }
    for name, (radius, keep) in jaw.items():
        part = Part(["Hair"])
        part.head_shell("Hair", radius, keep)
        part.finish(f"facial-hair-{name}", head, NECK)
    moustache = Part(["Hair"])
    moustache.box("Hair", (0, HEAD.y + 0.25, HEAD.z - 0.065), (0.16, 0.03, 0.035))
    moustache.finish("facial-hair-moustache", head, NECK)


# The face's features, in front of the face's forward flat, which slopes back
# from the eyes to the jaw: this far forward of the head's centre at each height.
NOSE_FORWARD = 0.272
MOUTH_FORWARD = 0.235


def brows_parts(head: bpy.types.Object) -> None:
    """One part per brow kind in livery.ts, in the hair's colour over each eye, its outer end raised or lowered."""
    for name, outer_up in {"level": 0.0, "knit": 0.25, "worried": -0.2}.items():
        part = Part(["Hair"])
        for side in (-1, 1):
            part.box("Hair", (side * 0.1, HEAD.y + 0.275, HEAD.z + 0.14), (0.12, 0.03, 0.028), turn=-side * outer_up)
        part.finish(f"brows-{name}", head, NECK)


def nose_parts(head: bpy.types.Object) -> None:
    for name, (width, height) in {"button": (0.05, 0.05), "broad": (0.075, 0.07)}.items():
        part = Part(["Skin"])
        part.box("Skin", (0, HEAD.y + NOSE_FORWARD, HEAD.z - 0.03), (width, 0.04, height))
        part.finish(f"nose-{name}", head, NECK)


def mouth_parts(head: bpy.types.Object) -> None:
    """One part per mouth kind in livery.ts, under the nose and clear of every beard."""
    at = (0, HEAD.y + MOUTH_FORWARD, HEAD.z - 0.1)
    line = Part(["Mouth"])
    line.box("Mouth", at, (0.11, 0.03, 0.02))
    line.finish("mouth-line", head, NECK)
    shut = Part(["Mouth"])
    shut.box("Mouth", at, (0.08, 0.03, 0.055))
    shut.finish("mouth-open", head, NECK)
    for name in ("smile", "grin"):
        part = Part(["Mouth", "Whites"])
        part.box("Mouth", (0, at[1], at[2] - 0.01), (0.066, 0.03, 0.02))
        for side in (-1, 1):
            # The corners turned up at their outer ends.
            part.box("Mouth", (side * 0.046, at[1] - 0.005, at[2] + 0.008), (0.035, 0.03, 0.02), turn=-side * 0.6)
        if name == "grin":
            part.box("Whites", (0, at[1] + 0.012, at[2] + 0.004), (0.055, 0.01, 0.022))
        part.finish(f"mouth-{name}", head, NECK)


def glasses(head: bpy.types.Object) -> None:
    """Round frames in front of the eyes, a bridge between them and arms back to the ears."""
    part = Part(["Carbon"])
    front = HEAD.y + 0.3
    for side in (-1, 1):
        part.ring("Carbon", (side * 0.09, front, HEAD.z + 0.04), 0.085, 0.012, 2 * math.pi, "Y")
        part.bar("Carbon", (side * 0.175, front, HEAD.z + 0.06), (side * 0.29, HEAD.y + 0.04, HEAD.z + 0.06), 0.012)
    part.bar("Carbon", (-0.01, front, HEAD.z + 0.05), (0.01, front, HEAD.z + 0.05), 0.012)
    part.finish("glasses", head, NECK)


def headset(head: bpy.types.Object) -> None:
    """
    The pit crew's: a band over the crown from ear to ear, a cup over each
    ear and a mic boom from the right cup to the corner of the mouth. It
    sits over any hair style without hiding it.
    """
    part = Part(["Carbon"])
    centre = (0, HEAD.y - 0.02, HEAD.z)
    part.ring("Carbon", centre, 0.335, 0.02, math.pi, "Y")
    for side in (-1, 1):
        part.cylinder("Carbon", (side * 0.315, HEAD.y - 0.02, HEAD.z - 0.02), Vector((1, 0, 0)), 0.09, 0.06, 8)
    mic = (0.12, HEAD.y + MOUTH_FORWARD, HEAD.z - 0.11)
    part.bar("Carbon", (0.3, HEAD.y + 0.02, HEAD.z - 0.08), mic, 0.0075)
    part.ball("Carbon", mic, 0.025)
    part.finish("headset", head, NECK)


def earrings(head: bpy.types.Object) -> None:
    part = Part(["Gold"])
    for side in (-1, 1):
        part.ball("Gold", (side * 0.29, HEAD.y - 0.02, HEAD.z - 0.1), 0.03)
    part.finish("earrings", head, NECK)


def wheel(name: str, spec: dict[str, float], side: int, parent: bpy.types.Object) -> None:
    """A wheel built about its axle and placed there, unrotated, for the tells to find by name."""
    part = Part(["Tyre", "Rim"])
    axis = Vector((1, 0, 0))
    part.cylinder("Tyre", (0, 0, 0), axis, spec["radius"], spec["width"], WHEEL_SEGMENTS)
    # The rim, a smaller octagon proud of the tyre's outer wall.
    part.cylinder("Rim", (side * spec["width"] / 2, 0, 0), axis, spec["radius"] * 0.58, 0.02, WHEEL_SEGMENTS)
    # Spokes reaching out over the dark tyre wall: the wheel only looks as it
    # started every 120 degrees, so the View's turning shows.
    face = side * (spec["width"] / 2 + 0.01)
    for spoke in range(WHEEL_SPOKES):
        turn = 2 * math.pi * spoke / WHEEL_SPOKES
        tip = spec["radius"] * 0.86
        part.bar("Rim", (face, 0, 0), (face, tip * math.sin(turn), tip * math.cos(turn)), SPOKE_RADIUS)
    obj = part.finish(name)
    obj.location = (side * spec["x"], spec["y"], axle_height(spec))
    obj.parent = parent


def build() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for name in PAINTED:
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
    driver = head(root)
    hair_parts(driver)
    facial_hair_parts(driver)
    earrings(driver)
    brows_parts(driver)
    nose_parts(driver)
    mouth_parts(driver)
    glasses(driver)
    headset(driver)
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

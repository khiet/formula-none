"""Triangle meshes for a track: ribbons along the path, box walls, pipes, the grass slab, and
the scenery's stripes, checkers and boards.

A mesh is `(vertices, faces)` with vertices as `(x, y, z)` tuples and faces
as index tuples, which is what Blender's `from_pydata` takes. Faces may be
quads; Blender triangulates them on export.
"""

from __future__ import annotations

import math

Vertex = tuple[float, float, float]
Face = tuple[int, ...]
Mesh = tuple[list[Vertex], list[Face]]


def ribbon(left: list[Vertex], right: list[Vertex], closed: bool) -> Mesh:
    """Quads between two rails of equal length; `closed` joins the last pair back to the first."""
    if len(left) != len(right) or len(left) < 2:
        raise ValueError("a ribbon needs two rails of the same length, at least two long")
    count = len(left)
    vertices = left + right
    pairs = count if closed else count - 1
    faces: list[Face] = []
    # Wound so the face normal points up when `left` really is the left rail.
    for i in range(pairs):
        j = (i + 1) % count
        faces.append((i, count + i, count + j, j))
    return vertices, faces


def box_rail(
    inner: list[Vertex], outer: list[Vertex], height: float, closed: bool = False
) -> Mesh:
    """A wall of `height` metres standing on two rails: both faces, the top
    and both ends, or with `closed` the last pair joined back to the first
    and no ends.

    Every face is wound to point out of the wall, which the physics relies on
    to tell a kart which way a face pushes.
    """
    if len(inner) != len(outer) or len(inner) < 2:
        raise ValueError("a wall needs two rails of the same length, at least two long")
    count = len(inner)

    def lift(v: Vertex) -> Vertex:
        return (v[0], v[1], v[2] + height)

    # Vertex blocks: inner base, inner top, outer top, outer base.
    vertices = inner + [lift(v) for v in inner] + [lift(v) for v in outer] + outer
    ib, it, ot, ob = 0, count, 2 * count, 3 * count
    faces: list[Face] = []
    for i in range(count if closed else count - 1):
        j = (i + 1) % count
        towards_inner = _direction(outer[i], inner[i])
        faces.append(_facing((ib + i, ib + j, it + j, it + i), vertices, towards_inner))
        faces.append(_facing((it + i, it + j, ot + j, ot + i), vertices, (0.0, 0.0, 1.0)))
        faces.append(_facing((ot + i, ot + j, ob + j, ob + i), vertices, _negate(towards_inner)))
    if closed:
        return vertices, faces
    last = count - 1
    faces.append(_facing((ib, it, ot, ob), vertices, _direction(inner[1], inner[0])))
    faces.append(
        _facing(
            (ib + last, ob + last, ot + last, it + last),
            vertices,
            _direction(inner[last - 1], inner[last]),
        )
    )
    return vertices, faces


def _direction(a: Vertex, b: Vertex) -> Vertex:
    return (b[0] - a[0], b[1] - a[1], b[2] - a[2])


def _negate(v: Vertex) -> Vertex:
    return (-v[0], -v[1], -v[2])


def _facing(face: Face, vertices: list[Vertex], direction: Vertex) -> Face:
    """`face`, reversed if needed so its normal has a positive component along `direction`."""
    a, b, c = (vertices[face[0]], vertices[face[1]], vertices[face[2]])
    u, v = _direction(a, b), _direction(a, c)
    normal = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    along = sum(n * d for n, d in zip(normal, direction))
    return face if along >= 0.0 else tuple(reversed(face))


def cylinder(centre: Vertex, radius: float, height: float, sides: int = 16) -> Mesh:
    """An upright capped cylinder standing on `centre`."""
    cx, cy, cz = centre
    ring = [
        (cx + radius * math.cos(math.tau * i / sides), cy + radius * math.sin(math.tau * i / sides))
        for i in range(sides)
    ]
    bottom = [(x, y, cz) for x, y in ring]
    top = [(x, y, cz + height) for x, y in ring]
    vertices = bottom + top
    faces: list[Face] = [(i, (i + 1) % sides, sides + (i + 1) % sides, sides + i) for i in range(sides)]
    faces.append(tuple(reversed(range(sides))))
    faces.append(tuple(range(sides, 2 * sides)))
    return vertices, faces


def slab(min_x: float, min_y: float, max_x: float, max_y: float, z: float) -> Mesh:
    """A flat rectangle at height `z`."""
    vertices: list[Vertex] = [(min_x, min_y, z), (max_x, min_y, z), (max_x, max_y, z), (min_x, max_y, z)]
    return vertices, [(0, 1, 2, 3)]


def subset(source: Mesh, faces: list[Face]) -> Mesh:
    """`faces` of `source` as a mesh of their own, with only the vertices they use."""
    vertices, _ = source
    index: dict[int, int] = {}
    kept: list[Vertex] = []
    remapped: list[Face] = []
    for face in faces:
        new_face = []
        for v in face:
            if v not in index:
                index[v] = len(kept)
                kept.append(vertices[v])
            new_face.append(index[v])
        remapped.append(tuple(new_face))
    return kept, remapped


def striped(source: Mesh, stripe: int) -> tuple[Mesh, Mesh]:
    """A ribbon split into two meshes whose quads alternate every `stripe` quads, in face order."""
    _, faces = source
    first = [face for i, face in enumerate(faces) if (i // stripe) % 2 == 0]
    second = [face for i, face in enumerate(faces) if (i // stripe) % 2 == 1]
    return subset(source, first), subset(source, second)


def quad(a: Vertex, b: Vertex, c: Vertex, d: Vertex) -> Mesh:
    """One four-sided face, wound as given."""
    return [a, b, c, d], [(0, 1, 2, 3)]


def merge(meshes: list[Mesh]) -> Mesh:
    """One mesh from many, faces re-indexed; nothing is welded."""
    vertices: list[Vertex] = []
    faces: list[Face] = []
    for mesh_vertices, mesh_faces in meshes:
        base = len(vertices)
        vertices.extend(mesh_vertices)
        faces.extend(tuple(base + v for v in face) for face in mesh_faces)
    return vertices, faces

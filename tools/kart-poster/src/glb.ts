// The exported kart as geometry: every mesh's triangles in the kart's frame,
// each with the name of its node and of the material it wears, and where each
// node's origin sits. Read straight from the binary glTF, so the poster needs
// neither Three.js nor Blender. Nodes are placed by translation alone, as
// build.py leaves them; a rotated or scaled node is refused rather than
// drawn in the wrong place.

export type Vec3 = readonly [number, number, number];

export interface Triangle {
  readonly node: string;
  readonly material: string;
  /** Counter-clockwise seen from the front, as glTF winds a face. */
  readonly corners: readonly [Vec3, Vec3, Vec3];
}

export interface Model {
  readonly triangles: readonly Triangle[];
  /** Each node's origin in the kart's frame, by node name, meshes or not. */
  readonly origins: ReadonlyMap<string, Vec3>;
  /** The nodes that carry a mesh, in the glb's order. */
  readonly meshed: readonly string[];
}

interface Gltf {
  readonly scenes?: readonly { readonly nodes?: readonly number[] }[];
  readonly nodes: readonly {
    readonly name?: string;
    readonly mesh?: number;
    readonly children?: readonly number[];
    readonly translation?: Vec3;
    readonly rotation?: readonly number[];
    readonly scale?: readonly number[];
  }[];
  readonly meshes: readonly {
    readonly primitives: readonly {
      readonly attributes: { readonly POSITION: number };
      readonly indices?: number;
      readonly material?: number;
      readonly mode?: number;
    }[];
  }[];
  readonly materials?: readonly { readonly name?: string }[];
  readonly accessors: readonly {
    readonly bufferView?: number;
    readonly byteOffset?: number;
    readonly componentType: number;
    readonly count: number;
    readonly type: string;
  }[];
  readonly bufferViews: readonly {
    readonly byteOffset?: number;
    readonly byteLength: number;
    readonly byteStride?: number;
  }[];
}

const COMPONENTS: Readonly<Record<string, number>> = { SCALAR: 1, VEC3: 3 };

const TRIANGLES = 4;

/** Parse a glb into its triangles and node origins. Throws on anything but a glb of unrotated, unscaled nodes. */
export function parseGlb(bytes: Uint8Array): Model {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const text = new TextDecoder();
  if (
    text.decode(bytes.subarray(0, 4)) !== "glTF" ||
    text.decode(bytes.subarray(16, 20)) !== "JSON"
  ) {
    throw new Error("not a binary glTF with a leading JSON chunk");
  }
  const jsonLength = view.getUint32(12, true);
  const gltf = JSON.parse(text.decode(bytes.subarray(20, 20 + jsonLength))) as Gltf;
  const binStart = 20 + jsonLength;
  if (text.decode(bytes.subarray(binStart + 4, binStart + 7)) !== "BIN") {
    throw new Error("the glb carries no binary chunk");
  }
  const bin = bytes.subarray(binStart + 8, binStart + 8 + view.getUint32(binStart, true));

  const read = (index: number): number[] => {
    const accessor = gltf.accessors[index];
    if (accessor === undefined || accessor.bufferView === undefined) {
      throw new Error(`accessor ${index} has no buffer view`);
    }
    const bufferView = gltf.bufferViews[accessor.bufferView];
    if (bufferView === undefined) {
      throw new Error(`accessor ${index} points at no buffer view`);
    }
    const components = COMPONENTS[accessor.type];
    if (components === undefined) {
      throw new Error(`accessor ${index} is a ${accessor.type}, not a scalar or a vector`);
    }
    const start = (bufferView.byteOffset ?? 0) + (accessor.byteOffset ?? 0);
    const data = new DataView(bin.buffer, bin.byteOffset + start);
    const readers: Record<number, [size: number, read: (offset: number) => number]> = {
      5121: [1, (at) => data.getUint8(at)],
      5123: [2, (at) => data.getUint16(at, true)],
      5125: [4, (at) => data.getUint32(at, true)],
      5126: [4, (at) => data.getFloat32(at, true)],
    };
    const reader = readers[accessor.componentType];
    if (reader === undefined) {
      throw new Error(`accessor ${index} has component type ${accessor.componentType}`);
    }
    const [size, get] = reader;
    const stride = bufferView.byteStride ?? size * components;
    const out: number[] = [];
    for (let item = 0; item < accessor.count; item += 1) {
      for (let component = 0; component < components; component += 1) {
        out.push(get(item * stride + component * size));
      }
    }
    return out;
  };

  const origins = new Map<string, Vec3>();
  const triangles: Triangle[] = [];
  const meshed: string[] = [];
  const place = (index: number, parent: Vec3) => {
    const node = gltf.nodes[index];
    if (node === undefined) {
      throw new Error(`node ${index} does not exist`);
    }
    if (node.rotation !== undefined || node.scale !== undefined) {
      throw new Error(
        `node ${node.name ?? index} is rotated or scaled; the poster places by translation alone`,
      );
    }
    const shift = node.translation ?? [0, 0, 0];
    const origin: Vec3 = [parent[0] + shift[0], parent[1] + shift[1], parent[2] + shift[2]];
    const name = node.name ?? `node-${index}`;
    origins.set(name, origin);
    const mesh = node.mesh === undefined ? undefined : gltf.meshes[node.mesh];
    if (mesh !== undefined) meshed.push(name);
    for (const primitive of mesh?.primitives ?? []) {
      if ((primitive.mode ?? TRIANGLES) !== TRIANGLES || primitive.indices === undefined) {
        throw new Error(`${name} has a primitive that is not indexed triangles`);
      }
      const positions = read(primitive.attributes.POSITION);
      const indices = read(primitive.indices);
      const material =
        primitive.material === undefined
          ? "unnamed"
          : (gltf.materials?.[primitive.material]?.name ?? "unnamed");
      const at = (vertex: number): Vec3 => [
        origin[0] + (positions[vertex * 3] ?? 0),
        origin[1] + (positions[vertex * 3 + 1] ?? 0),
        origin[2] + (positions[vertex * 3 + 2] ?? 0),
      ];
      for (let face = 0; face + 2 < indices.length; face += 3) {
        triangles.push({
          node: name,
          material,
          corners: [at(indices[face] ?? 0), at(indices[face + 1] ?? 0), at(indices[face + 2] ?? 0)],
        });
      }
    }
    for (const child of node.children ?? []) {
      place(child, origin);
    }
  };
  for (const root of gltf.scenes?.[0]?.nodes ?? []) {
    place(root, [0, 0, 0]);
  }
  return { triangles, origins, meshed };
}

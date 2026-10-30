import { Box3, type Material, Mesh, MeshLambertMaterial, type Object3D, Quaternion } from "three";
import { describe, expect, it } from "vitest";
import { COIN_SPRAY_SECONDS, coinMesh, coinMeshes, coinSpray } from "./coin-meshes";
import { COIN_GOLD, COIN_RIM } from "./palette";

function materialsOf(object: Object3D): Material[] {
  const found: Material[] = [];
  object.traverse((child) => {
    if (child instanceof Mesh) found.push(...[child.material].flat());
  });
  return found;
}

function coloursOf(object: Object3D): Set<number> {
  return new Set(
    materialsOf(object).flatMap((material) =>
      material instanceof MeshLambertMaterial ? [material.color.getHex()] : [],
    ),
  );
}

function worldTurn(object: Object3D): Quaternion {
  object.updateWorldMatrix(true, true);
  const body = object.children[0];
  if (body === undefined) throw new Error("a coin has a body");
  return body.getWorldQuaternion(new Quaternion());
}

const PLACED = [
  { position: [1, 0, -40], yaw: 0 },
  { position: [-3, 0.2, 12], yaw: Math.PI / 2 },
] as const;

describe("the coin", () => {
  it("is flat-shaded poster gold with a darker rim and F, lit by the sun", () => {
    const coin = coinMesh();
    for (const material of materialsOf(coin)) {
      expect(material).toBeInstanceOf(MeshLambertMaterial);
      expect(material).toHaveProperty("flatShading", true);
    }
    expect(coloursOf(coin)).toEqual(new Set([COIN_GOLD, COIN_RIM]));
  });

  it("stands on its edge about 0.8 m across, its centre about 0.6 m over the asphalt", () => {
    const bounds = new Box3().setFromObject(coinMesh());
    expect(bounds.max.y - bounds.min.y).toBeCloseTo(0.8, 1);
    expect((bounds.max.y + bounds.min.y) / 2).toBeCloseTo(0.6, 1);
    // Thin across its face.
    expect(bounds.max.z - bounds.min.z).toBeLessThan(0.3);
  });
});

describe("the track's coins", () => {
  it("draw a coin at each of the sidecar's coins, at its position", () => {
    const { mesh } = coinMeshes(PLACED);
    expect(mesh.children).toHaveLength(2);
    mesh.children.forEach((coin, index) => {
      expect(coin.position.toArray()).toEqual(PLACED[index]?.position);
      expect(coin.visible).toBe(true);
    });
  });

  it("hide a picked-up coin until it is back", () => {
    const coins = coinMeshes(PLACED);
    const shown = () => coins.mesh.children.map((coin) => coin.visible);
    coins.show([true, false]);
    expect(shown()).toEqual([true, false]);
    coins.show([true, true]);
    expect(shown()).toEqual([true, true]);
  });

  it("turn once a second on the clock, every coin in phase whatever its yaw", () => {
    const coins = coinMeshes(PLACED);
    const [first, second] = coins.mesh.children;
    if (first === undefined || second === undefined) throw new Error("two coins");
    coins.turn(0.25);
    const quarter = worldTurn(first);
    expect(quarter.angleTo(worldTurn(second))).toBeCloseTo(0, 5);
    expect(quarter.angleTo(new Quaternion())).toBeCloseTo(Math.PI / 2, 5);
    coins.turn(1.25);
    expect(worldTurn(first).angleTo(quarter)).toBeCloseTo(0, 5);
  });

  it("bob a little about their height, together", () => {
    const coins = coinMeshes(PLACED);
    const heights = new Set<number>();
    for (let t = 0; t < 3; t += 0.1) {
      coins.turn(t);
      const [first, second] = coins.mesh.children.map((coin) => coin.children[0]?.position.y);
      expect(first).toBe(second);
      heights.add(Number(first?.toFixed(3)));
    }
    expect(heights.size).toBeGreaterThan(1);
    expect(Math.max(...heights) - Math.min(...heights)).toBeLessThan(0.3);
  });
});

describe("the spin-out's coin spray", () => {
  const AT = [10, 0, -20] as const;
  const flying = (spray: ReturnType<typeof coinSpray>) =>
    spray.mesh.children.filter((coin) => coin.visible);

  it("throws as many coins as the kart lost up and out of it", () => {
    const spray = coinSpray();
    spray.burst(AT, 3);
    spray.update(0.1);
    const coins = flying(spray);
    expect(coins).toHaveLength(3);
    for (const coin of coins) {
      expect(coin.position.y).toBeGreaterThan(AT[1]);
      expect(coin.position.distanceTo({ x: AT[0], y: coin.position.y, z: AT[2] })).toBeGreaterThan(
        0,
      );
    }
    // Spread apart, not stacked.
    const [a, b] = coins;
    expect(a?.position.distanceTo(b?.position ?? a.position)).toBeGreaterThan(0.1);
  });

  it("is gone once the burst is over", () => {
    const spray = coinSpray();
    spray.burst(AT, 2);
    spray.update(COIN_SPRAY_SECONDS / 2);
    expect(flying(spray)).toHaveLength(2);
    spray.update(COIN_SPRAY_SECONDS / 2 + 0.01);
    expect(flying(spray)).toHaveLength(0);
  });

  it("throws nothing for no coins lost, and reuses its coins from burst to burst", () => {
    const spray = coinSpray();
    spray.burst(AT, 0);
    expect(flying(spray)).toHaveLength(0);
    spray.burst(AT, 3);
    spray.update(COIN_SPRAY_SECONDS + 0.1);
    spray.burst(AT, 3);
    expect(spray.mesh.children).toHaveLength(3);
    expect(flying(spray)).toHaveLength(3);
  });
});

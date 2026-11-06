import { describe, expect, it } from "vitest";
import { crewRoster } from "./crew-roster";
import { FACE_PARTS, partsOf } from "./driver-parts";
import { HAIRS, LIVERIES, livery } from "./livery";

describe("the parts a face wears", () => {
  it("dresses every driver in a brow, a nose, a mouth and their hair, all parts the head carries", () => {
    for (let slot = 0; slot < LIVERIES; slot += 1) {
      const { driver } = livery(slot);
      const worn = [...partsOf(driver)];
      for (const part of worn.filter((name) => name !== "facial-hair-none")) {
        expect(FACE_PARTS).toContain(part);
      }
      expect(worn).toContain(`hair-${driver.hair}`);
      expect(worn.filter((part) => part.startsWith("brows-"))).toHaveLength(1);
      expect(worn.filter((part) => part.startsWith("nose-"))).toHaveLength(1);
      expect(worn.filter((part) => part.startsWith("mouth-"))).toHaveLength(1);
      expect(worn).not.toContain("headset");
    }
  });

  it("puts a crew face's headset over any hair style, the hair still worn", () => {
    const [face] = crewRoster(0);
    if (face === undefined) throw new Error("a crew");
    for (const hair of HAIRS) {
      const worn = partsOf({ ...face, hair }, "headset");
      expect(worn).toContain("headset");
      expect(worn).toContain(`hair-${hair}`);
    }
  });

  it("dresses a driver's face alike on the kart and under a headset, but for the headset", () => {
    const { driver } = livery(0);
    const bare = partsOf(driver);
    const crewed = partsOf(driver, "headset");
    expect([...crewed].filter((part) => part !== "headset").sort()).toEqual([...bare].sort());
  });
});

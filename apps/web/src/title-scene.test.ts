// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { createTitleScene } from "./title-scene";

describe("Title Scene placeholder", () => {
  it("names the game and starts a race on the button", () => {
    const container = document.createElement("div");
    let races = 0;
    const scene = createTitleScene(container, {
      race: () => {
        races += 1;
      },
    });

    expect(container.querySelector("h1")?.textContent).toBe("Formula None");
    const button = container.querySelector<HTMLButtonElement>("button[name=race]");
    expect(button?.textContent).toBe("Race");
    button?.click();
    expect(races).toBe(1);

    scene.dispose();
    expect(container.childElementCount).toBe(0);
  });
});

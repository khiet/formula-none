export interface TitleScene {
  dispose(): void;
}

export interface TitleHandlers {
  /** Start a solo race on the selected track. */
  race(): void;
}

/**
 * The Title Scene, as a placeholder until rooms arrive: the game's name and
 * a button into a solo race. The button is addressable by its name.
 */
export function createTitleScene(container: HTMLElement, handlers: TitleHandlers): TitleScene {
  const scene = document.createElement("div");
  scene.id = "title-scene";
  const heading = document.createElement("h1");
  heading.textContent = "Formula None";
  const race = document.createElement("button");
  race.type = "button";
  race.name = "race";
  race.textContent = "Race";
  race.addEventListener("click", handlers.race);
  scene.append(heading, race);
  container.append(scene);
  return {
    dispose() {
      scene.remove();
    },
  };
}

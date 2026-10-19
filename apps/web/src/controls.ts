// The controls the Scenes share, built the same way so tests and styles
// find them the same way: a button is addressable by its name.

export function button(name: string, label: string, press: () => void): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.name = name;
  element.textContent = label;
  element.addEventListener("click", press);
  return element;
}

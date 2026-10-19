// The controls the Scenes share. A button is addressable by its name, so
// tests and styles find it without knowing which Scene built it.

export function button(name: string, label: string, press: () => void): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.name = name;
  element.textContent = label;
  element.addEventListener("click", press);
  return element;
}

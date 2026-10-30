// The Grand Prix poster the Title and Results Scenes are set on: rotating
// rays and chequered bands behind them, and the kart side on. All of it is
// decoration, hidden from assistive technology; index.html styles it.

import { DRIVER_PARTS, partsOf } from "./driver-parts";
import side from "./generated/kart/side.json";
import { hexCss, type Livery } from "./livery";
import { KART_PAINT } from "./palette";

/** The rays and the chequered bands, to go first in a Scene so they sit behind it. */
export function backdrop(): HTMLElement[] {
  return ["rays", "chequer-top", "chequer-bottom"].map((part) => {
    const element = document.createElement("div");
    element.className = part;
    element.setAttribute("aria-hidden", "true");
    return element;
  });
}

/** The custom property each of kart.glb's materials is painted from: the livery's, the driver's, or a fixed colour. */
const PROPERTIES: Readonly<Record<string, string>> = {
  Primary: "p",
  Secondary: "s",
  Accent: "a",
  Skin: "sk",
  Hair: "hair",
  ...Object.fromEntries(
    Object.keys(KART_PAINT).map((name) => [name[0]?.toUpperCase() + name.slice(1), name]),
  ),
};

/** The suffix index.html mixes each tone under. */
const TONES = { lit: "-lt", mid: "", shaded: "-dk" } as const;

/**
 * The kart side on, facing right: build.py's kart as `pnpm generate:poster`
 * projects it from kart.glb (ADR 0052), painted in the livery's primary,
 * secondary and accent as the View paints it, its driver bare-headed in
 * only the parts they wear. The paths come in painter order, one `<g>` per
 * run of a node; each wheel's group is `.wheel`, turning about its axle.
 * The colours ride on the SVG as custom properties; index.html derives
 * their lit and shaded tones.
 */
export function kartSide({ primary, secondary, accent, driver }: Livery): SVGSVGElement {
  const worn = partsOf(driver);
  let body = "";
  let run: string | undefined;
  for (const path of side.paths) {
    if (DRIVER_PARTS.has(path.node) && !worn.has(path.node)) continue;
    if (path.node !== run) {
      if (run !== undefined) body += "</g>";
      run = path.node;
      const [x, y] = (side.wheels as Readonly<Record<string, readonly number[]>>)[path.node] ?? [];
      body +=
        x === undefined || y === undefined
          ? `<g data-node="${path.node}">`
          : `<g class="wheel" data-node="${path.node}" style="transform-origin: ${x}px ${y}px">`;
    }
    // Stroked in its own fill so no hairline of poster shows between neighbouring faces.
    const fill = `var(--${PROPERTIES[path.material] ?? "p"}${TONES[path.tone as keyof typeof TONES] ?? ""})`;
    body += `<path d="${path.d}" fill="${fill}" stroke="${fill}" stroke-width="0.5" stroke-linejoin="round"/>`;
  }
  if (run !== undefined) body += "</g>";
  const template = document.createElement("template");
  template.innerHTML = `<svg class="kart" viewBox="${side.viewBox}" aria-hidden="true">
    <ellipse cx="260" cy="352" rx="232" ry="9" fill="rgb(0 0 0 / 0.18)"/>
    ${body}
  </svg>`;
  const kart = template.content.firstElementChild as SVGSVGElement;
  for (const [name, colour] of [
    ["p", primary],
    ["s", secondary],
    ["a", accent],
    ["sk", driver.skin],
    ["hair", driver.hairColour],
    ...Object.entries(KART_PAINT),
  ] as const) {
    kart.style.setProperty(`--${name}`, hexCss(colour));
  }
  return kart;
}

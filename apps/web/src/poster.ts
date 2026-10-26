// The Grand Prix poster the Title and Results Scenes are set on: rotating
// rays and chequered bands behind them, and the flat side-on kart. All of
// it is decoration, hidden from assistive technology; index.html styles it.

/** The rays and the chequered bands, to go first in a Scene so they sit behind it. */
export function backdrop(): HTMLElement[] {
  return ["rays", "chequer-top", "chequer-bottom"].map((part) => {
    const element = document.createElement("div");
    element.className = part;
    element.setAttribute("aria-hidden", "true");
    return element;
  });
}

/** A flat side-on kart, painted in the `--livery` of the element it sits in. */
export function kartSide(): SVGSVGElement {
  const template = document.createElement("template");
  template.innerHTML = `<svg class="kart" viewBox="0 0 120 60" aria-hidden="true">
    <ellipse cx="60" cy="56" rx="54" ry="4" fill="rgb(0 0 0 / 0.25)"/>
    <path class="body" d="M8 40 L18 26 L52 24 L64 14 L84 14 L92 26 L112 30 L114 42 Z" stroke="#111" stroke-width="3" stroke-linejoin="round"/>
    <rect x="96" y="20" width="18" height="6" rx="2" fill="#111"/>
    <circle cx="70" cy="10" r="9" fill="#fff" stroke="#111" stroke-width="3"/>
    <rect x="70" y="6" width="10" height="5" rx="2" fill="#4cc9f0" stroke="#111" stroke-width="2"/>
    <circle cx="28" cy="44" r="11" fill="#222" stroke="#111" stroke-width="3"/><circle cx="28" cy="44" r="4" fill="#bbb"/>
    <circle cx="94" cy="44" r="11" fill="#222" stroke="#111" stroke-width="3"/><circle cx="94" cy="44" r="4" fill="#bbb"/>
  </svg>`;
  return template.content.firstElementChild as SVGSVGElement;
}

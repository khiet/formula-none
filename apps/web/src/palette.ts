// The Instrument panel's and the item meshes' colours (ADR 0037), from the
// palette in docs/mockups/asset-catalog.html, which gives the yellow no lit
// or shaded tone, so those two are picked here. The View reads them
// as numbers; the panel writes them onto its root as CSS custom properties
// with `writePalette`, so index.html names a colour and never spells it.
// Every colour is sRGB, as a Three colour takes it.

import { hexCss } from "./livery";

/** A faceted colour: the lit, the mid and the shaded tone of one colour. */
export interface Tones {
  readonly lit: number;
  readonly mid: number;
  readonly shaded: number;
}

/** The posters' hard drop under every plate and every value. */
export const INK = 0x111111;
export const VALUE_WHITE = 0xffffff;
export const LABEL_GREY = 0xc9ced5;

/** The stack, the gear, FINISH and the standings: the colour of the kart's floor. */
export const CARBON: Tones = { lit: 0x4a4f57, mid: 0x2b2e33, shaded: 0x1b1d21 };
/** The item box and its slot, the one gold in the game besides the posters'. */
export const ITEM_BOX_GOLD: Tones = { lit: 0xf8e39a, mid: 0xe0b54a, shaded: 0xc99a2e };
/** The item slot lit by a fire. */
export const FIRING: Tones = { lit: VALUE_WHITE, mid: 0xfff6cf, shaded: 0xf8e39a };
/** The gear on the limiter. */
export const WHITE: Tones = { lit: VALUE_WHITE, mid: 0xeef0f2, shaded: 0xcfd2d6 };
/** Danger: the hazard cone and the wrong-way warning. */
export const HAZARD_ORANGE: Tones = { lit: 0xff7a4d, mid: 0xff5522, shaded: 0xc73c14 };
/** The connection banner and the countdown's start lights. */
export const POSTER_RED: Tones = { lit: 0xe8333b, mid: 0xc81d25, shaded: 0x8f141a };
export const GO_GREEN: Tones = { lit: 0x6fe8a6, mid: 0x3ddc84, shaded: 0x23a35e };

/** The shift lights' thirds and the tyre gauge's bands. */
export const SIGNAL_GREEN: Tones = { lit: 0x8ee88e, mid: 0x5ad15a, shaded: 0x3a9a3a };
export const SIGNAL_YELLOW: Tones = { lit: 0xf9da7e, mid: 0xf5c542, shaded: 0xb08a22 };
export const SIGNAL_RED: Tones = { lit: 0xff7a6e, mid: 0xe8483a, shaded: 0xa8261b };
/** The shift lights' last third and the limiter. */
export const SHIFT_BLUE: Tones = { lit: 0x8aa8ff, mid: 0x4f7dff, shaded: 0x2f4fc4 };

/** The course map draws the track as the world builds it. */
export const ASPHALT = 0x6b6b78;
export const WALL = 0xf0f0f0;

/** The item characters' greys, slot and road alike: the gull, the snail's foot, the cone's slab. */
export const GREYS = {
  white: VALUE_WHITE,
  body: 0xe3e6ea,
  pale: 0xd4d8dd,
  grey: 0xb9bec5,
  dark: 0x9aa0a8,
  deep: 0x6f747b,
} as const;
/** The gull's eye, locked on. */
export const GULL_EYE = 0xd00a12;
/** The snail's rocket, its flame and its porthole. */
export const ROCKET: Tones = { lit: 0xe8333b, mid: 0xd00a12, shaded: 0x8f070c };
export const ROCKET_FLAME = 0xffd23f;
export const ROCKET_WINDOW = 0x7fc3ff;
export const ROCKET_STRAP = 0x3a3d42;

/**
 * The lit and shaded tones of a colour that has only its mid, as the
 * catalog mixes a livery's: 72% of it with white, 66% of it with black.
 */
export function tonesOf(colour: number): Tones {
  const mix = (toward: number, share: number) =>
    [16, 8, 0].reduce((mixed, shift) => {
      const channel = (colour >> shift) & 0xff;
      return mixed | (Math.round(channel * share + toward * (1 - share)) << shift);
    }, 0);
  return { lit: mix(255, 0.72), mid: colour, shaded: mix(0, 0.66) };
}

/** Every named colour, as the custom property index.html reads it by. */
const PROPERTIES: Readonly<Record<string, number | Tones>> = {
  ink: INK,
  value: VALUE_WHITE,
  label: LABEL_GREY,
  carbon: CARBON,
  gold: ITEM_BOX_GOLD,
  firing: FIRING,
  white: WHITE,
  green: SIGNAL_GREEN,
  yellow: SIGNAL_YELLOW,
  red: SIGNAL_RED,
  blue: SHIFT_BLUE,
};

/** Write `tones` onto `element` as `--<name>-lit`, `--<name>` and `--<name>-shaded`. */
export function writeTones(element: HTMLElement | SVGElement, name: string, tones: Tones): void {
  element.style.setProperty(`--${name}-lit`, hexCss(tones.lit));
  element.style.setProperty(`--${name}`, hexCss(tones.mid));
  element.style.setProperty(`--${name}-shaded`, hexCss(tones.shaded));
}

/** Write the palette onto `element` as custom properties for the styles under it. */
export function writePalette(element: HTMLElement): void {
  for (const [name, colour] of Object.entries(PROPERTIES)) {
    if (typeof colour === "number") {
      element.style.setProperty(`--${name}`, hexCss(colour));
    } else {
      writeTones(element, name, colour);
    }
  }
}

/**
 * Make `element` a plate (GLOSSARY) in `tones`: index.html draws the
 * chamfered bands from these properties and drops the ink under them.
 */
export function plate(element: HTMLElement, tones: Tones): void {
  element.dataset.plate ??= "";
  writeTones(element, "plate", tones);
}

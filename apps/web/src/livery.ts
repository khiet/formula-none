// The eight liveries (ADR 0035): each a team's colours for the car and a
// key colour for everything that shows a kart in one colour, so the kart
// in the View and the player in the Room Scene read as the same team. The
// key colours are picked so every pair stands apart at race distance
// (livery.test.ts holds the margin, and wins over brand accuracy); the
// car's own colours stay faithful to the team. No kart shows a team name,
// crest, sponsor or race number (ADR 0038).

export const LIVERIES = 8;

/** How a driver's hair is cut, on the one shared head every kart carries. */
export const HAIRS = [
  "braids-tied-back",
  "short",
  "swept-side-part",
  "curly-fringe",
  "short-greying-temples",
  "swept-back",
  "short-curls",
] as const;
export type Hair = (typeof HAIRS)[number];

export const FACIAL_HAIRS = [
  "none",
  "light-stubble",
  "stubble",
  "heavy-stubble",
  "short-beard",
] as const;
export type FacialHair = (typeof FACIAL_HAIRS)[number];

/**
 * A slot's driver: a friendly caricature of the team's best-known driver
 * under a parody name, told apart only by what swaps on the shared head.
 * The kart shows the head bare (ADR 0036); the name shows nowhere.
 */
export interface Driver {
  readonly name: string;
  /** sRGB, as a Three colour takes it. */
  readonly skin: number;
  readonly hair: Hair;
  /** sRGB. */
  readonly hairColour: number;
  readonly facialHair: FacialHair;
  readonly earrings: boolean;
}

/** Every colour is sRGB, as a Three colour takes it. */
export interface Livery {
  /** After the team, never its name. */
  readonly name: string;
  /** The one colour for the minimap, the swatches, the kart labels and `liveryCss`. */
  readonly key: number;
  readonly primary: number;
  readonly secondary: number;
  readonly accent: number;
  readonly driver: Driver;
}

const SKIN = { deepBrown: 0x74492f, fair: 0xe5bb98, olive: 0xc48a5f } as const;

const PALETTE: readonly Livery[] = [
  // After Ferrari.
  {
    name: "Rosso",
    key: 0xd00a12,
    primary: 0xd00a12,
    secondary: 0x141414,
    accent: 0xffd400,
    driver: {
      name: "Louis Hamiltoon",
      skin: SKIN.deepBrown,
      hair: "braids-tied-back",
      hairColour: 0x1a1210,
      facialHair: "short-beard",
      earrings: true,
    },
  },
  // After Red Bull.
  {
    name: "Notte",
    key: 0x1f3fb0,
    primary: 0x16224d,
    secondary: 0xe2172b,
    accent: 0xffc72c,
    driver: {
      name: "Max Verstoppen",
      skin: SKIN.fair,
      hair: "short",
      hairColour: 0x8a6a48,
      facialHair: "light-stubble",
      earrings: false,
    },
  },
  // After Mercedes.
  {
    name: "Argento",
    key: 0xc8ccd0,
    primary: 0xbfc5cb,
    secondary: 0x16181b,
    accent: 0x00a19c,
    driver: {
      name: "Georgie Rustle",
      skin: SKIN.fair,
      hair: "swept-side-part",
      hairColour: 0x5a3a22,
      facialHair: "none",
      earrings: false,
    },
  },
  // After McLaren.
  {
    name: "Papaya",
    key: 0xff7a00,
    primary: 0xff7a00,
    secondary: 0x1e1e24,
    accent: 0x3ec3f5,
    driver: {
      name: "Lando Nourish",
      skin: SKIN.fair,
      hair: "curly-fringe",
      hairColour: 0x6b4528,
      facialHair: "none",
      earrings: false,
    },
  },
  // After Aston Martin.
  {
    name: "Verde",
    key: 0x00665a,
    primary: 0x00594f,
    secondary: 0x101614,
    accent: 0xcedc00,
    driver: {
      name: "Fernando Alonsno",
      skin: SKIN.olive,
      hair: "short-greying-temples",
      hairColour: 0x1c1c1f,
      facialHair: "stubble",
      earrings: false,
    },
  },
  // After Williams.
  {
    name: "Cobalto",
    key: 0x37bedd,
    primary: 0x0a2a5c,
    secondary: 0x37bedd,
    accent: 0xffffff,
    driver: {
      name: "Carlos Saintz",
      skin: SKIN.olive,
      hair: "swept-back",
      hairColour: 0x1c1c1f,
      facialHair: "heavy-stubble",
      earrings: false,
    },
  },
  // After Alpine.
  {
    name: "Rosa",
    key: 0xf282b4,
    primary: 0xf282b4,
    secondary: 0x0b5ab0,
    accent: 0xffffff,
    driver: {
      name: "Pierre Gassly",
      skin: SKIN.fair,
      hair: "short-curls",
      hairColour: 0x241a14,
      facialHair: "stubble",
      earrings: false,
    },
  },
  // After Audi.
  {
    name: "Titanio",
    key: 0x2a2a2e,
    primary: 0x2a2a2e,
    secondary: 0x9ea3a8,
    accent: 0xf50537,
    driver: {
      name: "Nico Hulkenbird",
      skin: SKIN.fair,
      hair: "short",
      hairColour: 0xb8955a,
      facialHair: "none",
      earrings: false,
    },
  },
];

export function livery(slot: number): Livery {
  const paint = PALETTE[slot % LIVERIES];
  if (paint === undefined) {
    throw new RangeError(`no livery for slot ${slot}`);
  }
  return paint;
}

/** An sRGB colour as CSS hex. */
export function hexCss(colour: number): string {
  return `#${colour.toString(16).padStart(6, "0")}`;
}

/** The livery's key colour as CSS, for the Room Scene's swatches and the kart labels. */
export function liveryCss(slot: number): string {
  return hexCss(livery(slot).key);
}

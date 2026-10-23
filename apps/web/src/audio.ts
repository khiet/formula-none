// The game's sound over Web Audio (ADR 0008). The files are fetched as
// the page loads and decoded the moment a gesture lets an AudioContext
// start, so the first play is never late and the browser never warns of
// autoplay. One race at a time sounds: an oscillator for the engine, a
// one-shot source per SFX and a looping source for the music, each behind
// a gain its toggle sets. Presentation: it reads readings, never state.

import hitUrl from "../../../assets/audio/hit.wav?url";
import hopUrl from "../../../assets/audio/hop.wav?url";
import itemFireUrl from "../../../assets/audio/item-fire.wav?url";
import itemPickupUrl from "../../../assets/audio/item-pickup.wav?url";
import lapUrl from "../../../assets/audio/lap.wav?url";
import miniTurboUrl from "../../../assets/audio/mini-turbo.wav?url";
import musicUrl from "../../../assets/audio/music-loop.wav?url";
import { enginePitchHz, type SoundCue, type SoundReading, soundCues } from "./audio-cues";
import type { AudioSettings } from "./audio-settings";

export const SOUND_URLS: Readonly<Record<SoundCue, string>> = {
  hop: hopUrl,
  miniTurbo: miniTurboUrl,
  itemPickup: itemPickupUrl,
  itemFire: itemFireUrl,
  hit: hitUrl,
  lap: lapUrl,
};
export const MUSIC_URL: string = musicUrl;

// The slice of Web Audio the game uses, so tests can stand a fake in for
// the browser's AudioContext, which satisfies these as it is.
export interface SoundParam {
  value: number;
  setTargetAtTime(target: number, startTime: number, timeConstant: number): unknown;
}
export interface SoundNode {
  connect(destination: SoundNode): unknown;
  disconnect(): void;
}
export interface SoundGain extends SoundNode {
  readonly gain: SoundParam;
}
export interface SoundOscillator extends SoundNode {
  type: OscillatorType;
  readonly frequency: SoundParam;
  start(when?: number): void;
  stop(when?: number): void;
}
export interface SoundBufferSource extends SoundNode {
  buffer: AudioBuffer | null;
  loop: boolean;
  start(when?: number): void;
  stop(when?: number): void;
}
export interface SoundContext {
  readonly currentTime: number;
  readonly destination: SoundNode;
  readonly state: AudioContextState;
  resume(): Promise<void>;
  decodeAudioData(bytes: ArrayBuffer): Promise<AudioBuffer>;
  createOscillator(): SoundOscillator;
  createGain(): SoundGain;
  createBufferSource(): SoundBufferSource;
}

export interface AudioOptions {
  readonly settings: AudioSettings;
  /** The speed the engine note reaches its top pitch at; the sim's top speed. */
  readonly topSpeedMetresPerSecond: number;
  /** Fetch one sound file's bytes; called for every file at creation. */
  fetchBytes(url: string): Promise<ArrayBuffer>;
  /** Make the AudioContext; called once, inside the first user gesture. */
  createContext(): SoundContext;
}

export interface Audio {
  /** Apply the toggles, at once to whatever is sounding. */
  settings(next: AudioSettings): void;
  /**
   * Call inside a user gesture: the first call makes the context and
   * decodes the fetched files, and a race already under way starts to
   * sound; a later call resumes a context the browser has suspended.
   */
  unlock(): Promise<void>;
  /** The sound of one race; stop the last before starting the next. */
  race(): RaceAudio;
}

export interface RaceAudio {
  /** The engine idling and the music looping, as the toggles allow, until `stop`. */
  start(): void;
  /** Follow the own kart one frame on: the engine's pitch from its speed, and the SFX its changes call for. */
  update(reading: SoundReading): void;
  /** Engine off, music stopped; later updates are silent. */
  stop(): void;
}

/** Each toggle's level under the mix: the SFX read over the engine, and the music sits under both. */
const LEVELS: Readonly<Record<keyof AudioSettings, number>> = {
  engine: 0.12,
  sfx: 0.8,
  music: 0.35,
};
const TOGGLES = Object.keys(LEVELS) as (keyof AudioSettings)[];
/** The engine note glides to each frame's pitch over this long, so it never clicks. */
const ENGINE_GLIDE_S = 0.04;

/** The decoded files, once the context exists to decode them, and the gain each toggle sets. */
interface Sounds {
  readonly context: SoundContext;
  readonly sfx: Partial<Record<SoundCue, AudioBuffer>>;
  readonly music: AudioBuffer | undefined;
  readonly gains: Readonly<Record<keyof AudioSettings, SoundGain>>;
}

export function createAudio(options: AudioOptions): Audio {
  let settings = options.settings;
  // Fetched at once; decoded on unlock, which needs the context.
  const fetched = new Map<string, Promise<ArrayBuffer | undefined>>();
  const fetching = (url: string) =>
    options.fetchBytes(url).catch((error: unknown) => {
      console.warn(`could not load the sound ${url}`, error);
      return undefined;
    });
  for (const url of [...Object.values(SOUND_URLS), MUSIC_URL]) {
    fetched.set(url, fetching(url));
  }

  let sounds: Sounds | undefined;
  let unlocking: Promise<void> | undefined;
  /** The race sounding now, if any, told when the context arrives. */
  let live: LiveRace | undefined;

  const decode = async (context: SoundContext, url: string): Promise<AudioBuffer | undefined> => {
    const bytes = await fetched.get(url);
    if (bytes === undefined) return undefined;
    try {
      return await context.decodeAudioData(bytes);
    } catch (error) {
      console.warn(`could not decode the sound ${url}`, error);
      return undefined;
    }
  };

  const gainOf = (context: SoundContext, toggle: keyof AudioSettings): SoundGain => {
    const gain = context.createGain();
    gain.gain.value = settings[toggle] ? LEVELS[toggle] : 0;
    gain.connect(context.destination);
    return gain;
  };

  const makeSounds = async (): Promise<void> => {
    const context = options.createContext();
    await context.resume();
    const cues = Object.keys(SOUND_URLS) as SoundCue[];
    const [music, ...buffers] = await Promise.all([
      decode(context, MUSIC_URL),
      ...cues.map((cue) => decode(context, SOUND_URLS[cue])),
    ]);
    const sfx: Partial<Record<SoundCue, AudioBuffer>> = {};
    cues.forEach((cue, index) => {
      const buffer = buffers[index];
      if (buffer !== undefined) sfx[cue] = buffer;
    });
    sounds = {
      context,
      sfx,
      music,
      gains: {
        engine: gainOf(context, "engine"),
        sfx: gainOf(context, "sfx"),
        music: gainOf(context, "music"),
      },
    };
    live?.begin(sounds);
  };

  return {
    settings(next) {
      settings = next;
      if (sounds === undefined) return;
      for (const toggle of TOGGLES) {
        sounds.gains[toggle].gain.value = next[toggle] ? LEVELS[toggle] : 0;
      }
      live?.music(next.music);
    },
    unlock() {
      if (unlocking === undefined) {
        // A context that could not be made is tried again on the next gesture.
        unlocking = makeSounds().catch((error: unknown) => {
          unlocking = undefined;
          throw error;
        });
        return unlocking;
      }
      return unlocking.then(() => {
        if (sounds !== undefined && sounds.context.state !== "running") {
          return sounds.context.resume();
        }
      });
    },
    race() {
      const race = new LiveRace(options.topSpeedMetresPerSecond, () => settings);
      return {
        start() {
          live?.stop();
          live = race;
          race.racing = true;
          if (sounds !== undefined) race.begin(sounds);
        },
        update: (reading) => race.update(reading),
        stop() {
          race.stop();
          if (live === race) live = undefined;
        },
      };
    },
  };
}

/**
 * One race's nodes: made when both the race has started and the context
 * exists, whichever comes second, and torn down at the race's stop.
 */
class LiveRace {
  racing = false;
  #sounds: Sounds | undefined;
  #engine: SoundOscillator | undefined;
  #music: SoundBufferSource | undefined;
  #last: SoundReading | undefined;
  readonly #topSpeed: number;
  readonly #settings: () => AudioSettings;

  constructor(topSpeed: number, settings: () => AudioSettings) {
    this.#topSpeed = topSpeed;
    this.#settings = settings;
  }

  begin(sounds: Sounds): void {
    if (!this.racing || this.#sounds !== undefined) return;
    this.#sounds = sounds;
    const engine = sounds.context.createOscillator();
    engine.type = "sawtooth";
    engine.frequency.value = enginePitchHz(this.#last?.speed ?? 0, this.#topSpeed);
    engine.connect(sounds.gains.engine);
    engine.start();
    this.#engine = engine;
    this.music(this.#settings().music);
  }

  /** Loop the music while `on`; a loop is started where none plays and stopped where one does. */
  music(on: boolean): void {
    const sounds = this.#sounds;
    if (sounds === undefined) return;
    if (on && this.racing && this.#music === undefined && sounds.music !== undefined) {
      const loop = sounds.context.createBufferSource();
      loop.buffer = sounds.music;
      loop.loop = true;
      loop.connect(sounds.gains.music);
      loop.start();
      this.#music = loop;
    } else if (!on && this.#music !== undefined) {
      this.#music.stop();
      this.#music.disconnect();
      this.#music = undefined;
    }
  }

  update(reading: SoundReading): void {
    if (!this.racing) return;
    const cues = soundCues(this.#last, reading);
    this.#last = reading;
    const sounds = this.#sounds;
    if (sounds === undefined) return;
    this.#engine?.frequency.setTargetAtTime(
      enginePitchHz(reading.speed, this.#topSpeed),
      sounds.context.currentTime,
      ENGINE_GLIDE_S,
    );
    if (!this.#settings().sfx) return;
    for (const cue of cues) {
      const buffer = sounds.sfx[cue];
      if (buffer === undefined) continue;
      const shot = sounds.context.createBufferSource();
      shot.buffer = buffer;
      shot.connect(sounds.gains.sfx);
      shot.start();
    }
  }

  stop(): void {
    this.racing = false;
    this.music(false);
    if (this.#engine !== undefined) {
      this.#engine.stop();
      this.#engine.disconnect();
      this.#engine = undefined;
    }
    this.#sounds = undefined;
  }
}

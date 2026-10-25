// The game's sound over Web Audio (ADR 0008). The files are fetched at
// creation and decoded as soon as a gesture lets an AudioContext start, so
// no decode waits for a first play and the browser never warns of autoplay.
// One race sounds at a time: an oscillator for the engine, a one-shot
// source per SFX (the sputter's file runs the stall's length), a looping
// source per loop while its state holds, pitched and swelled with the
// kart's speed, and a looping source for the music, each behind the gain
// its toggle sets; the loops sit under the SFX toggle. Presentation: it
// reads the own kart's readings and changes no sim state.

import boostPadUrl from "../../../assets/audio/boost-pad.wav?url";
import countdownBeepUrl from "../../../assets/audio/countdown-beep.wav?url";
import driftSkidUrl from "../../../assets/audio/drift-skid.wav?url";
import finishUrl from "../../../assets/audio/finish.wav?url";
import goUrl from "../../../assets/audio/go.wav?url";
import grassUrl from "../../../assets/audio/grass.wav?url";
import hitUrl from "../../../assets/audio/hit.wav?url";
import hopUrl from "../../../assets/audio/hop.wav?url";
import itemFireUrl from "../../../assets/audio/item-fire.wav?url";
import itemPickupUrl from "../../../assets/audio/item-pickup.wav?url";
import lapUrl from "../../../assets/audio/lap.wav?url";
import miniTurboUrl from "../../../assets/audio/mini-turbo.wav?url";
import musicUrl from "../../../assets/audio/music-loop.wav?url";
import sandUrl from "../../../assets/audio/sand.wav?url";
import sputterUrl from "../../../assets/audio/sputter.wav?url";
import tierChimeUrl from "../../../assets/audio/tier-chime.wav?url";
import wrongWayUrl from "../../../assets/audio/wrong-way.wav?url";
import {
  enginePitchHz,
  loopPlayback,
  type SoundCue,
  type SoundLoop,
  type SoundReading,
  soundCues,
  soundLoops,
} from "./audio-cues";
import type { AudioSettings } from "./audio-settings";

export const SOUND_URLS: Readonly<Record<SoundCue, string>> = {
  countdownBeep: countdownBeepUrl,
  go: goUrl,
  hop: hopUrl,
  miniTurbo: miniTurboUrl,
  itemPickup: itemPickupUrl,
  itemFire: itemFireUrl,
  hit: hitUrl,
  sputter: sputterUrl,
  lap: lapUrl,
  tierChime: tierChimeUrl,
  boostPad: boostPadUrl,
  wrongWay: wrongWayUrl,
  finish: finishUrl,
};
export const LOOP_URLS: Readonly<Record<SoundLoop, string>> = {
  driftSkid: driftSkidUrl,
  grass: grassUrl,
  sand: sandUrl,
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
  readonly playbackRate: SoundParam;
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
  /** The sim's top speed, which the engine note's pitch is scaled to. */
  readonly topSpeedMetresPerSecond: number;
  /** Fetch one sound file's bytes; called for every file at creation. */
  fetchBytes(url: string): Promise<ArrayBuffer>;
  /** Make the AudioContext; called by `unlock` inside a user gesture, again only if it threw. */
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
  /** The sound of one race; starting it stops the race sounding before. */
  race(): RaceAudio;
}

export interface RaceAudio {
  /** The engine idling and the music looping, as the toggles allow, until `stop`. */
  start(): void;
  /** The own kart's reading this frame: the engine's pitch and the loops from its state, the SFX from its changes since the last. */
  update(reading: SoundReading): void;
  /** Engine off, music stopped; later updates are silent. */
  stop(): void;
}

/** Each toggle's gain in the mix; the SFX read over the music, and the engine's constant note sits lowest. */
const LEVELS: Readonly<Record<keyof AudioSettings, number>> = {
  engine: 0.12,
  sfx: 0.8,
  music: 0.35,
};
const TOGGLES = Object.keys(LEVELS) as (keyof AudioSettings)[];
/** The time constant the engine note and the loops glide to each frame's pitch and level with, so neither jumps. */
const ENGINE_GLIDE_S = 0.04;

/** The decoded files, once the context exists to decode them, and the gain each toggle sets. */
interface Sounds {
  readonly context: SoundContext;
  readonly sfx: Partial<Record<SoundCue, AudioBuffer>>;
  readonly loops: Partial<Record<SoundLoop, AudioBuffer>>;
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
  for (const url of [...Object.values(SOUND_URLS), ...Object.values(LOOP_URLS), MUSIC_URL]) {
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
    const loopNames = Object.keys(LOOP_URLS) as SoundLoop[];
    const [music, ...buffers] = await Promise.all([
      decode(context, MUSIC_URL),
      ...cues.map((cue) => decode(context, SOUND_URLS[cue])),
      ...loopNames.map((loop) => decode(context, LOOP_URLS[loop])),
    ]);
    const sfx: Partial<Record<SoundCue, AudioBuffer>> = {};
    cues.forEach((cue, index) => {
      const buffer = buffers[index];
      if (buffer !== undefined) sfx[cue] = buffer;
    });
    const loops: Partial<Record<SoundLoop, AudioBuffer>> = {};
    loopNames.forEach((loop, index) => {
      const buffer = buffers[cues.length + index];
      if (buffer !== undefined) loops[loop] = buffer;
    });
    sounds = {
      context,
      sfx,
      loops,
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
  /** The loops running now, each through a gain of its own under the SFX gain. */
  readonly #loops = new Map<SoundLoop, { source: SoundBufferSource; gain: SoundGain }>();
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
    engine.frequency.value = enginePitchHz(
      this.#last?.speed ?? 0,
      this.#topSpeed,
      this.#last?.charge ?? null,
    );
    engine.connect(sounds.gains.engine);
    engine.start();
    this.#engine = engine;
    this.music(this.#settings().music);
  }

  /** Start the music loop when `on` and none plays; stop it when not `on`. */
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
      enginePitchHz(reading.speed, this.#topSpeed, reading.charge),
      sounds.context.currentTime,
      ENGINE_GLIDE_S,
    );
    const sfx = this.#settings().sfx;
    this.#runLoops(sounds, sfx ? soundLoops(reading) : new Set(), reading.speed);
    if (!sfx) return;
    for (const cue of cues) {
      const buffer = sounds.sfx[cue];
      if (buffer === undefined) continue;
      const shot = sounds.context.createBufferSource();
      shot.buffer = buffer;
      shot.connect(sounds.gains.sfx);
      shot.start();
    }
  }

  /** Stop the loops not in `wanted`, start the ones not running, and glide every one to `speed`. */
  #runLoops(sounds: Sounds, wanted: ReadonlySet<SoundLoop>, speed: number): void {
    for (const [loop, running] of this.#loops) {
      if (wanted.has(loop)) continue;
      running.source.stop();
      running.source.disconnect();
      running.gain.disconnect();
      this.#loops.delete(loop);
    }
    const now = sounds.context.currentTime;
    for (const loop of wanted) {
      const playback = loopPlayback(loop, speed, this.#topSpeed);
      const running = this.#loops.get(loop);
      if (running !== undefined) {
        running.source.playbackRate.setTargetAtTime(playback.rate, now, ENGINE_GLIDE_S);
        running.gain.gain.setTargetAtTime(playback.gain, now, ENGINE_GLIDE_S);
        continue;
      }
      const buffer = sounds.loops[loop];
      if (buffer === undefined) continue;
      const gain = sounds.context.createGain();
      gain.gain.value = playback.gain;
      gain.connect(sounds.gains.sfx);
      const source = sounds.context.createBufferSource();
      source.buffer = buffer;
      source.loop = true;
      source.playbackRate.value = playback.rate;
      source.connect(gain);
      source.start();
      this.#loops.set(loop, { source, gain });
    }
  }

  stop(): void {
    this.racing = false;
    this.music(false);
    if (this.#sounds !== undefined) this.#runLoops(this.#sounds, new Set(), 0);
    if (this.#engine !== undefined) {
      this.#engine.stop();
      this.#engine.disconnect();
      this.#engine = undefined;
    }
    this.#sounds = undefined;
  }
}

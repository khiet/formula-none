// The game's sound over Web Audio (ADR 0025). The files are fetched at
// creation and decoded as soon as a gesture lets an AudioContext start, so
// no decode waits for a first play and the browser never warns of autoplay.
// One race sounds at a time: the engine's graph, voiced from the gears and
// load `audio-engine.ts` derives from speed, a one-shot source per SFX (the
// sputter's file runs the stall's length), a looping source per loop while
// its state holds, pitched and swelled with the kart's speed, and a looping
// source for the music, each behind the gain its toggle sets; the loops sit
// under the SFX toggle, and so does the star music (ADR 0075): the own run's
// in place of the race music, which fades out and keeps its place, else the
// nearest other star kart's by distance. Presentation: it reads the own
// kart's readings and changes no sim state.

import airHornUrl from "../../../assets/audio/air-horn.wav?url";
import boostPadUrl from "../../../assets/audio/boost-pad.wav?url";
import bootsUrl from "../../../assets/audio/boots.wav?url";
import boxChimeUrl from "../../../assets/audio/box-chime.wav?url";
import coinLossUrl from "../../../assets/audio/coin-loss.wav?url";
import coinPickupUrl from "../../../assets/audio/coin-pickup.wav?url";
import countdownBeepUrl from "../../../assets/audio/countdown-beep.wav?url";
import driftSkidUrl from "../../../assets/audio/drift-skid.wav?url";
import droneBeepUrl from "../../../assets/audio/drone-beep.wav?url";
import finishUrl from "../../../assets/audio/finish.wav?url";
import goUrl from "../../../assets/audio/go.wav?url";
import grassUrl from "../../../assets/audio/grass.wav?url";
import gunStallUrl from "../../../assets/audio/gun-stall.wav?url";
import hitUrl from "../../../assets/audio/hit.wav?url";
import hopUrl from "../../../assets/audio/hop.wav?url";
import itemFireUrl from "../../../assets/audio/item-fire.wav?url";
import itemPickupUrl from "../../../assets/audio/item-pickup.wav?url";
import itemRollTickUrl from "../../../assets/audio/item-roll-tick.wav?url";
import itemRolledUrl from "../../../assets/audio/item-rolled.wav?url";
import jackDropUrl from "../../../assets/audio/jack-drop.wav?url";
import jackFailUrl from "../../../assets/audio/jack-fail.wav?url";
import jackLiftUrl from "../../../assets/audio/jack-lift.wav?url";
import lapUrl from "../../../assets/audio/lap.wav?url";
import lollipopSpinUrl from "../../../assets/audio/lollipop-spin.wav?url";
import miniTurboUrl from "../../../assets/audio/mini-turbo.wav?url";
import musicUrl from "../../../assets/audio/music-loop.wav?url";
import releaseChimeUrl from "../../../assets/audio/release-chime.wav?url";
import sandUrl from "../../../assets/audio/sand.wav?url";
import slipstreamUrl from "../../../assets/audio/slipstream.wav?url";
import sputterUrl from "../../../assets/audio/sputter.wav?url";
import starEndUrl from "../../../assets/audio/star-end.wav?url";
import starFireUrl from "../../../assets/audio/star-fire.wav?url";
import starMusicUrl from "../../../assets/audio/star-music.wav?url";
import tierChimeUrl from "../../../assets/audio/tier-chime.wav?url";
import wheelGunUrl from "../../../assets/audio/wheel-gun.wav?url";
import wheelGunActUrl from "../../../assets/audio/wheel-gun-act.wav?url";
import wheelGunOnUrl from "../../../assets/audio/wheel-gun-on.wav?url";
import wheelGunRedoUrl from "../../../assets/audio/wheel-gun-redo.wav?url";
import wheelOffUrl from "../../../assets/audio/wheel-off.wav?url";
import wheelOnUrl from "../../../assets/audio/wheel-on.wav?url";
import wrongWayUrl from "../../../assets/audio/wrong-way.wav?url";
import {
  type DroneBeep,
  distanceLevel,
  droneBeep,
  heardOnward,
  hornBlasts,
  loopPlayback,
  SOUND_AWAY_LOWPASS_HZ,
  type SoundCue,
  type SoundLoop,
  type SoundReading,
  STAR_RESTART_RISE_S,
  type StarMusic,
  soundCues,
  soundLoops,
  starMusic,
} from "./audio-cues";
import {
  ENGINE,
  type EngineState,
  type EngineVoice,
  engineStep,
  engineVoice,
} from "./audio-engine";
import type { AudioSettings } from "./audio-settings";
import type { Tuning } from "./bridge";

export const SOUND_URLS: Readonly<Record<SoundCue, string>> = {
  countdownBeep: countdownBeepUrl,
  go: goUrl,
  hop: hopUrl,
  miniTurbo: miniTurboUrl,
  itemPickup: itemPickupUrl,
  itemRollTick: itemRollTickUrl,
  itemRolled: itemRolledUrl,
  coinPickup: coinPickupUrl,
  coinLoss: coinLossUrl,
  itemFire: itemFireUrl,
  hit: hitUrl,
  sputter: sputterUrl,
  lap: lapUrl,
  tierChime: tierChimeUrl,
  boostPad: boostPadUrl,
  wrongWay: wrongWayUrl,
  finish: finishUrl,
  starFire: starFireUrl,
  starEnd: starEndUrl,
  airHorn: airHornUrl,
  droneBeep: droneBeepUrl,
  slipstream: slipstreamUrl,
  boxChime: boxChimeUrl,
  boots: bootsUrl,
  jackLift: jackLiftUrl,
  wheelGun: wheelGunUrl,
  wheelOff: wheelOffUrl,
  wheelOn: wheelOnUrl,
  wheelGunOn: wheelGunOnUrl,
  lollipopSpin: lollipopSpinUrl,
  jackDrop: jackDropUrl,
  releaseChime: releaseChimeUrl,
  gunStall: gunStallUrl,
  jackFail: jackFailUrl,
  wheelGunAct: wheelGunActUrl,
  wheelGunRedo: wheelGunRedoUrl,
};
export const LOOP_URLS: Readonly<Record<SoundLoop, string>> = {
  driftSkid: driftSkidUrl,
  grass: grassUrl,
  sand: sandUrl,
};
export const MUSIC_URL: string = musicUrl;
/** The star music, as long as a run, in place of the race music through the own run. */
export const STAR_MUSIC_URL: string = starMusicUrl;

// The slice of Web Audio the game uses, so tests can stand a fake in for
// the browser's AudioContext, which satisfies these as it is.
export interface SoundParam {
  value: number;
  setValueAtTime(value: number, startTime: number): unknown;
  setTargetAtTime(target: number, startTime: number, timeConstant: number): unknown;
}
export interface SoundNode {
  /** A node, or a param the node's signal is added to, as the limiter's chop is to the engine's level. */
  connect(destination: SoundNode | SoundParam): unknown;
  disconnect(): void;
}
export interface SoundGain extends SoundNode {
  readonly gain: SoundParam;
}
export interface SoundOscillator extends SoundNode {
  type: OscillatorType;
  readonly frequency: SoundParam;
  readonly detune: SoundParam;
  start(when?: number): void;
  stop(when?: number): void;
}
export interface SoundFilter extends SoundNode {
  type: BiquadFilterType;
  readonly frequency: SoundParam;
  readonly Q: SoundParam;
}
/** A decoded file, or a buffer the game filled itself, as the engine's noise is. */
export interface SoundBuffer {
  getChannelData(channel: number): Float32Array;
}
export interface SoundBufferSource extends SoundNode {
  buffer: SoundBuffer | null;
  loop: boolean;
  readonly playbackRate: SoundParam;
  /** `offset` seconds into the buffer, as the star music is joined mid-run. */
  start(when?: number, offset?: number): void;
  stop(when?: number): void;
}
export interface SoundContext {
  readonly currentTime: number;
  readonly sampleRate: number;
  readonly destination: SoundNode;
  readonly state: AudioContextState;
  resume(): Promise<void>;
  decodeAudioData(bytes: ArrayBuffer): Promise<SoundBuffer>;
  createBuffer(channels: number, length: number, sampleRate: number): SoundBuffer;
  createOscillator(): SoundOscillator;
  createGain(): SoundGain;
  createBiquadFilter(): SoundFilter;
  createBufferSource(): SoundBufferSource;
}

export interface AudioOptions {
  readonly settings: AudioSettings;
  /**
   * The sim's top speed, which the engine's gear bands and the loops' swell
   * are scaled to, and how long a star's run lasts, which the star music is
   * rendered to and joined against.
   */
  readonly tuning: Pick<Tuning, "topSpeedMPerS" | "starS">;
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
  /** The own kart's reading this frame: the engine's gear, revs and load and the loops from its state, the SFX from its changes since the last. */
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
/** The time constant the loops glide to each frame's pitch and level with, so neither jumps. */
const GLIDE_S = 0.04;
/** The time constant the race music and the star music fade with: about 0.2 s to settle. */
const MUSIC_FADE_S = 0.07;
/** How long after its fade begins a star music source is stopped. */
const MUSIC_FADE_STOP_S = 0.3;

/** The decoded files, once the context exists to decode them, and the gain each toggle sets. */
interface Sounds {
  readonly context: SoundContext;
  readonly sfx: Partial<Record<SoundCue, SoundBuffer>>;
  readonly loops: Partial<Record<SoundLoop, SoundBuffer>>;
  readonly music: SoundBuffer | undefined;
  readonly starMusic: SoundBuffer | undefined;
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
  for (const url of [
    ...Object.values(SOUND_URLS),
    ...Object.values(LOOP_URLS),
    MUSIC_URL,
    STAR_MUSIC_URL,
  ]) {
    fetched.set(url, fetching(url));
  }

  let sounds: Sounds | undefined;
  let unlocking: Promise<void> | undefined;
  /** The race sounding now, if any, told when the context arrives. */
  let live: LiveRace | undefined;

  const decode = async (context: SoundContext, url: string): Promise<SoundBuffer | undefined> => {
    const bytes = await fetched.get(url);
    if (bytes === undefined) return undefined;
    try {
      return await context.decodeAudioData(bytes);
    } catch (error) {
      console.warn(`could not decode the sound ${url}`, error);
      return undefined;
    }
  };

  /** Decode every file of `urls`, keeping the ones that decoded under their names. */
  const decodeAll = async <Name extends string>(
    context: SoundContext,
    urls: Readonly<Record<Name, string>>,
  ): Promise<Partial<Record<Name, SoundBuffer>>> => {
    const names = Object.keys(urls) as Name[];
    const buffers = await Promise.all(names.map((name) => decode(context, urls[name])));
    const decoded: Partial<Record<Name, SoundBuffer>> = {};
    names.forEach((name, index) => {
      const buffer = buffers[index];
      if (buffer !== undefined) decoded[name] = buffer;
    });
    return decoded;
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
    const [music, starMusic, sfx, loops] = await Promise.all([
      decode(context, MUSIC_URL),
      decode(context, STAR_MUSIC_URL),
      decodeAll(context, SOUND_URLS),
      decodeAll(context, LOOP_URLS),
    ]);
    sounds = {
      context,
      sfx,
      loops,
      music,
      starMusic,
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
      const race = new LiveRace(options.tuning.topSpeedMPerS, options.tuning.starS, () => settings);
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

/** A loop sounding now: its source through a gain of its own under the SFX gain. */
interface RunningLoop {
  readonly source: SoundBufferSource;
  readonly gain: SoundGain;
}

/** The star music sounding now: whose run, and its source through a gain of its own under the SFX gain. */
interface RunningStar {
  /** Null for the own run, else the other kart's grid slot. */
  readonly kart: number | null;
  readonly source: SoundBufferSource;
  readonly gain: SoundGain;
  /** How far into the run the music was last asked for, so a second star's jump back is seen. */
  secondsIn: number;
}

/**
 * One race's nodes: made when both the race has started and the context
 * exists, whichever comes second, and torn down at the race's stop.
 */
class LiveRace {
  racing = false;
  #sounds: Sounds | undefined;
  #engine: EngineGraph | undefined;
  /** The engine model, stepped from the first reading the context hears. */
  #engineState: EngineState | undefined;
  #music: SoundBufferSource | undefined;
  /**
   * The race music's own level, between its source and its toggle's gain,
   * which `settings` overwrites: silent through the own run, up otherwise.
   */
  #musicLevel: SoundGain | undefined;
  #star: RunningStar | undefined;
  /** The context's time the drone warning's next beep is due; 0 while no warning runs, so the first beep is at once. */
  #nextBeepAt = 0;
  readonly #loops = new Map<SoundLoop, RunningLoop>();
  #last: SoundReading | undefined;
  readonly #topSpeed: number;
  readonly #starRunSeconds: number;
  readonly #settings: () => AudioSettings;

  constructor(topSpeed: number, starRunSeconds: number, settings: () => AudioSettings) {
    this.#topSpeed = topSpeed;
    this.#starRunSeconds = starRunSeconds;
    this.#settings = settings;
  }

  begin(sounds: Sounds): void {
    if (!this.racing || this.#sounds !== undefined) return;
    this.#sounds = sounds;
    this.#engineState = this.#stepEngine(sounds, this.#last);
    this.#engine = new EngineGraph(
      sounds.context,
      sounds.gains.engine,
      engineVoice(this.#engineState),
    );
    this.music(this.#settings().music);
  }

  /** The engine model after `reading`, or idling at a standstill with none, at the context's clock. */
  #stepEngine(sounds: Sounds, reading: SoundReading | undefined): EngineState {
    return engineStep(
      this.#engineState,
      {
        speed: reading?.speed ?? 0,
        charge: reading?.charge ?? null,
        time: sounds.context.currentTime,
      },
      this.#topSpeed,
    );
  }

  /**
   * Start the music loop when `on` and none plays, through its own level,
   * silent while the own kart's star music plays; stop it when not `on`.
   */
  music(on: boolean): void {
    const sounds = this.#sounds;
    if (sounds === undefined) return;
    if (on && this.racing && this.#music === undefined && sounds.music !== undefined) {
      const level = sounds.context.createGain();
      level.gain.value = this.#star?.kart === null ? 0 : 1;
      level.connect(sounds.gains.music);
      const loop = sounds.context.createBufferSource();
      loop.buffer = sounds.music;
      loop.loop = true;
      loop.connect(level);
      loop.start();
      this.#music = loop;
      this.#musicLevel = level;
    } else if (!on && this.#music !== undefined) {
      this.#music.stop();
      this.#music.disconnect();
      this.#musicLevel?.disconnect();
      this.#music = undefined;
      this.#musicLevel = undefined;
    }
  }

  update(reading: SoundReading): void {
    if (!this.racing) return;
    const before = this.#last;
    const cues = soundCues(before, reading);
    this.#last = heardOnward(before, reading);
    const sounds = this.#sounds;
    if (sounds === undefined) return;
    this.#engineState = this.#stepEngine(sounds, reading);
    this.#engine?.follow(engineVoice(this.#engineState), this.#engineState.shift);
    const sfx = this.#settings().sfx;
    this.#runLoops(sounds, sfx ? soundLoops(reading) : new Set(), reading.speed);
    this.#runStar(sounds, sfx ? starMusic(reading, this.#starRunSeconds) : null, cues);
    if (!sfx) return;
    for (const cue of cues) {
      const buffer = sounds.sfx[cue];
      if (buffer === undefined) continue;
      const shot = sounds.context.createBufferSource();
      shot.buffer = buffer;
      shot.connect(sounds.gains.sfx);
      shot.start();
    }
    for (const blast of hornBlasts(before, reading)) {
      this.#playBlast(sounds, distanceLevel(blast.metres));
    }
    this.#runWarning(sounds, droneBeep(reading.incomingDroneMetres));
  }

  /** Play another kart's blast once, at `level` through the away lowpass; nothing out of earshot. */
  #playBlast(sounds: Sounds, level: number): void {
    const buffer = sounds.sfx.airHorn;
    if (buffer === undefined || level <= 0) return;
    const gain = sounds.context.createGain();
    gain.gain.value = level;
    gain.connect(sounds.gains.sfx);
    const filter = sounds.context.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = SOUND_AWAY_LOWPASS_HZ;
    filter.connect(gain);
    const shot = sounds.context.createBufferSource();
    shot.buffer = buffer;
    shot.connect(filter);
    shot.start();
  }

  /**
   * Beep as `beep` asks while a drone warning runs: at once as it starts,
   * then each time the gap the last beep set has passed, at the rate the
   * frame's distance gives; nothing once it is over, and at once again
   * for the next.
   */
  #runWarning(sounds: Sounds, beep: DroneBeep | null): void {
    if (beep === null) {
      this.#nextBeepAt = 0;
      return;
    }
    const now = sounds.context.currentTime;
    if (now < this.#nextBeepAt) return;
    this.#nextBeepAt = now + beep.gapS;
    const buffer = sounds.sfx.droneBeep;
    if (buffer === undefined) return;
    const shot = sounds.context.createBufferSource();
    shot.buffer = buffer;
    shot.playbackRate.value = beep.rate;
    shot.connect(sounds.gains.sfx);
    shot.start();
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
        running.source.playbackRate.setTargetAtTime(playback.rate, now, GLIDE_S);
        running.gain.gain.setTargetAtTime(playback.gain, now, GLIDE_S);
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

  /**
   * Play the star music `wanted` calls for: the own run's from the fire,
   * restarted on a second star's fire and otherwise left alone through a
   * correction, with the race music faded to silent and back at the end;
   * or another kart's, joined at its point in the run and faded by
   * distance, the race music untouched. Fading out what no longer plays.
   */
  #runStar(sounds: Sounds, wanted: StarMusic | null, cues: readonly SoundCue[]): void {
    const now = sounds.context.currentTime;
    const running = this.#star;
    // A second star puts the run back to its start: the own kart's is its
    // fire cue, another's its point in the run jumping back by more than a
    // correction could move it.
    const restart =
      wanted !== null &&
      (running === undefined ||
        running.kart !== wanted.kart ||
        (wanted.kart === null && cues.includes("starFire")) ||
        wanted.secondsIn + STAR_RESTART_RISE_S < running.secondsIn);
    if (running !== undefined && (wanted === null || restart)) {
      running.gain.gain.setTargetAtTime(0, now, MUSIC_FADE_S);
      running.source.stop(now + MUSIC_FADE_STOP_S);
      this.#star = undefined;
    }
    if (wanted !== null && restart && sounds.starMusic !== undefined) {
      const gain = sounds.context.createGain();
      gain.gain.value = wanted.level;
      gain.connect(sounds.gains.sfx);
      const source = sounds.context.createBufferSource();
      source.buffer = sounds.starMusic;
      if (wanted.lowpassHz === null) {
        source.connect(gain);
      } else {
        const filter = sounds.context.createBiquadFilter();
        filter.type = "lowpass";
        filter.frequency.value = wanted.lowpassHz;
        filter.connect(gain);
        source.connect(filter);
      }
      source.start(now, wanted.secondsIn);
      this.#star = { kart: wanted.kart, source, gain, secondsIn: wanted.secondsIn };
    } else if (wanted !== null && this.#star !== undefined) {
      this.#star.gain.gain.setTargetAtTime(wanted.level, now, GLIDE_S);
      this.#star.secondsIn = wanted.secondsIn;
    }
    // The race music keeps its place while silent, so it comes back
    // further along, not from the start.
    this.#musicLevel?.gain.setTargetAtTime(wanted?.kart === null ? 0 : 1, now, MUSIC_FADE_S);
  }

  stop(): void {
    this.racing = false;
    this.#nextBeepAt = 0;
    this.music(false);
    if (this.#star !== undefined) {
      this.#star.source.stop();
      this.#star.source.disconnect();
      this.#star.gain.disconnect();
      this.#star = undefined;
    }
    if (this.#sounds !== undefined) this.#runLoops(this.#sounds, new Set(), 0);
    this.#engine?.stop();
    this.#engine = undefined;
    this.#engineState = undefined;
    this.#sounds = undefined;
  }
}

/** The time constant the engine's note, filter and level glide with each frame. */
const ENGINE_GLIDE_S = 0.02;
/** The upshift's cut falls this fast, so it reads as a cut and not a dip. */
const CUT_GLIDE_S = 0.003;
/** How long a downshift's blip holds the note up before it settles. */
const BLIP_S = 0.08;
/** The noise the intake and the crackle draw on, looped; long enough that the loop is not heard. */
const NOISE_S = 2;
/** The sawtooths' gain each, under which the sub and the intake sit. */
const SAW_GAIN = 0.5;
const FILTER_Q = 1.2;
const INTAKE_Q = 0.8;
/** A crackle burst's shape: full at once, gone within a few milliseconds. */
const CRACKLE_RISE_S = 0.004;
const CRACKLE_FALL_S = 0.006;
/** The bursts come no closer than this and, on a light overrun, up to a quarter second apart. */
const CRACKLE_GAP_MIN_S = 0.03;
const CRACKLE_GAP_SPREAD_S = 0.25;
const CRACKLE_GAP_OVERRUN_S = 0.2;

/**
 * The engine's nodes (ADR 0025): two detuned sawtooths, a square sub an
 * octave down and the intake noise through one lowpass that opens with the
 * revs and the load, into a level gain the limiter's square chops; the same
 * noise feeds the crackle bursts past the filter. Made as the race begins
 * to sound and torn down at its stop; `follow` glides it to each frame's
 * voice.
 */
class EngineGraph {
  readonly #context: SoundContext;
  readonly #oscillators: SoundOscillator[] = [];
  readonly #saws: SoundOscillator[];
  readonly #sub: SoundOscillator;
  readonly #noise: SoundBufferSource;
  readonly #intakeFilter: SoundFilter;
  readonly #intakeGain: SoundGain;
  readonly #filter: SoundFilter;
  readonly #level: SoundGain;
  readonly #limiterDepth: SoundGain;
  readonly #crackle: SoundGain;
  readonly #nodes: SoundNode[] = [];
  /** The level is left alone while an upshift's cut runs, and the note while a downshift's blip holds. */
  #cutUntil = 0;
  #blipUntil = 0;
  #nextCrackle = 0;

  constructor(context: SoundContext, out: SoundGain, voice: EngineVoice) {
    this.#context = context;
    const gain = (value: number, to: SoundNode | SoundParam): SoundGain => {
      const node = context.createGain();
      node.gain.value = value;
      node.connect(to);
      this.#nodes.push(node);
      return node;
    };
    const oscillator = (type: OscillatorType, hz: number, to: SoundNode | SoundParam) => {
      const node = context.createOscillator();
      node.type = type;
      node.frequency.value = hz;
      node.connect(to);
      this.#oscillators.push(node);
      return node;
    };
    this.#level = gain(voice.level, out);
    this.#filter = context.createBiquadFilter();
    this.#filter.type = "lowpass";
    this.#filter.Q.value = FILTER_Q;
    this.#filter.frequency.value = voice.filterHz;
    this.#filter.connect(this.#level);
    this.#nodes.push(this.#filter);
    this.#saws = [-ENGINE.detuneCents / 2, ENGINE.detuneCents / 2].map((cents) => {
      const saw = oscillator("sawtooth", voice.noteHz, gain(SAW_GAIN, this.#filter));
      saw.detune.value = cents;
      return saw;
    });
    this.#sub = oscillator("square", voice.subHz, gain(ENGINE.sub, this.#filter));
    this.#intakeGain = gain(voice.intake, this.#filter);
    this.#intakeFilter = context.createBiquadFilter();
    this.#intakeFilter.type = "bandpass";
    this.#intakeFilter.Q.value = INTAKE_Q;
    this.#intakeFilter.frequency.value = voice.intakeHz;
    this.#intakeFilter.connect(this.#intakeGain);
    this.#nodes.push(this.#intakeFilter);
    this.#crackle = gain(0, out);
    this.#noise = context.createBufferSource();
    this.#noise.buffer = whiteNoise(context);
    this.#noise.loop = true;
    this.#noise.connect(this.#intakeFilter);
    this.#noise.connect(this.#crackle);
    this.#limiterDepth = gain(voice.limiterDepth, this.#level.gain);
    oscillator("square", ENGINE.limiterHz, this.#limiterDepth);
    for (const node of this.#oscillators) node.start();
    this.#noise.start();
  }

  /** Glide every node to `voice`; cut the level on an upshift, blip the note on a downshift, and fire a crackle burst when one is due. */
  follow(voice: EngineVoice, shift: EngineState["shift"]): void {
    const now = this.#context.currentTime;
    if (shift === "down") {
      this.#blipUntil = now + BLIP_S;
      this.#note(voice.noteHz * (1 + ENGINE.downshiftBlip), now);
      this.#note(voice.noteHz, this.#blipUntil);
    } else if (now >= this.#blipUntil) {
      this.#note(voice.noteHz, now);
    }
    this.#intakeFilter.frequency.setTargetAtTime(voice.intakeHz, now, ENGINE_GLIDE_S);
    this.#intakeGain.gain.setTargetAtTime(voice.intake, now, ENGINE_GLIDE_S);
    this.#filter.frequency.setTargetAtTime(voice.filterHz, now, ENGINE_GLIDE_S);
    if (shift === "up") {
      this.#cutUntil = now + ENGINE.upshiftCutSeconds;
      this.#level.gain.setTargetAtTime(0, now, CUT_GLIDE_S);
      this.#level.gain.setTargetAtTime(voice.level, this.#cutUntil, ENGINE_GLIDE_S);
    } else if (now >= this.#cutUntil) {
      this.#level.gain.setTargetAtTime(voice.level, now, ENGINE_GLIDE_S);
    }
    this.#limiterDepth.gain.setTargetAtTime(voice.limiterDepth, now, ENGINE_GLIDE_S);
    if (voice.crackle > 0 && now >= this.#nextCrackle) {
      this.#crackle.gain.setValueAtTime(voice.crackle, now);
      this.#crackle.gain.setTargetAtTime(0, now + CRACKLE_RISE_S, CRACKLE_FALL_S);
      this.#nextCrackle =
        now +
        CRACKLE_GAP_MIN_S +
        Math.random() * (CRACKLE_GAP_SPREAD_S - CRACKLE_GAP_OVERRUN_S * voice.overrun);
    }
  }

  /** Glide the sawtooths to `hz` and the sub to its octave under, from `at`. */
  #note(hz: number, at: number): void {
    for (const saw of this.#saws) saw.frequency.setTargetAtTime(hz, at, ENGINE_GLIDE_S);
    this.#sub.frequency.setTargetAtTime(hz / 2, at, ENGINE_GLIDE_S);
  }

  stop(): void {
    for (const node of this.#oscillators) {
      node.stop();
      node.disconnect();
    }
    this.#noise.stop();
    this.#noise.disconnect();
    for (const node of this.#nodes) node.disconnect();
  }
}

/** `NOISE_S` of white noise to loop, filled once per race. */
function whiteNoise(context: SoundContext): SoundBuffer {
  const buffer = context.createBuffer(1, context.sampleRate * NOISE_S, context.sampleRate);
  const samples = buffer.getChannelData(0);
  for (let i = 0; i < samples.length; i++) samples[i] = Math.random() * 2 - 1;
  return buffer;
}

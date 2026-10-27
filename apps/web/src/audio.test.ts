import { describe, expect, it } from "vitest";
import {
  type Audio,
  createAudio,
  LOOP_URLS,
  MUSIC_URL,
  SOUND_URLS,
  type SoundBuffer,
  type SoundBufferSource,
  type SoundContext,
  type SoundFilter,
  type SoundGain,
  type SoundNode,
  type SoundOscillator,
  type SoundParam,
} from "./audio";
import type { SoundCue, SoundLoop, SoundReading } from "./audio-cues";
import { ENGINE, engineBandEdges, FULL_THROTTLE_SECONDS_TO_TOP } from "./audio-engine";
import { DEFAULT_AUDIO_SETTINGS } from "./audio-settings";

const TOP_SPEED = 30;
const STILL: SoundReading = {
  speed: 0,
  airborne: false,
  miniTurbo: null,
  item: null,
  spunOut: false,
  boostSeconds: null,
  lap: 1,
  finished: false,
  count: null,
  charge: null,
  stalled: false,
  surface: "asphalt",
  drifting: false,
  wrongWay: false,
};

/** A decoded file stands in for its AudioBuffer: the test only asks which was played. */
interface FakeBuffer {
  readonly url: string;
}

/** A param that lands on each target at once, remembering the targets in order. */
function param(initial: number): SoundParam & { readonly targets: number[] } {
  const targets: number[] = [];
  return {
    value: initial,
    targets,
    setValueAtTime(value) {
      this.value = value;
      targets.push(value);
      return this;
    },
    setTargetAtTime(target) {
      this.value = target;
      targets.push(target);
      return this;
    },
  };
}

interface FakeNode extends SoundNode {
  readonly connectedTo: (SoundNode | SoundParam)[];
}

function node(): FakeNode {
  const connectedTo: (SoundNode | SoundParam)[] = [];
  return {
    connectedTo,
    connect(destination) {
      connectedTo.push(destination);
    },
    disconnect() {
      connectedTo.length = 0;
    },
  };
}

interface FakeOscillator extends SoundOscillator, FakeNode {
  readonly frequency: ReturnType<typeof param>;
  readonly detune: ReturnType<typeof param>;
  started: number;
  stopped: number;
}

interface FakeFilter extends SoundFilter, FakeNode {
  readonly frequency: ReturnType<typeof param>;
}

interface FakeSource extends SoundBufferSource, FakeNode {
  readonly playbackRate: ReturnType<typeof param>;
  started: number;
  stopped: number;
}

interface FakeGain extends SoundGain, FakeNode {
  readonly gain: ReturnType<typeof param>;
}

/** The slice of Web Audio the game uses, recording what was made of it. */
interface FakeContext extends SoundContext {
  currentTime: number;
  state: AudioContextState;
  readonly decoded: ArrayBuffer[];
  readonly oscillators: FakeOscillator[];
  readonly filters: FakeFilter[];
  readonly sources: FakeSource[];
  readonly gains: FakeGain[];
  resumes: number;
}

function fakeContext(): FakeContext {
  const destination = node();
  const context: FakeContext = {
    currentTime: 0,
    sampleRate: 48000,
    destination,
    state: "suspended",
    decoded: [],
    oscillators: [],
    filters: [],
    sources: [],
    gains: [],
    resumes: 0,
    resume() {
      this.resumes += 1;
      this.state = "running";
      return Promise.resolve();
    },
    decodeAudioData(bytes) {
      this.decoded.push(bytes);
      return Promise.resolve({ url: urlOf(bytes) } as unknown as SoundBuffer);
    },
    createBuffer(_channels, length) {
      const samples = new Float32Array(length);
      return { getChannelData: () => samples };
    },
    createBiquadFilter() {
      const filter: FakeFilter = { ...node(), type: "lowpass", frequency: param(350), Q: param(1) };
      this.filters.push(filter);
      return filter;
    },
    createOscillator() {
      const oscillator: FakeOscillator = {
        ...node(),
        type: "sine",
        frequency: param(440),
        detune: param(0),
        started: 0,
        stopped: 0,
        start() {
          this.started += 1;
        },
        stop() {
          this.stopped += 1;
        },
      };
      this.oscillators.push(oscillator);
      return oscillator;
    },
    createGain() {
      const gain: FakeGain = { ...node(), gain: param(1) };
      this.gains.push(gain);
      return gain;
    },
    createBufferSource() {
      const source: FakeSource = {
        ...node(),
        buffer: null,
        loop: false,
        playbackRate: param(1),
        started: 0,
        stopped: 0,
        start() {
          this.started += 1;
        },
        stop() {
          this.stopped += 1;
        },
      };
      this.sources.push(source);
      return source;
    },
  };
  return context;
}

/** Each file's bytes are its own buffer, so a decoded buffer names its file. */
const bytesByUrl = new Map<string, ArrayBuffer>();
function bytesOf(url: string): ArrayBuffer {
  let bytes = bytesByUrl.get(url);
  if (bytes === undefined) {
    bytes = new ArrayBuffer(8);
    bytesByUrl.set(url, bytes);
  }
  return bytes;
}
function urlOf(bytes: ArrayBuffer): string {
  for (const [url, candidate] of bytesByUrl) {
    if (candidate === bytes) return url;
  }
  throw new Error("bytes of no file");
}

interface Harness {
  readonly audio: Audio;
  readonly fetched: string[];
  readonly contexts: FakeContext[];
  /** The one context made, once unlock has made it. */
  context(): FakeContext;
  /** How many sources playing the file of `cue` have been started. */
  played(cue: SoundCue): number;
  /** The engine's two sawtooths, in the order made. */
  saws(): FakeOscillator[];
  /** The engine's square sub, under the sawtooths. */
  sub(): FakeOscillator;
  /** The engine's noise, which it fills itself rather than loads. */
  noise(): FakeSource[];
  /** The lowpass the engine's voices meet in. */
  engineFilter(): FakeFilter;
  /** The gain after the engine's filter that the upshift cuts and the limiter chops. */
  engineLevel(): FakeGain;
  /** The gain the limiter's chop is scaled by before it reaches the level. */
  limiterDepth(): FakeGain;
  /** The gain the crackle bursts open past the filter. */
  crackleGain(): FakeGain;
  music(): FakeSource[];
  /** The sources made for `loop`'s file, started or not. */
  loops(loop: SoundLoop): FakeSource[];
  /** The gain the engine reaches the speakers through, which its toggle sets. */
  engineGain(): FakeGain;
  /** The gain `source` is connected to. */
  gainOf(source: FakeNode): FakeGain;
}

function harness(settings = DEFAULT_AUDIO_SETTINGS): Harness {
  const fetched: string[] = [];
  const contexts: FakeContext[] = [];
  const audio = createAudio({
    settings,
    topSpeedMetresPerSecond: TOP_SPEED,
    fetchBytes(url) {
      fetched.push(url);
      return Promise.resolve(bytesOf(url));
    },
    createContext() {
      const context = fakeContext();
      contexts.push(context);
      return context;
    },
  });
  const context = () => {
    const made = contexts[0];
    if (made === undefined) throw new Error("no context was made");
    return made;
  };
  const sourcesOf = (url: string) =>
    context().sources.filter(
      (source) => (source.buffer as unknown as FakeBuffer | null)?.url === url,
    );
  const gainOf = (source: FakeNode) => {
    const gain = context().gains.find((candidate) => source.connectedTo.includes(candidate));
    if (gain === undefined) throw new Error("the source reaches no gain");
    return gain;
  };
  const filterOf = (source: FakeNode) => {
    const filter = context().filters.find((candidate) => source.connectedTo.includes(candidate));
    if (filter === undefined) throw new Error("the source reaches no filter");
    return filter;
  };
  const saws = () => context().oscillators.filter((made) => made.type === "sawtooth");
  const firstSaw = () => {
    const saw = saws()[0];
    if (saw === undefined) throw new Error("no engine sawtooth");
    return saw;
  };
  const engineFilter = () => filterOf(gainOf(firstSaw()));
  const engineLevel = () => gainOf(engineFilter());
  const noise = () =>
    context().sources.filter(
      (source) => source.buffer !== null && !("url" in (source.buffer as object)),
    );
  return {
    audio,
    fetched,
    contexts,
    context,
    played: (cue) => sourcesOf(SOUND_URLS[cue]).filter((source) => source.started > 0).length,
    saws,
    sub() {
      // The square that reaches the engine's filter through its own gain; the limiter's square reaches a param.
      const made = context().oscillators.find(
        (candidate) =>
          candidate.type === "square" &&
          context().gains.some(
            (gain) =>
              candidate.connectedTo.includes(gain) && gain.connectedTo.includes(engineFilter()),
          ),
      );
      if (made === undefined) throw new Error("no engine sub");
      return made;
    },
    noise,
    engineFilter,
    engineLevel,
    limiterDepth() {
      const level = engineLevel();
      const depth = context().gains.find((candidate) => candidate.connectedTo.includes(level.gain));
      if (depth === undefined) throw new Error("no limiter depth");
      return depth;
    },
    crackleGain() {
      const source = noise()[0];
      if (source === undefined) throw new Error("no engine noise");
      return gainOf(source);
    },
    music: () => sourcesOf(MUSIC_URL),
    loops: (loop) => sourcesOf(LOOP_URLS[loop]),
    engineGain: () => gainOf(engineLevel()),
    gainOf,
  };
}

/** Let every promise the audio chained settle. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("preloading and the autoplay rule", () => {
  it("fetches every sound at creation and makes no AudioContext until a gesture unlocks it", async () => {
    const { audio, fetched, contexts, context } = harness();
    expect(new Set(fetched)).toEqual(
      new Set([...Object.values(SOUND_URLS), ...Object.values(LOOP_URLS), MUSIC_URL]),
    );
    expect(fetched).toHaveLength(17);
    await settled();
    expect(contexts).toHaveLength(0);
    await audio.unlock();
    expect(contexts).toHaveLength(1);
    expect(context().resumes).toBe(1);
    // Decoded once, from the bytes already fetched, so the first play is not late.
    expect(context().decoded).toHaveLength(17);
    await audio.unlock();
    expect(contexts).toHaveLength(1);
    expect(context().decoded).toHaveLength(17);
  });

  it("resumes a context the browser suspended on a later gesture", async () => {
    const { audio, context } = harness();
    await audio.unlock();
    context().state = "suspended";
    await audio.unlock();
    expect(context().resumes).toBe(2);
  });
});

describe("the engine", () => {
  it("starts its graph with the race, through the engine's gain to the speakers, and stops it with the race", async () => {
    const { audio, saws, sub, noise, engineFilter, engineGain, limiterDepth, context } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    // Two sawtooths a detune apart, the sub an octave under them, and the limiter's chop.
    expect(saws()).toHaveLength(2);
    expect(saws().map((saw) => saw.detune.value)).toEqual([
      -ENGINE.detuneCents / 2,
      ENGINE.detuneCents / 2,
    ]);
    expect(saws()[0]?.frequency.value).toBe(ENGINE.idleHz);
    expect(sub().frequency.value).toBe(ENGINE.idleHz / 2);
    expect(context().oscillators).toHaveLength(4);
    for (const oscillator of context().oscillators) expect(oscillator.started).toBe(1);
    // The intake noise loops from a buffer the engine filled, not a file.
    expect(noise()).toHaveLength(1);
    expect(noise()[0]?.loop).toBe(true);
    expect(noise()[0]?.started).toBe(1);
    expect(engineFilter().type).toBe("lowpass");
    expect(engineGain().connectedTo).toContain(context().destination);
    expect(limiterDepth().gain.value).toBe(0);
    race.stop();
    for (const oscillator of context().oscillators) expect(oscillator.stopped).toBe(1);
    expect(noise()[0]?.stopped).toBe(1);
    expect(saws()[0]?.connectedTo).toHaveLength(0);
  });

  it("is muted by its toggle at once and unmuted again", async () => {
    const { audio, engineGain } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update(STILL);
    expect(engineGain().gain.value).toBeGreaterThan(0);
    audio.settings({ ...DEFAULT_AUDIO_SETTINGS, engine: false });
    expect(engineGain().gain.value).toBe(0);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    expect(engineGain().gain.value).toBeGreaterThan(0);
  });

  it("starts silent when the engine is off, and begins when the toggle comes on mid-race", async () => {
    const { audio, engineGain } = harness({ ...DEFAULT_AUDIO_SETTINGS, engine: false });
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update({ ...STILL, speed: 10 });
    expect(engineGain().gain.value).toBe(0);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    expect(engineGain().gain.value).toBeGreaterThan(0);
  });

  it("raises its note with speed, both sawtooths together and the sub an octave under, to the top note at top speed", async () => {
    const { audio, saws, sub, engineFilter } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update(STILL);
    expect(saws()[0]?.frequency.value).toBe(ENGINE.idleHz);
    const closed = engineFilter().frequency.value;
    race.update({ ...STILL, speed: 1 });
    const low = saws()[0]?.frequency.value ?? 0;
    expect(low).toBeGreaterThan(ENGINE.idleHz);
    expect(saws()[1]?.frequency.value).toBe(low);
    expect(sub().frequency.value).toBe(low / 2);
    expect(engineFilter().frequency.value).toBeGreaterThan(closed);
    race.update({ ...STILL, speed: TOP_SPEED });
    expect(saws()[0]?.frequency.value).toBeCloseTo(ENGINE.topHz, 6);
  });

  it("drops the note and cuts the level as it upshifts, and blips the note as it downshifts", async () => {
    const { audio, saws, engineLevel, context } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    const edges = engineBandEdges(TOP_SPEED);
    const top = edges[1] ?? 0;
    race.update({ ...STILL, speed: top - 0.01 });
    const inFirst = saws()[0]?.frequency.value ?? 0;
    const levelTargets = engineLevel().gain.targets.length;
    race.update({ ...STILL, speed: top + 0.01 });
    // Faster, yet the note falls: second gear.
    expect(saws()[0]?.frequency.value).toBeLessThan(inFirst);
    // The cut: the level to nothing now and back once the cut has run.
    expect(engineLevel().gain.targets.slice(levelTargets)).toEqual([0, 1]);
    // While the cut runs the level is left to it.
    race.update({ ...STILL, speed: top + 0.02 });
    expect(engineLevel().gain.targets).toHaveLength(levelTargets + 2);
    context().currentTime = 1;
    race.update({ ...STILL, speed: top + 0.03 });
    expect(engineLevel().gain.targets).toHaveLength(levelTargets + 3);
    // Back under the band's floor by more than the margin: first again, with the blip over the note it settles to.
    const settled = saws()[0]?.frequency.targets.length ?? 0;
    race.update({ ...STILL, speed: top - 0.5 });
    const blip = saws()[0]?.frequency.targets.slice(settled) ?? [];
    expect(blip).toHaveLength(2);
    expect(blip[0]).toBeCloseTo((blip[1] ?? 0) * (1 + ENGINE.downshiftBlip), 6);
  });

  it("crackles in bursts as the kart lifts off, spaced out, and never on the throttle", async () => {
    const { audio, crackleGain, context } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    const frame = 1 / 60;
    const step = (frame * TOP_SPEED) / FULL_THROTTLE_SECONDS_TO_TOP;
    /** Thirty frames from `from` at full throttle (1) or a full lift (-1), the clock advancing with them. */
    const run = (from: number, direction: 1 | -1) => {
      for (let i = 1; i <= 30; i++) {
        context().currentTime += frame;
        race.update({ ...STILL, speed: from + direction * i * step });
      }
    };
    context().currentTime += frame;
    race.update({ ...STILL, speed: 10 });
    run(10, 1);
    expect(crackleGain().gain.targets).toHaveLength(0);
    // Lifting off at the same rate: bursts, each opened and shut, fewer than the frames.
    run(16, -1);
    const targets = crackleGain().gain.targets;
    const bursts = targets.filter((target) => target > 0).length;
    expect(bursts).toBeGreaterThan(0);
    expect(bursts).toBeLessThan(30);
    expect(targets.filter((target) => target === 0)).toHaveLength(bursts);
    expect(targets[0]).toBeGreaterThan(0);
    expect(targets[1]).toBe(0);
  });

  it("chops at the limiter past top speed, and stops chopping back under it", async () => {
    const { audio, saws, limiterDepth } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update({ ...STILL, speed: TOP_SPEED * 1.3 });
    expect(limiterDepth().gain.value).toBeGreaterThan(0);
    expect(saws()[0]?.frequency.value).toBeGreaterThan(ENGINE.topHz);
    race.update({ ...STILL, speed: TOP_SPEED * 0.9 });
    expect(limiterDepth().gain.value).toBe(0);
    expect(saws()[0]?.frequency.value).toBeLessThan(ENGINE.topHz);
  });

  it("revs on the grid as the rocket start charges, on the limiter for a full boost, and idles from Go", async () => {
    const { audio, saws, limiterDepth } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update({ ...STILL, count: 3, charge: "clean" });
    expect(saws()[0]?.frequency.value).toBe(ENGINE.idleHz);
    race.update({ ...STILL, count: 2, charge: "weakBoost" });
    const weak = saws()[0]?.frequency.value ?? 0;
    expect(weak).toBeGreaterThan(ENGINE.idleHz);
    expect(weak).toBeLessThan(ENGINE.topHz);
    expect(limiterDepth().gain.value).toBe(0);
    race.update({ ...STILL, count: 1, charge: "fullBoost" });
    expect(saws()[0]?.frequency.value).toBeCloseTo(ENGINE.topHz, 6);
    expect(limiterDepth().gain.value).toBeGreaterThan(0);
    race.update({ ...STILL, count: "go", charge: null });
    expect(saws()[0]?.frequency.value).toBe(ENGINE.idleHz);
    expect(limiterDepth().gain.value).toBe(0);
  });
});

describe("the SFX", () => {
  const CUES: readonly [SoundCue, Partial<SoundReading>][] = [
    ["hop", { airborne: true }],
    ["itemPickup", { item: "boost" }],
    ["hit", { spunOut: true }],
    ["lap", { lap: 2 }],
    ["countdownBeep", { count: 3 }],
    ["go", { count: "go" }],
    ["sputter", { stalled: true }],
    ["tierChime", { miniTurbo: "blue", drifting: true }],
    ["boostPad", { boostSeconds: 0.6 }],
    ["wrongWay", { wrongWay: true }],
    ["finish", { finished: true }],
  ];

  it.each(CUES)(
    "plays %s once from its preloaded file as the state changes",
    async (cue, change) => {
      const { audio, played } = harness();
      await audio.unlock();
      const race = audio.race();
      race.start();
      race.update(STILL);
      expect(played(cue)).toBe(0);
      race.update({ ...STILL, ...change });
      expect(played(cue)).toBe(1);
      race.update({ ...STILL, ...change });
      expect(played(cue)).toBe(1);
    },
  );

  it("plays the mini-turbo on release and the fire as the slot empties", async () => {
    const { audio, played } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update({ ...STILL, miniTurbo: "red", item: "hazard" });
    race.update({ ...STILL, boostSeconds: 0.6 });
    expect(played("miniTurbo")).toBe(1);
    expect(played("itemFire")).toBe(1);
  });

  it("plays nothing with the SFX toggle off, and plays again once it is on", async () => {
    const { audio, played } = harness({ ...DEFAULT_AUDIO_SETTINGS, sfx: false });
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.update({ ...STILL, airborne: true });
    expect(played("hop")).toBe(0);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    race.update(STILL);
    race.update({ ...STILL, airborne: true });
    expect(played("hop")).toBe(1);
  });

  it("plays nothing after the race is stopped", async () => {
    const { audio, played } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.stop();
    race.update({ ...STILL, airborne: true });
    expect(played("hop")).toBe(0);
  });
});

describe("the loops", () => {
  const LOOPS: readonly [SoundLoop, Partial<SoundReading>][] = [
    ["driftSkid", { drifting: true, miniTurbo: "none" }],
    ["grass", { surface: "grass" }],
    ["sand", { surface: "sand" }],
  ];

  it.each(LOOPS)(
    "runs %s from its preloaded file as its state begins until it ends",
    async (loop, state) => {
      const { audio, loops, gainOf, context } = harness();
      await audio.unlock();
      const race = audio.race();
      race.start();
      race.update(STILL);
      expect(loops(loop)).toHaveLength(0);
      race.update({ ...STILL, ...state, speed: 15 });
      expect(loops(loop)).toHaveLength(1);
      const source = loops(loop)[0];
      expect(source?.loop).toBe(true);
      expect(source?.started).toBe(1);
      // Through its own gain and the SFX gain to the speakers.
      const sfxGain = gainOf(gainOf(source as FakeNode));
      expect(sfxGain.connectedTo).toContain(context().destination);
      race.update({ ...STILL, ...state, speed: 20 });
      expect(loops(loop)).toHaveLength(1);
      expect(source?.stopped).toBe(0);
      race.update(STILL);
      expect(source?.stopped).toBe(1);
      expect(source?.connectedTo).toHaveLength(0);
      race.update({ ...STILL, ...state, speed: 15 });
      expect(loops(loop)).toHaveLength(2);
    },
  );

  it("pitches and swells the skid with the kart's speed, gliding frame to frame", async () => {
    const { audio, loops, gainOf } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    const skidding = { ...STILL, drifting: true, miniTurbo: "none" as const };
    race.update({ ...skidding, speed: 15 });
    const source = loops("driftSkid")[0];
    if (source === undefined) throw new Error("no skid");
    expect(source.playbackRate.value).toBeCloseTo(1, 6);
    const atRendered = gainOf(source).gain.value;
    race.update({ ...skidding, speed: TOP_SPEED });
    expect(source.playbackRate.value).toBeCloseTo(1200 / 900, 6);
    expect(source.playbackRate.targets).toHaveLength(1);
    expect(gainOf(source).gain.value).toBeGreaterThan(atRendered);
    expect(gainOf(source).gain.targets).toHaveLength(1);
  });

  it("runs both the skid and the surface together, stopping each on its own", async () => {
    const { audio, loops } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    race.update({ ...STILL, drifting: true, miniTurbo: "none", surface: "sand", speed: 15 });
    expect(loops("driftSkid")).toHaveLength(1);
    expect(loops("sand")).toHaveLength(1);
    race.update({ ...STILL, surface: "sand", speed: 15 });
    expect(loops("driftSkid")[0]?.stopped).toBe(1);
    expect(loops("sand")[0]?.stopped).toBe(0);
    race.update(STILL);
    expect(loops("sand")[0]?.stopped).toBe(1);
  });

  it("starts no loop with the SFX toggle off, and stops a running one as it goes off", async () => {
    const { audio, loops } = harness({ ...DEFAULT_AUDIO_SETTINGS, sfx: false });
    await audio.unlock();
    const race = audio.race();
    race.start();
    const skidding = { ...STILL, drifting: true, miniTurbo: "none" as const, speed: 15 };
    race.update(skidding);
    expect(loops("driftSkid")).toHaveLength(0);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    race.update(skidding);
    expect(loops("driftSkid")).toHaveLength(1);
    audio.settings({ ...DEFAULT_AUDIO_SETTINGS, sfx: false });
    race.update(skidding);
    expect(loops("driftSkid")[0]?.stopped).toBe(1);
    expect(loops("driftSkid")).toHaveLength(1);
  });

  it("stops with the race and never starts after it", async () => {
    const { audio, loops } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    const skidding = { ...STILL, drifting: true, miniTurbo: "none" as const, speed: 15 };
    race.update(skidding);
    race.stop();
    expect(loops("driftSkid")[0]?.stopped).toBe(1);
    race.update(skidding);
    expect(loops("driftSkid")).toHaveLength(1);
  });

  it("starts a loop whose state was already under way as the gesture unlocks the context", async () => {
    const { audio, loops } = harness();
    const race = audio.race();
    race.start();
    const skidding = { ...STILL, drifting: true, miniTurbo: "none" as const, speed: 15 };
    race.update(skidding);
    await audio.unlock();
    race.update(skidding);
    expect(loops("driftSkid")).toHaveLength(1);
    expect(loops("driftSkid")[0]?.started).toBe(1);
  });
});

describe("a sound that fails to load", () => {
  it("is dropped on its own, cue or loop, while the rest play", async () => {
    const fetched: string[] = [];
    const contexts: FakeContext[] = [];
    const missing = [SOUND_URLS.hop, LOOP_URLS.driftSkid];
    const audio = createAudio({
      settings: DEFAULT_AUDIO_SETTINGS,
      topSpeedMetresPerSecond: TOP_SPEED,
      fetchBytes(url) {
        fetched.push(url);
        return missing.includes(url)
          ? Promise.reject(new Error("404"))
          : Promise.resolve(bytesOf(url));
      },
      createContext() {
        const context = fakeContext();
        contexts.push(context);
        return context;
      },
    });
    await audio.unlock();
    const context = contexts[0];
    if (context === undefined) throw new Error("no context was made");
    expect(context.decoded).toHaveLength(15);
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.update({
      ...STILL,
      airborne: true,
      lap: 2,
      drifting: true,
      miniTurbo: "none",
      surface: "sand",
    });
    const playing = context.sources
      .filter((source) => source.started > 0)
      .map((source) => (source.buffer as unknown as FakeBuffer | null)?.url);
    expect(playing).toContain(SOUND_URLS.lap);
    expect(playing).toContain(LOOP_URLS.sand);
    expect(playing).not.toContain(SOUND_URLS.hop);
    expect(playing).not.toContain(LOOP_URLS.driftSkid);
  });
});

describe("the music loop", () => {
  it("loops from the race's start to its stop", async () => {
    const { audio, music } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    expect(music()).toHaveLength(1);
    expect(music()[0]?.loop).toBe(true);
    expect(music()[0]?.started).toBe(1);
    race.stop();
    expect(music()[0]?.stopped).toBe(1);
  });

  it("stops with its toggle and starts again with it, and never starts while off", async () => {
    const { audio, music } = harness({ ...DEFAULT_AUDIO_SETTINGS, music: false });
    await audio.unlock();
    const race = audio.race();
    race.start();
    expect(music()).toHaveLength(0);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    expect(music()).toHaveLength(1);
    audio.settings({ ...DEFAULT_AUDIO_SETTINGS, music: false });
    expect(music()[0]?.stopped).toBe(1);
    audio.settings(DEFAULT_AUDIO_SETTINGS);
    expect(music()).toHaveLength(2);
    race.stop();
    expect(music()[1]?.stopped).toBe(1);
  });
});

describe("a race under way before the first gesture", () => {
  it("is silent until the gesture, then sounds from there, dropping the cues it missed", async () => {
    const { audio, contexts, saws, music, played } = harness();
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.update({ ...STILL, airborne: true, speed: 20 });
    expect(contexts).toHaveLength(0);
    await audio.unlock();
    // The engine picks up at the speed it finds: fifth, not a climb from idle.
    expect(saws()).toHaveLength(2);
    expect(saws()[0]?.started).toBe(1);
    expect(saws()[0]?.frequency.value).toBeGreaterThan(ENGINE.idleHz);
    expect(music()).toHaveLength(1);
    expect(played("hop")).toBe(0);
    race.update({ ...STILL, airborne: true, speed: 20 });
    race.update({ ...STILL, speed: 20 });
    race.update({ ...STILL, airborne: true, speed: 20 });
    expect(played("hop")).toBe(1);
  });

  it("stops before the gesture and stays silent when the gesture comes", async () => {
    const { audio, context } = harness();
    const race = audio.race();
    race.start();
    race.stop();
    await audio.unlock();
    expect(context().oscillators).toHaveLength(0);
    expect(context().sources).toHaveLength(0);
  });
});

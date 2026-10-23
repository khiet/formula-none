import { describe, expect, it } from "vitest";
import {
  type Audio,
  createAudio,
  MUSIC_URL,
  SOUND_URLS,
  type SoundBufferSource,
  type SoundContext,
  type SoundGain,
  type SoundNode,
  type SoundOscillator,
  type SoundParam,
} from "./audio";
import type { SoundCue, SoundReading } from "./audio-cues";
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
  count: "",
  charge: null,
  stalled: false,
};

/** A decoded file stands in for its AudioBuffer: the test only asks which was played. */
interface FakeBuffer {
  readonly url: string;
}

function param(initial: number): SoundParam & { readonly targets: number[] } {
  const targets: number[] = [];
  return {
    value: initial,
    targets,
    setTargetAtTime(target) {
      this.value = target;
      targets.push(target);
      return this;
    },
  };
}

interface FakeNode extends SoundNode {
  readonly connectedTo: SoundNode[];
}

function node(): FakeNode {
  const connectedTo: SoundNode[] = [];
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
  started: number;
  stopped: number;
}

interface FakeSource extends SoundBufferSource, FakeNode {
  started: number;
  stopped: number;
}

interface FakeGain extends SoundGain, FakeNode {
  readonly gain: ReturnType<typeof param>;
}

/** The slice of Web Audio the game uses, recording what was made of it. */
interface FakeContext extends SoundContext {
  state: AudioContextState;
  readonly decoded: ArrayBuffer[];
  readonly oscillators: FakeOscillator[];
  readonly sources: FakeSource[];
  readonly gains: FakeGain[];
  resumes: number;
}

function fakeContext(): FakeContext {
  const destination = node();
  const context: FakeContext = {
    currentTime: 0,
    destination,
    state: "suspended",
    decoded: [],
    oscillators: [],
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
      return Promise.resolve({ url: urlOf(bytes) } as unknown as AudioBuffer);
    },
    createOscillator() {
      const oscillator: FakeOscillator = {
        ...node(),
        type: "sine",
        frequency: param(440),
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
  engine(): FakeOscillator;
  music(): FakeSource[];
  /** The gain the engine's note reaches the speakers through. */
  engineGain(): FakeGain;
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
  const engine = () => {
    const made = context().oscillators.at(-1);
    if (made === undefined) throw new Error("no engine oscillator");
    return made;
  };
  return {
    audio,
    fetched,
    contexts,
    context,
    played: (cue) => sourcesOf(SOUND_URLS[cue]).filter((source) => source.started > 0).length,
    engine,
    music: () => sourcesOf(MUSIC_URL),
    engineGain() {
      const gain = context().gains.find((candidate) => engine().connectedTo.includes(candidate));
      if (gain === undefined) throw new Error("the engine reaches no gain");
      return gain;
    },
  };
}

/** Let every promise the audio chained settle. */
const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("preloading and the autoplay rule", () => {
  it("fetches every sound at creation and makes no AudioContext until a gesture unlocks it", async () => {
    const { audio, fetched, contexts, context } = harness();
    expect(new Set(fetched)).toEqual(new Set([...Object.values(SOUND_URLS), MUSIC_URL]));
    expect(fetched).toHaveLength(10);
    await settled();
    expect(contexts).toHaveLength(0);
    await audio.unlock();
    expect(contexts).toHaveLength(1);
    expect(context().resumes).toBe(1);
    // Decoded once, from the bytes already fetched, so the first play is not late.
    expect(context().decoded).toHaveLength(10);
    await audio.unlock();
    expect(contexts).toHaveLength(1);
    expect(context().decoded).toHaveLength(10);
  });

  it("resumes a context the browser suspended on a later gesture", async () => {
    const { audio, context } = harness();
    await audio.unlock();
    context().state = "suspended";
    await audio.unlock();
    expect(context().resumes).toBe(2);
  });
});

describe("the engine note", () => {
  it("is one looping oscillator per race whose pitch follows the own kart's speed", async () => {
    const { audio, engine, engineGain, context } = harness();
    await audio.unlock();
    const race = audio.race();
    race.start();
    expect(context().oscillators).toHaveLength(1);
    expect(engine().started).toBe(1);
    // Through its gain to the speakers.
    expect(engineGain().connectedTo).toContain(context().destination);
    race.update(STILL);
    const idle = engine().frequency.value;
    race.update({ ...STILL, speed: TOP_SPEED });
    const top = engine().frequency.value;
    expect(top).toBeGreaterThan(idle);
    race.update({ ...STILL, speed: TOP_SPEED / 2 });
    expect(engine().frequency.value).toBeCloseTo((idle + top) / 2, 6);
    race.update({ ...STILL, speed: -TOP_SPEED / 2 });
    expect(engine().frequency.value).toBeCloseTo((idle + top) / 2, 6);
    race.stop();
    expect(engine().stopped).toBe(1);
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
});

describe("the SFX", () => {
  const CUES: readonly [SoundCue, Partial<SoundReading>][] = [
    ["hop", { airborne: true }],
    ["itemPickup", { item: "boost" }],
    ["hit", { spunOut: true }],
    ["lap", { lap: 2 }],
    ["countdownBeep", { count: "3" }],
    ["go", { count: "GO" }],
    ["sputter", { stalled: true }],
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
    race.update({ ...STILL, miniTurbo: "orange", item: "hazard" });
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

describe("a sound that fails to load", () => {
  it("is dropped on its own, while the rest play", async () => {
    const fetched: string[] = [];
    const contexts: FakeContext[] = [];
    const audio = createAudio({
      settings: DEFAULT_AUDIO_SETTINGS,
      topSpeedMetresPerSecond: TOP_SPEED,
      fetchBytes(url) {
        fetched.push(url);
        return url === SOUND_URLS.hop
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
    expect(context.decoded).toHaveLength(9);
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.update({ ...STILL, airborne: true, lap: 2 });
    const playing = context.sources
      .filter((source) => source.started > 0)
      .map((source) => (source.buffer as unknown as FakeBuffer | null)?.url);
    expect(playing).toContain(SOUND_URLS.lap);
    expect(playing).not.toContain(SOUND_URLS.hop);
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
    const { audio, contexts, context, engine, music, played } = harness();
    const race = audio.race();
    race.start();
    race.update(STILL);
    race.update({ ...STILL, airborne: true, speed: 20 });
    expect(contexts).toHaveLength(0);
    await audio.unlock();
    expect(context().oscillators).toHaveLength(1);
    expect(engine().started).toBe(1);
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

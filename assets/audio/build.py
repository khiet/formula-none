"""Render the game's SFX (ADR 0007, ADR 0025) as WAV files.

Run from the repo root with `pnpm export:audio`; stdlib only. Every SFX is
synthesised here, so the set is this repo's own work and CC0 like the rest
of `assets/` (LICENSES.md). The music is one bar-aligned loop whose last
sample runs straight into its first.

The cues were picked on the sound effects iteration page
(`docs/mockups/sound-effects.html`), which synthesises each one in the
browser from the DSP kit ported below, so a render here is the page's pick.
The hit and the sputter predate the page and keep their original code path,
so their bytes do not churn.
"""

import math
import random
import struct
import wave
from typing import NamedTuple
from pathlib import Path

RATE = 22050
TAU = math.pi * 2
OUT = Path(__file__).parent
# The speed, in metres per second, the loops are rendered at; the game
# pitches and swells each loop from this speed to the kart's, so the
# bands and swells below are repeated in audio-cues.ts (LOOP_CURVES) and
# the two change together.
LOOP_SPEED = 15.0
# The sim's TOP_SPEED_M_PER_S (crates/sim/src/tuning.rs); a retune there
# moves it here.
TOP_SPEED = 30.0


def write(name, samples):
    """Write mono 16-bit samples in -1..1 to `name`.wav, clipped."""
    with wave.open(str(OUT / f"{name}.wav"), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        frames = (
            struct.pack("<h", int(max(-1.0, min(1.0, s)) * 32767)) for s in samples
        )
        out.writeframes(b"".join(frames))


# ---------------------------------------------------------------------------
# The page's DSP kit. Sample lists at RATE; deterministic, since the noise
# is seeded, so a render with the same parameters is the same buffer.
# ---------------------------------------------------------------------------


def sec(n):
    """Samples in `n` seconds, rounded half up as the page's Math.round does."""
    return max(0, math.floor(n * RATE + 0.5))


def sine(p):
    return math.sin(p)


def square(p):
    return 1.0 if math.sin(p) >= 0 else -1.0


def triangle(p):
    return 2 / math.pi * math.asin(math.sin(p))


def saw(p):
    return (p % TAU) / math.pi - 1


def pulse(p):
    return 1.0 if (p % TAU) / TAU < 0.25 else -1.0


MASK = 0xFFFFFFFF


def rng(seed):
    """A seeded uniform(-1, 1) source: mulberry32 on 32-bit unsigned words, as the page runs it."""
    a = (seed * 2654435761) & MASK

    def draw():
        nonlocal a
        a = (a + 0x6D2B79F5) & MASK
        t = a
        t = ((t ^ (t >> 15)) * (t | 1)) & MASK
        t ^= (t + (((t ^ (t >> 7)) * (t | 61)) & MASK)) & MASK
        return ((t ^ (t >> 14)) / 4294967296) * 2 - 1

    return draw


def tone(
    freq,
    length,
    wave_fn=sine,
    decay=6.0,
    gain=0.5,
    sweep_to=None,
    attack=0.0,
    vibrato_hz=0.0,
    vibrato_depth=0.0,
    curve="linear",
):
    """A note of `length` seconds; the pitch glides to `sweep_to` if given, linearly or exponentially."""
    n = sec(length)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / RATE
        u = i / n
        if sweep_to is None:
            f = freq
        elif curve == "exp":
            f = freq * (sweep_to / freq) ** u
        else:
            f = freq + (sweep_to - freq) * u
        if vibrato_hz > 0:
            f *= 1 + vibrato_depth * math.sin(TAU * vibrato_hz * t)
        phase += TAU * f / RATE
        a = min(1.0, t / attack) if attack > 0 else 1.0
        out.append(gain * a * math.exp(-decay * t) * wave_fn(phase))
    return out


def noise(length, decay=8.0, gain=0.5, seed=1, attack=0.0):
    draw = rng(seed)
    out = []
    for i in range(sec(length)):
        t = i / RATE
        a = min(1.0, t / attack) if attack > 0 else 1.0
        out.append(gain * a * math.exp(-decay * t) * draw())
    return out


def silence(length):
    return [0.0] * sec(length)


def mix(*parts):
    out = [0.0] * max(len(p) for p in parts)
    for part in parts:
        for i, s in enumerate(part):
            out[i] += s
    return out


def concat(*parts):
    return [s for part in parts for s in part]


def scale(samples, gain):
    return [s * gain for s in samples]


def envelope(samples, fn):
    """Multiply by `fn(t)` for t in seconds, for envelopes and slow modulation."""
    return [s * fn(i / RATE) for i, s in enumerate(samples)]


def chop(samples, hz, depth):
    """Amplitude chop at `hz`: a tremolo of `depth` (1 gates fully) with a raised-cosine shape."""
    return envelope(samples, lambda t: 1 - depth * 0.5 * (1 - math.cos(TAU * hz * t)))


def lowpass(samples, cutoff, passes=1):
    """One-pole lowpass; `passes` stacks it for a steeper slope."""
    out = samples
    dt = 1 / RATE
    rc = 1 / (TAU * cutoff)
    a = dt / (rc + dt)
    for _ in range(passes):
        y = 0.0
        step = []
        for x in out:
            y += a * (x - y)
            step.append(y)
        out = step
    return out


def highpass(samples, cutoff, passes=1):
    out = samples
    dt = 1 / RATE
    rc = 1 / (TAU * cutoff)
    a = rc / (rc + dt)
    for _ in range(passes):
        y = 0.0
        x1 = 0.0
        step = []
        for x in out:
            y = a * (y + x - x1)
            x1 = x
            step.append(y)
        out = step
    return out


def bandpass(samples, centre, width, passes=2):
    lo = max(20, centre - width / 2)
    hi = max(lo + 20, centre + width / 2)
    return highpass(lowpass(samples, hi, passes), lo, passes)


def loopable(samples, crossfade=0.05):
    """Crossfade the tail into the head so the buffer loops without a click."""
    n = len(samples)
    x = min(sec(crossfade), n // 2)
    out = samples[: n - x]
    for i in range(x):
        u = i / x
        out[i] = samples[n - x + i] * (1 - u) + samples[i] * u
    return out


def tame(samples, peak=0.95):
    """Peak-normalise to `peak` when the render overshoots it; `write` would clip otherwise."""
    top = max((abs(s) for s in samples), default=0.0)
    return scale(samples, peak / top) if top > peak else samples


def fraction(speed):
    return min(1.5, abs(speed) / TOP_SPEED)


def notes(freqs, length, wave_fn, decay, gain):
    return concat(*(tone(f, length, wave_fn, decay, gain) for f in freqs))


def chord(freqs, length, wave_fn, decay, gain):
    return mix(*(tone(f, length, wave_fn, decay, gain) for f in freqs))


# ---------------------------------------------------------------------------
# The picks, parameters as the page names them.
# ---------------------------------------------------------------------------


def countdown_beep(freq=1000, length=0.1, attack=0.005, sub=0.15, gain=0.35):
    """Lights: a clean sine bip with a soft attack and an octave under it."""
    return mix(
        tone(freq, length, decay=10, gain=gain, attack=attack),
        tone(freq / 2, length, decay=10, gain=sub, attack=attack),
    )


def go(from_hz=880, to_hz=1760, length=0.35, hiss=0.2, gain=0.35):
    """Lights out: a rising sine under a hiss of air."""
    return mix(
        tone(from_hz, length, sweep_to=to_hz, decay=3, gain=gain, curve="exp"),
        highpass(noise(length * 0.8, decay=6, gain=hiss, seed=2), 3000),
    )


def hop(click=0.06, from_hz=450, to_hz=1400, length=0.1, gain=0.45):
    """Pneumatic: a click of air then a short chirp."""
    return mix(
        highpass(noise(click, decay=40, gain=gain, seed=3), 1500),
        tone(from_hz, length, decay=15, sweep_to=to_hz, gain=gain * 0.8),
    )


def mini_turbo(from_hz=150, to_hz=1100, length=0.35, hiss=0.4, gain=0.35):
    """Blow-off: a turbo blow-off hiss under an exponential sawtooth rise."""
    return mix(
        highpass(noise(length, decay=5, gain=hiss, seed=4), 1200),
        tone(from_hz, length, saw, decay=4, sweep_to=to_hz, gain=gain, curve="exp"),
    )


def item_pickup(first=988, second=1319, ring=0.25, gain=0.4):
    """Coin: a short note then a long ringing one a fourth up."""
    return concat(
        tone(first, 0.08, decay=8, gain=gain),
        tone(second, ring, decay=6, gain=gain),
    )


def coin_pickup(first=1568, second=2093, gain=0.35):
    """Chime: two bright triangle notes a fourth apart, quicker and higher than the item box."""
    return concat(
        tone(first, 0.05, triangle, decay=14, gain=gain),
        tone(second, 0.14, triangle, decay=12, gain=gain),
    )


def coin_loss(freqs=(1760, 1397, 1175, 880), step=0.055, gain=0.3):
    """Jingle: the chime's triangle falling a note at a time."""
    return concat(*(tone(freq, step, triangle, decay=18, gain=gain) for freq in freqs))


def item_fire(from_hz=1200, to_hz=200, length=0.15, decay=10, gain=0.3):
    """Pew: a square zap falling fast."""
    return tone(from_hz, length, square, decay=decay, sweep_to=to_hz, gain=gain, curve="exp")


def lap(first=1200, second=1600, gap=0.05, gain=0.35):
    """Sector: two clean sine bips a fourth apart, like a timing beam."""
    return concat(
        tone(first, 0.1, decay=12, gain=gain),
        silence(gap),
        tone(second, 0.15, decay=10, gain=gain),
    )


def drift_skid(speed, centre=600, per_ms=20, width=400, judder_hz=22, depth=0.5, gain=0.85):
    """Judder: bandpassed noise chopped at a tyre's judder; the band climbs `per_ms` hertz per metre per second."""
    bed = noise(1.5, decay=0, gain=gain * (0.4 + 0.6 * fraction(speed)), seed=12)
    return loopable(chop(scale(bandpass(bed, centre + per_ms * speed, width), 4), judder_hz, depth))


def tier_chime(blue=880, orange=1320, length=0.1, decay=12, gain=0.35):
    """Two step: a pulse note for blue and a higher one for orange."""
    return concat(
        tone(blue, length, pulse, decay=decay, gain=gain),
        silence(0.3 - length),
        tone(orange, length * 1.2, pulse, decay=decay, gain=gain),
    )


def grass(speed, centre=1200, width=800, swish_hz=8, depth=0.5, gain=0.4):
    """Swish: a higher band of noise swishing slowly."""
    bed = noise(1.5, decay=0, gain=gain * (0.3 + 0.7 * fraction(speed)), seed=16)
    return loopable(chop(scale(bandpass(bed, centre, width), 5), swish_hz, depth))


def sand(speed, grit_hz=1800, grit=0.35, scrape_hz=40, rumble=0.35, gain=0.6):
    """Grit: bright gritty noise over a low rumble."""
    bright = noise(1.5, decay=0, gain=grit * (0.3 + 0.7 * fraction(speed)), seed=18)
    low = noise(1.5, decay=0, gain=rumble * gain, seed=19)
    return loopable(
        mix(
            chop(scale(lowpass(highpass(bright, grit_hz), 6000), 2), scrape_hz, 0.3),
            scale(lowpass(low, 200), 2),
        )
    )


def boost_pad(root=880, rise=1.12, length=0.35, click=0.7, gain=0.25):
    """Push to pass: a crisp click then a sine chord sliding up a tone."""
    return mix(
        noise(0.012, decay=100, gain=click, seed=24),
        tone(root, length, decay=4, sweep_to=root * rise, gain=gain),
        tone(root * 1.5, length, decay=4, sweep_to=root * 1.5 * rise, gain=gain * 0.7),
    )


def wrong_way(freq=330, chop_hz=30, length=0.45, gain=0.3):
    """Buzzer: a low square chopped hard."""
    return chop(tone(freq, length, square, decay=3, gain=gain), chop_hz, 1)


def finish(beep=880, high=1320, length=0.1, hold=0.5, gain=0.3):
    """Chequered: two pulse beeps at one pitch and a third higher, then a square chord."""
    return concat(
        notes([beep, beep, high], length, pulse, 10, gain),
        chord([high, high * 1.25, high * 1.5], hold, square, 3, gain * 0.5),
    )


# ---------------------------------------------------------------------------
# The original helpers, bodies unchanged under new names, for the hit and
# the sputter, whose bytes must not change.
# ---------------------------------------------------------------------------


def seconds(n):
    return int(n * RATE)


def legacy_tone(freq, length, wave_fn=math.sin, decay=6.0, gain=0.5, sweep_to=None):
    """A note of `length` seconds whose pitch glides to `sweep_to` if given."""
    out = []
    phase = 0.0
    total = seconds(length)
    for i in range(total):
        t = i / RATE
        f = freq if sweep_to is None else freq + (sweep_to - freq) * (i / total)
        phase += 2 * math.pi * f / RATE
        out.append(gain * math.exp(-decay * t) * wave_fn(phase))
    return out


def legacy_noise(length, decay=8.0, gain=0.5, rng=random.Random(1)):
    return [
        gain * math.exp(-decay * (i / RATE)) * rng.uniform(-1, 1)
        for i in range(seconds(length))
    ]


def hit():
    """A crash of noise over a low rumble."""
    # The original item fire drew a tenth of a second from the shared noise
    # source before the hit did; the hit's bytes follow those draws.
    legacy_noise(0.1, decay=30)
    return mix(
        legacy_noise(0.45, decay=7, gain=0.7),
        legacy_tone(70, 0.45, wave_fn=square, decay=5, gain=0.3),
    )


def sputter(length):
    """A choked engine for `length` seconds: coughs with noise over each."""
    rng = random.Random(3)
    out = []
    phase = 0.0
    coughs_per_s = 7.0
    for i in range(seconds(length)):
        t = i / RATE
        cough = 0.5 * (1 + math.cos(2 * math.pi * coughs_per_s * t))
        chop = cough * cough
        phase += 2 * math.pi * 55 / RATE
        rumble = 0.35 * square(phase)
        hiss = 0.25 * rng.uniform(-1, 1)
        out.append(chop * (rumble + hiss) * (1 - 0.3 * t / length))
    return out


def sfx():
    # Countdown: one short bip for each of 3, 2 and 1, all at one pitch, and
    # the lights-out rise for Go, so the rocket start can be timed by ear.
    write("countdown-beep", tame(countdown_beep()))
    write("go", tame(go()))
    write("hop", tame(hop()))
    write("mini-turbo", tame(mini_turbo()))
    write("item-pickup", tame(item_pickup()))
    write("coin-pickup", tame(coin_pickup()))
    write("coin-loss", tame(coin_loss()))
    write("item-fire", tame(item_fire()))
    write("lap", tame(lap()))
    # The chime plays whole as the drift's tier rises to blue and again to
    # orange; the sparks say which tier.
    write("tier-chime", tame(tier_chime()))
    write("boost-pad", tame(boost_pad()))
    write("wrong-way", tame(wrong_way()))
    # In place of the lap as the kart finishes.
    write("finish", tame(finish()))
    # Loops: 1.5 s, crossfaded to loop without a click, at LOOP_SPEED.
    write("drift-skid", tame(drift_skid(LOOP_SPEED)))
    write("grass", tame(grass(LOOP_SPEED)))
    write("sand", tame(sand(LOOP_SPEED)))
    write("hit", hit())
    # As long as the stall (`START_STALL_S`).
    write("sputter", sputter(2.0))


# The loop: 32 bars of 4/4 at 120 bpm (64 s) in C, four sections of eight
# bars with one chord a bar. A states the theme, B moves to the relative
# minor with longer notes, A' answers the theme an octave up, and the
# bridge drops to a half-time breakdown, builds back, and runs down the
# scale into A's first note, so the last bar leads straight into the first
# (ADR 0069).
BPM = 120
BEAT = 60 / BPM
BAR = 4 * BEAT
EIGHTH = BEAT / 2

TRIADS = {
    "C": (261.63, 329.63, 392.00),  # C E G
    "G": (196.00, 246.94, 293.66),  # G B D
    "Am": (220.00, 261.63, 329.63),  # A C E
    "F": (174.61, 220.00, 261.63),  # F A C
    "Em": (164.81, 196.00, 246.94),  # E G B
}


class Section(NamedTuple):
    chords: list
    lead: list
    grooves: list


def arpeggio(*bars):
    """Eight even eighths a bar, as (semitones, eighths) pairs."""
    assert all(len(bar) == 8 for bar in bars), "an arpeggio bar is eight eighths"
    return [(semitones, 1) for bar in bars for semitones in bar]


# Each section: its chords, its lead as (semitones from C4, eighths) pairs
# played an octave up, and its groove. "full" is four on the floor with
# eighth hi-hats; "backbeat" adds a snare on two and four; "half" keeps one
# kick and one sustained chord a bar and no hats; "fill" is a backbeat bar
# whose snare plays every eighth, swelling.
SECTIONS = [
    Section(
        ["C", "G", "Am", "F", "C", "G", "Am", "F"],
        arpeggio(
            [0, 4, 7, 4, 7, 12, 7, 4],
            [-5, 2, 7, 2, 7, 11, 7, 2],
            [-3, 4, 9, 4, 9, 12, 9, 4],
            [-7, 0, 5, 0, 5, 9, 5, 0],
            [0, 4, 7, 4, 7, 12, 7, 4],
            [-5, 2, 7, 2, 7, 11, 14, 11],
            [-3, 4, 9, 4, 9, 12, 16, 12],
        )
        + [(12, 1), (9, 1), (5, 1), (9, 1), (12, 4)],
        ["full"] * 8,
    ),
    Section(
        ["Am", "F", "C", "G", "Am", "F", "G", "G"],
        [
            (9, 3), (7, 1), (4, 4),
            (5, 3), (4, 1), (0, 4),
            (4, 2), (7, 2), (12, 4),
            (11, 3), (9, 1), (7, 4),
            (9, 3), (11, 1), (12, 4),
            (14, 3), (12, 1), (9, 4),
            (11, 2), (12, 2), (14, 2), (11, 2),
            (7, 8),
        ],
        ["backbeat"] * 7 + ["fill"],
    ),
    Section(
        ["C", "G", "Am", "F", "C", "G", "Am", "F"],
        arpeggio(
            [12, 7, 4, 7, 12, 16, 12, 7],
            [11, 7, 2, 7, 11, 14, 11, 7],
            [12, 9, 4, 9, 12, 16, 12, 9],
            [12, 9, 5, 9, 12, 17, 12, 9],
            [16, 12, 7, 12, 16, 19, 16, 12],
            [14, 11, 7, 11, 14, 19, 14, 11],
            [16, 12, 9, 12, 16, 21, 16, 12],
        )
        + [(17, 2), (16, 2), (14, 2), (12, 2)],
        ["backbeat"] * 8,
    ),
    Section(
        ["F", "G", "Em", "Am", "F", "G", "C", "G"],
        [
            (9, 6), (7, 2),
            (11, 6), (14, 2),
            (7, 4), (11, 4),
            (12, 8),
            (9, 2), (12, 2), (17, 2), (12, 2),
            (11, 2), (14, 2), (19, 2), (14, 2),
            (16, 2), (12, 2), (7, 2), (12, 2),
        ]
        + arpeggio([14, 12, 11, 9, 7, 5, 4, 2]),
        ["half"] * 4 + ["full"] * 3 + ["fill"],
    ),
]


def semitone_freq(semitones, base=261.63):
    return base * 2 ** (semitones / 12)


def music():
    bars = sum(len(section.chords) for section in SECTIONS)
    total = sec(bars * BAR)
    out = [0.0] * total
    # The same note recurs all through the loop, so each is rendered once.
    rendered = {}

    def note(*args, **kwargs):
        key = (args, tuple(sorted(kwargs.items())))
        if key not in rendered:
            rendered[key] = tone(*args, **kwargs)
        return rendered[key]

    def add(at_s, samples, gain=1.0):
        start = sec(at_s)
        for i, s in enumerate(samples):
            out[(start + i) % total] += s * gain  # a tail past the end wraps into the loop's head

    kick = tone(60, 0.15, decay=25, sweep_to=35, gain=0.5)
    hat = highpass(noise(0.03, decay=120, gain=0.08, seed=7), 5000)
    snare = mix(
        highpass(noise(0.15, decay=22, gain=0.22, seed=8), 1500),
        tone(190, 0.08, triangle, decay=30, gain=0.15),
    )

    bar = 0
    for chords, lead, grooves in SECTIONS:
        assert sum(length for _, length in lead) == 8 * len(chords), "a lead fills its section"
        assert len(grooves) == len(chords), "a groove for every bar"
        section_s = bar * BAR
        for chord_name, groove in zip(chords, grooves):
            triad = TRIADS[chord_name]
            root = triad[0] / 2  # the bass an octave down
            bar_s = bar * BAR
            if groove == "half":
                add(bar_s, note(root, BAR, square, decay=1.2, gain=0.12))
                add(bar_s, kick)
                for freq in triad:
                    add(bar_s, note(freq, BAR, triangle, decay=0.8, gain=0.07))
            else:
                for eighth in range(8):
                    t = bar_s + eighth * EIGHTH
                    if groove == "full":
                        pitch = root if eighth % 4 != 3 else triad[2] / 2
                    else:
                        pitch = root if eighth % 2 == 0 else root * 2  # the octave bounce
                    add(t, note(pitch, EIGHTH, square, decay=4, gain=0.12))
                    add(t, hat)
                    if eighth % 2 == 0:
                        add(t, kick)
                    if groove == "fill":
                        add(t, snare, 0.4 + 0.6 * eighth / 7)  # from 0.4 to full by the last eighth
                    elif groove == "backbeat" and eighth in (2, 6):
                        add(t, snare)
                for beat in range(4):
                    for freq in triad:
                        add(bar_s + beat * BEAT, note(freq, BEAT, triangle, decay=2.5, gain=0.07))
            bar += 1
        t = section_s
        for semitones, length in lead:
            span = length * EIGHTH
            # An eighth keeps the original loop's quick decay; a longer note
            # rings for its length rather than dying in the first eighth.
            decay = 5 if length == 1 else 2.5 / span
            add(t, note(semitone_freq(semitones) * 2, span, triangle, decay=decay, gain=0.14))
            t += span
    write("music-loop", tame(out))


if __name__ == "__main__":
    sfx()
    music()

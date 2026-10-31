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


# The loop: 48 bars of 4/4 at 160 bpm (72 s) in A minor, six sections of
# eight bars with one chord a bar (ADR 0069). A drive of drums, bass and
# arpeggio; a verse that states the lead; a chorus an octave up; a break
# that drops the kick and rises back; the chorus again a tone higher; and a
# drive that ends on E so the last bar resolves into the first.
BPM = 160
BEAT = 60 / BPM
BAR = 4 * BEAT
SIXTEENTH = BEAT / 4


def midi_freq(note):
    return 440.0 * 2 ** ((note - 69) / 12)


# Each chord's root, as a MIDI note in the bass octave, and its third.
CHORDS = {
    "Am": (45, 3),
    "F": (41, 4),
    "C": (48, 4),
    "G": (43, 4),
    "Dm": (50, 3),
    "E": (40, 4),
}


def triad(name, lift=0):
    root, third = CHORDS[name]
    root += lift
    return (root, root + third, root + 7)


class Section(NamedTuple):
    chords: list
    # The lead as (MIDI note, sixteenths) pairs filling the section; empty
    # for none.
    lead: list
    # "drive" is the full kit; "break" drops the kick, the snare and the
    # bass for the first half and rises back; a section's last bar is a fill.
    groove: str
    # Semitones the whole section is lifted by.
    lift: int = 0
    crash: bool = False


VERSE = [
    (69, 3), (72, 3), (76, 2), (74, 2), (72, 2), (76, 4),
    (77, 3), (76, 3), (72, 2), (69, 4), (72, 4),
    (67, 3), (72, 3), (76, 2), (79, 4), (76, 4),
    (74, 6), (71, 2), (67, 4), (71, 4),
    (69, 3), (72, 3), (76, 2), (81, 4), (79, 2), (76, 2),
    (77, 3), (81, 3), (77, 2), (76, 4), (72, 4),
    (74, 3), (79, 3), (74, 2), (71, 4), (74, 4),
    (71, 2), (74, 2), (79, 12),
]
CHORUS = [
    (81, 3), (81, 3), (84, 2), (81, 2), (79, 2), (77, 4),
    (79, 3), (79, 3), (83, 2), (79, 2), (77, 2), (74, 4),
    (76, 3), (76, 3), (81, 2), (84, 4), (83, 2), (81, 2),
    (76, 8), (72, 4), (76, 4),
    (81, 3), (81, 3), (84, 2), (86, 4), (84, 4),
    (83, 3), (81, 3), (79, 2), (83, 4), (86, 4),
    (88, 6), (84, 2), (79, 4), (84, 4),
    (83, 4), (80, 4), (76, 4), (80, 4),
]
CHORUS_CHORDS = ["F", "G", "Am", "Am", "F", "G", "C", "E"]
SECTIONS = [
    Section(["Am", "Am", "F", "F", "C", "C", "G", "G"], [], "drive", crash=True),
    Section(["Am", "F", "C", "G", "Am", "F", "G", "G"], VERSE, "drive"),
    Section(CHORUS_CHORDS, CHORUS, "drive", crash=True),
    Section(["Dm", "Dm", "Am", "Am", "F", "F", "G", "G"], [(74, 32), (76, 32), (77, 32), (79, 32)], "break"),
    Section(CHORUS_CHORDS, CHORUS, "drive", lift=2, crash=True),
    Section(["Am", "Am", "F", "F", "G", "G", "E", "E"], [], "drive"),
]
# The arpeggio's sixteenths as (chord tone, octaves up) over each bar.
ARPEGGIO = [(0, 1), (1, 1), (2, 1), (0, 2), (2, 1), (1, 1), (0, 2), (2, 1)] * 2
# The bass's eighths as semitones over the root: a gallop up the octave.
BASS = [0, 0, 12, 0, 0, 12, 0, 7]


def music():
    bars = sum(len(section.chords) for section in SECTIONS)
    total = sec(bars * BAR)
    drums = [0.0] * total
    keys = [0.0] * total
    # The same note recurs all through the loop, so each is rendered once.
    rendered = {}

    def once(key, render):
        if key not in rendered:
            rendered[key] = render()
        return rendered[key]

    def add(bus, at_s, samples, gain=1.0):
        start = sec(at_s)
        for i, s in enumerate(samples):
            bus[(start + i) % total] += s * gain  # a tail past the end wraps into the loop's head

    def saws(freq, length, decay, gain, cutoff, attack=0.0, vibrato=0.0):
        """Two saws a few cents apart, lowpassed: the lead's and the pad's body."""
        return once(
            ("saws", freq, length, decay, gain, cutoff, attack, vibrato),
            lambda: lowpass(
                mix(
                    tone(freq * 1.004, length, saw, decay, gain, attack=attack, vibrato_hz=5.5, vibrato_depth=vibrato),
                    tone(freq * 0.996, length, saw, decay, gain, attack=attack, vibrato_hz=5.5, vibrato_depth=vibrato),
                ),
                cutoff,
                2,
            ),
        )

    kick = mix(
        tone(150, 0.25, decay=12, sweep_to=45, gain=0.9, curve="exp"),
        highpass(noise(0.01, decay=200, gain=0.4, seed=40), 2000),
    )
    snare = mix(
        highpass(noise(0.18, decay=18, gain=0.5, seed=41), 1200),
        tone(200, 0.1, triangle, decay=25, gain=0.3),
    )
    hat = highpass(noise(0.04, decay=60, gain=0.3, seed=42), 7000)
    open_hat = highpass(noise(0.2, decay=14, gain=0.2, seed=43), 6000)
    crash = highpass(noise(1.5, decay=2.5, gain=0.25, seed=44), 4000)
    riser = highpass(noise(2 * BAR, decay=0, gain=0.2, seed=45), 3000)
    kicks = []

    bar = 0
    for section in SECTIONS:
        section_s = bar * BAR
        if section.crash:
            add(drums, section_s, crash)
        for index, name in enumerate(section.chords):
            bar_s = bar * BAR
            low, third, fifth = triad(name, section.lift)
            tones = (low, third, fifth)
            quiet = section.groove == "break" and index < 4
            fill = index == len(section.chords) - 1
            for step in range(16):
                t = bar_s + step * SIXTEENTH
                if not quiet:
                    if step % 4 == 0:
                        add(drums, t, kick)
                        kicks.append(sec(t))
                    if step in (4, 12) or (fill and step >= 8 and step % 2 == 0):
                        add(drums, t, snare, 1.0 if step in (4, 12) else 0.4 + 0.6 * (step - 8) / 6)
                    if step % 2 == 0 and step % 4 != 0:
                        add(drums, t, open_hat)
                add(drums, t, hat, 1.0 if step % 2 else 0.6)
                tone_index, octaves = ARPEGGIO[step]
                note = tones[tone_index] + 12 * octaves
                add(keys, t, once(("pluck", note), lambda: lowpass(tone(midi_freq(note), SIXTEENTH * 1.5, square, 14, 0.06), 6000)))
            if not quiet:
                for eighth, offset in enumerate(BASS):
                    note = low + offset
                    add(keys, bar_s + eighth * 2 * SIXTEENTH, saws(midi_freq(note), 2 * SIXTEENTH, 6, 0.11, 900))
            for note in tones:
                add(keys, bar_s, saws(midi_freq(note + 12), BAR, 0.6, 0.03, 1800, attack=0.05))
            if section.groove == "break" and index == 6:
                add(drums, bar_s, envelope(riser, lambda t: t / (2 * BAR)))  # swells over the last two bars
            bar += 1
        t = section_s
        for note, length in section.lead:
            span = length * SIXTEENTH
            # Short notes keep a quick decay; a held note rings for its length.
            decay = 6 if length <= 4 else 2.0 / span
            add(keys, t, saws(midi_freq(note + section.lift), span, decay, 0.09, 6000, attack=0.01, vibrato=0.004))
            t += span

    # The kick ducks everything but the drums, so the bass pumps around it.
    duck = [1.0] * total
    release = sec(0.12)
    for at in kicks:
        for i in range(release):
            duck[(at + i) % total] = min(duck[(at + i) % total], 1 - 0.55 * (1 - i / release))
    bus = [d + k * g for d, k, g in zip(drums, keys, duck)]
    # A soft clip for loudness; tame then keeps the peak under full scale.
    drive = 1.2
    write("music-loop", tame([math.tanh(drive * s) / math.tanh(drive) for s in bus]))


if __name__ == "__main__":
    sfx()
    music()

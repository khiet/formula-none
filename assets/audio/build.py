"""Render the game's SFX (ADR 0007, ADR 0025) as WAV files.

Run from the repo root with `pnpm export:audio`; stdlib only. Every SFX is
synthesised here, so the set is this repo's own work and CC0 like the rest
of `assets/` (LICENSES.md). The music is one bar-aligned loop whose last
sample runs straight into its first.

The cues were picked on the sound effects iteration page
(`docs/mockups/sound-effects.html`), which synthesises each one in the
browser from the DSP kit ported below, so a render here is the page's pick.
The hit and the sputter predate the page and keep their original code path,
so their bytes do not churn. The star's three sounds and the Air Horn's
blast were picked on `docs/mockups/star-and-horn-sounds.html` and are
specified, with the script ported below and the drone warning's beep, in
`docs/mockups/star-and-air-horn.html`.
"""

import math
import random
import struct
import wave
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


def place(out, at, samples, gain=1.0):
    """Add `samples` into `out` from `at` seconds, dropping what runs past its end."""
    start = sec(at)
    for i, s in enumerate(samples):
        if start + i >= len(out):
            break
        out[start + i] += s * gain


def crush(samples, hold):
    """Bit-crush by holding each sample for `hold` samples: a sample-rate drop."""
    return [samples[i - (i % hold)] for i in range(len(samples))]


def release(samples, seconds):
    """Fade the last `seconds` out linearly, so a cut-off tail does not click."""
    end = len(samples) / RATE
    return envelope(samples, lambda t: min(1.0, (end - t) / seconds))


def pops(length, per_second, pop=0.02, decay=80, gain=0.6, seed=7):
    """Random short bursts of noise at about `per_second`, for crackle and crunch."""
    n = sec(length)
    out = [0.0] * n
    draw = rng(seed)
    burst = rng(seed + 1)
    gap = RATE / per_second
    at = abs(draw()) * gap
    while at < n:
        start = math.floor(at + 0.5)
        g = gain * (0.5 + 0.5 * abs(draw()))
        for i in range(sec(pop)):
            if start + i >= n:
                break
            out[start + i] += g * math.exp(-decay * (i / RATE)) * burst()
        at += gap * (0.5 + abs(draw()))
    return out


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


def item_pickup(first=1047, second=1568, crack=0.55, gain=0.38):
    """Crack and chime: the box breaking, then two triangle notes a fifth apart, a fifth under the coin chime."""
    return mix(
        highpass(noise(0.07, decay=45, gain=crack, seed=3), 2500),
        concat(
            silence(0.02),
            tone(first, 0.06, triangle, decay=14, gain=gain),
            tone(second, 0.22, triangle, decay=9, gain=gain),
        ),
    )


def item_roll_tick(gain=0.6):
    """Click: a short bright noise burst, one per step of the slot's reel."""
    return highpass(noise(0.025, decay=140, gain=gain, seed=5), 3000)


def item_rolled(freqs=(1047, 1319, 1568), top=2093, gain=0.35):
    """Arpeggio up: triangle C, E, G then a ringing high C."""
    return concat(
        notes(freqs, 0.06, triangle, 10, gain),
        tone(top, 0.38, triangle, decay=6, gain=gain),
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


def box_chime(low=659, high=988, length=0.12, decay=8, gain=0.35):
    """Two rising pulse notes, spaced apart: the radio's call to come in for tyres."""
    return concat(
        tone(low, length, pulse, decay=decay, gain=gain),
        silence(0.06),
        tone(high, length * 1.5, pulse, decay=decay, gain=gain),
    )


def wheel_gun(bursts=16, every=0.05, burst=0.035, centre=2600, width=1800, gain=0.5, seed=5):
    """Rattle: a run of short bright bursts of bandpassed noise, a wheel gun hammering a nut off and on (ADR 0082)."""
    out = silence(bursts * every + 0.1)
    for i in range(bursts):
        place(out, i * every, bandpass(noise(burst, decay=50, gain=1.0, seed=seed + i), centre, width), gain)
    return release(out, 0.03)


def jack_drop(thud=95, length=0.22, gain=0.6, seed=9):
    """Thud: a low sine knock falling in pitch under a click of noise, the kart dropping off the jacks (ADR 0082)."""
    return mix(
        tone(thud, length, sine, decay=16, gain=gain, sweep_to=45, curve="exp"),
        noise(0.04, decay=80, gain=0.35, seed=seed),
    )


# The stop's other beats, picked on `docs/mockups/pit-crew-animations.html`
# ("The sounds of the stop"), each fired from the beat its pose reads.

# Every wheel gun is raised by this over its render: at the page's gain it
# peaks near 0.13, far under the chimes' 0.35.
WHEEL_GUN_LEVEL = 2.5
# The crew's boots, raised as the page plays them.
BOOTS_LEVEL = 1.3


def clack(seed, pitch=1.0):
    """A metal clack: a bright click of noise over a short knock."""
    return mix(
        bandpass(noise(0.05, decay=60, gain=0.9, seed=seed), 1800, 1600),
        tone(170 * pitch, 0.08, sine, decay=40, gain=0.4, sweep_to=90 * pitch, curve="exp"),
    )


def jack_lift(pitch=1.0, seed=31):
    """Pump: two clacks of the lever a beat apart, then the ram's creak rising as the kart comes up."""
    out = silence(0.7)
    place(out, 0, clack(seed, pitch))
    place(out, 0.2, clack(seed + 1, pitch))
    creak = tone(70 * pitch, 0.35, saw, decay=6, gain=0.6, sweep_to=140 * pitch)
    place(out, 0.28, lowpass(creak, 450), 0.8)
    return release(out, 0.05)


def wheel_off(seed=41):
    """Thump and roll: the worn wheel's rubber hitting the apron, then rolling away in a slow wobble."""
    roll = chop(lowpass(noise(0.8, decay=3.5, gain=0.8, seed=seed + 1), 320, 2), 9, 0.7)
    return mix(
        lowpass(noise(0.14, decay=28, gain=1.0, seed=seed), 500, 2),
        tone(120, 0.2, sine, decay=18, gain=0.5, sweep_to=60, curve="exp"),
        concat(silence(0.15), roll),
    )


def wheel_on(seed=51):
    """Clonk: the fresh wheel seated on the hub, a short square knock with a click on top."""
    return mix(
        lowpass(tone(240, 0.14, square, decay=26, gain=0.35, sweep_to=150, curve="exp"), 900),
        noise(0.03, decay=90, gain=0.5, seed=seed),
    )


def gun_stall(seed=60):
    """Stall: the gun's bursts in stutters that will not take, a labouring grind between them."""
    out = silence(1.3)
    for i, at in enumerate([0, 0.05, 0.1, 0.42, 0.47, 0.82, 0.87, 0.92, 0.97]):
        burst = noise(0.045, decay=45, gain=1.0, seed=seed + i)
        place(out, at, bandpass(burst, 2300 - i * 50, 1800), 0.5)
    for at, freq in [(0.15, 95), (0.55, 85)]:
        grind = chop(tone(freq, 0.25, saw, decay=4, gain=0.45), 28, 1)
        place(out, at, lowpass(grind, 500))
    return release(out, 0.04)


def jack_fail(seed=91):
    """Clang and drop: the jack's lever ringing, the nose hitting the ground harder than a jack drop, a rattle of loose parts."""
    rattle = pops(0.4, 30, pop=0.015, decay=100, gain=0.5, seed=seed + 1)
    return mix(
        tone(1180, 0.3, triangle, decay=10, gain=0.25),
        tone(1770, 0.25, triangle, decay=14, gain=0.12),
        concat(silence(0.12), jack_drop(thud=80, length=0.36, gain=0.7, seed=seed)),
        concat(silence(0.18), lowpass(rattle, 1500)),
    )


def lollipop_spin(seed=71):
    """Whoosh and clack: the disc swinging round and hitting its stop."""
    out = silence(0.5)
    whoosh = bandpass(noise(0.36, decay=0, gain=0.9, seed=seed), 1000, 1000)
    place(out, 0, envelope(whoosh, lambda t: math.sin(math.pi * min(1.0, t / 0.36))))
    place(out, 0.34, clack(seed + 1, 1.6), 0.6)
    return release(out, 0.04)


def release_chime(low=988, high=1319, length=0.12, decay=8, gain=0.35):
    """The box chime's answer: the same two pulse notes a fourth up, the radio's go."""
    return box_chime(low, high, length, decay, gain)


def boots(length=0.9, seed=21):
    """Boots: a patter of soft lowpassed pops, the crew running on concrete."""
    patter = pops(length, 16, pop=0.025, decay=90, gain=0.6, seed=seed)
    return release(lowpass(patter, 700, 2), 0.1)


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


# The presentation zones' ambience (ADR 0098): eight-second beds, so the loop
# is not heard over a straight, each swell a whole number of cycles in the
# loop. The game levels them by the zone's blend and never pitches them.
AMBIENCE_S = 8.0


def harbour(swell_hz=0.25, gain=0.5, seed=101):
    """Water slapping on the quay, swelling twice a loop, and two gulls calling over it."""
    water = noise(AMBIENCE_S + 0.05, decay=0, gain=gain, seed=seed)
    slap = envelope(
        scale(lowpass(water, 500, 2), 3),
        lambda t: 0.55 + 0.45 * math.sin(TAU * swell_hz * t),
    )
    out = mix(slap, silence(AMBIENCE_S + 0.05))
    for at, high in [(1.3, 1900), (1.62, 1750), (5.4, 2100)]:
        cry = tone(high, 0.22, triangle, decay=6, gain=0.12, sweep_to=high * 0.7, curve="exp")
        place(out, at, cry)
    return loopable(out)


def crowd(swell_hz=0.125, gain=0.5, seed=111):
    """A grandstand's murmur: a voice band of noise, a slow swell and a ripple of chatter."""
    bed = noise(AMBIENCE_S + 0.05, decay=0, gain=gain, seed=seed)
    murmur = chop(scale(bandpass(bed, 700, 900), 3), 6, 0.25)
    return loopable(envelope(murmur, lambda t: 0.7 + 0.3 * math.sin(TAU * swell_hz * t)))


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


# ---------------------------------------------------------------------------
# The star (ADR 0075): six seconds of fast 8-bit music in the race music's
# key and tempo, D minor at 160 bpm, where four bars are exactly the run,
# over its verse's chords; a rising run for the fire and three falling
# blips for the end. Original; none of it borrows from Nintendo's theme.
# STAR_RUN_S copies `STAR_S` in crates/sim/src/tuning.rs; a retune there
# moves it here and the music is rebuilt to the new length.
# ---------------------------------------------------------------------------

STAR_BEAT = 0.375
STAR_SIXTEENTH = STAR_BEAT / 4
STAR_RUN_S = 6.0
# The last second tumbles downward on its own, so the driver hears the end coming.
STAR_LAST_S = 5.0
STAR_CHORDS = ((62, 65, 69), (58, 62, 65), (60, 64, 67), (57, 61, 64))


def star_music():
    """Pulse-wave arpeggios climbing an octave halfway, a square lead, a triangle bass, a tick and a snare, then 22 notes tumbling down."""
    out = silence(STAR_RUN_S)
    snare = bandpass(noise(0.08, decay=35, gain=0.7, seed=13), 2000, 3000)
    tick = highpass(noise(0.03, decay=80, gain=0.2, seed=14), 6000)
    lead = (74, 77, 81, 77, 82, 77, 74, 77, 84, 79, 76, 79, 85)
    stop = math.floor(STAR_LAST_S / STAR_SIXTEENTH)
    for i in range(stop):
        bar = i // 16
        chord_notes = STAR_CHORDS[bar]
        steps = (chord_notes[0], chord_notes[1], chord_notes[2], chord_notes[0] + 12)
        n = steps[(0, 1, 2, 3, 2, 1)[i % 6]] + (12, 12, 24, 24)[bar]
        place(out, i * STAR_SIXTEENTH, tone(midi_freq(n), STAR_SIXTEENTH * 0.9, pulse, decay=10, gain=0.15))
        if i % 2 == 0:
            place(out, i * STAR_SIXTEENTH, tone(midi_freq(chord_notes[0] - 24), 0.16, triangle, decay=6, gain=0.45))
            place(out, i * STAR_SIXTEENTH, tick)
        if i % 8 == 4:
            place(out, i * STAR_SIXTEENTH, snare)
    for beat, n in enumerate(lead):
        place(
            out,
            beat * STAR_BEAT,
            tone(midi_freq(n), STAR_BEAT * 0.9, square, decay=3, gain=0.1, vibrato_hz=7, vibrato_depth=0.008),
        )
    for i in range(22):
        place(
            out,
            STAR_LAST_S + i * 0.0455,
            tone(midi_freq(91 - i), 0.05, square, decay=5, gain=0.17 * (1 - i / 30)),
        )
    return tame(out)


# The fire's level under the spec's: with the music, the race music still
# fading and the engine, the spec's level summed past 1.0 in the fire's
# first 24 ms, where Web Audio clips; this keeps the worst case under it.
STAR_FIRE_LEVEL = 0.7


def star_fire():
    """A two-octave rising run of square-wave notes, in place of the item fire."""
    out = silence(0.4)
    for i in range(18):
        place(out, i * 0.018, tone(midi_freq(62 + i * 2), 0.05, square, decay=4, gain=0.24))
    return scale(tame(out), STAR_FIRE_LEVEL)


def star_end():
    """Three falling square-wave blips as the run ends."""
    return tame(concat(*(tone(midi_freq(n), 0.08, square, decay=8, gain=0.3) for n in (76, 72, 64))))


def air_horn():
    """Two square waves a fifth apart, sagging in pitch, then bit-crushed: the grandstand horn."""
    return tame(
        release(
            crush(
                mix(
                    tone(196, 0.55, square, decay=1.5, gain=0.34, sweep_to=175, curve="exp"),
                    tone(294, 0.55, square, decay=1.5, gain=0.26, sweep_to=262, curve="exp"),
                ),
                8,
            ),
            0.03,
        )
    )


def drone_beep():
    """One square-wave beep; the client repeats it faster and higher as the drone closes."""
    return tone(1320, 0.07, square, decay=12, gain=0.3)


# Held under the other cues' levels: with no tone to fight the engine's, the
# whoosh reads as air and not as a hit.
SLIPSTREAM_LEVEL = 0.45


def slipstream(length=0.3, seed=21):
    """Whoosh: a swell of bandpassed noise with no tone, up over the first half and let go."""
    air = noise(length, decay=0.0, gain=1.0, seed=seed)
    air = envelope(air, lambda t: math.sin(math.pi * min(t / length, 1.0)) ** 2)
    air = lowpass(highpass(air, 400), 2600, passes=2)
    return scale(tame(release(air, 0.03)), SLIPSTREAM_LEVEL)


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
    # On every box the kart takes, then a click on each step of the slot's
    # reel and the arpeggio as the item lands (ADR 0070).
    write("item-pickup", tame(item_pickup()))
    write("item-roll-tick", tame(item_roll_tick()))
    write("item-rolled", tame(item_rolled()))
    write("coin-pickup", tame(coin_pickup()))
    write("coin-loss", tame(coin_loss()))
    write("item-fire", tame(item_fire()))
    write("lap", tame(lap()))
    # The chime plays whole as the drift's tier rises to blue and again to
    # orange; the sparks say which tier.
    write("tier-chime", tame(tier_chime()))
    # The BOX call (ADR 0080), as tyre life first falls to the cliff.
    write("box-chime", tame(box_chime()))
    # The stop (ADR 0082), a cue on each beat: the crew's boots on the
    # catch, the jacks up on the mark, the guns off, each wheel off and on,
    # the guns on in a shorter run, the sign's spin and on green the jack's
    # drop and the radio's go. The tier's act has the gun's stall, the
    # jack's failure, a short run as a stuck nut frees and the redo's runs.
    write("boots", scale(tame(boots()), BOOTS_LEVEL))
    write("jack-lift", tame(jack_lift()))
    write("wheel-gun", scale(tame(wheel_gun()), WHEEL_GUN_LEVEL))
    write("wheel-off", tame(wheel_off()))
    write("wheel-on", tame(wheel_on()))
    write("wheel-gun-on", scale(tame(wheel_gun(bursts=10)), WHEEL_GUN_LEVEL))
    write("lollipop-spin", tame(lollipop_spin()))
    write("jack-drop", tame(jack_drop()))
    write("release-chime", tame(release_chime()))
    write("gun-stall", tame(gun_stall()))
    write("jack-fail", tame(jack_fail()))
    write("wheel-gun-act", scale(tame(wheel_gun(bursts=6)), WHEEL_GUN_LEVEL))
    write("wheel-gun-redo", scale(tame(wheel_gun(bursts=8)), WHEEL_GUN_LEVEL))
    write("boost-pad", tame(boost_pad()))
    write("wrong-way", tame(wrong_way()))
    # In place of the lap as the kart finishes.
    write("finish", tame(finish()))
    # The star's run (ADR 0075): its music through the run, its fire in
    # place of the item fire, and its end.
    write("star-music", star_music())
    write("star-fire", star_fire())
    write("star-end", star_end())
    # The Air Horn's blast (ADR 0076), for any kart's fire and in place of
    # the item fire for the own kart, and the beep the drone warning
    # repeats.
    write("air-horn", air_horn())
    write("drone-beep", drone_beep())
    # The slipstream's whoosh (ADR 0077), as the own kart's tow takes.
    write("slipstream", slipstream())
    # Loops: 1.5 s, crossfaded to loop without a click, at LOOP_SPEED.
    write("drift-skid", tame(drift_skid(LOOP_SPEED)))
    write("grass", tame(grass(LOOP_SPEED)))
    write("sand", tame(sand(LOOP_SPEED)))
    write("harbour", tame(harbour()))
    write("crowd", tame(crowd()))
    write("hit", hit())
    # As long as the stall (`START_STALL_S`).
    write("sputter", sputter(2.0))


# ---------------------------------------------------------------------------
# The music loop (ADR 0069): 64 bars of 4/4 at 160 bpm (96 s) in D minor.
#
#   bars  0-7   intro: the groove filters open; a distant trumpet previews
#               the chorus hook
#   bars  8-15  verse: synth lead over piano
#   bars 16-23  build: galloping bass, snare roll and an engine rev
#   bars 24-31  chorus: trumpet melody over Bb C Am Dm, strings beneath
#   bars 32-39  breakdown: no kick, the trumpet over piano
#   bars 40-47  bridge: disco-rock band, climbing Gm Am Bb C, landing on D,
#               the dominant of the lifted key
#   bars 48-55  last chorus a tone up: trumpets in harmony, horn section,
#               strings, timpani, glockenspiel and piano
#   bars 56-63  final lap: the chorus again, strings on the melody, the
#               trumpets on a counter-line and a drum stop
#
# The last two bars of each chorus fall back to A, so the loop's end resolves
# into its first bar.
# ---------------------------------------------------------------------------


def midi_freq(note):
    return 440.0 * 2 ** ((note - 69) / 12)


# Each chord's root as a MIDI note in the bass register, and its third.
CHORDS = {
    "Dm": (38, 3),
    "Bb": (34, 4),
    "C": (36, 4),
    "A": (33, 4),
    "Gm": (43, 3),
    "D": (38, 4),
    "Am": (45, 3),
}
# The note each pad adds over its chord, in semitones above the root: the
# major sevenths, ninths and sevenths that give the chorus its ache.
COLOUR = {"Bb": 11, "C": 14, "Am": 10, "Dm": 14, "Gm": 10}

INTRO_CHORDS = ["Dm", "Dm", "Bb", "Bb", "C", "C", "A", "A"]
VERSE_CHORDS = ["Dm", "Bb", "C", "A", "Dm", "Bb", "C", "A"]
BUILD_CHORDS = ["Gm", "Gm", "Bb", "Bb", "C", "C", "A", "A"]
CHORUS_CHORDS = ["Bb", "C", "Am", "Dm", "Bb", "C", "A", "A"]
BRIDGE_CHORDS = ["Gm", "Am", "Bb", "C", "Gm", "Am", "C", "D"]
# The last chorus and the final lap lift their first six bars a tone, into E
# minor, and drop back for the two bars of A that lead into the loop's head.
LIFT = 2
LIFTED_BARS = 6

# Phrases are (MIDI note or None for a rest, sixteenths) pairs filling eight bars.
VERSE = [
    (74, 2), (72, 2), (69, 2), (74, 3), (76, 3), (77, 4),
    (77, 2), (76, 2), (74, 2), (70, 4), (74, 2), (77, 4),
    (79, 3), (77, 3), (76, 2), (72, 4), (76, 2), (79, 2),
    (76, 6), (73, 2), (69, 4), (73, 2), (76, 2),
    (74, 2), (72, 2), (69, 2), (74, 3), (76, 3), (81, 4),
    (82, 2), (81, 2), (77, 2), (74, 4), (77, 2), (82, 4),
    (84, 3), (82, 3), (81, 2), (79, 2), (77, 2), (76, 4),
    (73, 4), (76, 4), (81, 8),
]
# Long notes climbing over the build, then two beats of silence before the drop.
CLIMB = [(74, 16), (77, 16), (77, 8), (79, 8), (81, 16), (79, 8), (81, 8), (84, 16), (85, 16), (88, 8), (None, 8)]
CHORUS = [
    (74, 2), (77, 2), (81, 8), (79, 2), (77, 2),
    (76, 2), (77, 2), (79, 8), (74, 4),
    (72, 2), (76, 2), (79, 6), (77, 2), (76, 4),
    (77, 12), (74, 4),
    (74, 2), (77, 2), (86, 8), (84, 2), (82, 2),
    (81, 2), (82, 2), (84, 8), (79, 4),
    (81, 4), (85, 4), (88, 8),
    (86, 6), (85, 2), (81, 8),
]
# The chorus head, far away over the intro's Bb, C and A.
FANFARE = [
    (None, 32),
    (74, 2), (77, 2), (81, 8), (79, 2), (77, 2), (None, 16),
    (76, 2), (79, 2), (84, 8), (81, 2), (79, 2), (None, 16),
    (76, 2), (81, 2), (85, 12), (None, 16),
]
BRIDGE = [
    (70, 4), (69, 2), (67, 2), (70, 8),
    (72, 6), (69, 2), (76, 8),
    (77, 6), (74, 2), (72, 4), (74, 4),
    (76, 12), (72, 4),
    (74, 4), (70, 2), (74, 2), (79, 8),
    (76, 6), (72, 2), (81, 8),
    (79, 8), (76, 4), (79, 4),
    (78, 4), (81, 4), (None, 8),
]
# Long notes on the chorus's chords: under the first chorus an octave down,
# and on top of the final lap.
COUNTER = [
    (77, 8), (81, 8), (79, 8), (76, 8), (76, 8), (72, 8), (74, 16),
    (77, 8), (81, 8), (79, 8), (84, 8), (85, 8), (88, 8), (85, 16),
]


def lifted(phrase, lift=LIFT, bars=LIFTED_BARS):
    """Lift the notes that start within the first `bars` bars."""
    out, t = [], 0
    for note, length in phrase:
        out.append((note + lift if note is not None and t < 16 * bars else note, length))
        t += length
    return out


def harmonise(phrase, lift=LIFT, bars=LIFTED_BARS):
    """A chorus chord tone three to nine semitones above each note, so the harmony never rubs a semitone against the chord."""
    out, t = [], 0
    for note, length in phrase:
        if note is None:
            out.append((None, length))
        else:
            root, third = CHORDS[CHORUS_CHORDS[(t // 16) % len(CHORUS_CHORDS)]]
            r = root + (lift if t < 16 * bars else 0)
            tones = {r % 12, (r + third) % 12, (r + 7) % 12}
            out.append((next(note + k for k in range(3, 10) if (note + k) % 12 in tones), length))
        t += length
    return out


def chord_tones(name, lift=0):
    root, third = CHORDS[name]
    r = root + lift
    return [r, r + third, r + 7]


class Song:
    """Two buses the length of the loop: drums, and everything pitched, which the kick ducks and the reverb fills."""

    def __init__(self, bpm, bars):
        self.beat = 60 / bpm
        self.bar = 4 * self.beat
        self.six = self.beat / 4
        self.total = sec(bars * self.bar)
        self.drums = [0.0] * self.total
        self.keys = [0.0] * self.total
        self.kicks = []
        self.rendered = {}

    def once(self, key, render):
        """The same note recurs all through the loop, so each is rendered once."""
        if key not in self.rendered:
            self.rendered[key] = render()
        return self.rendered[key]

    def at(self, bar, step=0):
        """Seconds to sixteenth `step` of `bar`."""
        return bar * self.bar + step * self.six

    def add(self, bus, at_s, samples, gain=1.0):
        start = sec(at_s)
        total = self.total
        for i, s in enumerate(samples):
            bus[(start + i) % total] += s * gain  # a tail past the end wraps into the loop's head

    def kick(self, at_s, samples, gain=1.0):
        self.add(self.drums, at_s, samples, gain)
        self.kicks.append(sec(at_s))

    def melody(self, bar0, phrase, voice, echo=()):
        """Play `phrase` from `bar0` with `voice(note, seconds, sixteenths)`; `echo` repeats each note as (sixteenths later, gain)."""
        t = self.at(bar0)
        for note, length in phrase:
            span = length * self.six
            if note is not None:
                samples = voice(note, span, length)
                self.add(self.keys, t, samples)
                for delay, gain in echo:
                    self.add(self.keys, t + delay * self.six, samples, gain)
            t += span


def detuned(song, freq, length, decay, gain, cutoff, voices=2, spread=0.008, attack=0.0, vibrato=0.0):
    """Saws spread `spread` either side of `freq`, lowpassed: the stabs', pads' and strings' body."""
    def render():
        parts = []
        for v in range(voices):
            d = 1 + spread * (v / (voices - 1) - 0.5) * 2 if voices > 1 else 1
            parts.append(tone(freq * d, length, saw, decay, gain / math.sqrt(voices), attack=attack, vibrato_hz=5.5, vibrato_depth=vibrato))
        return lowpass(mix(*parts), cutoff, 2)

    return song.once(("detuned", freq, length, decay, gain, cutoff, voices, spread, attack, vibrato), render)


class Kit:
    def __init__(self, song):
        self.song = song
        self.kick = mix(
            tone(170, 0.28, decay=11, sweep_to=44, gain=1.0, curve="exp"),
            tone(55, 0.3, sine, decay=9, gain=0.35),
            highpass(noise(0.012, decay=200, gain=0.55, seed=50), 2500),
        )
        # Three bursts 11 ms apart, the last ringing longest.
        self.clap = mix(*[
            [0.0] * sec(0.011 * k) + highpass(lowpass(noise(0.16, decay=22 if k == 2 else 90, gain=0.45, seed=51 + k), 3000), 900)
            for k in range(3)
        ])
        self.hat = highpass(noise(0.035, decay=70, gain=0.28, seed=55), 8000)
        self.open_hat = highpass(noise(0.18, decay=16, gain=0.22, seed=56), 7000)
        self.crash = highpass(noise(2.0, decay=1.8, gain=0.3, seed=57), 4000)
        self.impact = mix(tone(90, 1.2, sine, decay=3, sweep_to=30, gain=0.5, curve="exp"), lowpass(noise(0.8, decay=5, gain=0.3, seed=58), 600))
        self.riser = highpass(noise(4 * song.bar, decay=0, gain=0.25, seed=59), 2000)
        self.toms = [
            mix(tone(f, 0.3, sine, decay=10, sweep_to=f * 0.55, gain=0.55), lowpass(noise(0.05, decay=60, gain=0.2, seed=90 + k), 2500))
            for k, f in enumerate((220, 170, 130, 95))
        ]
        # Snares whose body pitches up, for rolls that climb.
        self.roll = [
            mix(highpass(noise(0.12, decay=26, gain=0.45, seed=60 + k), 1200), tone(190 * 1.25**k, 0.08, triangle, decay=30, gain=0.3))
            for k in range(4)
        ]
        self.tambourine = mix(
            highpass(noise(0.07, decay=45, gain=0.22, seed=80), 7000),
            bandpass(noise(0.07, decay=50, gain=0.12, seed=81), 9500, 2000),
        )
        self.snare = mix(highpass(noise(0.2, decay=17, gain=0.5, seed=82), 1100), tone(200, 0.1, decay=25, gain=0.3))

    def snare_roll(self, bar0, bars):
        """Eighths, then sixteenths, swelling and pitching up over `bars` bars, stopping two beats short."""
        s = self.song
        steps = 16 * bars
        for k in range(steps - 8):
            u = k / steps
            if u < 0.5 and k % 2:
                continue
            s.add(s.drums, s.at(bar0, k), self.roll[min(3, int(u * 4))], 0.2 + 0.8 * u)

    def riser_into(self, bar):
        """A noise swell over the four bars before `bar`, cut two beats early."""
        s = self.song
        r = envelope(self.riser, lambda t: (t / (4 * s.bar)) ** 2 if t < 4 * s.bar - 2 * s.beat else 0.0)
        s.add(s.drums, s.at(bar - 4), r)

    def drop(self, bar):
        s = self.song
        s.add(s.drums, s.at(bar), self.crash)
        s.add(s.drums, s.at(bar), self.impact)

    def tom_fill(self, bar):
        s = self.song
        for k, step in enumerate((8, 10, 12, 13, 14, 15)):
            s.add(s.drums, s.at(bar, step), self.toms[min(3, k * 4 // 6)], 0.9)

    def stop(self, bar):
        """Silence the drums, kicks included, for the bar's second half, then a climbing snare pickup in its last beat."""
        s = self.song
        start, end = sec(s.at(bar, 8)), sec(s.at(bar + 1))
        for i in range(start, end):
            s.drums[i] = 0.0
        s.kicks = [k for k in s.kicks if not start <= k < end]
        for k, step in enumerate((12, 13, 14, 15)):
            s.add(s.drums, s.at(bar, step), self.roll[k], 0.5 + 0.15 * k)


def bass_note(song, note, root, length, cutoff, grit):
    """A driven saw an octave above a sine sub on the chord's root."""
    def render():
        body = lowpass(mix(tone(midi_freq(note) * 1.003, length, saw, 6, 0.2), tone(midi_freq(note) * 0.997, length, saw, 6, 0.2)), cutoff, 2)
        driven = [math.tanh(grit * x) / grit for x in body]
        sub = tone(midi_freq(root), length, sine, 3, 0.3)
        return [x + y for x, y in zip(driven, sub)]

    return song.once(("bass", note, root, length, cutoff, grit), render)


def synth_lead(song, gain=1.0):
    """The verse's saw-square lead; a held note rings for its length."""
    def voice(note, span, length):
        decay = 5 if length <= 4 else 1.4 / span

        def render():
            f = midi_freq(note)
            body = mix(
                tone(f, span, saw, decay, 0.06, attack=0.005, vibrato_hz=6, vibrato_depth=0.004),
                tone(f * 1.003, span, square, decay, 0.04, attack=0.005, vibrato_hz=6, vibrato_depth=0.004),
                tone(f * 0.995, span, saw, decay, 0.035, attack=0.005),
            )
            return [x * gain for x in lowpass(body, 5500, 2)]

        return song.once(("lead", note, span, gain), render)

    return voice


def brass(song, note, length, gain, bright=1.0, voices=1, swell=False, shake=False, fall=False):
    """A trumpet: a scoop up into pitch, brightness that follows loudness, and delayed vibrato.

    `swell` crescendos over the note; `shake` trills its second half a tone up;
    `fall` drops the pitch off its end.
    """
    def render():
        n = sec(length)
        out = [0.0] * n
        release = min(0.05, length * 0.3)
        for v in range(voices):
            base = midi_freq(note) * (1 + 0.004 * (v - (voices - 1) / 2))
            tilt = (base / 440) ** 0.3
            phase, y1, y2 = 0.0, 0.0, 0.0
            for i in range(n):
                t = i / RATE
                if swell:
                    a = (t / length) ** 1.6
                else:
                    a = min(1.0, t / 0.025) * (1 - 0.2 * min(1.0, max(0.0, (t - 0.03) / 0.15)))
                if t > length - release:
                    a *= max(0.0, (length - t) / release)
                f = base * (1 - 0.02 * math.exp(-t / 0.02))
                if shake and t > length * 0.45:
                    u = math.sin(TAU * 7 * (t - length * 0.45))
                    f *= 2 ** (1 / 12 * (1 + math.tanh(4 * u)))
                    a *= 1.1
                elif t > 0.2:
                    f *= 1 + 0.005 * math.sin(TAU * 5.5 * (t - 0.2)) * min(1.0, (t - 0.2) / 0.2)
                if fall and t > length * 0.55:
                    f *= 1 - 0.3 * ((t - length * 0.55) / (length * 0.45)) ** 2
                phase += TAU * f / RATE
                x = 0.75 * saw(phase) + 0.25 * square(phase)
                # A two-pole lowpass whose cutoff rises with loudness, as a real horn brightens.
                g = 1 - math.exp(-TAU * (250 + bright * (400 + 5000 * min(1.0, a)) * tilt) / RATE)
                y1 += g * (x - y1)
                y2 += g * (y1 - y2)
                out[i] += gain / math.sqrt(voices) * a * math.tanh(1.6 * y2)
        return out

    return song.once(("brass", note, round(length, 5), gain, bright, voices, swell, shake, fall), render)


def trumpet(song, gain, voices=1, bright=1.0, shake_from=None, fall_at=()):
    """A trumpet voice for `Song.melody`: notes `shake_from` sixteenths or longer shake, and the notes at indexes `fall_at` fall off."""
    count = [0]

    def voice(note, span, length):
        index = count[0]
        count[0] += 1
        shake = shake_from is not None and length >= shake_from
        return brass(song, note, span, gain, bright, voices, shake=shake, fall=index in fall_at)

    return voice


def horn_chord(song, bar, step, name, length, lift=0, gain=1.0, swell=False, fall=False):
    """Three trumpets close on the chord between E4 and E5, and a trombone on the root below."""
    r = CHORDS[name][0] + lift
    trumpets = sorted(60 + n % 12 + (12 if 60 + n % 12 < 64 else 0) for n in chord_tones(name, lift))
    t = song.at(bar, step)
    for n in trumpets:
        song.add(song.keys, t, brass(song, n, length, 0.17, voices=2, swell=swell, fall=fall), gain)
    song.add(song.keys, t, brass(song, 48 + r % 12, length, 0.2, bright=0.7, voices=2, swell=swell, fall=fall), gain)


def horn_answers(song, bar, index, name, lift=0, gain=1.0):
    """Stabs in the gap while the chorus melody holds its long note: short, short, long; quarter hits in its seventh bar."""
    if index in (0, 1, 2, 4, 5):
        for step, length in ((8, 1.5), (10, 1.5), (12, 3.5)):
            horn_chord(song, bar, step, name, length * song.six, lift, 0.8 * gain, fall=step == 12 and index == 5)
    elif index == 6:
        for step in (0, 4, 8, 12):
            horn_chord(song, bar, step, name, 1.8 * song.six, lift, 0.9 * gain)


def piano(song, note, length, gain=1.0):
    def render():
        f = midi_freq(note)
        body = mix(
            tone(f, length, triangle, 3.5, 0.10),
            tone(2 * f, length, sine, 6, 0.04),
            tone(3 * f, length, sine, 9, 0.015),
            lowpass(noise(0.01, decay=300, gain=0.03, seed=note), 3000),
        )
        return [x * gain for x in body]

    return song.once(("piano", note, length, gain), render)


def piano_bar(song, bar, name, gain=1.0):
    """A broken chord in eighths, climbing from the root an octave above the bass."""
    root, third = CHORDS[name]
    r = root + 12 if root < 45 else root
    for k, n in enumerate([r, r + 7, r + 12, r + third + 12, r + 19, r + third + 12, r + 12, r + 7]):
        song.add(song.keys, song.at(bar, 2 * k), piano(song, n, 2 * song.beat, gain))


def colour_pad(song, bar, name, lift=0, gain=1.0):
    if name in COLOUR:
        root = CHORDS[name][0] + lift
        song.add(song.keys, song.at(bar), detuned(song, midi_freq(root + COLOUR[name] + 24), song.bar, 0.8, 0.03 * gain, 2500, voices=5, spread=0.01, attack=0.04))


def strings(song, notes, length, gain, attack=0.25):
    def render():
        return mix(*(detuned(song, midi_freq(n), length, 0.3, gain, 3800, voices=7, spread=0.012, attack=attack, vibrato=0.004) for n in notes))

    return song.once(("strings", tuple(notes), round(length, 5), gain, attack), render)


def timpani(song, note, gain=1.0):
    def render():
        f = midi_freq(note)
        body = mix(
            tone(f, 1.2, sine, decay=3.5, sweep_to=f * 0.97, gain=0.45),
            tone(f * 1.5, 0.6, sine, decay=6, gain=0.12),
            lowpass(noise(0.06, decay=60, gain=0.25, seed=note), 900),
        )
        return [gain * x for x in body]

    return song.once(("timpani", note, gain), render)


def glockenspiel(song, note):
    """Inharmonic sine partials that die fast, an octave over the melody."""
    def render():
        f = midi_freq(note)
        body = mix(tone(f, 0.9, sine, decay=5, gain=0.05), tone(f * 2.76, 0.4, sine, decay=12, gain=0.02), tone(f * 5.4, 0.2, sine, decay=25, gain=0.01))
        return [2 * x for x in body]

    return song.once(("glockenspiel", note), render)


def rev(song, bar):
    """An engine revving into the drop two bars after `bar`: two detuned saws sweeping up, driven, cut two beats early."""
    def render():
        length = 2 * song.bar - 2 * song.beat
        body = mix(tone(70, length, saw, 0, 0.12, sweep_to=900, curve="exp"), tone(71.5, length, saw, 0, 0.12, sweep_to=920, curve="exp"))
        n = len(body)
        return lowpass([math.tanh(3 * x) * (i / n) ** 2 for i, x in enumerate(body)], 3000, 2)

    song.add(song.keys, song.at(bar), song.once(("rev",), render), 0.35)


class Band:
    """The disco-rock layers: cutting guitar, tambourine, picked rock bass, an arpeggio and a sung "ah"."""

    def __init__(self, song, kit):
        self.song = song
        self.kit = kit

    def cut(self, notes, accent):
        """A muted, slightly driven guitar chord, cut short like a disco strum."""
        s = self.song

        def render():
            body = mix(*(tone(midi_freq(n) * d, s.six * 0.9, saw, 38 if accent else 70, 0.07) for n in notes for d in (1.0, 1.004)))
            return bandpass([math.tanh(4 * x) / 4 for x in body], 1800, 2600)

        return s.once(("cut", tuple(notes), accent), render)

    def chant(self, note, span, gain):
        """Saws through two vowel formants: a wordless "ah" with vibrato."""
        def render():
            body = mix(*(tone(midi_freq(note) * d, span, saw, 1.0 / span, 0.09, attack=0.06, vibrato_hz=5.2, vibrato_depth=0.006) for d in (0.997, 1.0, 1.003)))
            return [gain * (a + 0.7 * b) for a, b in zip(bandpass(body, 750, 350), bandpass(body, 1200, 450))]

        return self.song.once(("chant", note, span, gain), render)

    def bar(self, bar, name, lift=0, four_snares=False, rock_bass=False, arp=False):
        s = self.song
        root = CHORDS[name][0] + lift
        tones = chord_tones(name, lift)
        voicing = [tones[0] + 24, tones[1] + 24, tones[2] + 24, tones[0] + 36]
        for step in range(16):
            t = s.at(bar, step)
            s.add(s.drums, t, self.kit.tambourine, 1.0 if step % 4 == 2 else 0.45)
            # Sixteenth cutting, accented on the offbeats.
            accent = step % 4 == 2 or step in (7, 15)
            s.add(s.keys, t, self.cut(voicing, accent), 1.0 if accent else 0.45)
            if four_snares and step % 4 == 0:
                s.add(s.drums, t, self.kit.snare, 0.7)
            if rock_bass and step % 2 == 0:
                # Picked eighths on the root, jumping the octave at the bar's end.
                s.add(s.keys, t, bass_note(s, root + (12 if step >= 14 else 0), root, 2 * s.six * 0.9, 1100, 5), 0.8)
            if arp:
                n = tones[(0, 1, 2, 1)[step % 4]] + 36
                s.add(s.keys, t, s.once(("arp", n), lambda: lowpass(tone(midi_freq(n), s.six * 0.8, saw, 12, 0.03), 6000)))


def groove_bar(song, kit, bar, name, lift=0, kick=True, clap=True, hats=True, open_hats=True, stabs=True,
               stab_cut=4500, bass="octave", bass_cut=1500, grit=2.5, pad=0.0, fill=False, cut_last_half=False, level=1.0):
    """One bar of the dance groove on `name`.

    `bass` is "octave" (disco eighths), "gallop" (root, root, octave per beat)
    or None; `pad` scales a held chord; `cut_last_half` stops the bar at its
    third beat for a drop; `level` scales the kick and the octave bass.
    """
    s = song
    root = CHORDS[name][0] + lift
    tones = chord_tones(name, lift)
    for step in range(16):
        if cut_last_half and step >= 8:
            break
        t = s.at(bar, step)
        if kick and step % 4 == 0:
            s.kick(t, kit.kick, level)
        if clap and (step in (4, 12) or (fill and step >= 12)):
            s.add(s.drums, t, kit.clap, 1.0 if step in (4, 12) else 0.6)
        if open_hats and step % 4 == 2:
            s.add(s.drums, t, kit.open_hat)
        elif hats and step % 4 != 2:
            s.add(s.drums, t, kit.hat, 0.5 + 0.5 * (step % 2))
        if bass == "octave" and step % 2 == 0:
            s.add(s.keys, t, bass_note(s, root + (12 if step % 4 == 2 else 0), root, 2 * s.six, bass_cut, grit), level)
        elif bass == "gallop":
            s.add(s.keys, t, bass_note(s, root + (0, 0, 12, 0)[step % 4], root, s.six * 1.1, bass_cut, grit), 0.9)
        if stabs and step % 4 == 2:
            for n in tones:
                s.add(s.keys, t, detuned(s, midi_freq(n + 24), 1.5 * s.six, 12, 0.05, stab_cut, voices=5, spread=0.012))
    if pad:
        for n in tones:
            s.add(s.keys, s.at(bar), detuned(s, midi_freq(n + 24), s.bar, 0.8, 0.028 * pad, 2500, voices=5, spread=0.01, attack=0.04))


def opening(s, kit):
    """Bars 0 to 39: intro, verse, build, chorus and breakdown."""
    s.add(s.drums, s.at(0), kit.impact, 0.8)  # the loop's downbeat lands like a green light
    for i, name in enumerate(INTRO_CHORDS):
        u = i / 7
        groove_bar(s, kit, i, name, clap=i >= 4, hats=i >= 2, open_hats=i >= 6,
                   stab_cut=600 * 7.5**u, bass_cut=350 * 4**u, fill=i == 7, level=0.55 + 0.45 * u)
    far = lambda n, span, length: s.once(("far", n, span), lambda: lowpass(brass(s, n, span, 0.28, bright=0.55), 2400))
    s.melody(0, FANFARE, far, echo=((3, 0.45), (6, 0.3), (9, 0.18)))

    lead = synth_lead(s)
    for i, name in enumerate(VERSE_CHORDS):
        groove_bar(s, kit, 8 + i, name, fill=i == 7)
        piano_bar(s, 8 + i, name, 0.6)
    s.add(s.drums, s.at(8), kit.crash)
    s.melody(8, VERSE, lead, echo=((3, 0.3),))

    for i, name in enumerate(BUILD_CHORDS):
        groove_bar(s, kit, 16 + i, name, kick=i < 4, clap=i < 4, open_hats=i < 6, bass="gallop", bass_cut=600 + 150 * i,
                   stab_cut=2500 + 400 * i, pad=0.6 + 0.1 * i, cut_last_half=i == 7)
        if 4 <= i < 7:
            s.kick(s.at(16 + i), kit.kick)
    kit.snare_roll(20, 4)
    kit.riser_into(24)
    rev(s, 22)
    s.melody(16, CLIMB, lead, echo=((3, 0.3),))

    kit.drop(24)
    for i, name in enumerate(CHORUS_CHORDS):
        groove_bar(s, kit, 24 + i, name, pad=1.0, grit=3.5, fill=i == 7)
        colour_pad(s, 24 + i, name)
    kit.tom_fill(31)
    # The chorus's last note falls off into the breakdown.
    s.melody(24, CHORUS, trumpet(s, 0.3, fall_at=(len(CHORUS) - 1,)), echo=((3, 0.3),))
    s.melody(24, [(n - 12, l) for n, l in CHORUS], synth_lead(s, 0.55))
    s.melody(24, [(n - 12, l) for n, l in COUNTER], lambda n, span, length: strings(s, [n], span, 0.05, attack=0.12))

    s.add(s.drums, s.at(32), kit.crash)
    for i, name in enumerate(CHORUS_CHORDS):
        groove_bar(s, kit, 32 + i, name, kick=False, clap=False, open_hats=False, stabs=False,
                   bass_cut=400 + 200 * i, pad=4.0, cut_last_half=i == 7, level=0.5 + 0.07 * i)
        colour_pad(s, 32 + i, name, gain=2.5)
        if i < 7:
            piano_bar(s, 32 + i, name, 1.0)
        if i >= 6:
            # The kick creeps back in under the roll.
            for beat in range(4 if i == 6 else 2):
                s.kick(s.at(32 + i, 4 * beat), kit.kick, 0.5 + 0.1 * beat)
    kit.snare_roll(36, 4)
    kit.riser_into(40)
    head = CHORUS[:16]
    s.melody(32, head + [(None, 128 - sum(l for _, l in head))], trumpet(s, 0.26, bright=0.75), echo=((3, 0.4), (6, 0.2)))


def bridge(s, kit, band, bar0):
    """Climbs from the chorus's own chords and lands on D, so the lifted chorus arrives as a resolution."""
    kit.drop(bar0)
    for i, name in enumerate(BRIDGE_CHORDS):
        bar = bar0 + i
        groove_bar(s, kit, bar, name, stabs=i >= 4, bass=None, pad=0.8 + 0.1 * i, cut_last_half=i == 7)
        band.bar(bar, name, rock_bass=True)
        colour_pad(s, bar, name, gain=1.2)
        if i < 7:
            piano_bar(s, bar, name, 0.5)
        if 4 <= i < 7:
            # The horns join halfway, so the brass grows into the last chorus.
            horn_chord(s, bar, 0, name, s.bar, swell=True, gain=0.35 + 0.1 * (i - 4))
    horn_chord(s, bar0 + 7, 0, "D", s.bar - 2 * s.six, swell=True, gain=1.3)
    kit.snare_roll(bar0 + 4, 4)
    kit.riser_into(bar0 + 8)
    rev(s, bar0 + 6)
    s.melody(bar0, BRIDGE, trumpet(s, 0.27), echo=((3, 0.25),))
    s.melody(bar0, [(n - 12 if n else None, l) for n, l in BRIDGE], lambda n, span, length: band.chant(n, span, 0.6))


def chorus_bed(s, kit, band, bar0, four_snares_from=4):
    """The lifted chorus's band, its octave-down double, harmony, chant and orchestra; the melody on top is the caller's."""
    kit.drop(bar0)
    for i, name in enumerate(CHORUS_CHORDS):
        up = LIFT if i < LIFTED_BARS else 0
        groove_bar(s, kit, bar0 + i, name, lift=up, pad=1.4, grit=4, fill=i == 7)
        band.bar(bar0 + i, name, lift=up, four_snares=i >= four_snares_from, arp=True)
        colour_pad(s, bar0 + i, name, lift=up, gain=1.5)
        if i == 4:
            s.add(s.drums, s.at(bar0 + i), kit.crash, 0.8)
    kit.tom_fill(bar0 + 7)
    melody = lifted(CHORUS)
    s.melody(bar0, [(n - 12, l) for n, l in melody], synth_lead(s, 0.6))
    s.melody(bar0, harmonise(melody), synth_lead(s, 0.45))
    s.melody(bar0, [(n - 12, l) for n, l in melody], lambda n, span, length: band.chant(n, span, 0.9))

    for i, name in enumerate(CHORUS_CHORDS):
        bar = bar0 + i
        up = LIFT if i < LIFTED_BARS else 0
        r = CHORDS[name][0] + up
        tones = chord_tones(name, up)
        # Strings sustained an octave above the pads, the root doubled on top,
        # and an eighth-note ostinato bowed short.
        s.add(s.keys, s.at(bar), strings(s, [t + 24 for t in tones] + [r + 36], s.bar, 0.035))
        for k in range(8):
            s.add(s.keys, s.at(bar, 2 * k), strings(s, [r + (36 if k % 2 else 24)], 1.6 * s.six, 0.05, attack=0.01), 0.8 if k % 2 else 1.0)
        s.add(s.drums, s.at(bar), timpani(s, r + 12, 1.0))
        s.add(s.drums, s.at(bar, 8), timpani(s, r + 19, 0.5))
        if i == 4:
            s.add(s.drums, s.at(bar), timpani(s, r + 12, 1.2))
    # A timpani roll into the turnaround.
    root = CHORDS[CHORUS_CHORDS[-1]][0]
    for k in range(8, 16):
        s.add(s.drums, s.at(bar0 + 7, k), timpani(s, root + 12, 0.25 + 0.06 * (k - 8)))
    s.add(s.drums, s.at(bar0 + 4), kit.crash, 0.6)


def music():
    s = Song(160, 64)
    kit = Kit(s)
    band = Band(s, kit)
    opening(s, kit)
    bridge(s, kit, band, 40)

    chorus_bed(s, kit, band, 48)
    melody = lifted(CHORUS)
    s.melody(48, melody, trumpet(s, 0.32, voices=2, shake_from=12), echo=((3, 0.25),))
    s.melody(48, harmonise(melody), trumpet(s, 0.22, voices=2))
    s.melody(48, [(n + 12, l) for n, l in melody], lambda n, span, length: glockenspiel(s, n))
    for i, name in enumerate(CHORUS_CHORDS):
        up = LIFT if i < LIFTED_BARS else 0
        # Hammered piano chords, above the bass and trombone so the low mids stay clear.
        for beat in range(4):
            for n in chord_tones(name, up + 24):
                s.add(s.keys, s.at(48 + i, 4 * beat), piano(s, n, s.beat), 0.8)
        horn_answers(s, 48 + i, i, name, up)
    horn_chord(s, 48, 0, "Bb", 2 * s.beat, lift=LIFT, fall=True, gain=1.4)

    # The final lap: strings take the melody and the trumpets a counter-line,
    # with a snare on every beat and the drums stopping under the held note.
    chorus_bed(s, kit, band, 56, four_snares_from=0)
    s.melody(56, melody, lambda n, span, length: strings(s, [n, n - 12], span, 0.06, attack=0.03))
    counter = lifted(COUNTER)
    s.melody(56, counter, trumpet(s, 0.3, voices=2, shake_from=16))
    s.melody(56, harmonise(counter), trumpet(s, 0.2, voices=2))
    for i, name in enumerate(CHORUS_CHORDS):
        horn_answers(s, 56 + i, i, name, LIFT if i < LIFTED_BARS else 0, 1.1)
    kit.stop(59)

    # The first 60 s sit a little under the choruses that follow, so the end is the loudest part.
    for bus in (s.drums, s.keys):
        for i in range(sec(s.at(40))):
            bus[i] *= 0.9
    write("music-loop", master(s))


def room(x, wet=0.16, predelay=0.02):
    """A Schroeder reverb (four damped combs, two allpasses) run twice around the loop, so its tail wraps the seam."""
    n = len(x)
    pre = sec(predelay)
    out = [0.0] * n
    for delay, feedback in ((558, 0.83), (594, 0.82), (638, 0.81), (678, 0.80)):
        buf = [0.0] * delay
        j, damped = 0, 0.0
        for lap in range(2):
            for i in range(n):
                y = buf[j]
                damped = 0.6 * y + 0.4 * damped
                buf[j] = x[(i - pre) % n] + feedback * damped
                j = (j + 1) % delay
                if lap:
                    out[i] += y * 0.25
    for delay in (278, 113):
        buf = [0.0] * delay
        j = 0
        res = [0.0] * n
        for lap in range(2):
            for i in range(n):
                b = buf[j]
                y = -0.5 * out[i] + b
                buf[j] = out[i] + 0.5 * b
                j = (j + 1) % delay
                if lap:
                    res[i] = y
        out = res
    return [a + wet * w for a, w in zip(x, lowpass(out, 5000))]


def master(s, duck_depth=0.5, release=0.12, drive=1.35, loudness=0.26):
    """Reverb the pitched bus, duck it under the kick, EQ, soft clip, and set the loudest 6 s to `loudness` RMS."""
    keys = room(s.keys)
    duck = [1.0] * s.total
    n = sec(release)
    for at in s.kicks:
        for i in range(n):
            j = (at + i) % s.total
            duck[j] = min(duck[j], 1 - duck_depth * (1 - i / n))
    bus = [d + k * g for d, k, g in zip(s.drums, keys, duck)]
    # Pull the sub back and lift 2.5 kHz and up, so the brass reads over the engines.
    low = lowpass(bus, 90, 2)
    air = [a - b for a, b in zip(bus, lowpass(bus, 2500, 1))]
    bus = [a - 0.55 * lo + 0.4 * hi for a, lo, hi in zip(bus, low, air)]
    out = tame([math.tanh(drive * x) / math.tanh(drive) for x in bus])
    w = sec(6)
    top = max(math.sqrt(sum(v * v for v in out[i:i + w]) / w) for i in range(0, len(out) - w + 1, w))
    return [v * loudness / top for v in out]


if __name__ == "__main__":
    sfx()
    music()

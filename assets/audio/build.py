"""Render the game's placeholder sounds (ADR 0007, ADR 0008) as WAV files.

Run from the repo root with `pnpm export:audio`; stdlib only. Every file is
synthesised here, so the whole set is this repo's own work and CC0 like the
rest of `assets/` (LICENSES.md). Each SFX is short and the music is one
bar-aligned loop whose last sample runs straight into its first.
"""

import math
import random
import struct
import wave
from pathlib import Path

RATE = 22050
OUT = Path(__file__).parent


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


def seconds(n):
    return int(n * RATE)


def tone(freq, length, wave_fn=math.sin, decay=6.0, gain=0.5, sweep_to=None):
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


def square(phase):
    return 1.0 if math.sin(phase) >= 0 else -1.0


def triangle(phase):
    return 2 / math.pi * math.asin(math.sin(phase))


def noise(length, decay=8.0, gain=0.5, rng=random.Random(1)):
    return [
        gain * math.exp(-decay * (i / RATE)) * rng.uniform(-1, 1)
        for i in range(seconds(length))
    ]


def mix(*parts):
    out = [0.0] * max(len(p) for p in parts)
    for part in parts:
        for i, s in enumerate(part):
            out[i] += s
    return out


def concat(*parts):
    return [s for part in parts for s in part]


def sfx():
    # Hop: a quick upward chirp as the kart leaves the ground.
    write("hop", tone(220, 0.12, decay=12, sweep_to=660))
    # Mini-turbo release: a sawtooth zip that climbs with the boost.
    write(
        "mini-turbo",
        tone(180, 0.3, wave_fn=lambda p: (p % (2 * math.pi)) / math.pi - 1, decay=5,
             sweep_to=900, gain=0.4),
    )
    # Item pickup: two rising blips.
    write("item-pickup", concat(tone(660, 0.08, decay=10), tone(990, 0.1, decay=10)))
    # Item fire: a low thump under a short burst.
    write("item-fire", mix(tone(110, 0.18, decay=14, sweep_to=50, gain=0.7), noise(0.1, decay=30)))
    # Hit: a crash of noise over a low rumble.
    write("hit", mix(noise(0.45, decay=7, gain=0.7), tone(70, 0.45, wave_fn=square, decay=5, gain=0.3)))
    # Lap: a three-note rising arpeggio.
    write(
        "lap",
        concat(
            tone(523.25, 0.1, wave_fn=triangle, decay=4),
            tone(659.25, 0.1, wave_fn=triangle, decay=4),
            tone(783.99, 0.2, wave_fn=triangle, decay=5),
        ),
    )


# The loop: four bars of 4/4 at 120 bpm (8 s). Chords I, V, vi, IV in C.
BPM = 120
BEAT = 60 / BPM
BAR = 4 * BEAT
CHORDS = [
    (261.63, 329.63, 392.00),  # C E G
    (196.00, 246.94, 293.66),  # G B D
    (220.00, 261.63, 329.63),  # A C E
    (174.61, 220.00, 261.63),  # F A C
]
# A lead line over the chords, one note per half beat, as semitone offsets
# from C4 played an octave up; None rests.
LEAD = [
    0, 4, 7, 4, 7, 12, 7, 4,
    -5, 2, 7, 2, 7, 11, 7, 2,
    -3, 4, 9, 4, 9, 12, 9, 4,
    -7, 0, 5, 0, 5, 9, 5, 0,
]


def note(semitones, base=261.63):
    return base * 2 ** (semitones / 12)


def music():
    total = seconds(4 * BAR)
    out = [0.0] * total
    rng = random.Random(7)

    def add(start, samples):
        for i, s in enumerate(samples):
            at = (start + i) % total  # a tail past the end wraps into the loop's head
            out[at] += s

    for bar, chord in enumerate(CHORDS):
        bar_start = seconds(bar * BAR)
        root = chord[0] / 2  # the bass an octave down
        for eighth in range(8):
            t = bar_start + seconds(eighth * BEAT / 2)
            pitch = root if eighth % 4 != 3 else chord[2] / 2
            add(t, tone(pitch, BEAT / 2, wave_fn=square, decay=4, gain=0.12))
            add(t, noise(0.03, decay=120, gain=0.08, rng=rng))  # hi-hat tick
            if eighth % 2 == 0:
                add(t, tone(60, 0.15, decay=25, sweep_to=35, gain=0.5))  # kick
        for beat in range(4):
            t = bar_start + seconds(beat * BEAT)
            for freq in chord:
                add(t, tone(freq, BEAT, wave_fn=triangle, decay=2.5, gain=0.07))
    for i, semitones in enumerate(LEAD):
        if semitones is None:
            continue
        t = seconds(i * BEAT / 2)
        add(t, tone(note(semitones) * 2, BEAT / 2, wave_fn=triangle, decay=5, gain=0.14))
    write("music-loop", out)


if __name__ == "__main__":
    sfx()
    music()

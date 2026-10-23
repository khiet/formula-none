"""Render the game's placeholder SFX (ADR 0007, ADR 0008) as WAV files.

Run from the repo root with `pnpm export:audio`; stdlib only. Every SFX is
synthesised here, so the set is this repo's own work and CC0 like the rest
of `assets/` (LICENSES.md). The music loop is a hand-placed track that this
script leaves alone.
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
    # Countdown: one short beep for each of 3, 2 and 1, all at one pitch,
    # and a higher, longer tone for Go, so the rocket start can be timed by
    # ear.
    write("countdown-beep", tone(880, 0.12, wave_fn=square, decay=12, gain=0.3))
    write("go", tone(1320, 0.5, wave_fn=square, decay=4, gain=0.3))
    # Sputter: the engine coughing through a stall at Go, as long as the
    # stall (`START_STALL_S`): a low rumble chopped into coughs under
    # bursts of noise.
    write("sputter", sputter(2.0))


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


if __name__ == "__main__":
    sfx()

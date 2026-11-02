# ADR 0025: Audio, with an engine that has gears, a limiter and load

## Status

Accepted (2026-10-25). Supersedes ADR 0008 in full. Extended by ADR 0075:
the star music is the one sound that hears another kart, the nearest star
kart's run by distance. Extended again by ADR 0076: every other kart's Air
Horn blast is heard once by the same rule, and the drone warning's beep
repeats on the audio's clock.

## Context

ADR 0008 gave the game Web Audio, a looping oscillator for the engine whose
pitch followed speed, preloaded SFX, one music loop and three toggles. The
cue inventory has since grown (issue 83): thirteen SFX, three loops that
run while a state holds, and the rocket start revving the note on the grid.
The engine stayed one sawtooth. It has no gears, no load and no limiter, so
a kart pulling away, lifting for a corner, braking or riding a boost all
sound like one slide of pitch.

The iteration page under `docs/mockups/sound-effects.html` (issue 82)
carries an F1-style rig, synthesised in the page from sliders, whose
defaults were tuned by ear against the picked cues. This record puts that
rig in the race and restates what still holds from ADR 0008, so one record
describes the game's sound.

## Decision

What holds from ADR 0008:

- Web Audio. Every SFX and loop is a WAV under `assets/audio/`, fetched at
  page load and decoded as soon as a gesture lets the AudioContext start,
  so the browser never warns of autoplay. One CC0 music loop.
- Three toggles (SFX, engine, music) persisted in localStorage alongside
  the nickname. Mix levels: engine 0.12, SFX 0.8, music 0.35.
- The cue inventory issue 83 ships: countdown beep, go, hop, mini-turbo,
  item pickup, item fire, hit, sputter, lap, tier chime, boost pad, wrong
  way and finish as one-shots; the drift skid, grass and sand as loops
  pitched and swelled with speed. Cues are edges between two readings of
  the own kart; loops are state one reading holds.

The engine, replacing the sawtooth:

- The graph: two sawtooths detuned 12 cents apart, a square sub an octave
  down at 0.35 and intake noise at 0.15, through one lowpass that opens
  from 400 Hz to 6000 Hz with the revs and the load, into the engine gain.
  A square low-frequency oscillator at 28 Hz chops the gain at the
  limiter. The same noise feeds crackle bursts on the overrun, past the
  filter.
- Gears are derived from speed in the client. Six gears whose speed bands
  widen geometrically by 1.4 each: edge k is `top * (r^k - 1) / (r^N - 1)`
  for the band ratio r and the gear count N, so the edges fall at 0, 1.8,
  4.4, 8.0, 13.1, 20.1 and 30 m/s. Revs run from the drop ratio 0.45 to 1
  across a band; the note runs from 110 Hz at idle to 720 Hz at the top of
  a band.
- An upshift cuts the note for 30 ms and drops the revs to 0.45. A
  downshift happens only once speed has fallen 5% of the band's width
  under its floor, so a steady speed never chatters, and blips the revs up
  by 0.1 before they settle.
- Past top speed, under a boost, sixth sits on the limiter: the revs cap
  at 1.08 and the chop runs while they are over 1.
- Load is inferred from the speed delta between readings against the sim's
  full-throttle acceleration, smoothed over a few frames: accelerating
  opens the filter; lifting closes it, drops the level to 0.6 and plays the
  crackle at 0.5.
- During the countdown the revs follow the rocket start's charge by tier:
  clean idles, a weak boost holds 0.5, a full boost and a stall hold the
  top note with the limiter's chop on.
- Where it lives: a pure `audio-engine.ts` module under `apps/web/src`
  holds the constants, the band edges, the gear choice with hysteresis, the
  limiter and the load, testable without an audio context, as the cues
  are. The oscillator graph stays in `audio.ts`, under the engine toggle.
  `enginePitchHz` retires with the sawtooth.
- The iteration page is the record of the decision's values, as ADR 0024's
  comparison page is of its. Retuning starts there.

## Consequences

- Audio is Presentation: it reads the own kart's readings and never changes
  sim state. The sim, the protocol and the bridge are unchanged and the
  replay fixtures stand.
- The engine is the own kart's alone. Other karts' engines, an Instrument
  panel gear or rev readout and any in-game tuning UI are out of scope.
- No audio asset pipeline beyond static files synthesised by
  `assets/audio/build.py`; no audio middleware. The engine ships no file:
  its noise is filled at the race's start.
- Tests drive the model through `engineStep` and the graph through a fake
  AudioContext, asserting gears, revs, load, the cut, the blip and the
  limiter, never what is heard.

# ADR 0069: The music loop runs a minute

## Status

Accepted (2026-10-31). Supersedes ADR 0066's "The music loop is the 8 s
loop the game first shipped with". The rest of ADR 0066, every sound
synthesised by `assets/audio/build.py`, stands.

## Context

ADR 0066 put the 8 s synthesised loop back when the 4m20s track from PR 59
turned out to have no recorded source. Eight seconds repeats seven or eight
times a lap, too often to race to, and its 120 bpm triangle waves were
tame beside the track it replaced. Issue 227 asks for a longer loop from
`build.py`, so every WAV under `assets/audio` stays regenerable and the CC0
row in `assets/LICENSES.md` needs no special case. Editing the removed
track was ruled out: a derivative of a recording the repo cannot show is
still a copy of it.

## Decision

- `build.py` renders the loop as 64 bars of 4/4 at 160 bpm in D minor,
  96 s of 16-bit mono PCM at 22050 Hz, about 4.2 MB.
- The tempo matches the removed track's measured 160 bpm; nothing else of
  it is used. The arrangement was picked by ear over several rounds of
  renders; it leans on disco-rock with a synthesised trumpet lead.
- Eight sections of eight bars: an intro whose filters open under a distant
  trumpet previewing the chorus; a verse; a build with a snare roll and an
  engine-rev sweep; the chorus (Bb C Am Dm) on trumpet; a breakdown over
  piano; a disco-rock bridge that climbs Gm Am Bb C and lands on D; the
  chorus a tone up with a horn section, strings, timpani and glockenspiel;
  and a final lap of the chorus with the strings on the melody and a drum
  stop. Each chorus ends on two bars of A, so the loop's end resolves into
  its first bar.
- Harmony lines take a chord tone above the melody, never a fixed interval,
  so no line rubs a semitone against the chord under it.
- The kick ducks everything pitched; a room reverb, run twice around the
  loop so its tail wraps the seam, an EQ that lifts the brass above the
  engines, and a soft clip finish the mix. The first 60 s sit under the
  choruses, and the loudest 6 s set the level, so the end is the peak.
- Repo-checks holds the committed loop to a minute or more of plain PCM.

## Consequences

- The web build ships 4.2 MB of music in place of 350 KB; the game still
  fetches it once and loops it under the music toggle (ADR 0025).
- A longer or different loop is a change to `build.py` and a rerun of
  `pnpm export:audio`, never a hand-placed file.

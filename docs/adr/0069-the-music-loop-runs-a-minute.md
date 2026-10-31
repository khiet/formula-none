# ADR 0069: The music loop runs a minute

## Status

Accepted (2026-10-31). Supersedes ADR 0066's "The music loop is the 8 s
loop the game first shipped with". The rest of ADR 0066, every sound
synthesised by `assets/audio/build.py`, stands.

## Context

ADR 0066 put the 8 s synthesised loop back when the 4m20s track from PR 59
turned out to have no recorded source. Eight seconds repeats seven or eight
times a lap, too often to race to. Issue 227 asks for a longer loop from
`build.py`, so every WAV under `assets/audio` stays regenerable and the CC0
row in `assets/LICENSES.md` needs no special case. Editing the removed
track was ruled out: a derivative of a recording the repo cannot show is
still a copy of it.

## Decision

- `build.py` renders the loop as 32 bars of 4/4 at 120 bpm in C, 64 s of
  16-bit mono PCM at 22050 Hz, about 2.8 MB.
- Four sections of eight bars, one chord a bar: A, the original theme over
  I, V, vi, IV; B, the relative minor with longer notes and a backbeat; A',
  the theme answered an octave up; and a bridge that drops to half time,
  builds back with a snare fill, and runs down the scale into A's first
  note, so the loop's end leads into its start.
- The loop uses the sound effects page's DSP kit, not the legacy helpers,
  so its bytes are new; the SFX bytes are unchanged.
- Repo-checks holds the committed loop to a minute or more of plain PCM.

## Consequences

- The web build ships 2.8 MB of music in place of 350 KB; the game still
  fetches it once and loops it under the music toggle (ADR 0025).
- A longer or different loop is a change to `build.py` and a rerun of
  `pnpm export:audio`, never a hand-placed file.

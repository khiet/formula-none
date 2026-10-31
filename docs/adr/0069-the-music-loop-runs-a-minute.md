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

- `build.py` renders the loop as 48 bars of 4/4 at 160 bpm in A minor,
  72 s of 16-bit mono PCM at 22050 Hz, about 3.2 MB.
- The energy matches the removed track's measured tempo (about 160 bpm),
  loudness and density; nothing else of it is used.
- Six sections of eight bars, one chord a bar: a drive of drums, galloping
  saw bass and a sixteenth-note arpeggio; a verse with a detuned saw lead;
  a chorus an octave up; a break that drops the kick and rises back; the
  chorus a tone higher; and a drive ending on E, so the loop's last bar
  resolves into its first.
- The kick ducks the bass, pad and lead, and a soft clip sets the loudness.
- Repo-checks holds the committed loop to a minute or more of plain PCM.

## Consequences

- The web build ships 3.2 MB of music in place of 350 KB; the game still
  fetches it once and loops it under the music toggle (ADR 0025).
- A longer or different loop is a change to `build.py` and a rerun of
  `pnpm export:audio`, never a hand-placed file.

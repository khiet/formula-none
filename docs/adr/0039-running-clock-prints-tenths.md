# ADR 0039: The running clock prints tenths

## Status

Accepted (2026-10-26). Supersedes ADR 0023's "Times print in the racer's
notation at the sim's precision" for the running clock only, the lap time
TIME shows while a lap runs: it prints tenths while the own kart is
racing. BEST, a finished kart's TIME, the finish standings and the Results
Scene keep the sim's precision.

## Context

ADR 0023 printed every time to the thousandth, 0'27"304. On the
Instrument panel's TIME readout, which the sim advances every tick, the
thousandths step by 16 or 17 ms at 60 Hz: the last two digits are noise
nobody can read. Kenney Mini Square, the panel's face, has no tabular
figures and draws its 1 narrower than the other digits, so the readout
also jittered sideways as the digits changed, and the plate with it.

## Decision

- While the own kart is racing, TIME prints minutes, seconds and tenths,
  0'11"9. The tenths are truncated from the thousandths the time would
  print, so 0'11"999 reads 0'11"9 and the running clock never reads ahead
  of the final time.
- Once the own kart has finished, the sim leaves its lap clock on the final
  lap's time and TIME prints it to the thousandth, 0'23"457.
- BEST and every finished time keep the sim's precision.
- Both notations live in `apps/web/src/format.ts`, beside each other.
- TIME and BEST set each figure, and each dash of a time there is none of
  yet, in a cell one digit wide, so the readouts keep their width and their
  labels stay put as the digits change.

## Consequences

- Presentation only. The sim, the protocol and the bridge are unchanged
  and the replay fixtures stand.
- TIME changes length once, at the finish, from 0'11"9 to 0'11"999.
- Holding a completed lap's time on TIME as the lap ends is a possible
  follow-up and not part of this record.

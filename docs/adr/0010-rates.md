# ADR 0010: Rates

## Status

Accepted (2026-10-18)

## Context

Tick, input and snapshot rates set the feel, the bandwidth and the server
load together, so they are decided once.

## Decision

- Sim tick 60 Hz on both client and server.
- Client input messages at 60 Hz carrying sequence, steer, throttle, brake,
  hop, use item, hold item: about 8 bytes.
- Server snapshots at 30 Hz carrying tick, last acknowledged input sequence
  per client, and for every kart position, rotation, velocity, drift state
  and charge, lap, checkpoint, spline progress, held item, spin-out timer,
  plus events since the previous snapshot (item picked, item fired, hit, lap
  completed, finished, respawned). Roughly 400 bytes per snapshot at eight
  karts.

## Consequences

- Every rule is written per tick; frame rate never enters the sim.
- Snapshot size is a budget: a new per-kart field costs eight times its size
  thirty times a second.
- Delta compression is deferred to v1.1.

## Outcome

Landed with issue 17 (2026-10-19).

- The server steps the sim from a tokio interval at the tick rate and
  sends a snapshot every second tick.
- A snapshot carries, per kart, two readouts the list above does not name
  because they are the sim's rules rather than derivable state: the race
  position and whether the wrong-way warning is up. With them the busy
  sample snapshot in the protocol's fixtures is 474 bytes at eight karts,
  against the roughly 400 estimated above.
- Issue 22 (2026-10-20) added the hazards on the track to every snapshot:
  position, owner and whether the owner still trails it, since a trailed
  hazard moves every tick. The busy sample, with one hazard, is 490 bytes.
- Issue 23 (2026-10-20) added the homing projectiles in flight to every
  snapshot: position, owner and target, since a projectile moves every
  tick and the client draws it between snapshots as it draws the karts.
  The busy sample, with one hazard and one projectile, is 505 bytes, past
  the quarter over 400 the size test allowed; the test now holds the
  sample to a quarter either side of 500, so the gate still catches a
  per-kart field that does not belong.
- Issue 64 (2026-10-29) added each kart's bump to every snapshot, carried
  only while the kart has one, so a prediction rewound to the snapshot
  takes the server's bump instead of keeping its own. The busy sample,
  with one kart bumped, is 570 bytes; the budget holds while few karts
  carry one, and a pile-up of all eight would pass the size test's cap
  for the ticks their bumps last.
- Issue 263 (2026-11-02) added each kart's star run to every snapshot, the
  seconds left while it has one, so every client shines the kart in the
  same phase. The busy sample, with one kart on a run, is 629 bytes; the
  test now holds the sample to a quarter either side of 600.

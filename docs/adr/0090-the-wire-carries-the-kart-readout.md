# ADR 0090: The wire carries the kart readout

## Status

Accepted (2026-11-06). Supersedes ADR 0026's consequence that a field the
snapshot should carry is added to `KartSnapshot` and to its two conversions,
and its one `From` each way for the kart and the sim types a kart holds.
Keeps the rest of ADR 0026: one readout, the flat transform buffer, and
protocol depending on sim.

## Context

ADR 0026 gave a kart one readout, `KartReadout`, and left the wire its own
twin, `KartSnapshot`, with twins of the drift, the stop, the slipstream, the
boost, the rocket start, the item and the item slot. Twenty of the
protocol's twenty-five `From` impls copied a kart fact between a sim type
and its twin, so a per-kart fact was written by hand in about eight places.
Adding the tyre life touched four protocol files for the copy alone.

The twins also disagreed. The wire left out whether the kart is in the pit
lane, and both the conversion and the interpolation wrote it as out of the
lane, so a rival on the limiter never showed PIT until the prediction's own
step said so.

The readout also stored readings worked out from other fields beside them:
the mini-turbo tier from the drift, and the boost's tier as "none" whenever
there was no boost.

The readout's speed looks like one of those, but is not. It is the speed
the handling drives the kart at along its travel, which the bots steer on.
The velocity also holds the bump and a drift's slip, so the speed along the
facing that the conversion worked out from it is a different number.

## Decision

- `Snapshot.karts` is `Vec<KartReadout>`. The sim's readout and the types it
  holds derive `Deserialize` beside `Serialize` and `TS`. `KartSnapshot` and
  its twins go with their conversions, and the wire's events name the sim's
  `Item`. Protocol keeps its conversions only for what is not a kart: the
  events, the hazards, the projectiles and the input.
- The readout stores each fact once. The boost is one `Option<Boost>`, its
  seconds left and its `BoostTier`, orange or red. The mini-turbo tier, the
  boost's seconds and the boost's tier as a mini-turbo tier are methods.
- The speed stays a field and rides the wire. The interpolation draws it
  between the two snapshots as it draws the velocity.
- The bridge hands JavaScript each readout beside its `KartReadings`: the
  surface under the kart and the readings worked out from the readout. The
  web app merges the two into the one value its View and Instrument panel
  read.
- Whether the kart is in the pit lane rides the wire from the server's
  readout. The interpolation draws it as the nearer of its two snapshots
  has it.
- The checkpoint in the readout is a `u32`, so it reaches JavaScript as a
  number from a message as it does from the bridge.
- ts-rs writes every sim type into `generated/sim`, and a protocol type that
  holds one imports it from there.
- The hash is untouched; the replay fixtures hash as they did.

## Consequences

- A per-kart fact is written by hand in `Kart::readout`, `Kart::adopt`, the
  prediction's `agrees` and the web app's resting kart. Nothing in protocol's
  conversions names a kart field.
- A rival's speed in the View and the audio is now the server's handling
  speed, not its velocity along its facing. The two differ in a drift and
  in a bump.
- A rival on the pit limiter shows PIT as the server has it.
- The busy snapshot sample grows from 720 to 760 bytes, the speed and the
  pit lane's flag at eight karts. The protocol's size test holds it at a
  budget of 640 bytes, with its tolerance of a quarter either way.
- The protocol version goes to 26.

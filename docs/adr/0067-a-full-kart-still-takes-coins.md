# ADR 0067: A kart with a full purse still takes coins

## Status

Accepted (2026-10-31). Supersedes ADR 0055's clause that a kart takes a
coin only while it carries fewer than ten. The rest of ADR 0055's rule,
with ADR 0056's coin, stands.

## Context

Under ADR 0055 a kart carrying a full purse drove over coins and left them
in place. A leader with ten coins could do nothing about the coins on its
line, so they always went to the karts chasing it. Issue 240 asks for the
opposite: a kart in front can take a line of coins to deny them to the
karts behind.

## Decision

- A kart whose centre is on a coin takes it whatever its count. Below a
  full purse the count rises by one; at ten it holds. Either way the coin
  is gone for the tuned 3 s.
- A full kart's take raises the same `CoinPicked` event as any other, so
  the server and every screen hide the same coin.
- A spun-out kart, a finished kart and any kart through the outro still
  take none.
- No cue marks a full kart's take: the coin disappearing is the only
  feedback. `coin-pickup.wav` still plays only as the own kart's count
  rises (ADR 0056), so the take is silent.

## Consequences

- The protocol is unchanged, and the replay fixtures pass without
  re-recording, since none of them drives a full kart over a coin.
- A full leader now takes the coins ahead of it, so the karts behind
  collect fewer coins than they did.
- A sound or Instrument panel cue for a denied coin, and bots steering to
  take or deny coins, stay out of scope.

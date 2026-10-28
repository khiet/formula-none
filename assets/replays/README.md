# Replay fixtures

A race replay is `{seed, track, bots, inputs[]}`. Each file here is one replay
with the state hash the sim reported at every 60th tick when it was
recorded. Two tests rerun every file and hold the sim to those hashes:
`crates/sim/tests/replay.rs` natively and `apps/web/src/replay.test.ts`
through the wasm build. That is the cross-platform determinism gate (ADR
0002): if the two builds ever disagree on the rules, one of them fails here.
Neither test compares the builds directly; each compares its build with the
recorded hashes, so both passing means the builds agree. The `Replay
determinism (native vs wasm)` CI job runs both on every PR.

`crates/sim/src/replay.rs` documents the format. Inputs are run-length
encoded, one input per human kart in grid order; the bots behind them on
the grid drive themselves from the seed, so the file carries only their
count. The seed is a decimal string and each hash 16 hex digits, since a
JSON number cannot carry every u64.

## Record a fixture from a browser session

Drive in the client (`pnpm --filter web dev`, `?track=oval` for the oval),
then press `R`. The browser downloads `<track>.replay.json`: your inputs
for every tick so far, the bots' count and the wasm sim's hashes. Move it
here under a descriptive name ending in `.replay.json`; both tests pick it
up by listing this directory.

A race begins with its countdown, and a recording starts with the race, so
every fixture opens with the countdown's ticks: idle inputs for a clean
start, or the throttle held through Go for a rocket start. The fixtures recorded
before the countdown existed were given an idle segment of its length in
front of their drive, so the drive itself is as it was recorded.

## Re-record after changing the rules

A change to the sim changes every hash. Once the change is intended, rerun
the fixtures natively and write the new hashes back:

```sh
cargo run -p sim --example replay -- --update assets/replays/*.replay.json
```

Without `--update` the same command reports and fails on any difference.
The wasm test then confirms the wasm build agrees with the new hashes.

## What the set must cover

`the_fixture_set_exercises_kart_contact_off_track_walls_both_tracks_both_mini_turbos_a_rocket_start_a_stall_the_kerb_and_the_wheel`
in the sim's tests requires the set, taken together, to have karts
touching, karts on the sand, a lone kart held by a wall, a run on
each track, a drift released into an orange mini-turbo and another into
a red one, a kart boosting at Go and a kart stalling there, a kart
launched by a sausage kerb and a kart spun out by the runaway wheel, so
trimming the set cannot quietly weaken the gate. `oval-hop-drift.replay.json` was
written by hand for the mini-turbo: a hop into a left drift held past
red through the first turn, then on the back straight a hop into a left
drift let go between orange and red and a hop into a right drift let go
before orange, ending against the back straight's outer wall; its holds
count the hop's airtime before the charge starts, and its steering keeps
the kart between the walls. `brands-patch-chicane.replay.json` was written by a scripted follower
from the grid: a line over the chicane's kerb, which launches the kart,
then a stop on the approach to hairpin two with the kart on the wheel's
crossing until the first pass, back from its first rebound off the wall,
spins it out, and three seconds of driving on once the pass is over. `oval-rocket-start.replay.json` and
`oval-stall.replay.json` were written by hand for the start: the throttle
held from the first half of the fourth light through Go, and from the top of the
countdown.

# Replay fixtures

A race replay is `{seed, track, bots, level, tyres, laps, inputs[]}`. Each file here is one replay
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
the grid drive themselves from the seed at the bot level the file names
(ADR 0078; a file naming none runs them at Basic), so the file carries only
their count and the level. It also names the Tyres pick (ADR 0083; a file
naming none races Off, so its hash has no tyre life in it) and the laps
(ADR 0084; a file naming none runs three). The seed is a decimal string and each hash 16 hex digits, since a
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

`the_fixture_set_exercises_kart_contact_off_track_walls_every_track_both_mini_turbos_a_rocket_start_a_stall_the_kerb_the_wheel_coins_and_ace_bots`
in the sim's tests requires the set, taken together, to have karts
touching, karts on the sand, a lone kart held by a wall, a run on
each track, a drift released into an orange mini-turbo and another into
a red one, a kart boosting at Go and a kart stalling there, a kart
launched by a sausage kerb, a kart spun out by the runaway wheel, a coin
taken and coins lost to a spin-out, a lap completed in a pit lane, a stop in a pit box and a race of Ace bots, so
trimming the set cannot quietly weaken the gate. `oval-hop-drift.replay.json` was
written by hand for the mini-turbo: a hop into a left drift held past
red through the first turn, then on the back straight a hop into a left
drift let go between orange and red and a hop into a right drift let go
before orange, ending against the back straight's outer wall; its holds
count the hop's airtime before the charge starts, and its steering keeps
the kart between the walls. `bell-deer-chicane.replay.json` was written by a scripted follower
from the grid on Bell Deer: down the underpass's centreline, taking
its five coins, a stop on the run to the hairpin with the kart on the
wheel's crossing until a pass spins it out and costs it three, then on
round Spoon and over the bridge, a line over the chicane's kerb, which
launches the kart, and three seconds past the line. Its steering is held
six ticks at a time in eighths. From the spin-out on it was rewritten when
the pickups came in: a follower steering for the path the kart drove before,
two percent faster on the two it kept, and for the kerb itself on the
chicane's approach, with the throttle and brake the old run held at the
same spline progress. `bell-deer-lap-through-the-pits.replay.json` was
written by a scripted follower too, on Tyres On: a lap of the centreline, then down
the lane through the start gate, a stop in its pit box, and on round the course;
it pins the stop's roll and its two clocks (ADR 0082). `bell-deer-first-corners.replay.json` was
written the same way: four karts from the grid through the first curve
and turn 2, one of them cutting across turn 2's sand. `oval-rocket-start.replay.json` and
`oval-stall.replay.json` were written by hand for the start: the throttle
held from the first half of the fourth light through Go, and from the top of the
countdown. `goldstone-lap-with-bots.replay.json` was written by the sim tests'
centreline follower, steering held six ticks at a time in eighths with the
throttle down: one kart idle through the countdown, then a lap of Goldstone
among seven bots. `monarch-lap-with-bots.replay.json` was written the same
way on Monarch. `oval-nose-to-tail.replay.json` was written by hand for the
slipstream (ADR 0077): three karts idle through the countdown, then the
first and third hold the throttle down the straight, the third 5 m behind
the first in the same column, so it is towed by the time both are at top
speed, while the second sits on the grid. `oval-ace-bots.replay.json` was
written by hand for the bot level (ADR 0078): one kart idle on the grid
through the countdown and thirty seconds of the race while seven Ace bots
lap the oval past it, so a retune of Ace's profile fails the gate where a
retune of Basic fails every other fixture with bots. `goldstone-bots-pit.replay.json`
was written by hand for the pit call (ADR 0081): one kart idle on the grid through
two minutes and more of a five-lap race on Tyres On while seven Ace bots lap
Goldstone and come in for new tyres.

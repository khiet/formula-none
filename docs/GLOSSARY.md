# Glossary

The vocabulary of Formula None. Code, tests, issues and ADRs use these words
with these meanings; when a word here and a word in the code disagree, fix the
code. Terms come from the PRD (issue 1); the client terms reuse the Purrjump
glossary where they fit.

## Race

### kart

One racer's vehicle, driven by a human or a bot. All karts are identical at
v1; the livery and the nickname floating over it tell one from another.

### bot

The driver inside the sim for a kart no human drives: the slots humans leave
empty at Start, a human's kart once they disconnect or go silent, a
finished kart until the race ends, and every kart through the outro. A bot
reads only what any observer of the race can, the track and the karts'
transforms and standings, and answers with the same input struct a human's
controller produces. Bots start on the grid behind the
humans.

### phase

Where a race is: Countdown, Racing, Outro or Finished. Sim state, stepped
like every other rule, so a solo race in the browser and a room on the
server run the same lifecycle. Countdown holds the karts on the grid for
the 3-2-1 count, during which the throttle sets up the rocket start.
Racing runs the laps and ends when every human's kart has finished or is
driven by a bot, or 60 s after the first human finished driving their own
kart; the unfinished are placed by progress at that point and the
standings become the results. Outro holds the Race View for 3 s with a bot
driving every kart and the FINISH banner up for every player.
Finished leaves the race as it is. A room's Countdown and Racing states
follow the race's phase, its Racing state covering the Outro too, and its
Results state its Finished (ADR 0032).

### finished

A kart past the line on its last lap. Its finish time and place are final,
it counts no more laps, and a bot drives it until the race ends, taking
and firing no items; its human's inputs are ignored. A finished player
sees the FINISH banner with their place and the finishers so far in place
of the lap readout.

### rocket start

What the throttle hold ending at Go earns, read from the hold's length; a
release before Go resets it, and the throttle counts as held above half.
Its outcomes are the start boost and the stall. A hold longer than 2.0 s,
begun before the 2 appeared, stalls the kart: 2.0 s dead to every control,
as in a spin-out. A hold of 1.5 to 2.0 s, begun in the first half of the 2,
earns the full start boost, 1.5 s at +40 percent, the boost item's values.
A hold of 1.0 to 1.5 s, begun in the second half of the 2, earns the weak
start boost, 0.6 s at +30 percent, the orange mini-turbo's values. A shorter
hold, begun at the 1 or later, starts clean. Bots roll the full boost or a
clean start once per race from the seed and never stall; a kart handed to
a bot during the countdown starts clean. The countdown is beeped so the
start can be timed by ear; sparks show every charging kart's tier, orange for
the weak boost and red for the full one, with no warning once the hold
has tipped into a stall. The snapshots carry each kart's start and stall
time left, and a client's prediction adopts the server's verdict.

### rubber band

The scale on a bot's top speed by its spline distance from the nearest
human: slower far ahead, faster far behind, between the tuned bounds. A
human's kart is never banded.

### room

A lobby identified by a four-letter code from an unambiguous alphabet. Holds
up to eight players, has a creator, and moves through Open, Countdown, Racing,
Results and Closing. A room enters Closing five minutes after the last human
leaves.

### session token

The secret a room deals a player at their join, which takes their seat back
over a new connection. Mid-race a bot drives the kart of a Disconnected
player until the token rejoins or the race ends; the seat, and the token
with it, is freed at the results.

### creator

The player who can press Start in a room. The role passes to the next player
if the creator leaves, so a room never gets stuck.

### tick

One fixed step of the sim, 60 per second on both client and server. Every
rule in the game is expressed per tick, never per frame or per wall-clock
second.

### snapshot

The server's authoritative state of every kart at a tick, sent at 30 Hz
together with the input sequence applied at that tick per client and the
events since the previous snapshot.

### checkpoint

An ordered gate on the track used for lap counting and wrong-way detection.
Twelve spaced evenly round the lap on each track, each spanning the run-off
from wall to wall, so a kart between the walls crosses every one in order.
A kart that leaves the world respawns at its last checkpoint facing the next
one; a kart that misses a checkpoint is not respawned and drives back
through it (ADR 0022).

### spline progress

Distance along the track centreline. Race position is ordered by lap, then
checkpoint index, then spline progress. Decreasing spline progress for two
seconds triggers the wrong-way warning.

### kart readout

What the sim says of one kart at a tick: its transform, speed and
velocity, drift and mini-turbo tier, item slot, spin-out, hop, boost,
rocket start and stall, and its place in the race. The sim reports one per
kart every tick; a snapshot carries all of it but the hop, which only the
sim stepping the kart knows; a prediction's kart adopts the snapshot's but
for the boost, which it keeps its own. The View, the Instrument panel and the bots read it. Not a
snapshot, which is the server's message about every kart.

### standing

Where a kart is in the race: its race position, laps completed, last
checkpoint crossed in order, spline progress, lap time, best lap, finish
time once it has finished and whether the wrong-way warning is up. The sim
reports one per kart every tick; the Instrument panel shows the local
kart's, and the Results Scene lists every kart's once the race is Finished.
Finishers hold their places in finishing order whatever they do after the
line.

### handling

How one kart answers its controls each tick: its speed, the hop, drift and
mini-turbo, boosts, the off-track cap, the freeze of a stall or spin-out,
and the hold of a wall and the bump off another kart. One rule in the sim,
run for every kart before the physics solver and told afterward of the
walls and karts it met.

### hop

A short jump (0.3 s airtime) that starts a drift when it lands while the kart
is steering above 40 percent of top speed.

### drift

Sliding through a corner after a hop. The drift direction is fixed at the hop;
counter-steer widens the arc and inward steer tightens it. Holding a drift
charges a mini-turbo.

### mini-turbo

The boost released at the end of a long enough drift: orange at 0.3 s of drift
(0.6 s boost), red at 0.6 s (1.0 s boost), both at +30 percent top speed.
Shown by sparks at the wheels while charging, orange then red, and on
release by a flame at the exhaust in the tier's colour.

### item box

One of a track's fixed pickup spots, from the sidecar: eight per track, in
rows across the asphalt. A kart whose centre is on a box with an empty item
slot picks it up; the box is gone for three seconds and comes back. Every
pickup is a server event naming the box, so every screen hides the same one.

### item slot

A kart's one place for an item: a boost, a hazard or a homing projectile.
Full, the kart drives over boxes without picking them up; a press of the
item button fires what it holds and empties it. Every fire is a server
event; the Instrument panel shows the local kart's slot, with the fire
predicted from the press until the server's event confirms it.

### roll

The item a pickup puts in the slot, drawn on the sim's RNG with weights by
the kart's race position band: the leaders mostly hazards, the back of the
field mostly homing. Only the server rolls; a client's prediction never
picks up a box and holds what the snapshots say.

### hazard

The defending item. Fired, it drops 1 m behind the kart, on the surface;
with the item button held it trails there until the button is let go. It
spins out the first kart to reach it and is used up, never the kart that
dropped it until that kart has driven clear of it, and a dropped hazard is
gone after 30 s. Every snapshot carries the hazards on the track; a client
draws a trailed one behind the kart as it draws that kart.

### homing projectile

The attacking item. Fired, it chases the kart next ahead of its owner by
race position: along the spline at 1.6 times top speed, steering for a
point ahead on the centreline, and straight at the kart once within lock
range. Reaching the kart it is used up: on a hazard the kart trails, which
goes with it, else on the kart, which spins out. Fired by the leader, with
no kart ahead, it is wasted. Every snapshot carries the projectiles in
flight; a client draws them between snapshots as it draws the other karts.

### spin-out

A kart stopped and spun by an item for a fixed time (1 s): dead to its
controls and moved by nothing until the time runs out. Every spin-out is a
server event, and the snapshots carry the time left; clients only show it.

### bump

The velocity a kart carries away from a kart it touched, decaying over a
fixed time (0.3 s to a third) and then gone. A wall gives none: it holds
the kart at the corner that met it, with no bounce. The handling adds it to
the kart's own velocity every tick, so a touch throws the pair apart instead
of letting one push the other along; the kart that drove in loses speed and
the kart hit keeps its own. A new bump replaces one under way, never adds
to it. A contact never turns a kart: only steering does. Resolved on the
server; the client's prediction runs the same rule against the other karts'
kinematic stand-ins and keeps its own bump on a rewind, since a snapshot
does not carry one.

### livery

The palette that tells one kart from another, borrowed from a team's colours:
primary, secondary and accent, the three the kart is painted in, and a
key colour for every single-colour use (the minimap, swatches, kart labels).
Eight liveries, assigned in room join order; each comes with its slot's
driver.

### reconciliation

The client replacing its predicted state with a snapshot and replaying the
inputs the server has not yet acknowledged. The visible correction is blended
over up to six frames rather than snapped.

### interpolation

How the client draws the other karts: between the two snapshots it has
received either side of a render clock 100 ms behind the newest, never
past the newest. The own kart's prediction meets them there as kinematic
bodies it cannot push.

### replay

A race as `{seed, track, bots, inputs[]}`: enough to rerun it anywhere.
Inputs are run-length encoded into segments, one input per human kart in
grid order for a run of ticks; the bots drive themselves from the seed. A
replay committed under `assets/replays` with the state hashes it reported is
a fixture of the determinism gate.

### state hash

A platform-independent hash of everything in the sim that influences future
ticks. A replay reports it after every 60th tick; the native and wasm builds
must report the same values for the same replay.

## Track

### sidecar

The data file exported from Blender next to a track's glTF: the centreline,
checkpoints, item box and boost pad positions, the spawn grid and the tagged
collision meshes. The sim reads the sidecar and never the glTF.

### surface

What is under a point on the track: asphalt, sand, grass, wall, or out of
world when nothing is below it. Collision meshes in the sidecar are tagged
with the first four; the sim answers with the topmost one under a point.

### wall

The barrier along each edge of the run-off, the whole way round every
track: the track limit. A kart can leave the asphalt for the sand but never
the course, so no corner can be cut and no checkpoint skipped. A wall holds
a kart at the corner that met it, with no bounce (ADR 0022).

## Client

### Scene

One screen of the client with its own lifecycle: Title, Room, Race, Results.
Exactly one Scene is active at a time.

### View

The part of a Scene that renders the world. The Race Scene has one View, the
3D track, drawn by Three.js from kart transforms the bridge reports.

### Instrument panel

The HUD overlaid on a View: position, lap, lap time, best lap, item slot,
speed, gear and tyre, wrong-way warning, connection banner, the countdown's
count over the View and the course map. Reads state; never drives it.

### plate

A readout's chamfered backing on the Instrument panel: a lit, a mid and a
shaded band of one colour, over the ink drop, with no outline (ADR 0037).
Carbon for most readouts, the local kart's primary for the position, the
item box gold for the item slot, a kart's key colour for its label.

### gear

The Instrument panel's reading of the local kart's speed as a gearbox: six
even bands up to top speed, N while the speed shows as 0 km/h, R while
reversing, and 6th held on the limiter over top speed. Worked out in the
client from the kart readout's speed; the sim has no gears (ADR 0031).

### shift lights

The row of twelve lights over the gear, lit green, red then blue in thirds
as the revs in the gear climb to the shift, all blue on the limiter.
Decorative: the gear and the speed are the text.

### tyre wear

How worn a kart's tyres are, as a percentage. The Instrument panel's tyre
gauge shows it on the local kart drawn from above, its tyres green above
60%, yellow above 30% and red below. The sim does not wear tyres yet, so the gauge holds at 100%
(ADR 0031).

### race readout

What the Race Scene reads of a race once a frame, as of the last tick:
every kart readout and standing, the phase, the local kart's item slot, the
item boxes there to pick up, the hazards and the projectiles. A solo race
reports it from the sim in the browser and a room's race from the server's
snapshots and events; the View, the Instrument panel and the audio each
read the one readout. Not a snapshot, which is the server's message.

### course map

The track's outline seen from above in a corner of the Instrument panel,
drawn from the sidecar's centreline as asphalt between white walls with a
chequered start, and a faceted octagon marker per kart in its key colour at
the transform the View draws that frame. The own kart's marker is larger
and ringed in white. The map is fixed: it never rotates with
the own kart.

### Presentation

Everything the player sees and hears that is not a rule: meshes, materials,
particles, camera motion, audio. Presentation never changes sim state.

### Feedback

A piece of Presentation tied to one mechanic so the mechanic has a visible or
audible tell: tyre marks on drift, sparks on mini-turbo charge, a dust puff
off-track, a spin animation on spin-out, beeps on the countdown and sparks
on a rocket start's charge, a sputter and dust at the wheels on its stall,
a shadow disc on the road under each kart so a hop's height reads, and on
every boost a flame at the exhaust and, for the own kart, wind lines and a
wider field of view.

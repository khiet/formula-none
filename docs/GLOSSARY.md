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
empty at Start, and a human's kart once they disconnect or the race ends
without them. A bot reads only what any observer of the race can, the track
and the karts' transforms and standings, and answers with the same input
struct a human's controller produces. Bots start on the grid behind the
humans.

### phase

Where a race is: Countdown, Racing or Finished. Sim state, stepped like
every other rule, so a solo race in the browser and a room on the server
run the same lifecycle. Countdown holds the karts on the grid for the 3-2-1
count, during which the throttle sets up the rocket start. Racing runs the
laps and ends 20 s after the first finisher or when every kart has
finished; the unfinished are placed by progress at that point. Finished
freezes the standings as the results. A room's Countdown and Racing states
follow the race's phase, and its Results state its Finished.

### rocket start

What the throttle hold ending at Go earns, read from the hold's length; a
release before Go resets it, and the throttle counts as held above half.
Its outcomes are the start boost and the stall. A hold longer than 2.0 s,
begun before the 2 appeared, stalls the kart: 2.0 s dead to every control,
as in a spin-out. A hold of 1.5 to 2.0 s, begun in the first half of the 2,
earns the full start boost, 1.5 s at +40 percent, the boost item's values.
A hold of 1.0 to 1.5 s, begun in the second half of the 2, earns the weak
start boost, 0.6 s at +30 percent, the blue mini-turbo's values. A shorter
hold, begun at the 1 or later, starts clean. Bots roll the full boost or a
clean start once per race from the seed and never stall; a kart handed to
a bot during the countdown starts clean. The countdown is beeped so the
start can be timed by ear; sparks show every charging kart's tier, blue for
the weak boost and orange for the full one, with no warning once the hold
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
Brands Patch has twelve. A kart respawns at its last checkpoint facing the
next one.

### spline progress

Distance along the track centreline. Race position is ordered by lap, then
checkpoint index, then spline progress. Decreasing spline progress for two
seconds triggers the wrong-way warning.

### standing

Where a kart is in the race: its race position, laps completed, last
checkpoint crossed in order, spline progress, lap time, best lap, finish
time once it has finished and whether the wrong-way warning is up. The sim
reports one per kart every tick; the Instrument panel shows the local
kart's, and the Results Scene lists every kart's once the race is Finished.
Finishers hold their places in finishing order whatever they do after the
line.

### hop

A short jump (0.3 s airtime) that starts a drift when it lands while the kart
is steering above 40 percent of top speed.

### drift

Sliding through a corner after a hop. The drift direction is fixed at the hop;
counter-steer widens the arc and inward steer tightens it. Holding a drift
charges a mini-turbo.

### mini-turbo

The boost released at the end of a long enough drift: blue at 0.8 s of drift
(0.6 s boost), orange at 1.6 s (1.0 s boost), both at +30 percent top speed.
Shown by sparks at the wheels while charging.

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

The velocity a kart carries away from a kart or wall it touched, decaying
over a fixed time (0.3 s to a third) and then gone. The handling adds it to
the kart's own velocity every tick, so a touch throws the pair apart instead
of letting one push the other along; the kart that drove in loses speed and
the kart hit keeps its own. A new bump replaces one under way, never adds
to it. A contact never turns a kart: only steering does. Resolved on the
server; the client's prediction runs the same rule against the other karts'
kinematic stand-ins and keeps its own bump on a rewind, since a snapshot
does not carry one.

### livery

The fixed colour that tells one kart from another. Eight liveries, assigned in
room join order.

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

## Client

### Scene

One screen of the client with its own lifecycle: Title, Room, Race, Results.
Exactly one Scene is active at a time.

### View

The part of a Scene that renders the world. The Race Scene has one View, the
3D track, drawn by Three.js from kart transforms the bridge reports.

### Instrument panel

The HUD overlaid on a View: position, lap, lap time, best lap, item slot,
wrong-way warning, connection banner, the countdown's count over the View
and the course map. Reads state; never drives it.

### course map

The track's outline seen from above in a corner of the Instrument panel,
drawn from the sidecar's centreline with a start/finish tick, and a marker
per kart in its livery at the transform the View draws that frame. The own
kart's marker is larger and ringed. The map is fixed: it never rotates with
the own kart.

### Presentation

Everything the player sees and hears that is not a rule: meshes, materials,
particles, camera motion, audio. Presentation never changes sim state.

### Feedback

A piece of Presentation tied to one mechanic so the mechanic has a visible or
audible tell: tyre marks on drift, sparks on mini-turbo charge, a dust puff
off-track, a spin animation on spin-out, beeps on the countdown and sparks
on a rocket start's charge, a sputter and dust at the wheels on its stall.

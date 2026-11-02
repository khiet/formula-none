# ADR 0077: The slipstream is a tow on the top speed cap, judged by the server alone

## Status

Accepted (2026-11-02). Extends ADR 0027's situation with the wake, and
the prediction's list of what it never runs (ADR 0009, the module doc of
`crates/sim/src/prediction.rs`) with the wake's verdict. The PRD's fiction
asks for "boost as DRS"; this is the mechanic behind it.

## Context

Every way a kart gained on the one ahead was an item, a drift or a pad.
Nothing rewarded simply following: a kart tucked behind another drove at
exactly the same top speed and could never close, so a race only stayed
close while the item table did the work. Issue 266 adds the slipstream:
a kart that holds another's wake for a second is towed to a higher top
speed cap, for as long as it stays there and for a second after it pulls
out, so it draws level and has to earn the pass. No item and no button,
open to every kart and every bot.

Three things had to be settled. What the tow is, since a boost would
replace a mini-turbo under way and defeat the brake. Who judges the wake,
since the prediction's race holds the own kart alone with the others
standing where the client draws them, 100 ms behind, so its verdict at the
wake's edge would differ from the server's. And how the own kart feels it,
since the tow has no button press to answer.

## Decision

- The wake. `step` judges it beside the situation build, from every body
  before the handling ticks, so the order the karts are ticked in never
  decides who is in whose wake. A kart is in a wake when some other kart
  travels at `SLIPSTREAM_LEADER_SPEED_FRACTION` (0.5) of the tuned top
  speed or more, its centre lies from 0 to `SLIPSTREAM_RANGE_M` (12 m, the
  bots' lookahead) behind that kart's centre along its travel and within
  `SLIPSTREAM_HALF_WIDTH_M` (1.2 m, twice a kart's half width) of it
  across, and the two travel within `SLIPSTREAM_ANGLE_RAD` (0.5) of each
  other, all seen from above. Travel is the body's velocity, not its
  facing, so a drifting leader's wake lies along the road; a kart that is
  not moving has no travel and is in no wake; two wakes count as one. The
  handling reads the verdict from its situation and never the physics
  world or the track (ADR 0027).
- The tow. The handling counts the ticks without a break in a wake, in
  ticks and not f32 seconds so the tow takes on the tick
  `SLIPSTREAM_CHARGE_S` (1 s) names; the tick the charge fills, and every
  tick in the wake after, arms the tow at `SLIPSTREAM_TOW_S` (1 s); every
  other tick a tow under way runs down. A tick out of a wake drops the
  charge. While the tow holds, the cap the speed ramps toward is the
  situation's top speed times `SLIPSTREAM_TOP_SPEED_FACTOR` (1.15): 34.5
  m/s, 37.95 m/s with a full purse. A cap and not a boost: it never calls
  the handling's boost, so a mini-turbo under way is never replaced, the
  brake still works, a boost that fires sets its own speed and the kart
  then settles back to the towed cap, and the off-track cap still wins
  after the ramp. A spin-out's freeze, a respawn and a star's run all
  clear the charge and the tow; a star kart neither charges nor holds a
  tow, and tows others as any kart does. Both are in the state hash.
- The readout carries `slipstream`, `Charging { s }` in a wake short of
  the charge and `Tow { left_s }` while towed, a tagged enum across the
  seams like the item slot (ADR 0026); the snapshot mirrors it and the
  protocol version goes up.
- The prediction never judges the wake. Its race carries a flag that
  leaves the situation's wake unjudged, and the handling then carries the
  slipstream on as the last snapshot left it, as the server would while
  the kart stays where the snapshot had it: a charge under way counts on,
  a full one keeps the tow armed, a tow with the charge gone runs down. A
  tow at the full tuned time is read as one still in the wake. The
  reconciliation compares the slipstream as it compares the timers, so a
  snapshot that disagrees is a rewind that adopts the server's word; that
  happens as the kart enters or leaves a wake, and moves the kart by
  nothing, since the cap is the same under both. A wake's entry therefore
  reaches the own kart about one round trip late, which no button press
  is waiting on, and the tow then takes on the server's tick.
- Bots are unchanged. A bot already drives its line behind the kart ahead
  and steers round it within the lookahead, so it is towed when it is
  tucked in and passes when it can; its wheel model already reads its
  speed. One correction the new traffic exposed: the bot's model of a
  boost item it holds now includes its coins, as the handling does, so it
  no longer fires a boost into a pass it judged a little too slow. A
  `const` assert holds the homing projectile faster than a towed kart with
  a full purse under the bots' widest rubber band, and the factor under
  the mini-turbo's, so a drift still pays more than a tow.
- The own kart feels it through the wind lines, the boost's tell: a charge
  throws them at a rate climbing from nothing to the least boost rate as
  it fills, a tow at the boost's full rate, fading over the tow's last
  moments as a boost's do; a boost under way wins. No flame, which is a
  boost's, and no field of view change. Other karts show nothing; the
  leader learns it is towed when the follower draws level. The slipstream
  whoosh, a swell of noise with no tone, plays once as the own kart's
  reading goes from charging or nothing to towed, and nothing as the tow
  holds, is re-armed or runs out, so a reconciliation is silent; the tow
  is not a boost, so the boost pad's inference never fires for it. Under
  the SFX toggle like every cue.
- The replay fixtures are re-recorded, since the hash gains the charge and
  the tow, and the fixture set must now tow a kart.

## Consequences

- A kart that follows for a second draws level at 34.5 m/s against 30 and
  then has the pass to make; a leader that pulls a lane across is out of
  the follower's wake and the tow runs out a second later.
- The rubber band and the tow multiply: a bot behind the nearest human by
  its full range is towed to 1.15 x 1.08 of top speed, which the asserts
  bound under the projectile's speed.
- The wake is a straight box behind the leader, so in a turn it reaches
  less far back than on a straight: on a 30 m radius about 8 m. A follower
  that holds the inside line through a turn can be out of the wake and
  back in before the tow runs out.
- The own kart's wake entry and exit each cost one rewind of the
  prediction that moves it by nothing; the tow's onset and end are
  predicted on the server's tick, so a 100 ms round trip shows no
  correction.
- A tell on other karts, a readout on the Instrument panel, bots seeking a
  wake on purpose and any retune of the range, factor or charge stay out
  of scope.

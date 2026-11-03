//! The bot driver (ADR 0002). A bot is handed what any observer of the race
//! can read, the track and every kart's transform and standing, and answers
//! with the [`KartInput`] a human's controller would, so a bot-driven kart is
//! just a kart whose input comes from here instead of a device.

use crate::tuning::{
    coin_factor, BotProfile, ACCELERATION_TO_TOP_S, BOT_AVOID_HALF_WIDTH_M, BOT_BRAKE_HORIZON_S,
    BOT_CORNER_SAMPLE_M, BOT_DRIFT_CURVATURE_RAD_PER_M, BOT_HAZARD_HOLD_S, BOT_HOP_STEER,
    BOT_HORN_HOLD_S, BOT_KERB_APPROACH_M, BOT_KERB_GAP_M, BOT_LEVELS, BOT_LINE_RIGHT_OF_CENTRE_M,
    BOT_LOOKAHEAD_M, BOT_PASS_GAP_M, BOT_REVERSE_S, BOT_STALL_S, BOT_STALL_SPEED_M_PER_S,
    BOT_WHEEL_MARGIN_S, BOT_WHEEL_SET_OFF_FRACTION, BRAKE_TO_STOP_S, DRIFT_SLIP_ANGLE_RAD,
    DRIFT_TURN_NEUTRAL, DRIFT_TURN_STEER_RANGE, ITEM_BOOST_FACTOR, KART_HALF_LENGTH_M,
    KART_HALF_WIDTH_M, KERB_HALF_LENGTH_M, KERB_HALF_WIDTH_M, REVERSE_TOP_SPEED_M_PER_S,
    STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S, TOP_SPEED_M_PER_S,
    WHEEL_LANDING_SPREAD_M, WHEEL_RADIUS_M, WHEEL_WARNING_S,
};
use crate::wheel::{pass_not_over, pass_start_s, Path};
use crate::{
    DriftDirection, HornBlast, Item, KartInput, KartReadout, Phase, Placement, Standing, Track,
};
use rapier3d::prelude::{Rotation, Vec3};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How hard a race's bots drive (ADR 0078): the creator's pick, fixed for
/// the room as its course is, and carried by a replay. Basic is the bots
/// as they were before there was a pick; the levels above it read the
/// profile of the same name in [`BOT_LEVELS`]. Ace above Pro is the racing
/// sense: a pro drives for a living, an ace wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum BotLevel {
    /// Lifts for corners, waits on an item, and the rubber band slows it
    /// when it gets ahead of the humans.
    #[default]
    Basic,
    /// Carries more speed through corners, uses an item sooner and waits
    /// less for the humans.
    Pro,
    /// Corners as a human does, uses an item the moment it lands, always
    /// rolls the full rocket start and never slows for a human behind it.
    Ace,
}

impl BotLevel {
    /// Every level, Basic first, in the order [`BOT_LEVELS`] is indexed.
    pub const ALL: [Self; 3] = [Self::Basic, Self::Pro, Self::Ace];

    /// The constants the bots read at this level.
    pub fn profile(self) -> &'static BotProfile {
        &BOT_LEVELS[self as usize]
    }

    /// The level as the wire spells it: `basic`, `pro` or `ace`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Pro => "pro",
            Self::Ace => "ace",
        }
    }

    /// The level spelled `name` on the wire, or `None` for any other
    /// spelling, so a caller that takes the level as text refuses an
    /// unknown one rather than racing Basic.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.name() == name)
    }
}

/// What a bot carries from one tick to the next: how long it has been
/// stalled, how long it still backs up, how long it has held its item and
/// how long it still trails a fired hazard. The timers steer the next tick,
/// so [`crate::State::hash`] covers them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Memory {
    /// Whether the bot held back for a runaway wheel on its last tick. Only
    /// reported, never steers, so the hash leaves it out.
    held_for_the_wheel: bool,
    stalled_s: f32,
    reversing_s: f32,
    /// Seconds the bot has seen an item in its slot; zero while it is empty.
    item_held_s: f32,
    /// Seconds the bot keeps the item button down after firing a hazard,
    /// trailing it; zero or less once dropped.
    hazard_hold_left_s: f32,
}

impl Memory {
    /// Whether the bot held back for a runaway wheel on its last tick.
    pub fn held_for_the_wheel(&self) -> bool {
        self.held_for_the_wheel
    }

    /// The timers, for the hash.
    pub(crate) fn timers(&self) -> [f32; 4] {
        [
            self.stalled_s,
            self.reversing_s,
            self.item_held_s,
            self.hazard_hold_left_s,
        ]
    }
}

/// The input the bot in `slot` gives its kart this tick, from the track,
/// every kart's readouts and the race clock as of the last tick, and the
/// race's seed, which decides the runaway wheel's path, updating what it
/// remembers. `profile` is the race's bot level's, how hard it drives.
// Each argument is a separate thing any observer of the race can read.
#[allow(clippy::too_many_arguments)]
pub fn drive(
    track: &Track,
    profile: &BotProfile,
    karts: &[KartReadout],
    standings: &[Standing],
    clock: Phase,
    seed: u64,
    slot: usize,
    memory: &mut Memory,
    dt: f32,
) -> KartInput {
    let kart = &karts[slot];
    let progress = standings[slot].spline_progress;
    let target_speed = corner_speed(profile, sharpest_ahead(track, progress, kart.speed));
    // On a star's run the wheel cannot hurt it and the karts in its way are
    // its targets, so it neither waits nor steers round anyone.
    let starring = kart.star_s.is_some();
    let waiting = match clock {
        Phase::Racing { elapsed_s, .. } if !starring => {
            waits_for_a_wheel(track, kart, target_speed, seed, elapsed_s, dt)
        }
        _ => false,
    };
    let in_the_way = if starring {
        None
    } else {
        kart_in_the_way(karts, slot)
    };
    // A kerb ahead outranks a pass, whose point may lie on the kerb.
    let kerb_line = line_clear_of_kerbs(track, kart.position, progress);
    let target = match (kerb_line, in_the_way) {
        (Some(line), _) => line_point(track, progress + BOT_LOOKAHEAD_M, line),
        (None, Some(other)) => pass_point(
            track,
            standings[other].spline_progress,
            karts[other].position,
        ),
        (None, None) => line_point(
            track,
            progress + BOT_LOOKAHEAD_M,
            BOT_LINE_RIGHT_OF_CENTRE_M,
        ),
    };
    let mut steer = pursuit_steer(kart, target);
    memory.held_for_the_wheel = false;

    // Stalled for the tuned time, the bot backs up for the tuned time with
    // its nose swinging toward the target, then drives on.
    // Held back for a wheel is not stalled, but only setting off clears the
    // stall: a kart jammed against a wall jitters in and out of waiting, and
    // each wait clearing it would hold the kart there until the pass is over.
    if memory.reversing_s <= 0.0 && kart.speed.abs() < BOT_STALL_SPEED_M_PER_S {
        if !waiting {
            memory.stalled_s += dt;
        }
        // Backing up from just past a crossing can carry it into a pass.
        if memory.stalled_s >= BOT_STALL_S && !backs_into_a_wheel(track, kart, seed, clock) {
            memory.stalled_s = 0.0;
            memory.reversing_s = BOT_REVERSE_S;
        }
    } else {
        memory.stalled_s = 0.0;
    }
    if memory.reversing_s > 0.0 {
        memory.reversing_s -= dt;
        // Reversing turns the kart the other way for a steer, so the nose
        // still swings toward the target.
        return KartInput {
            steer: -steer,
            brake: 1.0,
            ..KartInput::default()
        };
    }

    let curvature = track.curvature_at(progress + BOT_LOOKAHEAD_M);
    // No drift while a kerb is in range: a drift always turns into its
    // corner, so it can carry the kart onto the kerb whatever the bot aims at.
    let corner = curvature.abs() >= BOT_DRIFT_CURVATURE_RAD_PER_M && kerb_line.is_none();
    let into_corner = if curvature > 0.0 {
        DriftDirection::Left
    } else {
        DriftDirection::Right
    };
    let hop = match kart.drift {
        // Let a drift go when the corner is behind, or when the next bend
        // turns the other way: the release boosts, and a fresh press hops
        // again next tick.
        Some(drift) => corner && drift.direction == into_corner,
        None => {
            if corner {
                // The hop fixes the drift's direction from the steer, so
                // the steer must point into the corner as the kart hops.
                let inward = steer * into_corner.steer_sign();
                steer = into_corner.steer_sign() * inward.max(BOT_HOP_STEER);
            }
            corner
        }
    };

    let (throttle, brake) = if kart.speed > target_speed {
        (0.0, 1.0)
    } else if waiting {
        memory.held_for_the_wheel = true;
        // It stops short of the wheel's reach, which coasting would carry
        // it into. Within a tick of stopped the throttle joins the brake,
        // which then stops the kart dead instead of backing it up into
        // whatever is behind; already backing up, it coasts to a stop.
        // Handling's full brake, over one tick.
        let braked_in_a_tick = TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S * dt;
        if kart.speed > braked_in_a_tick {
            (0.0, 1.0)
        } else if kart.speed > 0.0 {
            (1.0, 1.0)
        } else {
            (0.0, 0.0)
        }
    } else {
        (1.0, 0.0)
    };
    // The brake tightens a drift (a brake drift), which the bot does not
    // use: on a tick its drift steers, it coasts to slow instead. A drift
    // steers while the hop is held, through a fall whether held or not, and
    // from the tick a held hop lands. Waiting for the wheel outranks it.
    let drift_steers = match kart.drift {
        Some(_) => hop || kart.airborne,
        None => hop && kart.airborne,
    };
    let (throttle, brake) = if drift_steers && !waiting && brake > 0.0 {
        (0.0, 0.0)
    } else {
        (throttle, brake)
    };

    // An item is fired on the first straight after it lands in the slot,
    // with no corner to slow for, and in any case once it has been held for
    // the tuned delay. A projectile needs a kart ahead to chase, so a leading
    // bot keeps it until it is passed. A boost is kept while the bot waits
    // for the wheel and while the boosted kart would have to, so it never
    // throws the bot into a pass it judged at its own speed; a star is not,
    // since the wheel cannot hurt a star kart. A fired hazard is trailed
    // for the tuned time, then dropped. A horn is worth saving: it is fired
    // as soon as its blast would reach a kart it can hit, and otherwise once
    // it has been held for the horn's own, longer, hold. The bot reads karts
    // and not projectiles, so it fires at no projectile chasing it.
    let held = kart.item_slot.held();
    let has_target = held != Some(Item::Homing) || standings[slot].position > 1;
    let boost_meets_a_wheel = held == Some(Item::Boost)
        && match clock {
            Phase::Racing { elapsed_s, .. } => {
                let boosted = KartReadout {
                    speed: ITEM_BOOST_FACTOR * TOP_SPEED_M_PER_S * coin_factor(kart.coins),
                    ..*kart
                };
                waiting || waits_for_a_wheel(track, &boosted, boosted.speed, seed, elapsed_s, dt)
            }
            _ => false,
        };
    let use_item = match held {
        Some(Item::Horn) => {
            memory.item_held_s += dt;
            memory.item_held_s >= BOT_HORN_HOLD_S || horn_hits_a_kart(track, karts, slot)
        }
        Some(_) => {
            memory.item_held_s += dt;
            has_target
                && !boost_meets_a_wheel
                && ((!corner && brake == 0.0) || memory.item_held_s >= profile.item_use_delay_s)
        }
        None => {
            memory.item_held_s = 0.0;
            false
        }
    };
    if use_item && held == Some(Item::Hazard) {
        memory.hazard_hold_left_s = BOT_HAZARD_HOLD_S;
    }
    let hold_item = use_item || memory.hazard_hold_left_s > 0.0;
    memory.hazard_hold_left_s -= dt;
    KartInput {
        steer,
        throttle,
        brake,
        hop,
        use_item,
        hold_item,
    }
}

/// Whether the bot's horn, fired now, would reach a kart its blast can
/// hit: another kart in reach that is neither spun out nor on a star's
/// run. One trailing a hazard counts, though the blast only takes the
/// hazard.
fn horn_hits_a_kart(track: &Track, karts: &[KartReadout], slot: usize) -> bool {
    let own = &karts[slot];
    let blast = HornBlast::new(track, Vec3::from(own.position), own.spline_progress);
    karts.iter().enumerate().any(|(other, kart)| {
        other != slot
            && kart.spin_out_s.is_none()
            && kart.star_s.is_none()
            && blast.reaches_kart(
                Vec3::from(kart.position),
                &Rotation::from_array(kart.rotation),
                kart.spline_progress,
            )
    })
}

/// The point `right` metres right of the centreline at `progress`, as `[x, z]`.
fn line_point(track: &Track, progress: f32, right: f32) -> [f32; 2] {
    let point = track.point_at(progress);
    let [tx, tz] = track.direction_at(progress);
    // Right of the direction of travel (x, z), seen from above, is (-z, x).
    [point[0] - tz * right, point[2] + tx * right]
}

/// The line, in metres right of the centreline, for a kart at `position`
/// reading `progress` to aim along from the tuned approach short of a
/// sausage kerb to its far end: the tuned gap from the kerb's near edge, or
/// `None` with no kerb in range. Pursuit cuts toward an apex, where a kerb
/// lies, so the usual line runs too close.
fn line_clear_of_kerbs(track: &Track, position: [f32; 3], progress: f32) -> Option<f32> {
    track.kerbs().iter().find_map(|kerb| {
        let (ahead, beside) = kerb.local(position);
        let approaching_or_alongside =
            (-KERB_HALF_LENGTH_M - BOT_KERB_APPROACH_M..=KERB_HALF_LENGTH_M).contains(&ahead)
                && beside.abs() < BOT_LOOKAHEAD_M;
        approaching_or_alongside.then(|| {
            // A kerb in range lies within the progress window of the kart.
            let progress = track.progress_near(kerb.position, progress);
            let centre = track.point_at(progress);
            let [tx, tz] = track.direction_at(progress);
            let kerb_right =
                (kerb.position[0] - centre[0]) * -tz + (kerb.position[2] - centre[2]) * tx;
            let clear = KERB_HALF_WIDTH_M + BOT_KERB_GAP_M;
            if kerb_right > 0.0 {
                BOT_LINE_RIGHT_OF_CENTRE_M.min(kerb_right - clear)
            } else {
                BOT_LINE_RIGHT_OF_CENTRE_M.max(kerb_right + clear)
            }
        })
    })
}

/// Whether the kart must stop for a runaway wheel at `elapsed_s` on the
/// race clock, in a race from `seed`: it is short of where any pass can
/// reach it, and the path of the pass under way or next, the tuned margin
/// either side, meets the kart driven on along its lane, either at its
/// speed or gaining speed flat out up to `target_speed`, what the bot
/// drives at for the corners ahead. The lane spans from where the kart is
/// across to the bot's line, which it steers for on the way. Once the kart
/// is within reach of a pass, it stops only where stopping keeps it clear.
fn waits_for_a_wheel(
    track: &Track,
    kart: &KartReadout,
    target_speed: f32,
    seed: u64,
    elapsed_s: f32,
    dt: f32,
) -> bool {
    // Along the track the wheel reaches a kart this far either side of its
    // centre, and across it this far either side of its lane.
    let along_reach = KART_HALF_LENGTH_M + WHEEL_RADIUS_M;
    let across_reach = KART_HALF_WIDTH_M + WHEEL_RADIUS_M;
    let speed = kart.speed.max(0.0);
    let top_speed = target_speed.max(speed);
    let flat_out = TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
    let setting_off = flat_out * BOT_WHEEL_SET_OFF_FRACTION;
    // How far along the kart is `after_s` from now, from `from` metres per
    // second, gaining `gain` metres per second each second up to the speed
    // it is held to.
    let driven = |from: f32, after_s: f32, gain: f32| {
        let gaining_s = if gain > 0.0 {
            ((top_speed - from) / gain).min(after_s)
        } else {
            after_s
        };
        from * gaining_s
            + gain * gaining_s * gaining_s / 2.0
            + top_speed.min(from + gain * gaining_s) * (after_s - gaining_s)
    };
    let lap = track.spline_length();
    let crossings = track.wheel_crossings().iter();
    crossings
        .zip(track.wheel_crossing_progress())
        .any(|(crossing, &crossing_progress)| {
            let line = Placement {
                position: crossing.position,
                yaw: crossing.yaw,
            };
            let (ahead, right) = line.local(kart.position);
            // Beside the crossing, as on another leg of the course, is not on it;
            // its run-off is.
            let (_, stretch_right) = line.local(track.point_at(kart.spline_progress));
            if stretch_right.abs() > crossing.half_width {
                return false;
            }
            // Along the track the way the kart faces: one turned back toward
            // the crossing from past it meets the wheel just as one on its way.
            // Turning round in the run-off it waits for nothing, or it would
            // never back out.
            let facing_back = libm::cosf(yaw(kart.rotation) - crossing.yaw) < 0.0;
            if facing_back && right.abs() > crossing.half_width {
                return false;
            }
            // Past the crossing along the lap by more than the wheel's reach and
            // a lookahead, it has left the crossing behind, though it may face
            // back toward it: turning through a hairpin in line with the
            // crossing, it waits for nothing.
            let past = (kart.spline_progress - crossing_progress).rem_euclid(lap);
            let left_behind = WHEEL_LANDING_SPREAD_M + along_reach + BOT_LOOKAHEAD_M;
            if facing_back && (left_behind..lap / 2.0).contains(&past) {
                return false;
            }
            let ahead = if facing_back { -ahead } else { ahead };
            let way = if facing_back { -1.0 } else { 1.0 };
            let short_of_reach = -(WHEEL_LANDING_SPREAD_M + along_reach) - ahead;
            let pass = pass_not_over(crossing, seed, elapsed_s);
            let path = Path::new(crossing, seed, pass);
            let starts_s = pass_start_s(pass);
            let ends_s = starts_s + path.lasts_s();
            // A kart that cannot reach the wheel before the pass is over has
            // nothing to stop for.
            if short_of_reach > driven(speed, ends_s - elapsed_s, flat_out) {
                return false;
            }
            // The bot's line is the centreline's offset; the crossing's centre
            // is on the centreline.
            let lane = (
                right.min(BOT_LINE_RIGHT_OF_CENTRE_M) - across_reach,
                right.max(BOT_LINE_RIGHT_OF_CENTRE_M) + across_reach,
            );
            // Whether the pass meets the kart driven on, anywhere from holding
            // its speed, or setting off from a standstill, to flat out, and
            // whether it meets the kart anywhere from stopped to holding its
            // speed. Held at a standstill the kart would be in the way of
            // every pass, so it would never go.
            let (mut going_meets, mut stopping_meets) = (false, false);
            for (from, to, into_pass_s, leg_s) in path.legs() {
                let leg_starts_s = starts_s + into_pass_s;
                let rate = [(to[0] - from[0]) / leg_s, (to[1] - from[1]) / leg_s];
                // The margin widens the wheel's reach by how far it rolls in it.
                let across_margin = rate[0].abs() * BOT_WHEEL_MARGIN_S;
                let reach = along_reach + rate[1].abs() * BOT_WHEEL_MARGIN_S;
                let mut at_s = elapsed_s.max(leg_starts_s);
                while at_s <= leg_starts_s + leg_s {
                    let into_leg_s = at_s - leg_starts_s;
                    let across = from[0] + rate[0] * into_leg_s;
                    if lane.0 - across_margin <= across && across <= lane.1 + across_margin {
                        let along = way * (from[1] + rate[1] * into_leg_s);
                        let after_s = at_s - elapsed_s;
                        let held = ahead + driven(speed, after_s, 0.0);
                        let least = held.max(ahead + driven(0.0, after_s, setting_off));
                        let flat = ahead + driven(speed, after_s, flat_out);
                        going_meets |= least - reach <= along && along <= flat + reach;
                        stopping_meets |= ahead - reach <= along && along <= held + reach;
                    }
                    at_s += dt;
                }
            }
            // Short of the reach, stopping is safe. Within it a kart stops only
            // where stopping keeps it clear, as one backed up onto the crossing
            // and setting off again; otherwise it drives on through.
            if short_of_reach > 0.0 {
                going_meets
            } else {
                going_meets && !stopping_meets
            }
        })
}

/// The nearest other kart that the bot in `slot` would run into: ahead of
/// it within the lookahead and within the tuned half width of its travel.
fn kart_in_the_way(karts: &[KartReadout], slot: usize) -> Option<usize> {
    let kart = &karts[slot];
    let (sin, cos) = libm::sincosf(yaw(kart.rotation));
    // Facing -Z at yaw zero; right of the facing is (cos, -sin) in x, z.
    let forward = [-sin, -cos];
    let right = [cos, -sin];
    karts
        .iter()
        .enumerate()
        .filter(|&(other, _)| other != slot)
        .filter_map(|(other, transform)| {
            let dx = transform.position[0] - kart.position[0];
            let dz = transform.position[2] - kart.position[2];
            let ahead = dx * forward[0] + dz * forward[1];
            let beside = dx * right[0] + dz * right[1];
            (ahead > 0.0 && ahead < BOT_LOOKAHEAD_M && beside.abs() < BOT_AVOID_HALF_WIDTH_M)
                .then_some((other, ahead))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(other, _)| other)
}

/// Where to aim to pass a kart at `position`, `progress` along the lap:
/// level with it and the tuned gap beside it, on the right when it is left
/// of the bot's line and never nearer the centreline than the line, else on
/// the left.
fn pass_point(track: &Track, progress: f32, position: [f32; 3]) -> [f32; 2] {
    let centre = track.point_at(progress);
    let [tx, tz] = track.direction_at(progress);
    let other_right = (position[0] - centre[0]) * -tz + (position[2] - centre[2]) * tx;
    let right = if other_right < BOT_LINE_RIGHT_OF_CENTRE_M {
        (other_right + BOT_PASS_GAP_M).max(BOT_LINE_RIGHT_OF_CENTRE_M)
    } else {
        other_right - BOT_PASS_GAP_M
    };
    line_point(track, progress, right)
}

/// Steer toward `target`, in `KartInput::steer` units: the lock whose yaw
/// rate at the kart's speed follows the arc from its travel to the target.
/// In a drift the travel sits outside the nose by the slip angle, and the
/// lock is mapped to the trim the steer has over the drift's own turn.
fn pursuit_steer(kart: &KartReadout, target: [f32; 2]) -> f32 {
    let slip = kart.drift.map_or(0.0, |drift| {
        drift.direction.steer_sign() * DRIFT_SLIP_ANGLE_RAD
    });
    let travel = yaw(kart.rotation) + slip;
    let (dx, dz) = (target[0] - kart.position[0], target[1] - kart.position[2]);
    let distance = libm::sqrtf(dx * dx + dz * dz).max(f32::EPSILON);
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = libm::sincosf(travel);
    let rightward = (dx * cos - dz * sin) / distance;
    // The arc through the kart and the target, tangent to the travel, has
    // curvature 2 sin(angle) / distance; the yaw rate for it is that times
    // the speed, and the lock is the yaw rate over the rate full lock gives.
    let curvature = 2.0 * rightward / distance;
    let lock = kart.speed.abs() * curvature / crate::handling::steer_rate(kart.speed);
    match kart.drift {
        None => lock.clamp(-1.0, 1.0),
        // A drift turns its own way at the neutral strength, trimmed by the
        // steer over the tuned range: invert that to the steer asking for
        // `lock`, as near as the range allows.
        Some(drift) => {
            let sign = drift.direction.steer_sign();
            ((lock * sign - DRIFT_TURN_NEUTRAL) / DRIFT_TURN_STEER_RANGE * sign).clamp(-1.0, 1.0)
        }
    }
}

/// The sharpest curvature, unsigned, of the centreline from the kart to as
/// far as it travels in the brake horizon, and no nearer than the lookahead.
fn sharpest_ahead(track: &Track, progress: f32, speed: f32) -> f32 {
    let horizon = (speed * BOT_BRAKE_HORIZON_S).max(BOT_LOOKAHEAD_M);
    let samples = (horizon / BOT_CORNER_SAMPLE_M) as usize;
    (0..=samples)
        .map(|sample| {
            track
                .curvature_at(progress + sample as f32 * BOT_CORNER_SAMPLE_M)
                .abs()
        })
        .fold(0.0, f32::max)
}

/// The speed a bot aims for through a turn of `curvature`: the profile's
/// fraction of the fastest speed at which full lock still holds the turn's
/// radius, given that the steer rate falls with speed. A straight asks for
/// more than any top speed, banded or boosted, so the throttle stays open
/// along it.
fn corner_speed(profile: &BotProfile, curvature: f32) -> f32 {
    // Full lock turns a kart at speed v with radius v / rate(v), where
    // rate(v) = low + (top - low) * v / TOP; solving radius = 1 / curvature
    // for v gives the speed below.
    let rate_fall_per_m_per_s =
        (STEER_RATE_LOW_SPEED_RAD_PER_S - STEER_RATE_TOP_SPEED_RAD_PER_S) / TOP_SPEED_M_PER_S;
    let fastest = STEER_RATE_LOW_SPEED_RAD_PER_S / (curvature + rate_fall_per_m_per_s);
    fastest * profile.corner_speed_fraction
}

/// Whether the kart, backing up from a stall at `clock`, could back into
/// the reach of a pass with no time to wait for it: it is on a crossing's
/// stretch, within the wheel's reach of the line and the backing's length,
/// and the flag is up or goes up before the backing ends.
fn backs_into_a_wheel(track: &Track, kart: &KartReadout, seed: u64, clock: Phase) -> bool {
    let Phase::Racing { elapsed_s, .. } = clock else {
        return false;
    };
    let reach = WHEEL_LANDING_SPREAD_M
        + KART_HALF_LENGTH_M
        + WHEEL_RADIUS_M
        + REVERSE_TOP_SPEED_M_PER_S * BOT_REVERSE_S;
    track.wheel_crossings().iter().any(|crossing| {
        let line = Placement {
            position: crossing.position,
            yaw: crossing.yaw,
        };
        let (ahead, _) = line.local(kart.position);
        let (_, stretch_right) = line.local(track.point_at(kart.spline_progress));
        let pass = pass_not_over(crossing, seed, elapsed_s);
        ahead.abs() <= reach
            && stretch_right.abs() <= crossing.half_width
            && pass_start_s(pass) - elapsed_s < BOT_REVERSE_S + WHEEL_WARNING_S
    })
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`:
/// zero faces -Z, growing counter-clockwise seen from above.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    libm::atan2f(2.0 * (w * y + x * z), 1.0 - 2.0 * (y * y + z * z))
}

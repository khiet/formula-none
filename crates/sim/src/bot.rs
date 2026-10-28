//! The bot driver (ADR 0002). A bot is handed what any observer of the race
//! can read, the track and every kart's transform and standing, and answers
//! with the [`KartInput`] a human's controller would, so a bot-driven kart is
//! just a kart whose input comes from here instead of a device.

use crate::tuning::{
    ACCELERATION_TO_TOP_S, BOT_AVOID_HALF_WIDTH_M, BOT_BRAKE_HORIZON_S, BOT_CORNER_SAMPLE_M,
    BOT_CORNER_SPEED_FRACTION, BOT_DRIFT_CURVATURE_RAD_PER_M, BOT_HAZARD_HOLD_S, BOT_HOP_STEER,
    BOT_ITEM_USE_DELAY_S, BOT_KERB_APPROACH_M, BOT_KERB_GAP_M, BOT_LINE_RIGHT_OF_CENTRE_M,
    BOT_LOOKAHEAD_M, BOT_PASS_GAP_M, BOT_REVERSE_S, BOT_STALL_S, BOT_STALL_SPEED_M_PER_S,
    BOT_WHEEL_MARGIN_S, BOT_WHEEL_SET_OFF_FRACTION, DRIFT_SLIP_ANGLE_RAD, DRIFT_TURN_NEUTRAL,
    DRIFT_TURN_STEER_RANGE, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M, KERB_HALF_LENGTH_M,
    KERB_HALF_WIDTH_M, REVERSE_TOP_SPEED_M_PER_S, STEER_RATE_LOW_SPEED_RAD_PER_S,
    STEER_RATE_TOP_SPEED_RAD_PER_S, TOP_SPEED_M_PER_S, WHEEL_LANDING_SPREAD_M, WHEEL_RADIUS_M,
    WHEEL_WARNING_S,
};
use crate::wheel::{pass_not_over, pass_start_s, Path};
use crate::{DriftDirection, Item, KartInput, KartReadout, Phase, Placement, Standing, Track};

/// What a bot carries from one tick to the next: how long it has been
/// stalled, how long it still backs up, how long it has held its item and
/// how long it still trails a fired hazard. Steers the next tick, so
/// [`crate::State::hash`] covers it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Memory {
    stalled_s: f32,
    reversing_s: f32,
    /// Seconds the bot has seen an item in its slot; zero while it is empty.
    item_held_s: f32,
    /// Seconds the bot keeps the item button down after firing a hazard,
    /// trailing it; zero or less once dropped.
    hazard_hold_left_s: f32,
}

impl Memory {
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
/// remembers.
// Each argument is a separate thing any observer of the race can read.
#[allow(clippy::too_many_arguments)]
pub fn drive(
    track: &Track,
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
    let target_speed = corner_speed(sharpest_ahead(track, progress, kart.speed));
    let waiting = match clock {
        Phase::Racing { elapsed_s, .. } => {
            waits_for_a_wheel(track, kart, target_speed, seed, elapsed_s, dt)
        }
        _ => false,
    };
    // A kerb ahead outranks a pass, whose point may lie on the kerb.
    let kerb_line = line_clear_of_kerbs(track, kart.position);
    let target = match (kerb_line, kart_in_the_way(karts, slot)) {
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

    // Stalled for the tuned time, the bot backs up for the tuned time with
    // its nose swinging toward the target, then drives on.
    // Held back for a wheel is not stalled.
    if memory.reversing_s <= 0.0 && kart.speed.abs() < BOT_STALL_SPEED_M_PER_S && !waiting {
        memory.stalled_s += dt;
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
        // It stops short of the wheel's reach, which coasting would carry
        // it into.
        (0.0, 1.0)
    } else {
        (1.0, 0.0)
    };

    // An item is fired on the first straight after it is picked up, with
    // no corner to slow for, and in any case once it has been held for the
    // tuned delay. A projectile needs a kart ahead to chase, so a leading
    // bot keeps it until it is passed. A fired hazard is trailed for the
    // tuned time, then dropped.
    let has_target = kart.held_item != Some(Item::Homing) || standings[slot].position > 1;
    let use_item = match kart.held_item {
        Some(_) => {
            memory.item_held_s += dt;
            has_target && ((!corner && brake == 0.0) || memory.item_held_s >= BOT_ITEM_USE_DELAY_S)
        }
        None => {
            memory.item_held_s = 0.0;
            false
        }
    };
    if use_item && kart.held_item == Some(Item::Hazard) {
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

/// The point `right` metres right of the centreline at `progress`, as `[x, z]`.
fn line_point(track: &Track, progress: f32, right: f32) -> [f32; 2] {
    let point = track.point_at(progress);
    let [tx, tz] = track.direction_at(progress);
    // Right of the direction of travel (x, z), seen from above, is (-z, x).
    [point[0] - tz * right, point[2] + tx * right]
}

/// The line, in metres right of the centreline, for a kart at `position` to
/// aim along from the tuned approach short of a sausage kerb to its far end:
/// the tuned gap from the kerb's near edge, or `None` with no kerb in range.
/// Pursuit cuts toward an apex, where a kerb lies, so the usual line runs
/// too close.
fn line_clear_of_kerbs(track: &Track, position: [f32; 3]) -> Option<f32> {
    track.kerbs().iter().find_map(|kerb| {
        let (ahead, beside) = kerb.local(position);
        let approaching_or_alongside =
            (-KERB_HALF_LENGTH_M - BOT_KERB_APPROACH_M..=KERB_HALF_LENGTH_M).contains(&ahead)
                && beside.abs() < BOT_LOOKAHEAD_M;
        approaching_or_alongside.then(|| {
            let progress = track.progress_at(kerb.position);
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
    track.wheel_crossings().iter().any(|crossing| {
        let line = Placement {
            position: crossing.position,
            yaw: crossing.yaw,
        };
        let (ahead, right) = line.local(kart.position);
        // Beside the crossing, as on the hairpin's way back, is not on it;
        // its run-off is.
        let (_, stretch_right) = line.local(track.point_at(track.progress_at(kart.position)));
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

/// The speed a bot aims for through a turn of `curvature`: the tuned fraction
/// of the fastest speed at which full lock still holds the turn's radius,
/// given that the steer rate falls with speed. A straight asks for more than
/// any top speed, banded or boosted, so the throttle stays open along it.
fn corner_speed(curvature: f32) -> f32 {
    // Full lock turns a kart at speed v with radius v / rate(v), where
    // rate(v) = low + (top - low) * v / TOP; solving radius = 1 / curvature
    // for v gives the speed below.
    let rate_fall_per_m_per_s =
        (STEER_RATE_LOW_SPEED_RAD_PER_S - STEER_RATE_TOP_SPEED_RAD_PER_S) / TOP_SPEED_M_PER_S;
    let fastest = STEER_RATE_LOW_SPEED_RAD_PER_S / (curvature + rate_fall_per_m_per_s);
    fastest * BOT_CORNER_SPEED_FRACTION
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
        let (_, stretch_right) = line.local(track.point_at(track.progress_at(kart.position)));
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

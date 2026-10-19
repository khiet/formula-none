//! The bot driver (ADR 0002). A bot is handed what any observer of the race
//! can read, the track and every kart's transform and standing, and answers
//! with the [`KartInput`] a human's controller would, so a bot-driven kart is
//! just a kart whose input comes from here instead of a device.

use crate::tuning::{
    BOT_AVOID_HALF_WIDTH_M, BOT_BRAKE_HORIZON_S, BOT_CORNER_SAMPLE_M, BOT_CORNER_SPEED_FRACTION,
    BOT_DRIFT_CURVATURE_RAD_PER_M, BOT_HOP_STEER, BOT_ITEM_USE_DELAY_S, BOT_LINE_RIGHT_OF_CENTRE_M,
    BOT_LOOKAHEAD_M, BOT_PASS_GAP_M, BOT_REVERSE_S, BOT_STALL_S, BOT_STALL_SPEED_M_PER_S,
    DRIFT_SLIP_ANGLE_RAD, DRIFT_TURN_NEUTRAL, DRIFT_TURN_STEER_RANGE,
    STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S, TOP_SPEED_M_PER_S,
};
use crate::{DriftDirection, KartInput, KartTransform, Standing, Track};

/// What a bot carries from one tick to the next: how long it has been
/// stalled, how long it still backs up, and how long it has held its
/// item. Steers the next tick, so [`crate::State::hash`] covers it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Memory {
    stalled_s: f32,
    reversing_s: f32,
    /// Seconds the bot has seen an item in its slot; zero while it is empty.
    item_held_s: f32,
}

impl Memory {
    /// The timers, for the hash.
    pub(crate) fn timers(&self) -> [f32; 3] {
        [self.stalled_s, self.reversing_s, self.item_held_s]
    }
}

/// The input the bot in `slot` gives its kart this tick, from the track and
/// every kart's readouts as of the last tick, updating what it remembers.
pub fn drive(
    track: &Track,
    karts: &[KartTransform],
    standings: &[Standing],
    slot: usize,
    memory: &mut Memory,
    dt: f32,
) -> KartInput {
    let kart = &karts[slot];
    let progress = standings[slot].spline_progress;
    let target = match kart_in_the_way(karts, slot) {
        Some(other) => pass_point(
            track,
            standings[other].spline_progress,
            karts[other].position,
        ),
        None => line_point(
            track,
            progress + BOT_LOOKAHEAD_M,
            BOT_LINE_RIGHT_OF_CENTRE_M,
        ),
    };
    let mut steer = pursuit_steer(kart, target);

    // Stalled for the tuned time, the bot backs up for the tuned time with
    // its nose swinging toward the target, then drives on.
    if memory.reversing_s <= 0.0 && kart.speed.abs() < BOT_STALL_SPEED_M_PER_S {
        memory.stalled_s += dt;
        if memory.stalled_s >= BOT_STALL_S {
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
    let corner = curvature.abs() >= BOT_DRIFT_CURVATURE_RAD_PER_M;
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

    let target_speed = corner_speed(sharpest_ahead(track, progress, kart.speed));
    let (throttle, brake) = if kart.speed > target_speed {
        (0.0, 1.0)
    } else {
        (1.0, 0.0)
    };

    // An item is fired on the first straight after it is picked up, with
    // no corner to slow for, and in any case once it has been held for the
    // tuned delay.
    let use_item = match kart.held_item {
        Some(_) => {
            memory.item_held_s += dt;
            (!corner && brake == 0.0) || memory.item_held_s >= BOT_ITEM_USE_DELAY_S
        }
        None => {
            memory.item_held_s = 0.0;
            false
        }
    };
    KartInput {
        steer,
        throttle,
        brake,
        hop,
        use_item,
        hold_item: use_item,
    }
}

/// The point `right` metres right of the centreline at `progress`, as `[x, z]`.
fn line_point(track: &Track, progress: f32, right: f32) -> [f32; 2] {
    let point = track.point_at(progress);
    let [tx, tz] = track.direction_at(progress);
    // Right of the direction of travel (x, z), seen from above, is (-z, x).
    [point[0] - tz * right, point[2] + tx * right]
}

/// The nearest other kart that the bot in `slot` would run into: ahead of
/// it within the lookahead and within the tuned half width of its travel.
fn kart_in_the_way(karts: &[KartTransform], slot: usize) -> Option<usize> {
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
fn pursuit_steer(kart: &KartTransform, target: [f32; 2]) -> f32 {
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
    let lock = kart.speed.abs() * curvature / crate::steer_rate(kart.speed);
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

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`:
/// zero faces -Z, growing counter-clockwise seen from above.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    libm::atan2f(2.0 * (w * y + x * z), 1.0 - 2.0 * (y * y + z * z))
}

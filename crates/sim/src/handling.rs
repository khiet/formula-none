//! The handling (GLOSSARY): how one kart answers its
//! controls on the ground and in the air. Hop, drift, mini-turbo, boost,
//! the surface cap, the freeze of a stall or spin-out, the hold of a wall
//! and the bump off another kart are one rule over one [`Handling`] per
//! kart, which [`Kart`](crate::Kart) holds.
//!
//! Each tick `step` runs [`Handling::tick`] for every kart before the
//! solver, setting the body to the velocity it returns, and after the
//! solver tells it of the walls and karts it met. Nothing here reads the
//! physics world or the track: the caller reads what lies under the kart
//! and hands it in, so these rules run, and are tested, without either.

use rapier3d::prelude::*;

use crate::hash::Fnv1a;
use crate::tuning::{
    BOOST_PAD_FACTOR, BOOST_PAD_S, DRIFT_ENTRY_SPEED_FRACTION, GRAVITY_M_PER_S2, HOP_AIRTIME_S,
    KART_BUMP_DECAY_S, KART_BUMP_MIN_M_PER_S, KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS,
    MINI_TURBO_BOOST_FACTOR, OFF_TRACK_SPEED_FACTOR, TOP_SPEED_M_PER_S,
};
use crate::{Drift, DriftDirection, KartInput, Surface};

/// A bump below this is spent: dropped to zero rather than decayed on
/// without end, so a settled kart hashes the same wherever it was bumped.
const BUMP_SPENT_M_PER_S: f32 = 0.01;

/// One kart's handling: its speed and everything that bends it.
#[derive(Clone)]
pub(crate) struct Handling {
    /// The unit direction the handling sent the kart this tick: its facing,
    /// turned by the slip angle in a drift. What a wall's hold and a bump's
    /// speed loss measure along. Rewritten every tick before it is read, so
    /// not hashed.
    travel: Vec3,
    /// Unit normals, pointing at the kart, of the walls it ended the last
    /// tick against. The next tick's handling sends no velocity into them.
    walls: Vec<Vec3>,
    /// Velocity away from the karts it has touched, metres per second
    /// along the ground, added to the velocity each tick and decaying over
    /// the tuned time. Zero while nothing has bumped it lately.
    bump: Vec3,
    /// Speed along the travel, metres per second. Held outside Rapier so
    /// contacts and friction never leak into the handling.
    speed: f32,
    /// Seconds of hop airtime left; zero or less on the ground. A timer
    /// rather than a Rapier contact, so the landing tick never depends on
    /// contact tolerances.
    airtime_left_s: f32,
    /// Whether the hop button was down last tick, so a hop needs a fresh
    /// press: holding the button through a landing never hops again.
    hop_was_held: bool,
    /// The direction the kart was steering when it hopped: the direction a
    /// drift on landing takes. `None` when it hopped straight.
    hop_direction: Option<DriftDirection>,
    drift: Option<Drift>,
    /// Whether the kart was on a boost pad last tick, so a pad fires once,
    /// as the kart enters it, and its clock runs from there.
    on_boost_pad: bool,
    /// Seconds of boost left; zero or less when not boosting.
    boost_left_s: f32,
    /// Top speed multiplier while boosting: whichever boost started last.
    boost_factor: f32,
}

/// Whether a kart answers its controls this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Freeze {
    /// It does.
    Free,
    /// Dead to every control, as in a stall: it rolls on as if nothing
    /// were held.
    Controls,
    /// Dead to every control and stopped where it stands, as in a
    /// spin-out: no speed, no boost and no bump.
    Stopped,
}

/// What the handling reads of the kart's body and the world under it at
/// the start of a tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Situation {
    /// The body's rotation; the kart faces -Z under the identity.
    pub(crate) rotation: Rotation,
    /// The body's vertical velocity, which gravity and the ground agree
    /// on; kept unless a hop launches the kart.
    pub(crate) vertical_m_per_s: f32,
    /// The surface under the kart's centre.
    pub(crate) surface: Surface,
    /// Whether the kart's centre is on a boost pad, airborne or not.
    pub(crate) boost_pad: bool,
    /// The cap on the kart's speed: the tuned top speed under its rubber
    /// band.
    pub(crate) top_speed: f32,
}

/// What the handling sends the body off with for the solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sent {
    pub(crate) velocity: Vec3,
    /// Radians per second about +Y.
    pub(crate) yaw_rate: f32,
}

impl Handling {
    /// A kart at rest on the ground facing -Z, with nothing under way.
    pub(crate) fn new() -> Self {
        Self {
            travel: Vec3::NEG_Z,
            walls: Vec::new(),
            bump: Vec3::ZERO,
            speed: 0.0,
            airtime_left_s: 0.0,
            hop_was_held: false,
            hop_direction: None,
            drift: None,
            on_boost_pad: false,
            boost_left_s: 0.0,
            boost_factor: 1.0,
        }
    }

    /// Speed along the travel, metres per second; negative when backing up.
    pub(crate) fn speed(&self) -> f32 {
        self.speed
    }

    pub(crate) fn drift(&self) -> Option<Drift> {
        self.drift
    }

    pub(crate) fn airborne(&self) -> bool {
        self.airtime_left_s > 0.0
    }

    /// Seconds of boost left, while boosting.
    pub(crate) fn boost_s(&self) -> Option<f32> {
        (self.boost_left_s > 0.0).then_some(self.boost_left_s)
    }

    /// The bump the kart carries, which the last tick's velocity held.
    pub(crate) fn bump(&self) -> Vec3 {
        self.bump
    }

    /// The way the kart is going: its travel, or back along it in reverse.
    pub(crate) fn heading(&self) -> Vec3 {
        if self.speed < 0.0 {
            -self.travel
        } else {
            self.travel
        }
    }

    /// Boost at `factor` times top speed for `seconds`, in place of any
    /// boost under way.
    pub(crate) fn boost(&mut self, seconds: f32, factor: f32) {
        self.boost_left_s = seconds;
        self.boost_factor = factor;
    }

    /// End any boost under way.
    pub(crate) fn end_boost(&mut self) {
        self.boost_left_s = 0.0;
    }

    /// Run the boost under way down by `ticks` ticks of `dt`, as `tick`
    /// runs it, so the two spend it on the same tick.
    pub(crate) fn spend_boost(&mut self, ticks: u32, dt: f32) {
        for _ in 0..ticks {
            if self.boost_left_s > 0.0 {
                self.boost_left_s -= dt;
            }
        }
    }

    /// Stop the kart: a drift or a boost ends and a hop lands without a
    /// drift. The bump is left to the next tick's freeze.
    pub(crate) fn stop(&mut self) {
        self.speed = 0.0;
        self.drift = None;
        self.hop_direction = None;
        self.boost_left_s = 0.0;
    }

    /// Start over at rest facing `travel`, as a respawn puts the kart, with
    /// nothing carried over from before.
    pub(crate) fn respawn(&mut self, travel: Vec3) {
        self.speed = 0.0;
        self.airtime_left_s = 0.0;
        self.hop_direction = None;
        self.drift = None;
        self.on_boost_pad = false;
        self.boost_left_s = 0.0;
        self.travel = travel;
        self.walls.clear();
        self.bump = Vec3::ZERO;
    }

    /// Take on a snapshot's word on the kart's motion: turned by `rotation`,
    /// in `drift`, moving at `velocity`. The bump is not carried by a
    /// snapshot, so the kart's own stands, and the speed is what the
    /// velocity holds along the travel once that bump is taken off, as
    /// `tick` built it.
    pub(crate) fn adopt(&mut self, rotation: Rotation, drift: Option<Drift>, velocity: Vec3) {
        self.drift = drift;
        let slip = drift.map_or(0.0, |drift| drift_slip(drift.direction));
        self.travel = rotation * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        self.speed = self.travel.dot(velocity - self.bump);
    }

    /// Feed everything a later tick reads into the state hash. `travel` is
    /// left out: every tick writes it before anything reads it.
    pub(crate) fn hash(&self, hasher: &mut Fnv1a) {
        use std::hash::Hasher;
        hasher.write_f32(self.speed);
        hasher.write_f32(self.airtime_left_s);
        hasher.write_u8(u8::from(self.hop_was_held));
        hasher.write_f32(self.hop_direction.map_or(0.0, DriftDirection::steer_sign));
        hasher.write_f32(self.drift.map_or(0.0, |drift| drift.direction.steer_sign()));
        hasher.write_f32(self.drift.map_or(0.0, |drift| drift.charge_s));
        hasher.write_u8(u8::from(self.on_boost_pad));
        hasher.write_f32(self.boost_left_s);
        hasher.write_f32(self.boost_factor);
        hasher.write_u64(self.walls.len() as u64);
        for wall in &self.walls {
            hasher.write_f32s(wall.to_array());
        }
        hasher.write_f32s(self.bump.to_array());
    }

    /// One tick of `dt` seconds of the kart answering `input`, frozen as
    /// `freeze` says, in `situation`. Returns what to send the body off
    /// with.
    pub(crate) fn tick(
        &mut self,
        input: &KartInput,
        freeze: Freeze,
        situation: Situation,
        dt: f32,
    ) -> Sent {
        let input = match freeze {
            Freeze::Free => input,
            Freeze::Controls | Freeze::Stopped => &KartInput::default(),
        };
        let mut vertical = situation.vertical_m_per_s;
        if self.airborne() {
            self.airtime_left_s -= dt;
            if !self.airborne() {
                self.drift = self.landing_drift(input);
            }
        } else if let Some(drift) = &mut self.drift {
            if input.hop && fast_enough_to_drift(self.speed) {
                drift.charge_s += dt;
            } else {
                // Letting go releases what the drift charged; losing the
                // speed to slide just ends it.
                if !input.hop {
                    self.boost_left_s = drift.mini_turbo().boost_s();
                    self.boost_factor = MINI_TURBO_BOOST_FACTOR;
                }
                self.drift = None;
            }
        } else if input.hop && !self.hop_was_held {
            self.airtime_left_s = HOP_AIRTIME_S;
            self.hop_direction = DriftDirection::from_steer(input.steer);
            vertical = hop_launch_speed();
        }
        self.hop_was_held = input.hop;
        self.speed = next_speed(self.speed, input, dt, situation.top_speed);
        // Off the asphalt the kart is held to the cap, not dragged down to
        // it, so it slows the tick it leaves the track. The cap reads the
        // surface below the kart, so a hop over a patch does not clear it.
        if situation.surface.slows_karts() {
            self.speed = self.speed.min(OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S);
        }
        let on_boost_pad = !self.airborne() && situation.boost_pad;
        if on_boost_pad && !self.on_boost_pad {
            self.boost(BOOST_PAD_S, BOOST_PAD_FACTOR);
        }
        self.on_boost_pad = on_boost_pad;
        if self.boost_left_s > 0.0 {
            self.boost_left_s -= dt;
            self.speed = TOP_SPEED_M_PER_S * self.boost_factor;
        }
        // A stopped kart loses whatever else it had going, a boost included:
        // a prediction that learns of a spin-out from a snapshot still holds
        // a boost the snapshot does not carry.
        if freeze == Freeze::Stopped {
            self.speed = 0.0;
            self.boost_left_s = 0.0;
            self.bump = Vec3::ZERO;
        }
        let (steer, slip) = match self.drift {
            Some(drift) => (
                drift_steer(drift.direction, input.steer),
                drift_slip(drift.direction),
            ),
            None => (input.steer.clamp(-1.0, 1.0), 0.0),
        };
        let forward = situation.rotation * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        self.bump = decayed_bump(self.bump, dt);
        // A wall the kart is against takes the velocity into it, speed and
        // bump alike, before the solver sees either; otherwise the kart
        // would press into the wall a little every tick. The bump stays
        // apart from the speed, so it is never counted against it twice.
        let mut velocity = forward * self.speed + self.bump;
        if !self.walls.is_empty() {
            velocity = stopped_by_walls(velocity, &self.walls);
            self.bump = stopped_by_walls(self.bump, &self.walls);
            self.speed = forward.dot(velocity - self.bump);
        }
        self.travel = forward;
        Sent {
            velocity: velocity + Vec3::Y * vertical,
            yaw_rate: yaw_rate(self.speed, steer),
        }
    }

    /// The wall holds the kart, with no bounce and no bump. `walls` are the
    /// walls the kart ended the tick against, remembered for the next
    /// tick; `velocity` is the body's after the solver and `sent_bump` the
    /// bump the tick sent it off with. Returns the velocity with the motion
    /// into the walls taken off: the motion along them is kept, and the
    /// speed becomes what is left along the travel once the bump is taken
    /// off. A kart facing straight at a wall stops dead; one scraping along
    /// at an angle loses the share of its speed that pointed into the wall
    /// each tick, so it slows until it steers away.
    pub(crate) fn hold_against_walls(
        &mut self,
        walls: Vec<Vec3>,
        velocity: Vec3,
        sent_bump: Vec3,
    ) -> Vec3 {
        self.walls = walls;
        if self.walls.is_empty() {
            return velocity;
        }
        // The solver may already have taken the velocity into the wall; the
        // speed is what is left along the travel either way. A touch with a
        // kart this tick may have cost the speed more already: the lesser
        // stands, since neither ever adds to it.
        let stopped = stopped_by_walls(velocity, &self.walls);
        let held = self
            .travel
            .dot(stopped - stopped_by_walls(sent_bump, &self.walls));
        if held.abs() < self.speed.abs() {
            self.speed = held;
        }
        stopped
    }

    /// The kart touched another along `normal`, a unit vector along the
    /// ground pointing at it, the two closing at `closing_speed`: it takes
    /// a bump away, the least one plus the tuned share of half the closing
    /// speed, and loses the tuned share of the closing speed, scaled by how
    /// square `heading` met the other, the way it was going, forward or in
    /// reverse. A bump already under way is raised to the new one, never
    /// added to, so a touch that lasts holds the karts apart without
    /// flinging them. The caller reads `heading` once before the tick's
    /// first touch, so a touch that cuts the speed to zero leaves the way
    /// the kart was going unchanged for the touches after it.
    pub(crate) fn bumped(&mut self, normal: Vec3, closing_speed: f32, heading: Vec3) {
        let wanted = KART_BUMP_MIN_M_PER_S + KART_BUMP_RESTITUTION * closing_speed / 2.0;
        let along = self.bump.dot(normal);
        if wanted > along {
            self.bump += normal * (wanted - along);
        }
        let square = (-heading.dot(normal)).max(0.0);
        let loss = KART_BUMP_SPEED_LOSS * square * closing_speed;
        self.speed = if self.speed < 0.0 {
            (self.speed + loss).min(0.0)
        } else {
            (self.speed - loss).max(0.0)
        };
    }

    /// The drift a landing starts: the hop's direction, when the kart hopped
    /// while steering, is still steering and holding the hop button as it
    /// lands, and is going fast enough to slide.
    fn landing_drift(&self, input: &KartInput) -> Option<Drift> {
        let direction = self.hop_direction?;
        (input.hop && input.steer != 0.0 && fast_enough_to_drift(self.speed)).then_some(Drift {
            direction,
            charge_s: 0.0,
        })
    }
}

/// Fast enough to slide: what a landing needs to start a drift and a drift
/// needs to go on.
fn fast_enough_to_drift(speed: f32) -> bool {
    speed > DRIFT_ENTRY_SPEED_FRACTION * TOP_SPEED_M_PER_S
}

/// `velocity` with its component into each of `walls` removed, so the
/// motion into a wall stops and the motion along it is kept. Motion away
/// from a wall is left alone.
fn stopped_by_walls(mut velocity: Vec3, walls: &[Vec3]) -> Vec3 {
    for &normal in walls {
        let into = velocity.dot(normal);
        if into < 0.0 {
            velocity -= normal * into;
        }
    }
    velocity
}

/// The bump `dt` seconds on: a third of itself every `KART_BUMP_DECAY_S`,
/// and gone once it is too small to move the kart.
fn decayed_bump(bump: Vec3, dt: f32) -> Vec3 {
    let decayed = bump * libm::expf(-dt / KART_BUMP_DECAY_S);
    if decayed.length_squared() < BUMP_SPENT_M_PER_S * BUMP_SPENT_M_PER_S {
        Vec3::ZERO
    } else {
        decayed
    }
}

/// The upward speed that brings a kart back to the track after the tuned
/// airtime under the tuned gravity, so the body's arc agrees with the timer.
fn hop_launch_speed() -> f32 {
    -GRAVITY_M_PER_S2 * HOP_AIRTIME_S / 2.0
}

/// Speed after one tick: throttle ramps toward `top_speed` and brake toward
/// a standstill, each at its tuned rate, and the brake on its own carries on
/// into reverse. With neither held the kart coasts toward a standstill and
/// stays there. `top_speed` is the cap under the kart's rubber band; the
/// ramp rates stay the tuned top speed's.
fn next_speed(speed: f32, input: &KartInput, dt: f32, top_speed: f32) -> f32 {
    use crate::tuning::{
        ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S, COAST_TO_STOP_S, REVERSE_TOP_SPEED_M_PER_S,
    };
    let throttle = input.throttle.clamp(0.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    let accelerate = throttle * TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
    let braking = brake * TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S;
    let coasted = if throttle == 0.0 && brake == 0.0 {
        let coast = TOP_SPEED_M_PER_S / COAST_TO_STOP_S * dt;
        // Toward zero from either side, never through it.
        speed - speed.clamp(-coast, coast)
    } else {
        speed
    };
    // With the throttle held the brake stops the kart and holds it; it never
    // backs up a kart that was going forward, and never speeds up one that
    // was already backing up.
    let floor = if throttle > 0.0 {
        speed.min(0.0)
    } else {
        -REVERSE_TOP_SPEED_M_PER_S
    };
    (coasted + (accelerate - braking) * dt).clamp(floor, top_speed)
}

/// The steer a drift turns with, in the units of `KartInput::steer` but
/// past full lock when the player steers into it: the drift's own direction
/// at the tuned neutral strength, tightened by inward steer and widened by
/// counter-steer, never reversed.
fn drift_steer(direction: DriftDirection, steer: f32) -> f32 {
    use crate::tuning::{DRIFT_TURN_NEUTRAL, DRIFT_TURN_STEER_RANGE};
    let inward = (steer * direction.steer_sign()).clamp(-1.0, 1.0);
    direction.steer_sign() * (DRIFT_TURN_NEUTRAL + DRIFT_TURN_STEER_RANGE * inward)
}

/// The angle between where a drifting kart points and where it travels:
/// the nose points into the corner, the kart slides toward the outside.
/// Steering right is a negative rotation about +Y, so the outside of a
/// right drift is a positive one.
fn drift_slip(direction: DriftDirection) -> f32 {
    direction.steer_sign() * crate::tuning::DRIFT_SLIP_ANGLE_RAD
}

/// Yaw rate for a steer value: positive steer turns a kart moving forward
/// right, which is a negative rotation about +Y, and swings the nose of a
/// kart backing up the other way, as a car's does. Faster karts steer slower,
/// and a near-stationary kart barely turns at all. Full lock is 1; a drift
/// may ask for more.
fn yaw_rate(speed: f32, steer: f32) -> f32 {
    use crate::tuning::STEER_FULL_EFFECT_SPEED_M_PER_S;
    let grip = (speed.abs() / STEER_FULL_EFFECT_SPEED_M_PER_S).clamp(0.0, 1.0);
    -steer * steer_rate(speed) * grip * speed.signum()
}

/// The yaw rate full lock gives at `speed`: the tuned low-speed rate
/// falling to the tuned top-speed rate. Shared with the bot, so its model
/// of the handling is the handling.
pub(crate) fn steer_rate(speed: f32) -> f32 {
    use crate::tuning::{STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S};
    let speed_fraction = (speed.abs() / TOP_SPEED_M_PER_S).clamp(0.0, 1.0);
    STEER_RATE_LOW_SPEED_RAD_PER_S
        + (STEER_RATE_TOP_SPEED_RAD_PER_S - STEER_RATE_LOW_SPEED_RAD_PER_S) * speed_fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{
        ITEM_BOOST_FACTOR, ITEM_BOOST_S, MINI_TURBO_BLUE_BOOST_S, MINI_TURBO_BLUE_CHARGE_S,
        MINI_TURBO_ORANGE_BOOST_S, MINI_TURBO_ORANGE_CHARGE_S, TICK_S,
    };
    use crate::MiniTurbo;

    /// Flat asphalt with no pad, facing -Z, at the tuned top speed.
    fn asphalt() -> Situation {
        Situation {
            rotation: Rotation::IDENTITY,
            vertical_m_per_s: 0.0,
            surface: Surface::Asphalt,
            boost_pad: false,
            top_speed: TOP_SPEED_M_PER_S,
        }
    }

    fn going(speed: f32) -> Handling {
        let mut handling = Handling::new();
        handling.speed = speed;
        handling
    }

    fn tick(handling: &mut Handling, input: KartInput) -> Sent {
        handling.tick(&input, Freeze::Free, asphalt(), TICK_S)
    }

    fn ticks(handling: &mut Handling, input: KartInput, count: u32) {
        for _ in 0..count {
            tick(handling, input);
        }
    }

    const FULL_THROTTLE: KartInput = KartInput {
        steer: 0.0,
        throttle: 1.0,
        brake: 0.0,
        hop: false,
        use_item: false,
        hold_item: false,
    };

    fn hop_right() -> KartInput {
        KartInput {
            steer: 1.0,
            hop: true,
            ..FULL_THROTTLE
        }
    }

    /// Ticks of the hop's airtime, the landing's included.
    fn airtime_ticks() -> u32 {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let mut count = 0;
        tick(&mut handling, hop_right());
        while handling.airborne() {
            tick(&mut handling, hop_right());
            count += 1;
        }
        count
    }

    /// A kart at top speed hopped right and landed into a drift, the
    /// button still down.
    fn drifting_right() -> Handling {
        let mut handling = going(TOP_SPEED_M_PER_S);
        ticks(&mut handling, hop_right(), 1 + airtime_ticks());
        assert!(handling.drift.is_some(), "the landing starts a drift");
        handling
    }

    /// The tick a drift's charge, summed a tick at a time in f32 as `tick`
    /// sums it, first reaches `charge_s`.
    fn tick_charge_reaches(charge_s: f32) -> u32 {
        let mut sum = 0.0_f32;
        let mut count = 0;
        while sum < charge_s {
            sum += TICK_S;
            count += 1;
        }
        count
    }

    #[test]
    fn a_hop_launches_the_kart_to_land_after_the_tuned_airtime() {
        let mut handling = going(TOP_SPEED_M_PER_S);

        let sent = tick(&mut handling, hop_right());

        assert!(handling.airborne());
        assert!(sent.velocity.y > 0.0, "launched upward");
        assert_eq!(airtime_ticks(), (HOP_AIRTIME_S / TICK_S).round() as u32);
    }

    #[test]
    fn a_landing_drifts_only_above_the_entry_speed() {
        // Held just inside either side of the edge through the hop, by a
        // rubber band at that speed.
        let landed = |speed: f32| {
            let mut handling = going(speed);
            let situation = Situation {
                top_speed: speed,
                ..asphalt()
            };
            for _ in 0..1 + airtime_ticks() {
                handling.tick(&hop_right(), Freeze::Free, situation, TICK_S);
            }
            handling.drift
        };
        let edge = DRIFT_ENTRY_SPEED_FRACTION * TOP_SPEED_M_PER_S;

        assert_eq!(
            landed(edge + 0.1),
            Some(Drift {
                direction: DriftDirection::Right,
                charge_s: 0.0
            })
        );
        assert_eq!(landed(edge), None);
    }

    #[test]
    fn the_blue_mini_turbo_charges_on_the_tick_the_sum_crosses_and_pays_its_boost() {
        let blue = tick_charge_reaches(MINI_TURBO_BLUE_CHARGE_S);
        let mut handling = drifting_right();

        ticks(&mut handling, hop_right(), blue - 1);
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::None);
        tick(&mut handling, hop_right());
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Blue);

        tick(&mut handling, FULL_THROTTLE);
        assert_eq!(handling.drift, None);
        assert_eq!(handling.boost_s(), Some(MINI_TURBO_BLUE_BOOST_S - TICK_S));
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S * MINI_TURBO_BOOST_FACTOR);
    }

    #[test]
    fn the_orange_mini_turbo_charges_on_the_tick_the_sum_crosses_and_pays_its_boost() {
        let orange = tick_charge_reaches(MINI_TURBO_ORANGE_CHARGE_S);
        let mut handling = drifting_right();

        ticks(&mut handling, hop_right(), orange - 1);
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Blue);
        tick(&mut handling, hop_right());
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Orange);

        tick(&mut handling, FULL_THROTTLE);
        assert_eq!(handling.boost_s(), Some(MINI_TURBO_ORANGE_BOOST_S - TICK_S));
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S * MINI_TURBO_BOOST_FACTOR);
    }

    #[test]
    fn a_boost_pad_fires_once_on_entry_at_its_factor() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let on_pad = Situation {
            boost_pad: true,
            ..asphalt()
        };

        handling.tick(&FULL_THROTTLE, Freeze::Free, on_pad, TICK_S);
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S * BOOST_PAD_FACTOR);
        assert_eq!(handling.boost_s(), Some(BOOST_PAD_S - TICK_S));

        handling.tick(&FULL_THROTTLE, Freeze::Free, on_pad, TICK_S);
        assert_eq!(
            handling.boost_s(),
            Some(BOOST_PAD_S - TICK_S - TICK_S),
            "staying on the pad does not refire it"
        );
    }

    #[test]
    fn a_boost_pad_under_a_hop_does_not_fire() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let on_pad = Situation {
            boost_pad: true,
            ..asphalt()
        };

        handling.tick(&hop_right(), Freeze::Free, on_pad, TICK_S);

        assert_eq!(handling.boost_s(), None);
    }

    #[test]
    fn the_boost_item_holds_the_kart_at_its_factor_for_its_time() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        handling.boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR);

        let mut boosted_ticks = 0;
        loop {
            tick(&mut handling, FULL_THROTTLE);
            if handling.speed != TOP_SPEED_M_PER_S * ITEM_BOOST_FACTOR {
                break;
            }
            boosted_ticks += 1;
        }

        // The clock runs down in f32 a tick at a time, so the last tick may
        // hold a sliver of boost past the rounded count.
        let mut left = ITEM_BOOST_S;
        let mut spent_ticks = 0;
        while left > 0.0 {
            left -= TICK_S;
            spent_ticks += 1;
        }
        assert_eq!(boosted_ticks, spent_ticks);
        assert_eq!(handling.boost_s(), None);
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S, "back to the cap");
    }

    #[test]
    fn sand_and_grass_hold_the_kart_to_the_off_track_cap() {
        for surface in [Surface::Sand, Surface::Grass] {
            let mut handling = going(TOP_SPEED_M_PER_S);
            let off = Situation {
                surface,
                ..asphalt()
            };

            handling.tick(&FULL_THROTTLE, Freeze::Free, off, TICK_S);

            assert_eq!(
                handling.speed,
                OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S,
                "{surface:?}"
            );
        }
    }

    #[test]
    fn a_stall_freezes_every_control() {
        let mut handling = Handling::new();
        let everything = KartInput {
            steer: 1.0,
            throttle: 1.0,
            hop: true,
            ..KartInput::default()
        };

        let sent = handling.tick(&everything, Freeze::Controls, asphalt(), TICK_S);

        assert_eq!(handling.speed, 0.0);
        assert!(!handling.airborne());
        assert_eq!(sent.yaw_rate, 0.0);
    }

    #[test]
    fn a_spin_out_freezes_every_control_and_stops_the_kart() {
        let mut handling = drifting_right();
        handling.boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR);
        handling.bumped(Vec3::X, 4.0, handling.heading());

        let sent = handling.tick(&hop_right(), Freeze::Stopped, asphalt(), TICK_S);

        assert_eq!(handling.speed, 0.0);
        assert_eq!(handling.boost_s(), None);
        assert_eq!(handling.bump, Vec3::ZERO);
        assert_eq!(handling.drift, None, "letting go of the controls ends it");
        assert_eq!(sent.velocity, Vec3::ZERO);
        assert_eq!(sent.yaw_rate, 0.0);
    }

    #[test]
    fn a_bump_decays_to_about_a_third_in_its_time() {
        let mut handling = Handling::new();
        handling.bumped(Vec3::X, 4.0, handling.heading());
        let start = handling.bump.x;

        ticks(
            &mut handling,
            KartInput::default(),
            (KART_BUMP_DECAY_S / TICK_S).round() as u32,
        );

        // A third, loosely: a factor of e every decay time.
        let left = handling.bump.x / start;
        assert!((left - (-1.0_f32).exp()).abs() < 1e-3, "{left}");
    }

    #[test]
    fn a_new_bump_replaces_one_under_way_rather_than_adding_to_it() {
        let mut handling = Handling::new();
        handling.bumped(Vec3::X, 4.0, handling.heading());
        let first = handling.bump.x;

        handling.bumped(Vec3::X, 2.0, handling.heading());
        assert_eq!(handling.bump.x, first, "a weaker bump leaves it be");

        handling.bumped(Vec3::X, 8.0, handling.heading());
        assert_eq!(
            handling.bump.x,
            KART_BUMP_MIN_M_PER_S + KART_BUMP_RESTITUTION * 8.0 / 2.0
        );
    }
}

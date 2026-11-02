//! The handling (GLOSSARY): how one kart answers its
//! controls on the ground and in the air. Hop, drift, mini-turbo, boost,
//! the slipstream's tow, the surface cap, the freeze of a stall or
//! spin-out, the hold of a wall and the bump off another kart are one rule
//! over one [`Handling`] per kart, which [`Kart`](crate::Kart) holds.
//!
//! Each tick `step` runs [`Handling::tick`] for every kart before the
//! solver, setting the body to the velocity it returns, and after the
//! solver tells it of the walls and karts it met. Nothing here reads the
//! physics world or the track: the caller reads what lies under the kart
//! and hands it in, so these rules run, and are tested, without either
//! (ADR 0027).

use rapier3d::prelude::*;

use crate::hash::Fnv1a;
use crate::tuning::{
    BOOST_PAD_FACTOR, BOOST_PAD_S, DRIFT_ENTRY_SPEED_FRACTION, GRAVITY_M_PER_S2, HOP_AIRTIME_S,
    KART_BUMP_DECAY_S, KART_BUMP_MIN_M_PER_S, KERB_SPEED_KEEP, MINI_TURBO_BOOST_FACTOR,
    OFF_TRACK_SPEED_FACTOR, SLIPSTREAM_CHARGE_TICKS, SLIPSTREAM_TOP_SPEED_FACTOR, SLIPSTREAM_TOW_S,
    TICK_S, TOP_SPEED_M_PER_S,
};
use crate::{Drift, DriftDirection, KartInput, MiniTurbo, Slipstream, Surface};

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
    /// Seconds of hop airtime left; zero or less once it has run out. A
    /// timer rather than a Rapier contact, so a hop on the flat lands on the
    /// tuned tick whatever the contact tolerances; one over falling ground
    /// lands later, when `was_on_the_ground` says the road has come up to it.
    airtime_left_s: f32,
    /// Whether the kart stood on the track last tick, as its situation
    /// said: the next tick reads it to know whether the kart was still in
    /// the air before taking the new situation's word.
    was_on_the_ground: bool,
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
    /// Whether the kart's centre was on a sausage kerb last tick, in the air
    /// or not, so a kerb launches once, as the kart reaches it: never a kart
    /// that reached it in the air, nor one still on it after a launch.
    on_kerb: bool,
    /// Seconds of boost left; zero or less when not boosting.
    boost_left_s: f32,
    /// Top speed multiplier while boosting: whichever boost started last.
    boost_factor: f32,
    /// The tier the boost burns in, the flame's tell: whichever boost
    /// started last. Only reported, so it stays out of the hash.
    boost_tier: MiniTurbo,
    /// Ticks without a break the kart has held another's wake, saturating
    /// at the tuned charge; zero the tick it is out of one. Ticks, not f32
    /// seconds, so the tow takes on the tick the tuned charge names.
    slipstream_charge_ticks: u32,
    /// Seconds of tow left: held at the tuned time every tick the kart is
    /// in a wake with a full charge, run down every other tick. Zero or
    /// less when not towed.
    tow_left_s: f32,
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
    /// Whether the kart's centre is on a sausage kerb, airborne or not.
    pub(crate) kerb: bool,
    pub(crate) on_the_ground: bool,
    /// What the coins the kart carries multiply its forward top speed by,
    /// the off-track cap and a boost included; 1 with none.
    pub(crate) coin_factor: f32,
    /// The cap on the kart's speed: the tuned top speed under its rubber
    /// band, its coins and a star's run.
    pub(crate) top_speed: f32,
    /// Whether the kart is on a star's run, which the off-track cap does
    /// not apply to; every course is walled along both run-off edges, so
    /// this opens no shortcut.
    pub(crate) starring: bool,
    /// Whether the kart sits in another kart's wake this tick, as `step`
    /// judges it from the bodies; `None` when the race cannot judge it. A
    /// prediction's race holds the own kart alone, so there the charge
    /// and the tow are the snapshots' word, carried on as the last one
    /// left them: a charge under way counts on and a full one keeps the
    /// tow armed, until a snapshot says the kart is out of the wake
    /// (ADR 0077).
    pub(crate) wake: Option<bool>,
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
            was_on_the_ground: true,
            hop_was_held: false,
            hop_direction: None,
            drift: None,
            on_boost_pad: false,
            on_kerb: false,
            boost_left_s: 0.0,
            boost_factor: 1.0,
            boost_tier: MiniTurbo::None,
            slipstream_charge_ticks: 0,
            tow_left_s: 0.0,
        }
    }

    /// The slipstream the kart is in: the tow while one holds, else the
    /// charge while one is under way, else `None`.
    pub(crate) fn slipstream(&self) -> Option<Slipstream> {
        if self.tow_left_s > 0.0 {
            Some(Slipstream::Tow {
                left_s: self.tow_left_s,
            })
        } else if self.slipstream_charge_ticks > 0 {
            Some(Slipstream::Charging {
                s: self.slipstream_charge_ticks as f32 * TICK_S,
            })
        } else {
            None
        }
    }

    /// Take on a snapshot's word on the slipstream, as `slipstream`
    /// reports it. A tow at the full tuned time is one still in the wake,
    /// re-armed that tick, so the charge behind it is taken as full; any
    /// shorter tow is running down with the charge gone.
    pub(crate) fn adopt_slipstream(&mut self, slipstream: Option<Slipstream>) {
        match slipstream {
            None => {
                self.slipstream_charge_ticks = 0;
                self.tow_left_s = 0.0;
            }
            Some(Slipstream::Charging { s }) => {
                self.slipstream_charge_ticks = (s / TICK_S + 0.5) as u32;
                self.tow_left_s = 0.0;
            }
            Some(Slipstream::Tow { left_s }) => {
                self.slipstream_charge_ticks = if left_s >= SLIPSTREAM_TOW_S {
                    SLIPSTREAM_CHARGE_TICKS
                } else {
                    0
                };
                self.tow_left_s = left_s;
            }
        }
    }

    /// Speed along the travel, metres per second; negative when backing up.
    pub(crate) fn speed(&self) -> f32 {
        self.speed
    }

    pub(crate) fn drift(&self) -> Option<Drift> {
        self.drift
    }

    /// In a hop's airtime, or off the road for any reason: a kart that hops
    /// as the road falls away is still in the air when the airtime runs out.
    pub(crate) fn airborne(&self) -> bool {
        self.airtime_left_s > 0.0 || !self.was_on_the_ground
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

    /// The tier the boost under way burns in; `MiniTurbo::None` while not
    /// boosting.
    pub(crate) fn boost_tier(&self) -> MiniTurbo {
        if self.boost_left_s > 0.0 {
            self.boost_tier
        } else {
            MiniTurbo::None
        }
    }

    /// Boost at `factor` times top speed for `seconds`, burning in `tier`,
    /// in place of any boost under way.
    pub(crate) fn boost(&mut self, seconds: f32, factor: f32, tier: MiniTurbo) {
        self.boost_left_s = seconds;
        self.boost_factor = factor;
        self.boost_tier = tier;
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
        self.was_on_the_ground = true;
        self.hop_direction = None;
        self.drift = None;
        self.on_boost_pad = false;
        self.on_kerb = false;
        self.boost_left_s = 0.0;
        self.slipstream_charge_ticks = 0;
        self.tow_left_s = 0.0;
        self.travel = travel;
        self.walls.clear();
        self.bump = Vec3::ZERO;
    }

    /// Take on a snapshot's word on the kart's motion: turned by `rotation`,
    /// in `drift`, moving at `velocity` and carrying `bump`, which replaces
    /// the kart's own. The speed is what the velocity holds along the
    /// travel once the bump is taken off, as `tick` built it.
    pub(crate) fn adopt(
        &mut self,
        rotation: Rotation,
        drift: Option<Drift>,
        velocity: Vec3,
        bump: Vec3,
    ) {
        self.drift = drift;
        let slip = drift.map_or(0.0, |drift| drift_slip(drift.direction));
        self.travel = rotation * Rotation::from_rotation_y(slip) * Vec3::NEG_Z;
        self.bump = bump;
        self.speed = self.travel.dot(velocity - bump);
    }

    /// Feed everything a later tick reads into the state hash. `travel` is
    /// left out: every tick writes it before anything reads it.
    pub(crate) fn hash(&self, hasher: &mut Fnv1a) {
        use std::hash::Hasher;
        hasher.write_f32(self.speed);
        hasher.write_f32(self.airtime_left_s);
        hasher.write_u8(u8::from(self.was_on_the_ground));
        hasher.write_u8(u8::from(self.hop_was_held));
        hasher.write_f32(self.hop_direction.map_or(0.0, DriftDirection::steer_sign));
        hasher.write_f32(self.drift.map_or(0.0, |drift| drift.direction.steer_sign()));
        hasher.write_f32(self.drift.map_or(0.0, |drift| drift.charge_s));
        hasher.write_u8(u8::from(self.on_boost_pad));
        hasher.write_u8(u8::from(self.on_kerb));
        hasher.write_f32(self.boost_left_s);
        hasher.write_f32(self.boost_factor);
        hasher.write_u32(self.slipstream_charge_ticks);
        hasher.write_f32(self.tow_left_s);
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
        let was_airborne = self.airborne();
        self.was_on_the_ground = situation.on_the_ground;
        let launched = situation.kerb && !self.on_kerb && !was_airborne && situation.on_the_ground;
        self.on_kerb = situation.kerb;
        if launched {
            // A kerb throws the kart into a hop that lands straight, and a
            // drift under way ends there unreleased: its charge is lost. A
            // boost ends too, or it would hold the speed the launch takes.
            self.airtime_left_s = HOP_AIRTIME_S;
            self.hop_direction = None;
            self.drift = None;
            self.boost_left_s = 0.0;
            vertical = hop_launch_speed();
        } else if was_airborne {
            if self.airtime_left_s > 0.0 {
                self.airtime_left_s -= dt;
            }
            // A drift that went over a crest keeps on through the fall; a
            // hop is never taken in one, so it lands into its own. The hop's
            // direction is spent here, so a later fall lands into nothing.
            if !self.airborne() && self.drift.is_none() {
                self.drift = self.landing_drift(input);
                self.hop_direction = None;
            }
        } else if let Some(drift) = &mut self.drift {
            if input.hop && fast_enough_to_drift(self.speed) {
                drift.charge_s += dt;
            } else {
                // Letting go releases what the drift charged; losing the
                // speed to slide just ends it.
                if !input.hop {
                    let tier = drift.mini_turbo();
                    self.boost(tier.boost_s(), MINI_TURBO_BOOST_FACTOR, tier);
                }
                self.drift = None;
            }
        } else if input.hop && !self.hop_was_held && situation.on_the_ground {
            self.airtime_left_s = HOP_AIRTIME_S;
            self.hop_direction = DriftDirection::from_steer(input.steer);
            vertical = hop_launch_speed();
        }
        self.hop_was_held = input.hop;
        // The slipstream (ADR 0077). A tick in a wake counts toward the
        // charge; the tick it fills, and every tick in the wake after, arms
        // the tow at its full time. Every other tick a tow under way runs
        // down, so a kart back in a wake with a tow still running recharges
        // against it. A tick out of a wake drops the charge. A star kart
        // neither charges nor holds a tow; its run already outpaces one.
        let wake =
            !situation.starring && situation.wake.unwrap_or(self.slipstream_charge_ticks > 0);
        let mut armed = false;
        if wake {
            self.slipstream_charge_ticks =
                (self.slipstream_charge_ticks + 1).min(SLIPSTREAM_CHARGE_TICKS);
            if self.slipstream_charge_ticks == SLIPSTREAM_CHARGE_TICKS {
                self.tow_left_s = SLIPSTREAM_TOW_S;
                armed = true;
            }
        } else {
            self.slipstream_charge_ticks = 0;
        }
        if situation.starring {
            self.tow_left_s = 0.0;
        }
        let towed = self.tow_left_s > 0.0;
        if towed && !armed {
            self.tow_left_s -= dt;
        }
        // The tow is a cap and not a boost: the kart ramps up to it at the
        // normal rate, the brake works under it and a boost that fires
        // sets its own speed, after which the kart settles back to it.
        let top_speed = if towed {
            situation.top_speed * SLIPSTREAM_TOP_SPEED_FACTOR
        } else {
            situation.top_speed
        };
        self.speed = next_speed(self.speed, input, dt, top_speed);
        if launched {
            self.speed *= KERB_SPEED_KEEP;
        }
        // Off the asphalt the kart is held to the cap, not dragged down to
        // it, so it slows the tick it leaves the track. The cap reads the
        // surface below the kart, so a hop over a patch does not clear it.
        if situation.surface.slows_karts() && !situation.starring {
            self.speed = self
                .speed
                .min(OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S * situation.coin_factor);
        }
        let on_boost_pad = !self.airborne() && situation.boost_pad;
        if on_boost_pad && !self.on_boost_pad {
            self.boost(BOOST_PAD_S, BOOST_PAD_FACTOR, MiniTurbo::Red);
        }
        self.on_boost_pad = on_boost_pad;
        if self.boost_left_s > 0.0 {
            self.boost_left_s -= dt;
            self.speed = TOP_SPEED_M_PER_S * situation.coin_factor * self.boost_factor;
        }
        // A stopped kart loses whatever else it had going, a boost included:
        // a prediction that learns of a spin-out from a snapshot still holds
        // its own boost, which it does not adopt.
        if freeze == Freeze::Stopped {
            self.speed = 0.0;
            self.boost_left_s = 0.0;
            self.bump = Vec3::ZERO;
            self.slipstream_charge_ticks = 0;
            self.tow_left_s = 0.0;
        }
        let (steer, slip) = match self.drift {
            Some(drift) => (
                drift_steer(drift.direction, input),
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

    /// The kart touched another kart or a tyre wall along `normal`, a unit
    /// vector along the ground pointing out of what it touched, the two
    /// closing at `closing_speed`: it takes a bump away, the least one plus
    /// `restitution` of the closing speed, and loses `speed_loss` of the
    /// closing speed, scaled by how square `heading` met what it touched,
    /// the way it was going, forward or in reverse. A bump already under
    /// way is raised to the new one, never added to, so a touch that lasts
    /// holds the kart off without flinging it. The caller reads `heading`
    /// once before the tick's first touch, so a touch that cuts the speed
    /// to zero leaves the way the kart was going unchanged for the touches
    /// after it.
    pub(crate) fn bumped(
        &mut self,
        normal: Vec3,
        closing_speed: f32,
        heading: Vec3,
        restitution: f32,
        speed_loss: f32,
    ) {
        let wanted = KART_BUMP_MIN_M_PER_S + restitution * closing_speed;
        let along = self.bump.dot(normal);
        if wanted > along {
            self.bump += normal * (wanted - along);
        }
        let square = (-heading.dot(normal)).max(0.0);
        let loss = speed_loss * square * closing_speed;
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
/// at the tuned neutral strength, tightened by inward steer and by the
/// brake, widened by counter-steer, never reversed.
fn drift_steer(direction: DriftDirection, input: &KartInput) -> f32 {
    use crate::tuning::{DRIFT_TURN_BRAKE, DRIFT_TURN_NEUTRAL, DRIFT_TURN_STEER_RANGE};
    let inward = (input.steer * direction.steer_sign()).clamp(-1.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    direction.steer_sign()
        * (DRIFT_TURN_NEUTRAL + DRIFT_TURN_STEER_RANGE * inward + DRIFT_TURN_BRAKE * brake)
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
        ITEM_BOOST_FACTOR, ITEM_BOOST_S, KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS,
        MINI_TURBO_ORANGE_BOOST_S, MINI_TURBO_ORANGE_CHARGE_S, MINI_TURBO_RED_BOOST_S,
        MINI_TURBO_RED_CHARGE_S,
    };
    use crate::MiniTurbo;

    /// Flat asphalt with no pad, facing -Z, at the tuned top speed.
    fn asphalt() -> Situation {
        Situation {
            rotation: Rotation::IDENTITY,
            vertical_m_per_s: 0.0,
            surface: Surface::Asphalt,
            boost_pad: false,
            kerb: false,
            on_the_ground: true,
            coin_factor: 1.0,
            top_speed: TOP_SPEED_M_PER_S,
            starring: false,
            wake: Some(false),
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
    fn a_hop_over_falling_ground_lands_only_on_the_road_and_a_press_in_the_air_does_not_hop() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let falling = Situation {
            vertical_m_per_s: -2.0,
            on_the_ground: false,
            ..asphalt()
        };
        tick(&mut handling, hop_right());
        for _ in 0..airtime_ticks() {
            handling.tick(&KartInput::default(), Freeze::Free, falling, TICK_S);
        }

        let sent = handling.tick(&hop_right(), Freeze::Free, falling, TICK_S);
        assert!(handling.airborne(), "still in the air past the airtime");
        assert_eq!(sent.velocity.y, -2.0, "a press in the air does not hop");

        tick(&mut handling, hop_right());
        assert!(!handling.airborne(), "lands when the road comes up");
        assert_eq!(
            handling.drift.map(|drift| drift.direction),
            Some(DriftDirection::Right),
            "into the drift the hop set up"
        );
    }

    #[test]
    fn a_press_on_the_tick_the_road_drops_away_does_not_hop() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let leaving = Situation {
            on_the_ground: false,
            ..asphalt()
        };

        let sent = handling.tick(&hop_right(), Freeze::Free, leaving, TICK_S);

        assert_eq!(sent.velocity.y, 0.0, "no launch off the road");
    }

    #[test]
    fn a_press_in_the_air_off_a_crest_lands_into_no_drift() {
        let mut handling = drifting_right();
        tick(&mut handling, FULL_THROTTLE);
        assert_eq!(handling.drift, None, "released");
        let over_the_crest = Situation {
            on_the_ground: false,
            ..asphalt()
        };
        for _ in 0..10 {
            handling.tick(&hop_right(), Freeze::Free, over_the_crest, TICK_S);
        }

        tick(&mut handling, hop_right());

        assert!(!handling.airborne(), "landed");
        assert_eq!(handling.drift, None, "no hop to land a drift from");
    }

    #[test]
    fn a_drift_over_a_crest_lands_with_the_charge_it_took_off_with() {
        let mut handling = drifting_right();
        ticks(&mut handling, hop_right(), 10);
        let over_the_crest = Situation {
            on_the_ground: false,
            ..asphalt()
        };
        // The tick the road drops away still began on it, and charges.
        handling.tick(&hop_right(), Freeze::Free, over_the_crest, TICK_S);
        let before = handling.drift.expect("drifting").charge_s;
        for _ in 0..10 {
            handling.tick(&hop_right(), Freeze::Free, over_the_crest, TICK_S);
        }
        assert!(handling.airborne(), "off the road over the crest");

        tick(&mut handling, hop_right());

        assert_eq!(
            handling.drift,
            Some(Drift {
                direction: DriftDirection::Right,
                charge_s: before
            })
        );
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
    fn the_orange_mini_turbo_charges_on_the_tick_the_sum_crosses_and_pays_its_boost() {
        let orange = tick_charge_reaches(MINI_TURBO_ORANGE_CHARGE_S);
        let mut handling = drifting_right();

        ticks(&mut handling, hop_right(), orange - 1);
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::None);
        tick(&mut handling, hop_right());
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Orange);

        tick(&mut handling, FULL_THROTTLE);
        assert_eq!(handling.drift, None);
        assert_eq!(handling.boost_s(), Some(MINI_TURBO_ORANGE_BOOST_S - TICK_S));
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S * MINI_TURBO_BOOST_FACTOR);
    }

    #[test]
    fn the_red_mini_turbo_charges_on_the_tick_the_sum_crosses_and_pays_its_boost() {
        let red = tick_charge_reaches(MINI_TURBO_RED_CHARGE_S);
        let mut handling = drifting_right();

        ticks(&mut handling, hop_right(), red - 1);
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Orange);
        tick(&mut handling, hop_right());
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Red);

        tick(&mut handling, FULL_THROTTLE);
        assert_eq!(handling.boost_s(), Some(MINI_TURBO_RED_BOOST_S - TICK_S));
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
        assert_eq!(handling.boost_tier(), MiniTurbo::Red);

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
    fn a_kerb_ends_a_boost_under_way_so_the_launch_costs_a_boosting_kart_its_share() {
        let mut handling = going(TOP_SPEED_M_PER_S * ITEM_BOOST_FACTOR);
        handling.boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR, MiniTurbo::Red);
        let on_kerb = Situation {
            kerb: true,
            ..asphalt()
        };

        handling.tick(&FULL_THROTTLE, Freeze::Free, on_kerb, TICK_S);

        assert!(handling.airborne());
        assert_eq!(handling.boost_s(), None);
        assert_eq!(handling.speed, TOP_SPEED_M_PER_S * KERB_SPEED_KEEP);
    }

    #[test]
    fn the_boost_item_holds_the_kart_at_its_factor_for_its_time() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        handling.boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR, MiniTurbo::Red);

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
    fn a_star_kart_is_not_held_to_the_off_track_cap() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        let off = Situation {
            surface: Surface::Grass,
            starring: true,
            ..asphalt()
        };

        handling.tick(&FULL_THROTTLE, Freeze::Free, off, TICK_S);

        assert_eq!(handling.speed, TOP_SPEED_M_PER_S);
    }

    /// Flat asphalt in another kart's wake.
    fn in_a_wake() -> Situation {
        Situation {
            wake: Some(true),
            ..asphalt()
        }
    }

    #[test]
    fn a_star_kart_neither_charges_nor_holds_a_tow_in_a_wake() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        for _ in 0..SLIPSTREAM_CHARGE_TICKS {
            handling.tick(&FULL_THROTTLE, Freeze::Free, in_a_wake(), TICK_S);
        }
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Tow {
                left_s: SLIPSTREAM_TOW_S
            })
        );
        let starring = Situation {
            starring: true,
            ..in_a_wake()
        };

        handling.tick(&FULL_THROTTLE, Freeze::Free, starring, TICK_S);
        assert_eq!(handling.slipstream(), None);
        for _ in 0..SLIPSTREAM_CHARGE_TICKS {
            handling.tick(&FULL_THROTTLE, Freeze::Free, starring, TICK_S);
        }
        assert_eq!(handling.slipstream(), None, "a star kart never charges");
    }

    #[test]
    fn unjudged_the_slipstream_carries_on_as_the_last_word_left_it() {
        // In a prediction's race the wake is nobody's to judge: a charge
        // under way counts on, a full one keeps the tow armed, and a tow
        // with the charge gone runs down.
        let unjudged = Situation {
            wake: None,
            ..asphalt()
        };
        let mut handling = going(TOP_SPEED_M_PER_S);
        handling.tick(&FULL_THROTTLE, Freeze::Free, unjudged, TICK_S);
        assert_eq!(handling.slipstream(), None, "nothing to carry on");

        handling.adopt_slipstream(Some(Slipstream::Charging { s: 2.0 * TICK_S }));
        handling.tick(&FULL_THROTTLE, Freeze::Free, unjudged, TICK_S);
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Charging { s: 3.0 * TICK_S })
        );

        handling.adopt_slipstream(Some(Slipstream::Tow {
            left_s: SLIPSTREAM_TOW_S,
        }));
        handling.tick(&FULL_THROTTLE, Freeze::Free, unjudged, TICK_S);
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Tow {
                left_s: SLIPSTREAM_TOW_S
            }),
            "a full tow is one still in the wake"
        );

        handling.adopt_slipstream(Some(Slipstream::Tow { left_s: 0.5 }));
        handling.tick(&FULL_THROTTLE, Freeze::Free, unjudged, TICK_S);
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Tow {
                left_s: 0.5 - TICK_S
            })
        );
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

    /// A touch from a kart on the -X side closing at `closing_speed`, with
    /// the kart contact's share of the bounce.
    fn bumped_by_a_kart(handling: &mut Handling, closing_speed: f32) {
        let heading = handling.heading();
        handling.bumped(
            Vec3::X,
            closing_speed,
            heading,
            KART_BUMP_RESTITUTION / 2.0,
            KART_BUMP_SPEED_LOSS,
        );
    }

    #[test]
    fn a_spin_out_freezes_every_control_and_stops_the_kart() {
        let mut handling = drifting_right();
        handling.boost(ITEM_BOOST_S, ITEM_BOOST_FACTOR, MiniTurbo::Red);
        bumped_by_a_kart(&mut handling, 4.0);

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
        bumped_by_a_kart(&mut handling, 4.0);
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
        bumped_by_a_kart(&mut handling, 4.0);
        let first = handling.bump.x;

        bumped_by_a_kart(&mut handling, 2.0);
        assert_eq!(handling.bump.x, first, "a weaker bump leaves it be");

        bumped_by_a_kart(&mut handling, 8.0);
        assert_eq!(
            handling.bump.x,
            KART_BUMP_MIN_M_PER_S + KART_BUMP_RESTITUTION * 8.0 / 2.0
        );
    }
}

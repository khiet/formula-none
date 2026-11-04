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
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hash::Fnv1a;
use crate::tuning::{
    BOOST_PAD_FACTOR, BOOST_PAD_S, DRIFT_ENTRY_SPEED_FRACTION, GRAVITY_M_PER_S2, HOP_AIRTIME_S,
    KART_BUMP_DECAY_S, KART_BUMP_MIN_M_PER_S, KERB_SPEED_KEEP, MINI_TURBO_BOOST_FACTOR,
    OFF_TRACK_SPEED_FACTOR, PIT_LIMIT_M_PER_S, SLIPSTREAM_CHARGE_TICKS,
    SLIPSTREAM_TOP_SPEED_FACTOR, SLIPSTREAM_TOW_S, TICK_S, TOP_SPEED_M_PER_S,
    TYRE_ACCELERATION_FACTORS, TYRE_BRAKING_FACTORS, TYRE_LIFE_COLUMNS_PERCENT,
    TYRE_LIFE_NEW_PERCENT, TYRE_STEER_RATE_FACTORS, TYRE_TOP_SPEED_FACTORS,
    TYRE_WASH_OUT_FULL_M_PER_S2, TYRE_WASH_OUT_LIMIT_M_PER_S2, TYRE_WASH_OUT_MOST_RAD,
    TYRE_WASH_OUT_RATE_RAD_PER_S, TYRE_WEAR_BRAKING_PER_S, TYRE_WEAR_BUMP_MAX_PERCENT,
    TYRE_WEAR_BUMP_MIN_PERCENT, TYRE_WEAR_BUMP_PER_M_PER_S, TYRE_WEAR_CORNERING_PER_S,
    TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2, TYRE_WEAR_DRIFT_PER_S, TYRE_WEAR_KERB_PERCENT,
    TYRE_WEAR_MINI_TURBO_PERCENT, TYRE_WEAR_OFF_TRACK_PER_S, TYRE_WEAR_ROLLING_PER_S,
    TYRE_WEAR_SPIN_OUT_PERCENT, TYRE_WEAR_WALL_PER_S,
};
use crate::{Drift, DriftDirection, KartInput, MiniTurbo, Slipstream, Surface};

/// A bump below this is spent: dropped to zero rather than decayed on
/// without end, so a settled kart hashes the same wherever it was bumped.
const BUMP_SPENT_M_PER_S: f32 = 0.01;

/// Whether a race's tyres wear (ADR 0080): the creator's pick, fixed for the
/// room as its course is, and carried by a replay. Off is the race as it
/// was before there were tyres to manage, and what a kept room or a replay
/// without a pick comes back as, whatever a later default becomes (ADR 0083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Tyres {
    #[default]
    Off,
    On,
}

impl Tyres {
    /// Every pick, Off first.
    pub const ALL: [Self; 2] = [Self::Off, Self::On];

    /// The pick as the wire spells it: `off` or `on`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }

    /// The pick spelled `name` on the wire, or `None` for any other
    /// spelling, so a caller that takes the pick as text refuses an unknown
    /// one rather than racing Off.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tyres| tyres.name() == name)
    }
}

/// One kart's handling: its speed and everything that bends it.
#[derive(Clone)]
pub(crate) struct Handling {
    /// Whether this kart's tyres wear; fixed for the race. With Off the
    /// tyre fields below never change and stay out of the hash, so a race
    /// on Off hashes as one from before there were tyres (ADR 0083).
    tyres: Tyres,
    /// How much life the tyres have left, from 100 for new to 0. A respawn
    /// keeps it: only a stop raises it (ADR 0080).
    tyre_life: f32,
    /// Radians the line of travel slides wide of the facing past the
    /// cornering limit, on top of a drift's slip, signed as that slip is.
    /// Zero until the tyres are worn enough to wash out.
    wash_out_rad: f32,
    /// Whether anything wears the kart this tick: on the ground, moving,
    /// neither spun out nor on a star's run, as `tick` left it. What says
    /// whether a touch after the solver costs life. Rewritten every tick
    /// before it is read, so not hashed.
    wearing: bool,
    /// The touches under way, one per collider touched. A touch can last
    /// many ticks, so each costs its worst closing speed once rather than
    /// per tick; another collider is another bump.
    touches: Vec<Touch>,
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
    /// Whether the kart is on the pit limiter, between a pit lane's lines
    /// (ADR 0081): held to the limit, whatever raised its cap or boosts it,
    /// and worn only by rolling.
    pub(crate) pit_lane: bool,
}

/// What the handling sends the body off with for the solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sent {
    pub(crate) velocity: Vec3,
    /// Radians per second about +Y.
    pub(crate) yaw_rate: f32,
}

impl Handling {
    /// A kart at rest on the ground facing -Z on new tyres, with nothing
    /// under way.
    pub(crate) fn new(tyres: Tyres) -> Self {
        Self {
            tyres,
            tyre_life: TYRE_LIFE_NEW_PERCENT,
            wash_out_rad: 0.0,
            wearing: false,
            touches: Vec::new(),
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

    /// How much life the tyres have left, as a percentage.
    pub(crate) fn tyre_life(&self) -> f32 {
        self.tyre_life
    }

    /// Take on a snapshot's word on the tyres: wear from another kart or a
    /// hit is the server's to know.
    pub(crate) fn adopt_tyre_life(&mut self, tyre_life: f32) {
        self.tyre_life = tyre_life;
    }

    /// Whether this kart's tyres wear: the creator's pick.
    pub(crate) fn tyres(&self) -> Tyres {
        self.tyres
    }

    /// A stop's new set: life back to new (ADR 0082).
    pub(crate) fn new_tyres(&mut self) {
        self.tyre_life = TYRE_LIFE_NEW_PERCENT;
    }

    /// The tyres lose `percent` of their life, if they wear at all.
    fn wear(&mut self, percent: f32) {
        if self.tyres == Tyres::On {
            self.tyre_life = (self.tyre_life - percent).max(0.0);
        }
    }

    /// A spin-out takes its tuned life, whatever caused it and wherever the
    /// kart was.
    pub(crate) fn wear_for_spin_out(&mut self) {
        self.wear(TYRE_WEAR_SPIN_OUT_PERCENT);
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
    /// nothing carried over from before but the tyres.
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
        self.wash_out_rad = 0.0;
        self.travel = travel;
        self.walls.clear();
        self.bump = Vec3::ZERO;
        self.touches.clear();
    }

    /// Take on a snapshot's word on the kart's motion: turned by `rotation`,
    /// in `drift`, moving at `velocity` and carrying `bump`, which replaces
    /// the kart's own. The speed is what the velocity holds along the travel
    /// once the bump is taken off, as `tick` built it. A snapshot carries no
    /// wash-out: the kart keeps its own, which follows from its inputs and
    /// its tyre life. It cannot be read back off the snapshot's velocity,
    /// which was sent along the facing a tick of yaw before the rotation.
    pub(crate) fn adopt(
        &mut self,
        rotation: Rotation,
        drift: Option<Drift>,
        velocity: Vec3,
        bump: Vec3,
    ) {
        self.drift = drift;
        let slip = drift.map_or(0.0, |drift| drift_slip(drift.direction));
        self.travel = rotation * Rotation::from_rotation_y(slip + self.wash_out_rad) * Vec3::NEG_Z;
        self.bump = bump;
        self.speed = self.travel.dot(velocity - bump);
    }

    /// Feed everything a later tick reads into the state hash. `travel` and
    /// `wearing` are left out: every tick writes them before anything reads
    /// them. The tyres are left out on Off, where nothing changes them, so
    /// a race on Off hashes as one recorded before there were tyres.
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
        if self.tyres == Tyres::On {
            hasher.write_f32(self.tyre_life);
            hasher.write_f32(self.wash_out_rad);
            hasher.write_u64(self.touches.len() as u64);
            for touch in &self.touches {
                let (index, generation) = touch.with.into_raw_parts();
                hasher.write_u32(index);
                hasher.write_u32(generation);
                hasher.write_u8(u8::from(touch.this_tick));
                hasher.write_f32(touch.wear_paid);
            }
        }
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
        // A tick without a touch of a collider ends the touch of it.
        self.touches.retain_mut(|touch| {
            let goes_on = touch.this_tick;
            touch.this_tick = false;
            goes_on
        });
        // The life this tick starts on; what it wears counts from the next.
        let grip = TyreGrip::at(self.tyre_life);
        let mut released = false;
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
                    released = tier != MiniTurbo::None;
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
        self.speed = next_speed(self.speed, input, dt, top_speed * grip.top_speed, grip);
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
        // The limiter beats every cap and boost above; it holds the kart to
        // the limit outright, as the off-track cap does, and spends a boost
        // that would resume at the exit line.
        if situation.pit_lane {
            self.boost_left_s = 0.0;
            self.speed = self.speed.min(PIT_LIMIT_M_PER_S);
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
        // A kart in the air or stopped has no load on its tyres to slide on.
        let wanted = if freeze == Freeze::Stopped || self.airborne() {
            0.0
        } else {
            wash_out_wanted(
                self.speed,
                yaw_rate(self.speed, steer, grip.steer_rate),
                grip.most_wash_out_rad,
            )
        };
        let building = TYRE_WASH_OUT_RATE_RAD_PER_S * dt;
        self.wash_out_rad += (wanted - self.wash_out_rad).clamp(-building, building);
        let forward =
            situation.rotation * Rotation::from_rotation_y(slip + self.wash_out_rad) * Vec3::NEG_Z;
        let moving = self.speed != 0.0;
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
        if situation.pit_lane {
            velocity = self.held_to_the_pit_limit(velocity);
        }
        self.travel = forward;
        let yaw_rate = yaw_rate(self.speed, steer, grip.steer_rate);
        self.wear_for_tick(
            input,
            freeze,
            &situation,
            TickWear {
                launched,
                released,
                moving,
                yaw_rate,
            },
            dt,
        );
        Sent {
            velocity: velocity + Vec3::Y * vertical,
            yaw_rate,
        }
    }

    /// What this tick took off the tyres (ADR 0080), read as the tick left
    /// the kart. A kerb launch and a release are paid on the tick they
    /// happen, the launch's though it leaves the kart in the air; the rates
    /// are for a kart moving on the ground, spun out or starring neither.
    fn wear_for_tick(
        &mut self,
        input: &KartInput,
        freeze: Freeze,
        situation: &Situation,
        tick_wear: TickWear,
        dt: f32,
    ) {
        let running = freeze != Freeze::Stopped && !situation.starring;
        let rolling = running && !self.airborne() && tick_wear.moving;
        // The pit lane is safe ground: nothing but the roll wears a kart in
        // it, a touch included (ADR 0081).
        self.wearing = rolling && !situation.pit_lane;
        if situation.pit_lane {
            if rolling {
                self.wear(TYRE_WEAR_ROLLING_PER_S * dt);
            }
            return;
        }
        let mut lost = 0.0;
        if running && tick_wear.launched {
            lost += TYRE_WEAR_KERB_PERCENT;
        }
        if running && tick_wear.released {
            lost += TYRE_WEAR_MINI_TURBO_PERCENT;
        }
        if self.wearing {
            // Squared by multiplying, so the native and wasm builds agree.
            let cornering =
                self.speed * tick_wear.yaw_rate / TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2;
            let mut per_s = TYRE_WEAR_ROLLING_PER_S
                + TYRE_WEAR_CORNERING_PER_S * cornering * cornering
                + TYRE_WEAR_BRAKING_PER_S * input.brake.clamp(0.0, 1.0);
            if self.drift.is_some() {
                per_s += TYRE_WEAR_DRIFT_PER_S;
            }
            if situation.surface.slows_karts() {
                per_s += TYRE_WEAR_OFF_TRACK_PER_S;
            }
            if !self.walls.is_empty() {
                per_s += TYRE_WEAR_WALL_PER_S;
            }
            lost += per_s * dt;
        }
        self.wear(lost);
    }

    /// `velocity` with its motion along the ground no more than the pit limit,
    /// whatever made it up: the speed and the bump give up the same share, so
    /// the speed still reads back from a velocity with the bump taken off.
    pub(crate) fn held_to_the_pit_limit(&mut self, velocity: Vec3) -> Vec3 {
        let along_the_ground = Vec3::new(velocity.x, 0.0, velocity.z).length();
        let mut held = velocity;
        if along_the_ground > PIT_LIMIT_M_PER_S {
            let share = PIT_LIMIT_M_PER_S / along_the_ground;
            self.speed *= share;
            self.bump *= share;
            held.x *= share;
            held.z *= share;
        }
        self.speed = self.speed.min(PIT_LIMIT_M_PER_S);
        held
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

    /// The kart touched the collider `with`, the two closing at
    /// `closing_speed`: the touch's tuned life, or what a harder moment of a
    /// touch under way adds to what it has cost so far. A kart that is not
    /// wearing pays nothing, though the touch still counts as under way.
    pub(crate) fn wear_for_touch(&mut self, with: ColliderHandle, closing_speed: f32) {
        let at = self
            .touches
            .iter()
            .position(|touch| touch.with == with)
            .unwrap_or_else(|| {
                self.touches.push(Touch {
                    with,
                    this_tick: true,
                    wear_paid: 0.0,
                });
                self.touches.len() - 1
            });
        self.touches[at].this_tick = true;
        if !self.wearing {
            return;
        }
        let worn = (TYRE_WEAR_BUMP_MIN_PERCENT + TYRE_WEAR_BUMP_PER_M_PER_S * closing_speed)
            .min(TYRE_WEAR_BUMP_MAX_PERCENT);
        let paid = self.touches[at].wear_paid;
        if worn > paid {
            self.wear(worn - paid);
            self.touches[at].wear_paid = worn;
        }
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

/// What tyres on one life leave of the kart (ADR 0080). Factors of exactly 1
/// and no wash-out at new, so a race on Tyres Off drives bit for bit as one
/// from before there were tyres.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TyreGrip {
    /// Multiplies the cap on the kart's speed; a boost sets its own.
    pub(crate) top_speed: f32,
    /// Multiplies the rate full lock turns at.
    pub(crate) steer_rate: f32,
    /// Multiply the throttle's and the brake's ramp.
    pub(crate) acceleration: f32,
    pub(crate) braking: f32,
    /// The most the line of travel slides wide of the facing, radians.
    pub(crate) most_wash_out_rad: f32,
}

impl TyreGrip {
    /// The grip at `life` percent, interpolated between the tuned columns.
    pub(crate) fn at(life: f32) -> Self {
        let across = |column| across_life(life, column);
        Self {
            top_speed: across(TYRE_TOP_SPEED_FACTORS),
            steer_rate: across(TYRE_STEER_RATE_FACTORS),
            acceleration: across(TYRE_ACCELERATION_FACTORS),
            braking: across(TYRE_BRAKING_FACTORS),
            most_wash_out_rad: across(TYRE_WASH_OUT_MOST_RAD),
        }
    }
}

/// `column`, one value per life in `TYRE_LIFE_COLUMNS_PERCENT`, read at `life`
/// in a straight line between its two neighbours. Each column's life gives its
/// own value exactly, so new tyres multiply by exactly 1.
fn across_life(life: f32, column: [f32; TYRE_LIFE_COLUMNS_PERCENT.len()]) -> f32 {
    for at in 1..column.len() {
        let (above, below) = (
            TYRE_LIFE_COLUMNS_PERCENT[at - 1],
            TYRE_LIFE_COLUMNS_PERCENT[at],
        );
        if life >= below {
            let share = (life - below) / (above - below);
            return column[at - 1] * share + column[at] * (1.0 - share);
        }
    }
    column[column.len() - 1]
}

/// The slide a kart turning at `yaw_rate` asks of tyres that can give `most`:
/// nothing up to the tuned cornering limit, all of it from the tuned full
/// load, and wide of the turn, as a drift's slip is. A kart backing up does
/// not slide.
fn wash_out_wanted(speed: f32, yaw_rate: f32, most: f32) -> f32 {
    if speed <= 0.0 {
        return 0.0;
    }
    let past_the_limit = (speed * yaw_rate.abs() - TYRE_WASH_OUT_LIMIT_M_PER_S2)
        / (TYRE_WASH_OUT_FULL_M_PER_S2 - TYRE_WASH_OUT_LIMIT_M_PER_S2);
    -yaw_rate.signum() * most * past_the_limit.clamp(0.0, 1.0)
}

/// A touch of one kart or tyre wall under way.
#[derive(Clone)]
struct Touch {
    with: ColliderHandle,
    /// Whether it touched since the last `tick` began.
    this_tick: bool,
    /// The life this touch has cost so far.
    wear_paid: f32,
}

/// What a tick did that wears the tyres, beyond the kart's state as it left it.
struct TickWear {
    /// A sausage kerb launched the kart.
    launched: bool,
    /// A drift let go paid a mini-turbo.
    released: bool,
    /// The kart was going somewhere before a wall took its speed, so one
    /// pinned against a wall under throttle is held, not stopped, while one
    /// parked against it wears nothing.
    moving: bool,
    /// Radians per second about +Y.
    yaw_rate: f32,
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
/// stays there. `top_speed` is the cap under the kart's rubber band and its
/// tyres; the ramp rates stay the tuned top speed's, under `grip`. A coast
/// asks nothing of the tyres.
fn next_speed(speed: f32, input: &KartInput, dt: f32, top_speed: f32, grip: TyreGrip) -> f32 {
    use crate::tuning::{
        ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S, COAST_TO_STOP_S, REVERSE_TOP_SPEED_M_PER_S,
    };
    let throttle = input.throttle.clamp(0.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    let accelerate = throttle * TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S * grip.acceleration;
    let braking = brake * TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S * grip.braking;
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
/// may ask for more. `steer_factor` is what the tyres leave of the rate.
fn yaw_rate(speed: f32, steer: f32, steer_factor: f32) -> f32 {
    use crate::tuning::STEER_FULL_EFFECT_SPEED_M_PER_S;
    let grip = (speed.abs() / STEER_FULL_EFFECT_SPEED_M_PER_S).clamp(0.0, 1.0);
    -steer * steer_rate(speed, steer_factor) * grip * speed.signum()
}

/// The yaw rate full lock gives at `speed`: the tuned low-speed rate
/// falling to the tuned top-speed rate, times `steer_factor`, what the tyres
/// leave of it. Shared with the bot, so its model of the handling is the
/// handling.
pub(crate) fn steer_rate(speed: f32, steer_factor: f32) -> f32 {
    use crate::tuning::{STEER_RATE_LOW_SPEED_RAD_PER_S, STEER_RATE_TOP_SPEED_RAD_PER_S};
    let speed_fraction = (speed.abs() / TOP_SPEED_M_PER_S).clamp(0.0, 1.0);
    (STEER_RATE_LOW_SPEED_RAD_PER_S
        + (STEER_RATE_TOP_SPEED_RAD_PER_S - STEER_RATE_LOW_SPEED_RAD_PER_S) * speed_fraction)
        * steer_factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S};
    use crate::tuning::{
        ITEM_BOOST_FACTOR, ITEM_BOOST_S, KART_BUMP_RESTITUTION, KART_BUMP_SPEED_LOSS,
        MINI_TURBO_ORANGE_BOOST_S, MINI_TURBO_ORANGE_CHARGE_S, MINI_TURBO_RED_BOOST_S,
        MINI_TURBO_RED_CHARGE_S, TICKS_PER_SECOND, TYRE_WASH_OUT_MOST_RAD,
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
            pit_lane: false,
            starring: false,
            wake: Some(false),
        }
    }

    fn going(speed: f32) -> Handling {
        let mut handling = Handling::new(Tyres::Off);
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
    fn a_tow_keeps_running_down_in_a_wake_until_the_charge_fills_again() {
        // Towed, out of the wake for a while, then back in: the tow runs
        // down through the recharge and is re-armed only as the charge
        // fills, so weaving in and out of a wake never freezes the clock.
        let mut handling = going(TOP_SPEED_M_PER_S);
        for _ in 0..SLIPSTREAM_CHARGE_TICKS {
            handling.tick(&FULL_THROTTLE, Freeze::Free, in_a_wake(), TICK_S);
        }
        let out_for = 10;
        ticks(&mut handling, FULL_THROTTLE, out_for);
        let Some(Slipstream::Tow { left_s: left_out }) = handling.slipstream() else {
            panic!("the tow outlasts a short spell out of the wake");
        };
        assert!((left_out - (SLIPSTREAM_TOW_S - out_for as f32 * TICK_S)).abs() < 1e-5);

        handling.tick(&FULL_THROTTLE, Freeze::Free, in_a_wake(), TICK_S);
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Tow {
                left_s: left_out - TICK_S
            }),
            "back in the wake the tow still runs down"
        );
        for _ in 1..SLIPSTREAM_CHARGE_TICKS {
            handling.tick(&FULL_THROTTLE, Freeze::Free, in_a_wake(), TICK_S);
        }
        assert_eq!(
            handling.slipstream(),
            Some(Slipstream::Tow {
                left_s: SLIPSTREAM_TOW_S
            }),
            "re-armed as the charge fills"
        );
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
        let mut handling = Handling::new(Tyres::Off);
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
        let mut handling = Handling::new(Tyres::Off);
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
        let mut handling = Handling::new(Tyres::Off);
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

    /// Tyres On, on `situation`, at `speed`: the life one second of
    /// `input` takes.
    fn lost_in_a_second(speed: f32, input: KartInput, situation: Situation) -> f32 {
        let mut handling = Handling::new(Tyres::On);
        handling.speed = speed;
        let top = Situation {
            top_speed: speed.abs(),
            ..situation
        };
        let ticks = (1.0 / TICK_S).round() as u32;
        for _ in 0..ticks {
            handling.speed = speed;
            handling.tick(&input, Freeze::Free, top, TICK_S);
        }
        TYRE_LIFE_NEW_PERCENT - handling.tyre_life
    }

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "lost {actual}, expected {expected}"
        );
    }

    #[test]
    fn rolling_on_the_ground_wears_at_the_tuned_rate() {
        close(
            lost_in_a_second(10.0, FULL_THROTTLE, asphalt()),
            TYRE_WEAR_ROLLING_PER_S,
        );
    }

    #[test]
    fn braking_wears_by_the_brake_input() {
        let half_brake = KartInput {
            brake: 0.5,
            ..FULL_THROTTLE
        };
        close(
            lost_in_a_second(10.0, half_brake, asphalt()),
            TYRE_WEAR_ROLLING_PER_S + 0.5 * TYRE_WEAR_BRAKING_PER_S,
        );
    }

    /// A kart on tyres of `life` at `speed`, wearing nothing further, so a
    /// measurement holds the one life.
    fn on_life(life: f32, speed: f32) -> Handling {
        let mut handling = going(speed);
        handling.tyre_life = life;
        handling
    }

    fn after_a_second(life: f32, from: f32, input: KartInput) -> f32 {
        let mut handling = on_life(life, from);
        ticks(&mut handling, input, TICKS_PER_SECOND);
        handling.speed
    }

    #[test]
    fn worn_tyres_hold_the_kart_under_the_tables_top_speed() {
        let factors = TYRE_TOP_SPEED_FACTORS;
        let halfway = (factors[1] + factors[2]) / 2.0;
        let halfway_life = (TYRE_LIFE_COLUMNS_PERCENT[1] + TYRE_LIFE_COLUMNS_PERCENT[2]) / 2.0;
        for (life, factor) in TYRE_LIFE_COLUMNS_PERCENT
            .into_iter()
            .zip(factors)
            .chain([(halfway_life, halfway)])
        {
            let top = after_a_second(life, TOP_SPEED_M_PER_S, FULL_THROTTLE);
            let cap = TOP_SPEED_M_PER_S * factor;
            assert!((top - cap).abs() < 1e-3, "at {life}%: {top}");
        }
    }

    #[test]
    fn worn_tyres_accelerate_and_brake_less_by_the_table() {
        let flat_out = TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
        let full_brake = TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S;
        let brake = KartInput {
            brake: 1.0,
            ..KartInput::default()
        };
        for (at, life) in TYRE_LIFE_COLUMNS_PERCENT.into_iter().enumerate() {
            let gained = after_a_second(life, 0.0, FULL_THROTTLE);
            let expected = flat_out * TYRE_ACCELERATION_FACTORS[at];
            assert!((gained - expected).abs() < 1e-2, "at {life}%: {gained}");
            let shed = 25.0 - after_a_second(life, 25.0, brake);
            let expected = full_brake * TYRE_BRAKING_FACTORS[at];
            assert!((shed - expected).abs() < 1e-2, "at {life}%: {shed}");
        }
    }

    #[test]
    fn worn_tyres_turn_less_at_the_same_speed_and_lock() {
        let lock = KartInput {
            steer: 1.0,
            ..KartInput::default()
        };
        let turning = |life| {
            on_life(life, 20.0)
                .tick(&lock, Freeze::Free, asphalt(), TICK_S)
                .yaw_rate
        };
        let new = turning(TYRE_LIFE_NEW_PERCENT);
        assert!(new < 0.0, "full lock turns right");
        for (at, life) in TYRE_LIFE_COLUMNS_PERCENT.into_iter().enumerate() {
            assert!((turning(life) / new - TYRE_STEER_RATE_FACTORS[at]).abs() < 1e-3);
        }
    }

    #[test]
    fn a_mini_turbo_charges_in_the_same_ticks_on_worn_tyres() {
        let red = tick_charge_reaches(MINI_TURBO_RED_CHARGE_S);
        let mut handling = drifting_right();
        handling.tyre_life = 0.0;
        ticks(&mut handling, hop_right(), red - 1);
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Orange);
        tick(&mut handling, hop_right());
        assert_eq!(MiniTurbo::of(handling.drift), MiniTurbo::Red);
    }

    /// The angle, signed as a slip is, between where a kart held at `steer`
    /// for a second points and where the handling sends it.
    fn slid_after_a_second(life: f32, speed: f32, steer: f32) -> f32 {
        let mut handling = on_life(life, speed);
        let input = KartInput {
            steer,
            ..FULL_THROTTLE
        };
        let situation = Situation {
            top_speed: speed / TyreGrip::at(life).top_speed,
            ..asphalt()
        };
        let mut sent = None;
        for _ in 0..TICKS_PER_SECOND {
            sent = Some(handling.tick(&input, Freeze::Free, situation, TICK_S));
        }
        let velocity = sent.expect("ticked").velocity;
        libm::atan2f(-velocity.x, -velocity.z)
    }

    #[test]
    fn wash_out_moves_the_line_of_travel_only_past_the_limit() {
        // New tyres and the life above the cliff's column never slide,
        // whatever the load; at 0% full lock at 25 m/s is past the limit and
        // a gentle steer is not.
        let gone = TYRE_LIFE_COLUMNS_PERCENT[3];
        assert_eq!(slid_after_a_second(TYRE_LIFE_NEW_PERCENT, 25.0, 1.0), 0.0);
        assert_eq!(
            slid_after_a_second(TYRE_LIFE_COLUMNS_PERCENT[1], 25.0, 1.0),
            0.0
        );
        assert_eq!(slid_after_a_second(gone, 25.0, 0.2), 0.0);
        assert!(slid_after_a_second(gone, 25.0, 1.0) > 0.0);
    }

    #[test]
    fn wash_out_slides_wide_of_the_corner_and_no_further_than_the_table() {
        // Full lock at the speed each life tops out at is past the full load.
        for at in [2, 3] {
            let life = TYRE_LIFE_COLUMNS_PERCENT[at];
            let most = TYRE_WASH_OUT_MOST_RAD[at];
            let flat_out = TOP_SPEED_M_PER_S * TYRE_TOP_SPEED_FACTORS[at];
            let right = slid_after_a_second(life, flat_out, 1.0);
            let left = slid_after_a_second(life, flat_out, -1.0);
            assert!((right - most).abs() < 1e-3, "at {life}%: {right}");
            assert!((left + most).abs() < 1e-3, "at {life}%: {left}");
        }
    }

    #[test]
    fn wash_out_builds_over_ticks_so_a_keyed_steer_does_not_kink_the_line() {
        let mut handling = on_life(0.0, 25.0);
        let lock = KartInput {
            steer: 1.0,
            ..FULL_THROTTLE
        };
        tick(&mut handling, lock);
        assert!(
            handling.wash_out_rad.abs() < 0.02,
            "{}",
            handling.wash_out_rad
        );
    }

    #[test]
    fn a_kart_in_the_air_or_respawned_lets_a_wash_out_go() {
        let mut handling = on_life(0.0, 25.0);
        let lock = KartInput {
            steer: 1.0,
            ..FULL_THROTTLE
        };
        ticks(&mut handling, lock, 30);
        assert!(handling.wash_out_rad != 0.0);

        let mut in_the_air = handling.clone();
        ticks(&mut in_the_air, KartInput { hop: true, ..lock }, 15);
        assert!(in_the_air.airborne());
        assert_eq!(in_the_air.wash_out_rad, 0.0);

        handling.respawn(Vec3::NEG_Z);
        assert_eq!(handling.wash_out_rad, 0.0);
    }

    #[test]
    fn taking_a_snapshots_word_keeps_the_karts_own_wash_out_and_reads_the_speed_along_it() {
        let mut handling = on_life(0.0, 25.0);
        let lock = KartInput {
            steer: 1.0,
            ..FULL_THROTTLE
        };
        let situation = Situation {
            rotation: Rotation::from_rotation_y(0.7),
            ..asphalt()
        };
        let mut sent = None;
        for _ in 0..30 {
            sent = Some(handling.tick(&lock, Freeze::Free, situation, TICK_S));
        }
        let velocity = sent.expect("ticked").velocity;
        let (speed, washed) = (handling.speed, handling.wash_out_rad);
        assert!(washed != 0.0);

        handling.speed = 3.0;
        handling.adopt(situation.rotation, None, velocity, Vec3::ZERO);

        assert_eq!(handling.wash_out_rad, washed);
        assert!((handling.speed - speed).abs() < 1e-3);
    }

    #[test]
    fn cornering_wears_with_the_square_of_speed_times_yaw_rate() {
        let speed = 20.0;
        let corner = KartInput {
            steer: 1.0,
            ..FULL_THROTTLE
        };
        let a = speed * steer_rate(speed, 1.0);
        close(
            lost_in_a_second(speed, corner, asphalt()),
            TYRE_WEAR_ROLLING_PER_S
                + TYRE_WEAR_CORNERING_PER_S
                    * (a / TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2)
                    * (a / TYRE_WEAR_CORNERING_REFERENCE_M_PER_S2),
        );
    }

    #[test]
    fn grass_and_sand_wear_at_the_tuned_rate() {
        for surface in [Surface::Grass, Surface::Sand] {
            let off = Situation {
                surface,
                ..asphalt()
            };
            close(
                lost_in_a_second(5.0, FULL_THROTTLE, off),
                TYRE_WEAR_ROLLING_PER_S + TYRE_WEAR_OFF_TRACK_PER_S,
            );
        }
    }

    #[test]
    fn a_drift_wears_on_top_of_cornering_and_its_release_pays_once() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        handling.tyres = Tyres::On;
        ticks(&mut handling, hop_right(), 1 + airtime_ticks());
        assert!(handling.drift.is_some());
        let at_landing = handling.tyre_life;
        tick(&mut handling, hop_right());
        let a_tick = at_landing - handling.tyre_life;
        assert!(a_tick > TYRE_WEAR_DRIFT_PER_S * TICK_S, "lost {a_tick}");

        // Charged past orange, a release costs the tuned life on its tick.
        ticks(
            &mut handling,
            hop_right(),
            tick_charge_reaches(MINI_TURBO_ORANGE_CHARGE_S),
        );
        let before = handling.tyre_life;
        tick(&mut handling, FULL_THROTTLE);
        let released = before - handling.tyre_life;
        assert!(released >= TYRE_WEAR_MINI_TURBO_PERCENT, "lost {released}");
        let before = handling.tyre_life;
        tick(&mut handling, FULL_THROTTLE);
        assert!(before - handling.tyre_life < TYRE_WEAR_MINI_TURBO_PERCENT);
    }

    #[test]
    fn a_kerb_launch_takes_the_tuned_life_and_nothing_wears_in_the_air_after() {
        let mut handling = going(TOP_SPEED_M_PER_S);
        handling.tyres = Tyres::On;
        let on_kerb = Situation {
            kerb: true,
            ..asphalt()
        };
        handling.tick(&FULL_THROTTLE, Freeze::Free, on_kerb, TICK_S);
        close(
            TYRE_LIFE_NEW_PERCENT - handling.tyre_life,
            TYRE_WEAR_KERB_PERCENT,
        );
        let launched = handling.tyre_life;
        handling.tick(&FULL_THROTTLE, Freeze::Free, on_kerb, TICK_S);
        assert_eq!(handling.tyre_life, launched);
    }

    #[test]
    fn a_kart_on_a_stars_run_wears_nothing() {
        let starring = Situation {
            starring: true,
            kerb: true,
            ..asphalt()
        };
        assert_eq!(lost_in_a_second(10.0, FULL_THROTTLE, starring), 0.0);
    }

    #[test]
    fn a_kart_held_against_a_wall_wears_at_the_tuned_rate() {
        let mut handling = Handling::new(Tyres::On);
        handling.speed = 10.0;
        handling.walls = vec![Vec3::X];
        // Facing -Z along a wall on its side, so the kart scrapes on.
        tick(&mut handling, FULL_THROTTLE);
        close(
            TYRE_LIFE_NEW_PERCENT - handling.tyre_life,
            (TYRE_WEAR_ROLLING_PER_S + TYRE_WEAR_WALL_PER_S) * TICK_S,
        );
    }

    #[test]
    fn a_kart_pinned_head_on_under_throttle_wears_and_one_parked_there_does_not() {
        // Facing -Z with the wall ahead, its normal pointing back at the kart.
        let at_the_wall = || {
            let mut handling = Handling::new(Tyres::On);
            handling.walls = vec![Vec3::Z];
            handling
        };
        let mut pinned = at_the_wall();
        tick(&mut pinned, FULL_THROTTLE);
        assert!(pinned.speed.abs() < 1e-3, "the wall holds it");
        close(
            TYRE_LIFE_NEW_PERCENT - pinned.tyre_life,
            (TYRE_WEAR_ROLLING_PER_S + TYRE_WEAR_WALL_PER_S) * TICK_S,
        );

        let mut parked = at_the_wall();
        tick(&mut parked, KartInput::default());
        assert_eq!(parked.tyre_life, TYRE_LIFE_NEW_PERCENT);
    }

    fn collider(index: u32) -> ColliderHandle {
        ColliderHandle::from_raw_parts(index, 0)
    }

    /// A kart moving on Tyres On, ready to be touched after the solver.
    fn wearing() -> Handling {
        let mut handling = going(10.0);
        handling.tyres = Tyres::On;
        tick(&mut handling, FULL_THROTTLE);
        handling
    }

    #[test]
    fn a_bump_costs_by_its_closing_speed_up_to_the_most() {
        for (closing_speed, expected) in [
            (0.0, TYRE_WEAR_BUMP_MIN_PERCENT),
            (
                4.0,
                TYRE_WEAR_BUMP_MIN_PERCENT + 4.0 * TYRE_WEAR_BUMP_PER_M_PER_S,
            ),
            (200.0, TYRE_WEAR_BUMP_MAX_PERCENT),
        ] {
            let mut handling = wearing();
            let before = handling.tyre_life;
            handling.wear_for_touch(collider(1), closing_speed);
            close(before - handling.tyre_life, expected);
        }
    }

    #[test]
    fn a_touch_that_lasts_costs_its_worst_closing_speed_once() {
        let mut handling = wearing();
        let before = handling.tyre_life;
        handling.wear_for_touch(collider(1), 4.0);
        // The same tick's other manifold, weaker.
        handling.wear_for_touch(collider(1), 2.0);
        let once = TYRE_WEAR_BUMP_MIN_PERCENT + 4.0 * TYRE_WEAR_BUMP_PER_M_PER_S;
        close(before - handling.tyre_life, once);

        // Still touching next tick, at nothing and then harder.
        handling.tick(&FULL_THROTTLE, Freeze::Free, asphalt(), 0.0);
        handling.wear_for_touch(collider(1), 0.0);
        close(before - handling.tyre_life, once);
        handling.tick(&FULL_THROTTLE, Freeze::Free, asphalt(), 0.0);
        handling.wear_for_touch(collider(1), 6.0);
        let worst = TYRE_WEAR_BUMP_MIN_PERCENT + 6.0 * TYRE_WEAR_BUMP_PER_M_PER_S;
        close(before - handling.tyre_life, worst);

        // A tick clear of it ends the touch; the next one costs anew.
        handling.tick(&FULL_THROTTLE, Freeze::Free, asphalt(), 0.0);
        handling.tick(&FULL_THROTTLE, Freeze::Free, asphalt(), 0.0);
        handling.wear_for_touch(collider(1), 0.0);
        close(
            before - handling.tyre_life,
            worst + TYRE_WEAR_BUMP_MIN_PERCENT,
        );
    }

    #[test]
    fn two_colliders_touched_at_once_are_two_bumps() {
        let mut handling = wearing();
        let before = handling.tyre_life;
        handling.wear_for_touch(collider(1), 0.0);
        handling.wear_for_touch(collider(2), 0.0);
        close(
            before - handling.tyre_life,
            2.0 * TYRE_WEAR_BUMP_MIN_PERCENT,
        );
    }

    #[test]
    fn a_touch_on_a_stopped_kart_or_with_tyres_off_costs_nothing() {
        let mut stopped = Handling::new(Tyres::On);
        tick(&mut stopped, KartInput::default());
        stopped.wear_for_touch(collider(1), 4.0);
        assert_eq!(stopped.tyre_life, TYRE_LIFE_NEW_PERCENT);

        let mut off = going(10.0);
        tick(&mut off, FULL_THROTTLE);
        off.wear_for_touch(collider(1), 4.0);
        off.wear_for_spin_out();
        assert_eq!(off.tyre_life, TYRE_LIFE_NEW_PERCENT);
    }

    #[test]
    fn a_respawn_keeps_the_tyres() {
        let mut handling = wearing();
        let worn = handling.tyre_life;
        handling.respawn(Vec3::NEG_Z);
        assert_eq!(handling.tyre_life, worn);
    }
}

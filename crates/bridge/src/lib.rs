//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and [`Prediction`]: create a race on a
//! track, step it with one flat buffer of inputs, read the karts'
//! transforms as one flat buffer and the rest of their readouts as values
//! of the generated `KartReadout` type; or predict the player's own kart in
//! a room's race from the inputs it sends and the snapshots it receives.
//! The transforms Three.js reads every frame cross the wasm boundary once
//! per tick instead of once per kart per field (ADR 0003); everything else
//! rides typed (ADR 0026).

use protocol::{ClientMessage, Input, ServerMessage, Snapshot};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sim::{
    BotLevel, Item, KartInput, KartReadout, Phase, Projectile, RaceDistance, RaceEvent, Replay,
    Standing, State, Surface, Track, Tyres, Wheel,
};
use wasm_bindgen::prelude::*;

/// Floats per kart in the input buffer: steer, throttle, brake, hop, use
/// item, hold item. A button is pressed when its float is not zero.
const FLOATS_PER_INPUT: usize = 6;

/// Floats per kart in a transform buffer: position x, y, z, then the
/// rotation quaternion x, y, z, w. Y is up; identity rotation faces -Z.
pub const FLOATS_PER_TRANSFORM: usize = 7;

/// Written in the phase buffer where the cap has not started; a time is
/// never negative.
const NO_TIME: f32 = -1.0;

/// What the browser reads of the sim's tuning (ADR 0014), as a value of
/// the generated `Tuning` type.
#[wasm_bindgen]
pub fn tuning() -> Result<JsValue, String> {
    to_js(&sim::tuning::TUNING)
}

/// The replay runner, in wasm: rerun `replay` (the JSON `sim::replay`
/// documents) on the track described by `sidecar` and report the state hash
/// at every `sim::tuning::REPLAY_HASH_INTERVAL_TICKS`th tick. The native
/// runner is `sim::Replay::run`; the determinism gate holds the two to the
/// same hashes (ADR 0002). Fails, with the sim's reason, when the replay is
/// unreadable or the sidecar is not the replay's track.
#[wasm_bindgen]
pub fn replay_hashes(replay: &str, sidecar: &str) -> Result<Vec<u64>, String> {
    let replay = Replay::from_json(replay).map_err(|error| error.to_string())?;
    let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
    replay.run(track).map_err(|error| error.to_string())
}

/// The protocol version this build speaks (ADR 0013). The generated
/// `PROTOCOL_VERSION` is the same constant; the web app checks they agree.
#[wasm_bindgen]
pub fn protocol_version() -> u16 {
    protocol::PROTOCOL_VERSION
}

/// The wire bytes of the `ClientMessage` the JavaScript value `message`
/// holds, in the shape the generated TypeScript types give it. Fails, with
/// the reason, when the value is not a message.
#[wasm_bindgen]
pub fn encode_client_message(message: JsValue) -> Result<Vec<u8>, String> {
    encode_message::<ClientMessage>(message)
}

/// The `ClientMessage` that `bytes` carry, as a JavaScript value of the
/// generated type. Fails, with the reason, when they carry anything else.
#[wasm_bindgen]
pub fn decode_client_message(bytes: &[u8]) -> Result<JsValue, String> {
    decode_message::<ClientMessage>(bytes)
}

/// The wire bytes of the `ServerMessage` the JavaScript value `message`
/// holds; see [`encode_client_message`].
#[wasm_bindgen]
pub fn encode_server_message(message: JsValue) -> Result<Vec<u8>, String> {
    encode_message::<ServerMessage>(message)
}

/// The `ServerMessage` that `bytes` carry; see [`decode_client_message`].
#[wasm_bindgen]
pub fn decode_server_message(bytes: &[u8]) -> Result<JsValue, String> {
    decode_message::<ServerMessage>(bytes)
}

/// A message leaves wasm in the shape the generated types describe: a
/// missing `Option` is `null`, not `undefined`, and a `u64` is a `bigint`.
const TO_JS: serde_wasm_bindgen::Serializer = serde_wasm_bindgen::Serializer::new()
    .serialize_missing_as_null(true)
    .serialize_large_number_types_as_bigints(true);

fn encode_message<M: DeserializeOwned + Serialize>(message: JsValue) -> Result<Vec<u8>, String> {
    let message: M = serde_wasm_bindgen::from_value(message).map_err(|error| error.to_string())?;
    Ok(protocol::encode(&message))
}

fn decode_message<M: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<JsValue, String> {
    let message: M = protocol::decode(bytes).map_err(|error| error.to_string())?;
    message.serialize(&TO_JS).map_err(|error| error.to_string())
}

/// A sim value leaves wasm in the shape its generated type describes: a
/// missing `Option` is `null`, and a `usize`, which ts-rs types as a
/// `number`, is one. No sim value the bridge hands over holds a `u64`.
const SIM_TO_JS: serde_wasm_bindgen::Serializer =
    serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true);

fn to_js<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, String> {
    value
        .serialize(&SIM_TO_JS)
        .map_err(|error| error.to_string())
}

/// One race's sim, owned by the browser for prediction of the local kart.
#[wasm_bindgen]
pub struct Race {
    /// `sim::step` consumes and returns the state, so it is parked in an
    /// `Option` for the duration of a step. It is `None` only inside `step`.
    state: Option<State>,
}

/// The race distance spelled `name` on the wire, or why there is none.
fn distance_named(name: &str) -> Result<RaceDistance, String> {
    RaceDistance::from_name(name).ok_or_else(|| format!("no race distance called {name:?}"))
}

/// The Tyres pick spelled `name` on the wire, or why there is none.
fn tyres_named(name: &str) -> Result<Tyres, String> {
    Tyres::from_name(name).ok_or_else(|| format!("no Tyres pick called {name:?}"))
}

#[wasm_bindgen]
impl Race {
    /// A fresh race from `seed` on the track described by `sidecar` (the JSON
    /// exported next to the track's glTF) with `human_count` karts on the
    /// front of its spawn grid and `bot_count` bot-driven karts on the slots
    /// behind them, driving at `bot_level`, spelled as the wire does
    /// (`basic`, `pro`, `ace`), with the Tyres pick `tyres` (`off`, `on`), over
    /// `distance` (`sprint`, `grand-prix`).
    /// Fails when the sidecar is unusable or has fewer spawn slots than the
    /// karts, or the level or a pick is unknown; the reason is a plain string
    /// so the same code runs in native tests.
    #[wasm_bindgen(constructor)]
    pub fn new(
        seed: u64,
        human_count: usize,
        bot_count: usize,
        sidecar: &str,
        bot_level: &str,
        tyres: &str,
        distance: &str,
    ) -> Result<Race, String> {
        let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
        let bot_level = BotLevel::from_name(bot_level)
            .ok_or_else(|| format!("no bot level called {bot_level:?}"))?;
        let tyres = tyres_named(tyres)?;
        let distance = distance_named(distance)?;
        let kart_count = human_count + bot_count;
        if kart_count > track.spawn_grid().len() {
            return Err(format!(
                "{kart_count} karts but {} has {} spawn slots",
                track.name(),
                track.spawn_grid().len()
            ));
        }
        Ok(Self {
            state: Some(
                State::with_tyres(seed, human_count, bot_count, track, bot_level, tyres)
                    .with_distance(distance),
            ),
        })
    }

    /// Advance one tick of `dt` seconds. `inputs` holds six floats per human
    /// kart in grid order: steer, throttle, brake, then the hop, use item
    /// and hold item buttons as 0 or 1. Its length must be six times the
    /// human count; the bots drive themselves.
    pub fn step(&mut self, inputs: &[f32], dt: f32) {
        // Checked before the state is taken so a bad call leaves the race usable.
        assert_eq!(
            inputs.len(),
            self.state().human_count() * FLOATS_PER_INPUT,
            "one input per human kart"
        );
        let inputs: Vec<KartInput> = inputs
            .as_chunks::<FLOATS_PER_INPUT>()
            .0
            .iter()
            .map(
                |&[steer, throttle, brake, hop, use_item, hold_item]| KartInput {
                    steer,
                    throttle,
                    brake,
                    hop: hop != 0.0,
                    use_item: use_item != 0.0,
                    hold_item: hold_item != 0.0,
                },
            )
            .collect();
        let state = self
            .state
            .take()
            .expect("state is only absent during a step");
        self.state = Some(sim::step(state, &inputs, dt));
    }

    /// `FLOATS_PER_TRANSFORM` floats per kart in grid order: its transform.
    pub fn kart_transforms(&self) -> Vec<f32> {
        transform_floats(&self.state().kart_readouts())
    }

    /// One `[readout, surface]` pair per kart in grid order, as values of
    /// the generated `KartReadout` and `Surface` types: the readout less
    /// the transform, and the surface under the kart.
    #[wasm_bindgen(js_name = kart_readouts)]
    pub fn kart_readouts_js(&self) -> Result<JsValue, String> {
        to_js(&self.kart_readouts())
    }

    /// One standing per kart in grid order, as values of the generated
    /// `Standing` type. `sim::State::standings` documents the ordering.
    #[wasm_bindgen(js_name = standings)]
    pub fn standings_js(&self) -> Result<JsValue, String> {
        to_js(&self.standings())
    }

    /// Four floats per item box of the track, in the sidecar's order: its
    /// position x, y, z and its yaw in radians about +Y, zero facing -Z.
    /// Fixed for the race; the View places a box at each.
    pub fn item_boxes(&self) -> Vec<f32> {
        self.state()
            .track()
            .item_boxes()
            .iter()
            .flat_map(|item_box| {
                let [x, y, z] = item_box.position;
                [x, y, z, item_box.yaw]
            })
            .collect()
    }

    /// One float per item box of the track, in the sidecar's order: 1
    /// while the box is there to pick up, 0 while it is gone after a pickup.
    pub fn item_boxes_present(&self) -> Vec<f32> {
        self.state()
            .item_boxes_present()
            .into_iter()
            .map(|present| f32::from(u8::from(present)))
            .collect()
    }

    /// The item boxes the kart in grid slot `kart` took on the last step,
    /// whether its slot was empty and an item began to roll or it was full
    /// and the box gave nothing.
    pub fn boxes_taken_last_step(&self, kart: usize) -> u32 {
        let taken = self
            .state()
            .events()
            .iter()
            .filter(|event| matches!(event, RaceEvent::ItemPicked { kart: by, .. } if *by == kart))
            .count();
        u32::try_from(taken).expect("a step takes few boxes")
    }

    /// The grid slots of the karts that fired a horn on the last step, in
    /// the order they fired, so the View can grow each blast's ring where
    /// the kart stands.
    pub fn horns_fired_last_step(&self) -> Vec<u32> {
        self.state()
            .events()
            .iter()
            .filter_map(|event| match event {
                RaceEvent::ItemFired {
                    kart,
                    item: Item::Horn,
                } => Some(u32::try_from(*kart).expect("a grid slot fits")),
                _ => None,
            })
            .collect()
    }

    /// Four floats per coin of the track, in the sidecar's order: its
    /// position x, y, z and its yaw in radians about +Y, zero facing -Z.
    /// Fixed for the race.
    pub fn coins(&self) -> Vec<f32> {
        self.state()
            .track()
            .coins()
            .iter()
            .flat_map(|coin| {
                let [x, y, z] = coin.position;
                [x, y, z, coin.yaw]
            })
            .collect()
    }

    /// One float per coin of the track, in the sidecar's order: 1 while the
    /// coin is there to pick up, 0 while it is gone after a pickup.
    pub fn coins_present(&self) -> Vec<f32> {
        self.state()
            .coins_present()
            .into_iter()
            .map(|present| f32::from(u8::from(present)))
            .collect()
    }

    /// Five floats per hazard on the track, in the order they were fired:
    /// its position x, y, z, the grid index of the kart that fired it, and
    /// 1 while that kart still trails it, else 0.
    pub fn hazards(&self) -> Vec<f32> {
        self.state()
            .hazards()
            .iter()
            .flat_map(|hazard| {
                let [x, y, z] = hazard.position;
                [
                    x,
                    y,
                    z,
                    hazard.owner as f32,
                    f32::from(u8::from(hazard.held)),
                ]
            })
            .collect()
    }

    /// Five floats per homing projectile in flight, in the order they were
    /// fired: its position x, y, z, the grid index of the kart that fired
    /// it and the grid index of the kart it chases.
    pub fn projectiles(&self) -> Vec<f32> {
        projectile_floats(&self.state().projectiles())
    }

    /// The runaway wheel at each of the track's crossings as of the last
    /// step, in the floats [`wheel_floats`] documents.
    pub fn wheels(&self) -> Vec<f32> {
        wheel_floats(self.state().track(), &self.state().wheels())
    }

    /// Three floats for where the race is (`sim::Phase`): the phase as
    /// `sim::Phase::code` (0 countdown, 1 racing, 2 outro, 3 finished);
    /// then the countdown's seconds to Go, or the race clock in seconds
    /// while racing, or the outro's seconds left, or 0 once finished; then
    /// the seconds left of the cap after the first human's finish, or -1
    /// before it has started and once the race has ended.
    pub fn phase(&self) -> Vec<f32> {
        phase_floats(self.state().phase()).to_vec()
    }

    /// `sim::State::hash` of the race as of the last step: everything that
    /// influences future ticks, bit-identical across platforms. A recording
    /// keeps it at every `sim::tuning::REPLAY_HASH_INTERVAL_TICKS`th tick.
    pub fn state_hash(&self) -> u64 {
        self.state().hash()
    }

    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("state is only absent during a step")
    }
}

impl Race {
    /// One standing per kart in grid order: what `standings` carries to
    /// JavaScript.
    pub fn standings(&self) -> Vec<Standing> {
        self.state().standings()
    }

    /// Every kart's readout in grid order, with the surface under it: what
    /// `kart_transforms` and `kart_readouts` carry to JavaScript.
    pub fn kart_readouts(&self) -> Vec<(KartReadout, Surface)> {
        let state = self.state();
        state
            .kart_readouts()
            .into_iter()
            .enumerate()
            .map(|(slot, kart)| (kart, state.kart_surface(slot)))
            .collect()
    }
}

/// The player's own kart in a room's race, predicted here from the inputs
/// the client sends and reconciled with the snapshots it receives, and the
/// other karts interpolated between those snapshots, which the prediction
/// meets as kinematic bodies (ADR 0009). `sim::Prediction` and
/// `sim::Interpolation` document the rules; this only carries the
/// protocol's messages to them.
#[wasm_bindgen]
pub struct Prediction {
    inner: sim::Prediction,
    interpolation: sim::Interpolation,
}

#[wasm_bindgen]
impl Prediction {
    /// The kart on grid slot `kart` of the race from `seed` on the track
    /// described by `sidecar`, at the top of its countdown, its bot at
    /// `bot_level` (spelled as the wire does) once it has finished, with
    /// the Tyres pick `tyres`, over `distance`. Fails when the sidecar is
    /// unusable, its grid has no such slot or the level or a pick is unknown.
    #[wasm_bindgen(constructor)]
    pub fn new(
        seed: u64,
        kart: usize,
        sidecar: &str,
        bot_level: &str,
        tyres: &str,
        distance: &str,
    ) -> Result<Prediction, String> {
        let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
        let bot_level = BotLevel::from_name(bot_level)
            .ok_or_else(|| format!("no bot level called {bot_level:?}"))?;
        let tyres = tyres_named(tyres)?;
        let distance = distance_named(distance)?;
        if kart >= track.spawn_grid().len() {
            return Err(format!(
                "kart {kart} but {} has {} spawn slots",
                track.name(),
                track.spawn_grid().len()
            ));
        }
        Ok(Self {
            inner: sim::Prediction::with_tyres(seed, kart, track, bot_level, tyres)
                .with_distance(distance),
            interpolation: sim::Interpolation::new(),
        })
    }

    /// Predict one tick from `input`, a value of the generated `Input` type
    /// as the client sends it this tick, after running the interpolation a
    /// tick on, against the other karts where it now draws them. Fails,
    /// with the reason, when the value is not one.
    pub fn tick(&mut self, input: JsValue) -> Result<(), String> {
        let input: Input =
            serde_wasm_bindgen::from_value(input).map_err(|error| error.to_string())?;
        self.interpolation.advance();
        self.inner.tick(
            input.sequence,
            KartInput::from(input),
            &self.interpolation.kart_readouts(),
        );
        Ok(())
    }

    /// Reconcile with `snapshot`, a value of the generated `Snapshot` type,
    /// in which this kart is `kart`, and hand it to the interpolation.
    /// Returns how far the predicted kart moved for it, in metres. Fails,
    /// with the reason, when the value is not a snapshot or has no such
    /// kart.
    pub fn reconcile(&mut self, snapshot: JsValue, kart: usize) -> Result<f32, String> {
        let snapshot: Snapshot =
            serde_wasm_bindgen::from_value(snapshot).map_err(|error| error.to_string())?;
        let (Some(own), Some(&acknowledged)) =
            (snapshot.karts.get(kart), snapshot.acknowledged.get(kart))
        else {
            return Err(format!("kart {kart} is not in the snapshot"));
        };
        self.interpolation.push(
            snapshot.tick,
            snapshot.karts.iter().map(KartReadout::from).collect(),
            snapshot.projectiles.iter().map(Projectile::from).collect(),
        );
        Ok(self
            .inner
            .reconcile(snapshot.tick, acknowledged, &KartReadout::from(own)))
    }

    /// The kart as it is drawn, in the floats `Race::kart_transforms`
    /// gives each kart.
    pub fn kart_transform(&self) -> Vec<f32> {
        transform_floats(&[self.inner.kart_readout()])
    }

    /// The kart as it is drawn, as the pair `Race::kart_readouts` gives
    /// each kart.
    #[wasm_bindgen(js_name = kart_readout)]
    pub fn kart_readout_js(&self) -> Result<JsValue, String> {
        to_js(&self.kart_readout())
    }

    /// Every kart where the interpolation draws it, in grid order, in the
    /// floats `Race::kart_transforms` gives each; empty before the first
    /// snapshot. This kart's entry is the snapshots', not the prediction's.
    pub fn interpolated_kart_transforms(&self) -> Vec<f32> {
        transform_floats(&self.interpolation.kart_readouts())
    }

    /// Every kart where the interpolation draws it, as the pairs
    /// `Race::kart_readouts` gives; empty before the first snapshot.
    #[wasm_bindgen(js_name = interpolated_kart_readouts)]
    pub fn interpolated_kart_readouts_js(&self) -> Result<JsValue, String> {
        to_js(&self.interpolated_kart_readouts())
    }

    /// Every projectile in flight where the interpolation draws it, in
    /// the five floats `Race::projectiles` gives each; empty before the
    /// first snapshot.
    pub fn interpolated_projectiles(&self) -> Vec<f32> {
        projectile_floats(&self.interpolation.projectiles())
    }

    /// The runaway wheel at each of the track's crossings at the predicted
    /// kart's tick, in the floats `Race::wheels` gives: the wheel that hits
    /// the predicted kart.
    pub fn wheels(&self) -> Vec<f32> {
        wheel_floats(self.inner.track(), &self.inner.wheels())
    }

    /// The server's race has ended: the prediction runs its outro from now
    /// on, as `sim::Prediction::end_race` documents.
    pub fn end_race(&mut self) {
        self.inner.end_race();
    }

    /// The race clock at the predicted kart's tick, in the three floats
    /// `Race::phase` gives; never finished, since the server's results end
    /// the race.
    pub fn phase(&self) -> Vec<f32> {
        phase_floats(self.inner.phase()).to_vec()
    }

    /// Snapshots that moved the predicted kart further than rounding, over
    /// the seconds raced so far.
    pub fn corrections_per_second(&self) -> f32 {
        self.inner.corrections_per_second()
    }
}

impl Prediction {
    /// The kart as it is drawn, with the surface under it.
    fn kart_readout(&self) -> (KartReadout, Surface) {
        (self.inner.kart_readout(), self.inner.kart_surface())
    }

    /// Every kart where the interpolation draws it, with the surface under
    /// it on the track.
    fn interpolated_kart_readouts(&self) -> Vec<(KartReadout, Surface)> {
        self.interpolation
            .kart_readouts()
            .into_iter()
            .map(|kart| (kart, self.inner.surface_at(kart.position)))
            .collect()
    }
}

/// The transforms of `karts`, `FLOATS_PER_TRANSFORM` floats each.
fn transform_floats(karts: &[KartReadout]) -> Vec<f32> {
    karts
        .iter()
        .flat_map(|kart| kart.position.into_iter().chain(kart.rotation))
        .collect()
}

/// The five floats per projectile `Race::projectiles` documents.
fn projectile_floats(projectiles: &[Projectile]) -> Vec<f32> {
    projectiles
        .iter()
        .flat_map(|projectile| {
            let [x, y, z] = projectile.position;
            [x, y, z, projectile.owner as f32, projectile.target as f32]
        })
        .collect()
}

/// Eight floats per crossing of `track`, in its order: 0 while the wheel
/// is quiet, 1 while the flag is up before a pass, 2 while it rolls; then,
/// while the flag is up, the foot of the wall it will roll in from x, y, z
/// and the yaw it will roll toward, then zeros; while it rolls, its centre
/// x, y, z, the yaw it rolls toward (radians
/// about +Y, zero facing -Z), how far along the pass's path it is and how
/// far through the leg under way, wall to wall, each 0 to 1, and the
/// crossing's own yaw, along which its walls run; else zeros.
fn wheel_floats(track: &Track, wheels: &[Wheel]) -> Vec<f32> {
    wheels
        .iter()
        .zip(track.wheel_crossings())
        .flat_map(|(wheel, crossing)| match *wheel {
            Wheel::Quiet => [0.0; 8],
            Wheel::Warning {
                from: [x, y, z],
                yaw,
                ..
            } => [1.0, x, y, z, yaw, 0.0, 0.0, 0.0],
            Wheel::Rolling {
                position: [x, y, z],
                yaw,
                fraction,
                leg_fraction,
                ..
            } => [2.0, x, y, z, yaw, fraction, leg_fraction, crossing.yaw],
        })
        .collect()
}

/// The three floats `Race::phase` documents.
fn phase_floats(phase: Phase) -> [f32; 3] {
    let (seconds, cap_left_s) = match phase {
        Phase::Countdown { remaining_s } | Phase::Outro { remaining_s } => (remaining_s, NO_TIME),
        Phase::Racing {
            elapsed_s,
            cap_left_s,
        } => (elapsed_s, cap_left_s.unwrap_or(NO_TIME)),
        Phase::Finished => (0.0, NO_TIME),
    };
    [f32::from(phase.code()), seconds, cap_left_s]
}

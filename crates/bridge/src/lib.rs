//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and [`Prediction`]: create a race on a
//! track, step it with one flat buffer of inputs, read one flat buffer of
//! kart readouts; or predict the player's own kart in a room's race from the
//! inputs it sends and the snapshots it receives. Flat buffers cross the
//! wasm boundary once per tick instead of once per kart per field.

use protocol::{ClientMessage, Input, KartSnapshot, ServerMessage, Snapshot};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sim::{
    Drift, DriftDirection, Item, KartInput, KartTransform, Phase, Projectile, Replay, SnapshotKart,
    State, Surface, Track,
};
use wasm_bindgen::prelude::*;

/// Floats per kart in the input buffer: steer, throttle, brake, hop, use
/// item, hold item. A button is pressed when its float is not zero.
const FLOATS_PER_INPUT: usize = 6;

/// Written in the standings and phase buffers where a kart has no best lap
/// or finish time yet, and where the grace has not started; a time is never
/// negative.
const NO_TIME: f32 = -1.0;

/// Fixed sim rate, so the client's tick loop reads the tuned value instead of
/// repeating it (ADR 0014).
#[wasm_bindgen]
pub fn ticks_per_second() -> u32 {
    sim::tuning::TICKS_PER_SECOND
}

/// Laps in a race, so the Instrument panel's lap counter reads the tuned
/// value (ADR 0014).
#[wasm_bindgen]
pub fn laps_per_race() -> u32 {
    sim::tuning::LAPS_PER_RACE
}

/// Karts on the grid; the slots the humans leave empty are filled by bots
/// at Start (ADR 0014).
#[wasm_bindgen]
pub fn karts_per_race() -> usize {
    sim::tuning::KARTS_PER_RACE
}

/// The kart's width, height and length in metres, so the View draws the box
/// the sim collides with (ADR 0014).
#[wasm_bindgen]
pub fn kart_size_m() -> Vec<f32> {
    use sim::tuning::{KART_HALF_HEIGHT_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M};
    vec![
        2.0 * KART_HALF_WIDTH_M,
        2.0 * KART_HALF_HEIGHT_M,
        2.0 * KART_HALF_LENGTH_M,
    ]
}

/// How often a recording keeps the state hash, so the browser's recorder
/// and the fixtures agree on the gate's resolution (ADR 0014).
#[wasm_bindgen]
pub fn replay_hash_interval_ticks() -> u32 {
    sim::tuning::REPLAY_HASH_INTERVAL_TICKS
}

/// Seconds the karts are held on the grid before Go (ADR 0014), so a
/// client racing from snapshots reads the count from the server's tick.
#[wasm_bindgen]
pub fn countdown_s() -> f32 {
    sim::tuning::COUNTDOWN_S
}

/// How long a client shows a fire it predicted from a press before it
/// takes the server not to have fired (ADR 0014).
#[wasm_bindgen]
pub fn predicted_fire_timeout_s() -> f32 {
    sim::tuning::PREDICTED_FIRE_TIMEOUT_S
}

/// Seconds a picked-up item box is gone before it is back (ADR 0014), so a
/// client racing from the server's events hides a box for as long as the
/// server does.
#[wasm_bindgen]
pub fn item_box_respawn_s() -> f32 {
    sim::tuning::ITEM_BOX_RESPAWN_S
}

/// Metres behind a kart a trailed hazard rides (ADR 0014), so a client
/// draws a hazard a snapshot says is held where the server keeps it.
#[wasm_bindgen]
pub fn hazard_drop_distance_m() -> f32 {
    sim::tuning::HAZARD_DROP_DISTANCE_M
}

/// Seconds a spin-out lasts (ADR 0014), so the View's spin animation runs
/// once over it.
#[wasm_bindgen]
pub fn spin_out_s() -> f32 {
    sim::tuning::SPIN_OUT_S
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

/// How long the client gives WebTransport before falling back to WebSocket
/// (ADR 0011), read from the tuned value (ADR 0014).
#[wasm_bindgen]
pub fn webtransport_connect_timeout_s() -> f32 {
    sim::tuning::WEBTRANSPORT_CONNECT_TIMEOUT_S
}

/// How long a Ping waits for its Pong (ADR 0014).
#[wasm_bindgen]
pub fn ping_timeout_s() -> f32 {
    sim::tuning::PING_TIMEOUT_S
}

/// How often an unanswered Ping is sent again (ADR 0014).
#[wasm_bindgen]
pub fn ping_resend_s() -> f32 {
    sim::tuning::PING_RESEND_S
}

/// Silence from the server mid-race before the client shows the connection
/// banner and reconnects (ADR 0012).
#[wasm_bindgen]
pub fn connection_silence_s() -> f32 {
    sim::tuning::CONNECTION_SILENCE_S
}

/// How long a failed reconnect waits before the next try (ADR 0014).
#[wasm_bindgen]
pub fn reconnect_retry_s() -> f32 {
    sim::tuning::RECONNECT_RETRY_S
}

/// The fewest characters a nickname may have (ADR 0014); the Title Scene
/// validates against it.
#[wasm_bindgen]
pub fn nickname_min_chars() -> usize {
    sim::tuning::NICKNAME_MIN_CHARS
}

/// The most characters a nickname may have (ADR 0014).
#[wasm_bindgen]
pub fn nickname_max_chars() -> usize {
    sim::tuning::NICKNAME_MAX_CHARS
}

/// Letters in a room code (ADR 0012), so the Title Scene's code field is
/// sized to it.
#[wasm_bindgen]
pub fn room_code_length() -> usize {
    sim::tuning::ROOM_CODE_LENGTH
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

/// One race's sim, owned by the browser for prediction of the local kart.
#[wasm_bindgen]
pub struct Race {
    /// `sim::step` consumes and returns the state, so it is parked in an
    /// `Option` for the duration of a step. It is `None` only inside `step`.
    state: Option<State>,
}

#[wasm_bindgen]
impl Race {
    /// A fresh race from `seed` on the track described by `sidecar` (the JSON
    /// exported next to the track's glTF) with `human_count` karts on the
    /// front of its spawn grid and `bot_count` bot-driven karts on the slots
    /// behind them. Fails when the sidecar is unusable or has fewer spawn
    /// slots than the karts; the reason is a plain string so the same code
    /// runs in native tests.
    #[wasm_bindgen(constructor)]
    pub fn new(
        seed: u64,
        human_count: usize,
        bot_count: usize,
        sidecar: &str,
    ) -> Result<Race, String> {
        let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
        let kart_count = human_count + bot_count;
        if kart_count > track.spawn_grid().len() {
            return Err(format!(
                "{kart_count} karts but {} has {} spawn slots",
                track.name(),
                track.spawn_grid().len()
            ));
        }
        Ok(Self {
            state: Some(State::with_bots(seed, human_count, bot_count, track)),
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

    /// Fourteen floats per kart in grid order: position x, y, z, rotation
    /// quaternion x, y, z, w, speed in metres per second along the kart's
    /// facing (negative backing up), the surface under the kart as
    /// `sim::Surface::code` (0 asphalt, 1 sand, 2 grass, 3 wall, 4 out of
    /// world), the drift direction (-1 left, 1 right, 0 not drifting), the
    /// mini-turbo tier the drift has charged (0 none, 1 blue, 2 orange),
    /// the seconds of drift charge, the item in the kart's slot as
    /// `sim::Item::code` (0 empty, 1 boost, 2 hazard, 3 homing) and the
    /// seconds of spin-out left, 0 when the kart is not spun out. Y is up;
    /// identity rotation faces -Z.
    pub fn kart_transforms(&self) -> Vec<f32> {
        let state = self.state();
        state
            .kart_transforms()
            .iter()
            .enumerate()
            .flat_map(|(slot, kart)| transform_floats(kart, state.kart_surface(slot)))
            .collect()
    }

    /// Eight floats per kart in grid order: race position from 1, the lap
    /// the kart is on from 1 (staying on the last once the race's laps are
    /// done), the last checkpoint crossed in order (0 is the start line),
    /// spline progress in metres along the centreline, seconds into the
    /// current lap, the best lap in seconds or -1 before a lap is
    /// completed, 1 while the wrong-way warning is up, else 0, and the race
    /// clock at the kart's finish or -1 while it is still racing.
    /// `sim::State::standings` documents the ordering.
    pub fn standings(&self) -> Vec<f32> {
        self.state()
            .standings()
            .iter()
            .flat_map(|standing| {
                [
                    standing.position as f32,
                    standing.lap() as f32,
                    standing.checkpoint as f32,
                    standing.spline_progress,
                    standing.lap_time_s,
                    standing.best_lap_s.unwrap_or(NO_TIME),
                    f32::from(u8::from(standing.wrong_way)),
                    standing.finish_time_s.unwrap_or(NO_TIME),
                ]
            })
            .collect()
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
        projectile_floats(self.state().projectiles())
    }

    /// Three floats for where the race is (`sim::Phase`): the phase as
    /// `sim::Phase::code` (0 countdown, 1 racing, 2 finished); then the
    /// countdown's seconds to Go, or the race clock in seconds while racing,
    /// or 0 once finished; then the seconds left of the grace after the
    /// first finisher, or -1 before anyone has finished and once the race
    /// is over.
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
    /// described by `sidecar`, at the top of its countdown. Fails when the
    /// sidecar is unusable or its grid has no such slot.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, kart: usize, sidecar: &str) -> Result<Prediction, String> {
        let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
        if kart >= track.spawn_grid().len() {
            return Err(format!(
                "kart {kart} but {} has {} spawn slots",
                track.name(),
                track.spawn_grid().len()
            ));
        }
        Ok(Self {
            inner: sim::Prediction::new(seed, kart, track),
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
            KartInput {
                steer: input.steer_unit(),
                throttle: input.throttle_unit(),
                brake: input.brake_unit(),
                hop: input.hop,
                use_item: input.use_item,
                hold_item: input.hold_item,
            },
            &self.interpolation.kart_transforms(),
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
            snapshot.karts.iter().map(snapshot_kart).collect(),
            snapshot
                .projectiles
                .iter()
                .map(|projectile| Projectile {
                    position: projectile.position,
                    owner: usize::from(projectile.owner),
                    target: usize::from(projectile.target),
                })
                .collect(),
        );
        Ok(self
            .inner
            .reconcile(snapshot.tick, acknowledged, &snapshot_kart(own)))
    }

    /// The kart as it is drawn, in the fourteen floats `Race::kart_transforms`
    /// gives each kart.
    pub fn kart_transform(&self) -> Vec<f32> {
        transform_floats(&self.inner.kart_transform(), self.inner.kart_surface()).to_vec()
    }

    /// Every kart where the interpolation draws it, in grid order, in the
    /// fourteen floats `Race::kart_transforms` gives each, with the surface
    /// under it; empty before the first snapshot. This kart's entry is the
    /// snapshots', not the prediction's.
    pub fn interpolated_kart_transforms(&self) -> Vec<f32> {
        self.interpolation
            .kart_transforms()
            .iter()
            .flat_map(|kart| transform_floats(kart, self.inner.surface_at(kart.position)))
            .collect()
    }

    /// Every projectile in flight where the interpolation draws it, in
    /// the five floats `Race::projectiles` gives each; empty before the
    /// first snapshot.
    pub fn interpolated_projectiles(&self) -> Vec<f32> {
        projectile_floats(&self.interpolation.projectiles())
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

/// One kart's fourteen floats, as `Race::kart_transforms` documents them.
fn transform_floats(kart: &KartTransform, surface: Surface) -> [f32; 14] {
    let (direction, tier, charge_s) = match kart.drift {
        Some(drift) => (
            drift.direction.steer_sign(),
            f32::from(drift.mini_turbo().code()),
            drift.charge_s,
        ),
        None => (0.0, 0.0, 0.0),
    };
    let [x, y, z] = kart.position;
    let [qx, qy, qz, qw] = kart.rotation;
    [
        x,
        y,
        z,
        qx,
        qy,
        qz,
        qw,
        kart.speed,
        f32::from(surface.code()),
        direction,
        tier,
        charge_s,
        f32::from(kart.held_item.map_or(0, Item::code)),
        kart.spin_out_s.unwrap_or(0.0),
    ]
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

/// The three floats `Race::phase` documents.
fn phase_floats(phase: Phase) -> [f32; 3] {
    let (seconds, grace_left_s) = match phase {
        Phase::Countdown { remaining_s } => (remaining_s, NO_TIME),
        Phase::Racing {
            elapsed_s,
            grace_left_s,
        } => (elapsed_s, grace_left_s.unwrap_or(NO_TIME)),
        Phase::Finished => (0.0, NO_TIME),
    };
    [f32::from(phase.code()), seconds, grace_left_s]
}

/// The sim's item for the protocol's.
fn item(item: protocol::Item) -> Item {
    match item {
        protocol::Item::Boost => Item::Boost,
        protocol::Item::Hazard => Item::Hazard,
        protocol::Item::Homing => Item::Homing,
    }
}

/// What a protocol snapshot says of one kart, in the sim's terms.
fn snapshot_kart(kart: &KartSnapshot) -> SnapshotKart {
    SnapshotKart {
        position: kart.position,
        rotation: kart.rotation,
        velocity: kart.velocity,
        drift: kart.drift.map(|drift| Drift {
            direction: match drift.direction {
                protocol::DriftDirection::Left => DriftDirection::Left,
                protocol::DriftDirection::Right => DriftDirection::Right,
            },
            charge_s: drift.charge_s,
        }),
        held_item: kart.held_item.map(item),
        spin_out_s: kart.spin_out_s,
        lap: u32::from(kart.lap),
        checkpoint: usize::from(kart.checkpoint),
        spline_progress: kart.spline_progress,
    }
}

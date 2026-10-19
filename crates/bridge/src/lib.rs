//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and nothing else: create a race on a track,
//! step it with one flat buffer of inputs, read one flat buffer of kart
//! readouts. Flat buffers cross the wasm boundary once per tick instead of
//! once per kart per field.

use protocol::{ClientMessage, ServerMessage};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sim::{KartInput, Phase, Replay, State, Track};
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

    /// Twelve floats per kart in grid order: position x, y, z, rotation
    /// quaternion x, y, z, w, speed in metres per second along the kart's
    /// facing (negative backing up), the surface under the kart as
    /// `sim::Surface::code` (0 asphalt, 1 sand, 2 grass, 3 wall, 4 out of
    /// world), the drift direction (-1 left, 1 right, 0 not drifting), the
    /// mini-turbo tier the drift has charged (0 none, 1 blue, 2 orange) and
    /// the seconds of drift charge. Y is up; identity rotation faces -Z.
    pub fn kart_transforms(&self) -> Vec<f32> {
        let state = self.state();
        state
            .kart_transforms()
            .iter()
            .enumerate()
            .flat_map(|(slot, kart)| {
                let (direction, tier, charge_s) = match kart.drift {
                    Some(drift) => (
                        drift.direction.steer_sign(),
                        f32::from(drift.mini_turbo().code()),
                        drift.charge_s,
                    ),
                    None => (0.0, 0.0, 0.0),
                };
                kart.position.into_iter().chain(kart.rotation).chain([
                    kart.speed,
                    f32::from(state.kart_surface(slot).code()),
                    direction,
                    tier,
                    charge_s,
                ])
            })
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

    /// Three floats for where the race is (`sim::Phase`): the phase as
    /// `sim::Phase::code` (0 countdown, 1 racing, 2 finished); then the
    /// countdown's seconds to Go, or the race clock in seconds while racing,
    /// or 0 once finished; then the seconds left of the grace after the
    /// first finisher, or -1 before anyone has finished and once the race
    /// is over.
    pub fn phase(&self) -> Vec<f32> {
        let phase = self.state().phase();
        let (seconds, grace_left_s) = match phase {
            Phase::Countdown { remaining_s } => (remaining_s, NO_TIME),
            Phase::Racing {
                elapsed_s,
                grace_left_s,
            } => (elapsed_s, grace_left_s.unwrap_or(NO_TIME)),
            Phase::Finished => (0.0, NO_TIME),
        };
        vec![f32::from(phase.code()), seconds, grace_left_s]
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

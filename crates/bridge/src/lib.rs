//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and nothing else: create a race on a track,
//! step it with one flat buffer of inputs, read one flat buffer of kart
//! readouts. Flat buffers cross the wasm boundary once per tick instead of
//! once per kart per field.

use protocol::{ClientMessage, ServerMessage};
use sim::{KartInput, Replay, State, Track};
use wasm_bindgen::prelude::*;

/// Floats per kart in the input buffer: steer, throttle, brake, hop, use
/// item, hold item. A button is pressed when its float is not zero.
const FLOATS_PER_INPUT: usize = 6;

/// Written in the standings buffer where a kart has no best lap yet; a lap
/// time is never negative.
const NO_BEST_LAP: f32 = -1.0;

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

/// The bytes of a `protocol::ClientMessage::Ping` carrying `nonce`; the
/// server answers with a Pong carrying the same nonce whichever transport
/// delivers it (ADR 0011).
#[wasm_bindgen]
pub fn encode_ping(nonce: u32) -> Vec<u8> {
    protocol::encode(&ClientMessage::Ping { nonce })
}

/// The nonce of the `protocol::ServerMessage::Pong` that `bytes` carry.
/// Fails, with the reason, when they carry anything else.
#[wasm_bindgen]
pub fn decode_pong(bytes: &[u8]) -> Result<u32, String> {
    match protocol::decode::<ServerMessage>(bytes) {
        Ok(ServerMessage::Pong { nonce }) => Ok(nonce),
        Err(error) => Err(error.to_string()),
    }
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
    /// exported next to the track's glTF) with `kart_count` karts on its
    /// spawn grid. Fails when the sidecar is unusable or has fewer spawn slots
    /// than `kart_count`; the reason is a plain string so the same code runs
    /// in native tests.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, kart_count: usize, sidecar: &str) -> Result<Race, String> {
        let track = Track::from_sidecar(sidecar).map_err(|error| error.to_string())?;
        if kart_count > track.spawn_grid().len() {
            return Err(format!(
                "{kart_count} karts but {} has {} spawn slots",
                track.name(),
                track.spawn_grid().len()
            ));
        }
        Ok(Self {
            state: Some(State::new(seed, kart_count, track)),
        })
    }

    /// Advance one tick of `dt` seconds. `inputs` holds six floats per kart
    /// in grid order: steer, throttle, brake, then the hop, use item and hold
    /// item buttons as 0 or 1. Its length must be six times the kart count.
    pub fn step(&mut self, inputs: &[f32], dt: f32) {
        // Checked before the state is taken so a bad call leaves the race usable.
        assert_eq!(
            inputs.len(),
            self.state().kart_transforms().len() * FLOATS_PER_INPUT,
            "one input per kart"
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

    /// Six floats per kart in grid order: race position from 1, the lap the
    /// kart is on from 1 (staying on the last once the race's laps are
    /// done), the last checkpoint crossed in order (0 is the start line),
    /// spline progress in metres along the centreline, seconds into the
    /// current lap, and the best lap in seconds or -1 before a lap is
    /// completed. `sim::State::standings` documents the ordering.
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
                    standing.best_lap_s.unwrap_or(NO_BEST_LAP),
                ]
            })
            .collect()
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

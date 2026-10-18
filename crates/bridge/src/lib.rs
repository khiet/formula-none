//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and nothing else: create a race on a track,
//! step it with one flat buffer of inputs, read one flat buffer of kart
//! readouts. Flat buffers cross the wasm boundary once per tick instead of
//! once per kart per field.

use sim::{KartInput, Replay, State, Track};
use wasm_bindgen::prelude::*;

/// Floats per kart in the input buffer: steer, throttle, brake.
const FLOATS_PER_INPUT: usize = 3;

/// Fixed sim rate, so the client's tick loop reads the tuned value instead of
/// repeating it (ADR 0014).
#[wasm_bindgen]
pub fn ticks_per_second() -> u32 {
    sim::tuning::TICKS_PER_SECOND
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

    /// Advance one tick of `dt` seconds. `inputs` holds three floats per kart
    /// in grid order: steer, throttle, brake. Its length must be three times
    /// the kart count.
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
            .map(|&[steer, throttle, brake]| KartInput {
                steer,
                throttle,
                brake,
            })
            .collect();
        let state = self
            .state
            .take()
            .expect("state is only absent during a step");
        self.state = Some(sim::step(state, &inputs, dt));
    }

    /// Nine floats per kart in grid order: position x, y, z, rotation
    /// quaternion x, y, z, w, speed in metres per second, then the surface
    /// under the kart as `sim::Surface::code` (0 asphalt, 1 sand, 2 grass,
    /// 3 wall, 4 out of world). Y is up; identity rotation faces -Z.
    pub fn kart_transforms(&self) -> Vec<f32> {
        let state = self.state();
        state
            .kart_transforms()
            .iter()
            .enumerate()
            .flat_map(|(slot, kart)| {
                kart.position
                    .into_iter()
                    .chain(kart.rotation)
                    .chain([kart.speed, f32::from(state.kart_surface(slot).code())])
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

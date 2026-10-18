//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Race`] and nothing else: create a race, step it with
//! one flat buffer of inputs, read one flat buffer of transforms. Flat buffers
//! cross the wasm boundary once per tick instead of once per kart per field.

use sim::{KartInput, State};
use wasm_bindgen::prelude::*;

/// Floats per kart in the input buffer: steer, throttle, brake.
const FLOATS_PER_INPUT: usize = 3;

/// Fixed sim rate, so the client's tick loop reads the tuned value instead of
/// repeating it (ADR 0014).
#[wasm_bindgen]
pub fn ticks_per_second() -> u32 {
    sim::tuning::TICKS_PER_SECOND
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
    /// A fresh race from `seed` with `kart_count` karts on the grid.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, kart_count: usize) -> Self {
        Self {
            state: Some(State::new(seed, kart_count)),
        }
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

    /// Eight floats per kart in grid order: position x, y, z, rotation
    /// quaternion x, y, z, w, then speed in metres per second. Y is up;
    /// identity rotation faces -Z.
    pub fn kart_transforms(&self) -> Vec<f32> {
        self.state()
            .kart_transforms()
            .iter()
            .flat_map(|kart| {
                kart.position
                    .into_iter()
                    .chain(kart.rotation)
                    .chain([kart.speed])
            })
            .collect()
    }

    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("state is only absent during a step")
    }
}

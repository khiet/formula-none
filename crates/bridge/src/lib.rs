//! Thin WebAssembly surface over `sim` and `protocol`; holds no game rules.
//!
//! The web app talks to [`Sim`] and nothing else: create a race, step it with
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
pub struct Sim {
    /// `sim::step` consumes and returns the state, so it is parked in an
    /// `Option` for the duration of a step. It is `None` only inside `step`.
    state: Option<State>,
}

#[wasm_bindgen]
impl Sim {
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
        let inputs: Vec<KartInput> = inputs
            .chunks_exact(FLOATS_PER_INPUT)
            .map(|kart| KartInput {
                steer: kart[0],
                throttle: kart[1],
                brake: kart[2],
            })
            .collect();
        let state = self
            .state
            .take()
            .expect("state is only absent during a step");
        self.state = Some(sim::step(state, &inputs, dt));
    }

    /// Seven floats per kart in grid order: position x, y, z then rotation
    /// quaternion x, y, z, w. Y is up; identity rotation faces -Z.
    pub fn kart_transforms(&self) -> Vec<f32> {
        self.state()
            .kart_transforms()
            .iter()
            .flat_map(|kart| kart.position.into_iter().chain(kart.rotation))
            .collect()
    }

    fn state(&self) -> &State {
        self.state
            .as_ref()
            .expect("state is only absent during a step")
    }
}

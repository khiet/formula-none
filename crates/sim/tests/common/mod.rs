//! What the sim's tests share: the tracks, the countdown, the clock, the
//! held inputs and drivers, and edits to a track's sidecar JSON.
//! Each test binary compiles this module and uses only some of it.
#![allow(dead_code)]

use sim::tuning::{BOT_LOOKAHEAD_M, KART_BUMP_DECAY_S, TICKS_PER_SECOND};
use sim::{step, KartInput, KartReadout, Phase, State, Track};

pub const OVAL: &str = include_str!("../../../../assets/tracks/oval/oval.track.json");
pub const BELL_DEER: &str =
    include_str!("../../../../assets/tracks/bell-deer/bell-deer.track.json");
pub const GOLDSTONE: &str =
    include_str!("../../../../assets/tracks/goldstone/goldstone.track.json");
pub const MONARCH: &str = include_str!("../../../../assets/tracks/monarch/monarch.track.json");
/// A figure-eight whose legs cross at the origin, one over the other:
/// the leg through the crossing first in the lap rides a bridge 5 m high,
/// the other is on the ground. Flat asphalt 180 m by 70 m lies under all of
/// it, with nothing beyond. The start line is the east tip, with the lap
/// running -Z from it; the only other checkpoint is the west tip.
pub const FIGURE_EIGHT: &str = include_str!("../figure-eight.track.json");
pub const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
pub const SEED: u64 = 0x5EED;

pub fn oval() -> Track {
    Track::from_sidecar(OVAL).expect("the oval fixture loads")
}

pub fn bell_deer() -> Track {
    Track::from_sidecar(BELL_DEER).expect("Bell Deer loads")
}

pub fn goldstone() -> Track {
    Track::from_sidecar(GOLDSTONE).expect("Goldstone loads")
}

pub fn monarch() -> Track {
    Track::from_sidecar(MONARCH).expect("Monarch loads")
}

pub fn figure_eight() -> Track {
    Track::from_sidecar(FIGURE_EIGHT).expect("the figure-eight fixture loads")
}

/// The track `sidecar` describes, as one of the edits below leaves it.
pub fn track(sidecar: &str) -> Track {
    Track::from_sidecar(sidecar).expect("the edited sidecar loads")
}

/// The race past its countdown, every kart idle on the grid until Go.
pub fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

pub fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

/// Seconds for a bump to decay to nothing a kart could show: five of its
/// decay times leave under a percent of it.
pub const BUMP_SPENT_S: f32 = 5.0 * KART_BUMP_DECAY_S;

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
pub fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// Full throttle, steering toward the point `target` (x, z) seen from above.
pub fn steer_toward(state: &State, kart: usize, target: [f32; 2]) -> KartInput {
    const STEER_GAIN: f32 = 4.0;
    let readout = state.kart_readouts()[kart];
    let (dx, dz) = (
        target[0] - readout.position[0],
        target[1] - readout.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw(readout.rotation).sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    KartInput {
        steer: (rightward * STEER_GAIN).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// Full throttle toward a line `right_of_centre_m` right of the centreline
/// (negative is left), aimed some way ahead of the kart.
pub fn follow_line(state: &State, kart: usize, right_of_centre_m: f32) -> KartInput {
    let progress = state.standings()[kart].spline_progress;
    let track = state.track();
    let ahead = track.point_at(progress + BOT_LOOKAHEAD_M);
    let further = track.point_at(progress + BOT_LOOKAHEAD_M + 1.0);
    // Right of the direction of travel (dx, dz), seen from above, is (-dz, dx).
    let (tx, tz) = (further[0] - ahead[0], further[2] - ahead[2]);
    let travel = (tx * tx + tz * tz).sqrt().max(f32::EPSILON);
    let target = [
        ahead[0] - tz / travel * right_of_centre_m,
        ahead[2] + tx / travel * right_of_centre_m,
    ];
    steer_toward(state, kart, target)
}

/// A line a little right of the centreline: enough to lap either track on
/// the asphalt and clear of the tyre walls, which stand left of the
/// centreline on both: in the oval's turns, and on Bell Deer inside the
/// hairpin and Spoon and outside turn 2.
pub fn follow_centreline(state: &State, kart: usize) -> KartInput {
    const RIGHT_OF_CENTRE_M: f32 = 2.5;
    follow_line(state, kart, RIGHT_OF_CENTRE_M)
}

fn edited(sidecar: &str, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut sidecar: serde_json::Value = serde_json::from_str(sidecar).expect("valid JSON");
    edit(&mut sidecar);
    sidecar.to_string()
}

/// `sidecar` with its walls taken away, the tyre walls kept. The walls keep a
/// kart off the grass and in the world, and these are the rules for a kart
/// that got there anyway, whatever track it was on.
pub fn without_walls(sidecar: &str) -> String {
    edited(sidecar, |sidecar| {
        let meshes = sidecar["surfaces"].as_array().expect("surfaces").clone();
        sidecar["surfaces"] = meshes
            .into_iter()
            .filter(|mesh| !mesh["name"].as_str().expect("a name").starts_with("Wall"))
            .collect();
    })
}

/// `sidecar` with spawn slot `slot` moved to `position` facing `yaw`, so a
/// kart can be started anywhere on or beside the track.
pub fn with_slot_at(sidecar: &str, slot: usize, position: [f32; 3], yaw: f32) -> String {
    edited(sidecar, |sidecar| {
        sidecar["spawn_grid"][slot] = serde_json::json!({ "position": position, "yaw": yaw });
    })
}

/// `sidecar` with the surface mesh `name` slid along the ground, unturned,
/// so its footprint is centred on `centre` (x, z). No track stands a tyre
/// wall against a wall, so a test that needs one moves it there.
pub fn with_mesh_centred_on(sidecar: &str, name: &str, centre: [f32; 2]) -> String {
    edited(sidecar, |sidecar| {
        let mesh = sidecar["surfaces"]
            .as_array_mut()
            .expect("surfaces")
            .iter_mut()
            .find(|mesh| mesh["name"] == name)
            .expect("the named mesh");
        let positions = mesh["positions"].as_array_mut().expect("positions");
        let coordinate = |value: &serde_json::Value| value.as_f64().expect("a coordinate");
        let count = (positions.len() / 3) as f64;
        let mean = |axis: usize| {
            positions[axis..]
                .iter()
                .step_by(3)
                .map(coordinate)
                .sum::<f64>()
                / count
        };
        let shift = [
            f64::from(centre[0]) - mean(0),
            0.0,
            f64::from(centre[1]) - mean(2),
        ];
        for (index, value) in positions.iter_mut().enumerate() {
            *value = serde_json::json!(coordinate(value) + shift[index % 3]);
        }
    })
}

/// `sidecar` with no runaway wheel, so a test driving past its crossing
/// meets nothing it did not ask for.
pub fn without_wheel_crossings(sidecar: &str) -> String {
    edited(sidecar, |sidecar| {
        sidecar["wheel_crossings"] = serde_json::json!([]);
    })
}

pub fn without_item_boxes(sidecar: &str) -> String {
    edited(sidecar, |sidecar| {
        sidecar["item_boxes"] = serde_json::json!([]);
    })
}

/// `sidecar` with one more item box under each of the spawn `slots`, in
/// order after the boxes it had, so a kart on that slot stands on a box at Go.
pub fn with_boxes_under(sidecar: &str, slots: impl IntoIterator<Item = usize>) -> String {
    edited(sidecar, |sidecar| {
        for slot in slots {
            let position = sidecar["spawn_grid"][slot]["position"].clone();
            sidecar["item_boxes"]
                .as_array_mut()
                .expect("the sidecar lists its boxes")
                .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
        }
    })
}

/// `sidecar` with one more item box at each of `positions`, facing -Z, in
/// order after the boxes it had.
pub fn with_boxes_at(sidecar: &str, positions: impl IntoIterator<Item = [f32; 3]>) -> String {
    edited(sidecar, |sidecar| {
        for position in positions {
            sidecar["item_boxes"]
                .as_array_mut()
                .expect("the sidecar lists its boxes")
                .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
        }
    })
}

/// `sidecar` with no coins, so a test driving over the coins' lines meets
/// nothing it did not ask for.
pub fn without_coins(sidecar: &str) -> String {
    edited(sidecar, |sidecar| {
        sidecar["coins"] = serde_json::json!([]);
    })
}

/// `sidecar` with one more coin at each of `positions`, facing -Z, in order
/// after the coins it had.
pub fn with_coins_at(sidecar: &str, positions: impl IntoIterator<Item = [f32; 3]>) -> String {
    edited(sidecar, |sidecar| {
        for position in positions {
            sidecar["coins"]
                .as_array_mut()
                .expect("the sidecar lists its coins")
                .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
        }
    })
}

/// The position of spawn slot `slot` in `sidecar`.
pub fn slot_position(sidecar: &str, slot: usize) -> [f32; 3] {
    let sidecar: serde_json::Value = serde_json::from_str(sidecar).expect("valid JSON");
    serde_json::from_value(sidecar["spawn_grid"][slot]["position"].clone()).expect("a position")
}

/// A race of `kart_count` karts on the oval, past its countdown.
pub fn race(kart_count: usize) -> State {
    started(State::new(SEED, kart_count, oval()))
}

/// A one-kart race stepped through `inputs`, one a tick.
pub fn run(mut state: State, inputs: impl IntoIterator<Item = KartInput>) -> State {
    for input in inputs {
        state = step(state, &[input], DT);
    }
    state
}

/// The readout of the race's only kart; fails the test when there are more.
pub fn only_kart(state: &State) -> KartReadout {
    let readouts = state.kart_readouts();
    assert_eq!(readouts.len(), 1);
    readouts[0]
}

pub fn throttle() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// Held past a stop, the brake reverses the kart.
pub fn brake() -> KartInput {
    KartInput {
        brake: 1.0,
        ..KartInput::default()
    }
}

/// The item button pressed and let go in one tick.
pub fn tap() -> KartInput {
    KartInput {
        use_item: true,
        ..KartInput::default()
    }
}

/// The oval with one more item box under each of the spawn `slots`.
pub fn oval_with_boxes_under(slots: impl IntoIterator<Item = usize>) -> Track {
    track(&with_boxes_under(OVAL, slots))
}

pub fn oval_without_item_boxes() -> Track {
    track(&without_item_boxes(OVAL))
}

/// A driver that holds the throttle with the wheel centred.
pub fn straight(_: &State, _: usize) -> KartInput {
    throttle()
}

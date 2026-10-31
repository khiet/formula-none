//! The hop off a slope at the sim's public seam: Bell Deer's bridge
//! comes down to ground level as the road falls away under a hop, so a kart
//! hopping there is still in the air when the hop's airtime runs out. Tests
//! drive one kart down the ramp and read `kart_readouts`.

mod common;

use common::{
    follow_centreline, only_kart, started, track, with_slot_at, without_wheel_crossings, BELL_DEER,
    DT, SEED,
};

use sim::{step, KartInput, KartReadout, State};

/// Where the drive to the ramp starts: on the flat, far enough short of the
/// bridge to reach the deck near top speed.
const RUN_UP_PROGRESS_M: f32 = 950.0;
/// Where the road starts down from the deck on the far side of the bridge
/// (layouts/bell_deer.py); it is back at ground level by 1080 m.
const DESCENT_PROGRESS_M: f32 = 1028.0;
/// Seconds from the top of the ramp to the flat beyond it, and then some.
const DESCENT_S: f32 = 2.5;

/// The kart alone on Bell Deer, started on the run-up to the bridge
/// facing along the track.
fn on_the_run_up() -> State {
    let base = track(BELL_DEER);
    let point = base.point_at(RUN_UP_PROGRESS_M);
    let [dx, dz] = base.direction_at(RUN_UP_PROGRESS_M);
    // The kart faces -Z under the identity, so a heading along (dx, dz).
    let yaw = (-dx).atan2(-dz);
    let sidecar = with_slot_at(
        &without_wheel_crossings(BELL_DEER),
        0,
        [point[0], point[1] + 0.5, point[2]],
        yaw,
    );
    started(State::new(SEED, 1, track(&sidecar)))
}

/// How far the kart's centre is above the centreline's road at its spline
/// progress.
fn above_the_road(state: &State) -> f32 {
    let progress = state.standings()[0].spline_progress;
    only_kart(state).position[1] - state.track().point_at(progress)[1]
}

/// Drive to the top of the ramp, then on down it for `DESCENT_S` with the
/// hop button as `hop` says on each tick from the top, straight ahead.
/// Returns the kart's readout and its height above the road on every tick
/// of the descent.
fn down_the_ramp(hop: impl Fn(usize) -> bool) -> Vec<(KartReadout, f32)> {
    let mut state = on_the_run_up();
    while state.standings()[0].spline_progress < DESCENT_PROGRESS_M {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
    }
    let mut descent = Vec::new();
    for tick in 0..(DESCENT_S / DT) as usize {
        let input = KartInput {
            steer: 0.0,
            hop: hop(tick),
            ..follow_centreline(&state, 0)
        };
        state = step(state, &[input], DT);
        let height = above_the_road(&state);
        descent.push((only_kart(&state), height));
    }
    descent
}

fn peak(descent: &[(KartReadout, f32)]) -> f32 {
    descent
        .iter()
        .map(|(_, height)| *height)
        .fold(f32::MIN, f32::max)
}

#[test]
fn tapping_hop_down_the_ramp_lifts_the_kart_no_higher_than_one_hop() {
    let one_hop = peak(&down_the_ramp(|tick| tick == 0));
    let tapped = peak(&down_the_ramp(|tick| tick % 2 == 0));

    assert!(
        tapped < one_hop + 0.05,
        "tapping reached {tapped} m above the road, one hop {one_hop} m"
    );
}

#[test]
fn a_kart_hopping_off_the_ramp_is_airborne_until_it_reaches_the_road() {
    let descent = down_the_ramp(|tick| tick == 0);

    // On the flat beyond the ramp. A kart rests a little higher on the ramp
    // itself, level on its uphill edge, so in the air means well above this.
    let (last, resting) = descent.last().expect("a descent");
    let in_the_air: Vec<_> = descent
        .iter()
        .filter(|(_, height)| *height > resting + 0.3)
        .collect();

    assert!(!in_the_air.is_empty(), "the hop leaves the road behind");
    assert!(
        in_the_air.iter().all(|(readout, _)| readout.airborne),
        "reported on the ground while still in the air"
    );
    assert!(!last.airborne, "landed by the foot of the ramp");
}

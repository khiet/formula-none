//! Spline progress where the course crosses over itself, at the sim's public
//! seam. On the figure-eight fixture the nearest centreline point to a kart
//! beside one leg can lie on the other leg near the crossing, so progress is
//! searched near the last reading. Readings come from `State::standings`
//! and `State::kart_readouts`, projectiles from `State::projectiles`.

mod common;

use common::{
    figure_eight, follow_centreline, started, tap, throttle, ticks, track, with_boxes_under,
    with_slot_at, DT, FIGURE_EIGHT, SEED,
};

use sim::{step, Item, KartInput, RaceEvent, State, Track};

/// How far a reading moved since the last one, the shorter way round the
/// lap: negative is backwards.
fn moved(track: &Track, from: f32, to: f32) -> f32 {
    let lap = track.spline_length();
    (to - from + lap / 2.0).rem_euclid(lap) - lap / 2.0
}

fn plan_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The spline progress of the top of the bridge, where the legs cross.
fn bridge_top(track: &Track) -> f32 {
    let steps = (track.spline_length() * 10.0) as usize;
    (0..steps)
        .map(|step| step as f32 / 10.0)
        .max_by(|&a, &b| track.point_at(a)[1].total_cmp(&track.point_at(b)[1]))
        .expect("a lap has length")
}

/// Where a kart `right_m` right of the centreline at `progress` stands, and
/// the yaw that faces it along the lap.
fn beside_centreline(track: &Track, progress: f32, right_m: f32) -> ([f32; 3], f32) {
    let point = track.point_at(progress);
    let [x, z] = track.direction_at(progress);
    // Right of the direction of travel (x, z), seen from above, is (-z, x);
    // yaw zero faces -Z and grows counter-clockwise.
    let position = [point[0] - z * right_m, 0.0, point[2] + x * right_m];
    (position, (-x).atan2(-z))
}

#[test]
fn a_kart_driven_across_the_crossing_on_each_leg_keeps_its_own_legs_progress() {
    let track = figure_eight();
    let mut state = started(State::new(SEED, 1, figure_eight()));
    let mut last = state.standings()[0].spline_progress;
    let mut travelled = 0.0;
    let mut nearest_crossing = [f32::INFINITY; 2];
    // A little over a lap from the grid crosses the bridge leg, then the
    // ground leg, both 2.5 m right of the centreline.
    while travelled < track.spline_length() + 20.0 {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
        let progress = state.standings()[0].spline_progress;
        let position = state.kart_readouts()[0].position;
        let step_m = moved(&track, last, progress);
        assert!(
            (0.0..2.0).contains(&step_m),
            "progress went from {last} to {progress} at {position:?}"
        );
        assert!(
            plan_distance(position, track.point_at(progress)) < 6.0,
            "progress {progress} is not beside the kart at {position:?}"
        );
        let leg = usize::from(progress > track.spline_length() / 2.0);
        nearest_crossing[leg] = nearest_crossing[leg].min(plan_distance(position, [0.0; 3]));
        travelled += step_m;
        last = progress;
    }
    assert!(
        nearest_crossing.iter().all(|&distance| distance < 3.0),
        "the kart passed the crossing on both legs: {nearest_crossing:?}"
    );
}

#[test]
fn a_kart_respawned_far_from_its_last_reading_reads_its_checkpoints_progress() {
    let track = figure_eight();
    let mut state = started(State::new(SEED, 1, figure_eight()));
    // The grid reads nearly a lap, behind the start line.
    let past_the_bridge = bridge_top(&track) + 10.0..track.spline_length() / 2.0;
    while !past_the_bridge.contains(&state.standings()[0].spline_progress) {
        let input = follow_centreline(&state, 0);
        state = step(state, &[input], DT);
    }
    // Straight on off the asphalt beyond the west lobe, short of the west
    // tip, so the start line is the last checkpoint.
    let mut respawned = false;
    for _ in 0..ticks(8.0) {
        state = step(state, &[throttle()], DT);
        if state.events().contains(&RaceEvent::Respawned { kart: 0 }) {
            respawned = true;
            break;
        }
    }
    assert!(respawned, "the kart left the world and respawned");
    let readout = state.kart_readouts()[0];
    assert_eq!(readout.checkpoint, 0);
    let start = track.checkpoint_progress()[0];
    assert!(
        moved(&track, start, readout.spline_progress).abs() < 1.0,
        "respawned at the start line ({start}) but reads {}",
        readout.spline_progress
    );
}

#[test]
fn a_projectile_flying_over_the_crossing_keeps_its_own_legs_height() {
    let base = figure_eight();
    let bridge = bridge_top(&base);
    // The target waits up the bridge leg; the owner fires from just short
    // of the crossing, 2.5 m right of the centreline, so the projectile
    // starts nearer the ground leg's centreline than the bridge's as it
    // closes on the crossing.
    let (target_at, target_yaw) = beside_centreline(&base, bridge + 30.0, 0.0);
    let (owner_at, owner_yaw) = beside_centreline(&base, bridge - 4.0, 2.5);
    let sidecar = with_slot_at(FIGURE_EIGHT, 0, target_at, target_yaw);
    let sidecar = with_slot_at(&sidecar, 1, owner_at, owner_yaw);
    let sidecar = with_boxes_under(&sidecar, [1]);
    let idle = KartInput::default();
    let mut state = (0..)
        .map(|seed| {
            let state = started(State::new(seed, 2, track(&sidecar)));
            step(state, &[idle, idle], DT)
        })
        .find(|state| {
            state.events().iter().any(|event| {
                matches!(
                    event,
                    RaceEvent::ItemPicked {
                        kart: 1,
                        item: Item::Homing,
                        ..
                    }
                )
            })
        })
        .expect("some seed rolls the projectile");

    state = step(state, &[idle, tap()], DT);
    assert_eq!(state.projectiles().len(), 1, "the projectile flies");
    let mut closest = f32::INFINITY;
    for _ in 0..ticks(5.0) {
        let Some(projectile) = state.projectiles().first().copied() else {
            break;
        };
        let from_crossing = plan_distance(projectile.position, [0.0; 3]);
        closest = closest.min(from_crossing);
        if from_crossing < 4.0 {
            assert!(
                projectile.position[1] > 4.0,
                "the projectile at {:?} dropped off the bridge",
                projectile.position
            );
        }
        state = step(state, &[idle, idle], DT);
    }
    assert!(closest < 2.0, "the projectile passed {closest} m from the crossing");
}

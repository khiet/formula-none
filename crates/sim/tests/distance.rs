//! The race distance (ADR 0083): a Sprint finishes on its third lap and a
//! Grand Prix on its fifth, for the state and for a prediction of it.

mod common;

use common::{oval, started, ticks, DT, SEED};
use sim::tuning::{GRAND_PRIX_LAPS, LAPS_PER_RACE};
use sim::{step, BotLevel, Prediction, RaceDistance, Standing, State};

/// A lone bot's standing as it finishes a race on the oval over `distance`,
/// or fails the test when it has not after ninety seconds a lap.
fn standing_at_the_finish(distance: RaceDistance) -> Standing {
    let mut state =
        started(State::with_bots(SEED, 0, 1, oval(), BotLevel::Basic).with_distance(distance));
    for _ in 0..ticks(distance.laps() as f32 * 90.0) {
        state = step(state, &[], DT);
        if state.standings()[0].finish_time_s.is_some() {
            break;
        }
    }
    let standing = state.standings()[0];
    assert!(
        standing.finish_time_s.is_some(),
        "{distance:?}: {standing:?}"
    );
    standing
}

#[test]
fn a_sprint_finishes_a_kart_on_its_third_lap() {
    let standing = standing_at_the_finish(RaceDistance::Sprint);
    assert_eq!(standing.laps_completed, LAPS_PER_RACE);
    assert_eq!(standing.lap, 3);
}

#[test]
fn a_grand_prix_finishes_a_kart_on_its_fifth_lap() {
    let standing = standing_at_the_finish(RaceDistance::GrandPrix);
    assert_eq!(standing.laps_completed, GRAND_PRIX_LAPS);
    assert_eq!(standing.lap, 5);
}

#[test]
fn a_race_nobody_picked_a_distance_for_is_a_sprint() {
    let state = started(State::with_bots(SEED, 0, 1, oval(), BotLevel::Basic));
    let picked = started(
        State::with_bots(SEED, 0, 1, oval(), BotLevel::Basic)
            .with_distance(RaceDistance::default()),
    );
    assert_eq!(RaceDistance::default(), RaceDistance::Sprint);
    assert_eq!(state.hash(), picked.hash());
}

#[test]
fn the_wire_spells_each_distance_and_refuses_any_other() {
    for distance in RaceDistance::ALL {
        assert_eq!(RaceDistance::from_name(distance.name()), Some(distance));
    }
    assert_eq!(
        RaceDistance::from_name("grand-prix"),
        Some(RaceDistance::GrandPrix)
    );
    assert_eq!(RaceDistance::from_name("marathon"), None);
}

/// The lap a prediction of a race over `distance` holds after a snapshot puts
/// its kart on lap 4.
fn predicted_lap_after_a_snapshot_on_lap_four(distance: RaceDistance) -> u32 {
    let server = State::with_bots(SEED, 1, 0, oval(), BotLevel::Basic);
    let mut prediction = Prediction::new(SEED, 0, oval(), BotLevel::Basic).with_distance(distance);
    prediction.tick(1, Default::default(), &[]);
    let mut reading = server.kart_readouts()[0];
    reading.lap = 4;
    prediction.reconcile(1, 1, &reading);
    prediction.standing().lap
}

#[test]
fn a_prediction_of_a_grand_prix_holds_a_lap_past_a_sprints_last() {
    assert_eq!(
        predicted_lap_after_a_snapshot_on_lap_four(RaceDistance::GrandPrix),
        4
    );
    assert_eq!(
        predicted_lap_after_a_snapshot_on_lap_four(RaceDistance::Sprint),
        3
    );
}

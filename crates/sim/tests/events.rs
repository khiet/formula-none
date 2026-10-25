//! The events a tick reports at the sim's public seam: a lap completed, a
//! kart finished and a kart respawned, read from `State::events` after the
//! `step` that produced them and gone after the next. The server repeats
//! them to the clients; the sim only reports them. The races run on the
//! oval without its item boxes, so a kart on the racing line reports only
//! the lap, the finish and the respawn; `items.rs` covers the rest.

mod common;

use common::{
    follow_centreline, oval_without_item_boxes, throttle, ticks, track, with_slot_at,
    without_walls, DT, OVAL, SEED,
};

use sim::tuning::{LAPS_PER_RACE, RESPAWN_DELAY_S};
use sim::{step, KartInput, Phase, RaceEvent, State, Surface};

/// A quarter turn clockwise seen from above: from facing -Z to facing +X.
const FACING_PLUS_X: f32 = -std::f32::consts::FRAC_PI_2;

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
        assert!(state.events().is_empty(), "nothing happens on the grid");
    }
    state
}

/// Step every human along the centreline, collecting each tick's events
/// with the tick they came on, until `done` or `seconds` have passed.
fn drive_collecting(
    mut state: State,
    seconds: f32,
    done: impl Fn(&State) -> bool,
) -> (State, Vec<(usize, RaceEvent)>) {
    let mut events = Vec::new();
    for tick in 0..ticks(seconds) {
        if done(&state) {
            return (state, events);
        }
        let inputs: Vec<KartInput> = (0..state.human_count())
            .map(|kart| follow_centreline(&state, kart))
            .collect();
        state = step(state, &inputs, DT);
        events.extend(state.events().iter().map(|&event| (tick, event)));
    }
    panic!("not done within {seconds} s");
}

#[test]
fn a_fresh_race_reports_no_events() {
    assert!(State::new(SEED, 2, oval_without_item_boxes())
        .events()
        .is_empty());
}

#[test]
fn completing_a_lap_reports_it_once_with_the_lap_time_the_standing_records() {
    let state = started(State::new(SEED, 1, oval_without_item_boxes()));
    let (state, events) = drive_collecting(state, 60.0, |state| {
        state.standings()[0].laps_completed == 1
    });
    let standing = state.standings()[0];
    let best = standing.best_lap_s.expect("a lap was completed");
    assert_eq!(
        events,
        vec![(
            events[0].0,
            RaceEvent::LapCompleted {
                kart: 0,
                lap: 1,
                lap_time_s: best
            }
        )],
        "one event, on the tick the lap completed"
    );

    // The next tick reports nothing: events are the last tick's only.
    let input = follow_centreline(&state, 0);
    let state = step(state, &[input], DT);
    assert!(state.events().is_empty());
}

#[test]
fn finishing_reports_the_finish_with_the_place_taken_and_the_clock() {
    let state = started(State::new(SEED, 2, oval_without_item_boxes()));
    let (state, events) = drive_collecting(state, 200.0, |state| {
        matches!(state.phase(), Phase::Finished)
    });
    let standings = state.standings();
    let finishes: Vec<RaceEvent> = events
        .iter()
        .map(|(_, event)| *event)
        .filter(|event| matches!(event, RaceEvent::Finished { .. }))
        .collect();
    assert_eq!(finishes.len(), 2, "both karts finished: {events:?}");
    for (place, event) in finishes.iter().enumerate() {
        let RaceEvent::Finished {
            kart,
            position,
            finish_time_s,
        } = *event
        else {
            unreachable!()
        };
        assert_eq!(position, place as u32 + 1, "finishers take places in order");
        assert_eq!(standings[kart].position, position);
        assert_eq!(standings[kart].finish_time_s, Some(finish_time_s));
    }
    let laps: Vec<RaceEvent> = events
        .iter()
        .map(|(_, event)| *event)
        .filter(|event| matches!(event, RaceEvent::LapCompleted { .. }))
        .collect();
    assert_eq!(
        laps.len(),
        2 * LAPS_PER_RACE as usize,
        "every lap of both karts was reported: {laps:?}"
    );
    // The last lap's completion and the finish come on the same tick.
    let last_lap_tick = events
        .iter()
        .filter(|(_, event)| matches!(event, RaceEvent::LapCompleted { kart: 0, lap, .. } if *lap == LAPS_PER_RACE))
        .map(|(tick, _)| *tick)
        .next()
        .expect("kart 0 completed its last lap");
    let finish_tick = events
        .iter()
        .filter(|(_, event)| matches!(event, RaceEvent::Finished { kart: 0, .. }))
        .map(|(tick, _)| *tick)
        .next()
        .expect("kart 0 finished");
    assert_eq!(last_lap_tick, finish_tick);
}

#[test]
fn coming_back_from_out_of_the_world_reports_a_respawn_on_that_tick() {
    // Slot 0 turned to face +X drives off the front straight, across the
    // run-off and the grass, and over the edge of the world; the oval's
    // walls, which would hold it on the sand, are taken away first.
    let track = track(&without_walls(&with_slot_at(
        OVAL,
        0,
        [28.0, 0.0, 4.0],
        FACING_PLUS_X,
    )));
    let mut state = started(State::new(SEED, 1, track));

    let mut was_out = false;
    for _ in 0..ticks(8.0) {
        state = step(state, &[throttle()], DT);
        let out = state.kart_surface(0) == Surface::OutOfWorld;
        if was_out && !out {
            assert_eq!(state.events(), &[RaceEvent::Respawned { kart: 0 }]);
            return;
        }
        assert!(
            state.events().is_empty(),
            "nothing to report before the respawn: {:?}",
            state.events()
        );
        was_out = out;
    }
    panic!(
        "the kart never respawned within {} s",
        RESPAWN_DELAY_S + 6.5
    );
}

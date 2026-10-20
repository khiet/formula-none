//! The hazard at the sim's public seam: a fired hazard drops the tuned
//! distance behind the kart, trails it while the item button is held and
//! drops on release, spins out the kart that hits it for the tuned time
//! with its inputs ignored, never the kart that dropped it on release, and
//! is gone the tuned lifetime after the drop. Hazards are read from
//! `State::hazards` and spin-outs from `State::events` after the `step`
//! that produced them.

use sim::tuning::{
    HAZARD_DROP_DISTANCE_M, HAZARD_LIFETIME_S, KART_HALF_LENGTH_M, SPIN_OUT_S, TICKS_PER_SECOND,
};
use sim::{step, Item, KartInput, Phase, RaceEvent, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

/// The oval with one more item box, under each of the first `slots` spawn
/// slots, so a kart there stands on a box at Go.
fn oval_with_boxes_under_slots(slots: usize) -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    for slot in 0..slots {
        let position = sidecar["spawn_grid"][slot]["position"].clone();
        sidecar["item_boxes"]
            .as_array_mut()
            .expect("the fixture lists its boxes")
            .push(serde_json::json!({ "position": position, "yaw": 0.0 }));
    }
    Track::from_sidecar(&sidecar.to_string()).expect("the oval with more boxes loads")
}

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

/// A race of `humans` karts, every one on a box under its slot, with a
/// hazard in the slot of kart `holder` from the first racing tick: the seeds
/// are tried in order until one rolls it.
fn holding_a_hazard(humans: usize, holder: usize) -> State {
    for seed in 0.. {
        let state = started(State::new(
            seed,
            humans,
            oval_with_boxes_under_slots(humans),
        ));
        let state = step(state, &vec![KartInput::default(); humans], DT);
        let rolled = state.events().iter().any(|event| {
            matches!(
                event,
                RaceEvent::ItemPicked { kart, item: Item::Hazard, .. } if *kart == holder
            )
        });
        if rolled {
            return state;
        }
    }
    unreachable!()
}

/// A press and release in one tick: fires and drops.
fn tap() -> KartInput {
    KartInput {
        use_item: true,
        ..KartInput::default()
    }
}

#[test]
fn a_fired_hazard_drops_the_tuned_distance_behind_the_kart() {
    let state = holding_a_hazard(1, 0);
    let [x, y, z] = state.kart_transforms()[0].position;
    assert!(state.hazards().is_empty(), "nothing dropped yet");

    // The grid faces -Z, so behind the kart is +Z, on the surface under it.
    let state = step(state, &[tap()], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: 0,
            item: Item::Hazard
        }]
    );
    assert_eq!(state.kart_transforms()[0].held_item, None);
    let [hazard] = state.hazards()[..] else {
        panic!("one hazard: {:?}", state.hazards());
    };
    assert_eq!(hazard.owner, 0);
    assert!(!hazard.held, "a tap drops the hazard the tick it is fired");
    let [hx, hy, hz] = hazard.position;
    assert!((hx - x).abs() < 1e-3, "dropped at x {hx}, the kart at {x}");
    assert!(
        (hz - (z + HAZARD_DROP_DISTANCE_M)).abs() < 1e-3,
        "dropped at z {hz}, the kart at {z}"
    );
    assert!(hy < y, "on the surface, below the kart's centre");
}

/// Driving on with the item button held down.
fn drive_holding() -> KartInput {
    KartInput {
        throttle: 1.0,
        hold_item: true,
        ..KartInput::default()
    }
}

fn drive() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// The hazard's distance from the kart's centre on the ground, in metres.
fn distance_behind(state: &State, kart: usize) -> f32 {
    let [kx, _, kz] = state.kart_transforms()[kart].position;
    let [hx, _, hz] = state.hazards()[0].position;
    ((hx - kx).powi(2) + (hz - kz).powi(2)).sqrt()
}

#[test]
fn a_held_hazard_trails_the_kart_while_the_item_button_is_held_and_drops_on_release() {
    // The button going down fires the hazard and keeps it trailing.
    let mut state = step(holding_a_hazard(1, 0), &[drive_holding()], DT);
    assert!(state.hazards()[0].held);
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive_holding()], DT);
        let [hazard] = state.hazards()[..] else {
            panic!("one hazard: {:?}", state.hazards());
        };
        assert!(hazard.held, "held while the button is");
        assert!(
            (distance_behind(&state, 0) - HAZARD_DROP_DISTANCE_M).abs() < 1e-2,
            "trailing at {} m",
            distance_behind(&state, 0)
        );
    }
    assert!(state.kart_transforms()[0].speed > 5.0, "driving on");

    // Let go: the hazard stays where it was and the kart drives away from it.
    let state = step(state, &[drive()], DT);
    let dropped = state.hazards()[0];
    assert!(!dropped.held);
    let mut state = state;
    for _ in 0..ticks(0.5) {
        state = step(state, &[drive()], DT);
        assert_eq!(state.hazards()[0].position, dropped.position);
    }
    assert!(distance_behind(&state, 0) > 5.0, "driven clear of it");
    assert!(
        !state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::SpunOut { .. })),
        "the dropping kart never hits its own hazard on release"
    );
}

fn reverse() -> KartInput {
    KartInput {
        brake: 1.0,
        ..KartInput::default()
    }
}

/// A one-kart race the tick its kart is spun out by its own hazard: dropped
/// at Go, driven away from and backed into.
fn spun_out() -> State {
    let mut state = step(holding_a_hazard(1, 0), &[tap()], DT);
    for _ in 0..ticks(1.5) {
        state = step(state, &[drive()], DT);
    }
    for _ in 0..ticks(8.0) {
        state = step(state, &[reverse()], DT);
        if state
            .events()
            .contains(&RaceEvent::SpunOut { kart: 0, by: 0 })
        {
            return state;
        }
    }
    panic!("never backed into its own hazard");
}

#[test]
fn hitting_a_hazard_spins_the_kart_out_for_the_tuned_time_with_its_inputs_ignored() {
    let state = spun_out();
    assert!(state.hazards().is_empty(), "the hazard is used up");
    let hit = state.kart_transforms()[0];
    assert_eq!(hit.speed, 0.0, "stopped");
    assert_eq!(hit.velocity, [0.0, 0.0, 0.0]);
    let left = hit.spin_out_s.expect("spinning out");
    assert!(
        (left - SPIN_OUT_S).abs() < 1e-5,
        "{left} s of spin-out left"
    );

    // Full throttle moves it nowhere until the spin-out runs out.
    let mut state = state;
    let mut spun_ticks = 0;
    while let Some(left) = state.kart_transforms()[0].spin_out_s {
        assert!(left > 0.0);
        state = step(state, &[drive()], DT);
        spun_ticks += 1;
        let [x, y, z] = state.kart_transforms()[0].position;
        let [hx, hy, hz] = hit.position;
        assert!(
            (x - hx).abs() < 1e-4 && (y - hy).abs() < 1e-4 && (z - hz).abs() < 1e-4,
            "moved while spun out: {:?} from {:?}",
            [x, y, z],
            hit.position
        );
        assert_eq!(state.kart_transforms()[0].rotation, hit.rotation);
        assert!(spun_ticks <= ticks(SPIN_OUT_S) + 2, "never recovered");
    }
    let expected = ticks(SPIN_OUT_S);
    assert!(
        (expected - 1..=expected + 1).contains(&spun_ticks),
        "spun out for {spun_ticks} ticks, expected about {expected}"
    );
    for _ in 0..ticks(0.5) {
        state = step(state, &[drive()], DT);
    }
    assert!(
        state.kart_transforms()[0].speed > 3.0,
        "driving again: {} m/s",
        state.kart_transforms()[0].speed
    );
}

#[test]
fn a_dropped_hazard_is_gone_the_tuned_lifetime_after_the_drop() {
    // Dropped at Go and left alone: the kart drives off and parks.
    let mut state = step(holding_a_hazard(1, 0), &[tap()], DT);
    for _ in 0..ticks(1.0) {
        state = step(state, &[drive()], DT);
    }
    let mut since_drop = ticks(1.0) + 1;
    while !state.hazards().is_empty() {
        state = step(state, &[KartInput::default()], DT);
        since_drop += 1;
        assert!(
            since_drop <= ticks(HAZARD_LIFETIME_S) + 2,
            "the hazard never despawned"
        );
    }
    let expected = ticks(HAZARD_LIFETIME_S);
    assert!(
        (expected - 1..=expected + 1).contains(&since_drop),
        "gone after {since_drop} ticks, expected about {expected}"
    );
    assert!(
        state.events().is_empty(),
        "a despawn is nobody's business: {:?}",
        state.events()
    );
}

#[test]
fn a_held_hazard_does_not_age_and_spins_out_the_kart_that_runs_into_it_from_behind() {
    // Slot 2 stands directly behind slot 0 on the oval's grid. Kart 0 holds
    // its hazard past the lifetime a dropped one gets, and kart 2 drives
    // into the hazard before it can reach kart 0.
    let mut state = holding_a_hazard(3, 0);
    let hold = KartInput {
        hold_item: true,
        ..KartInput::default()
    };
    let idle = KartInput::default();
    state = step(state, &[hold, idle, idle], DT);
    for _ in 0..ticks(HAZARD_LIFETIME_S + 1.0) {
        state = step(state, &[hold, idle, idle], DT);
    }
    let [hazard] = state.hazards()[..] else {
        panic!("still held: {:?}", state.hazards());
    };
    assert!(hazard.held);
    assert_eq!(state.events(), &[], "nobody hit anything on the grid");

    let mut hit = None;
    for _ in 0..ticks(3.0) {
        state = step(state, &[hold, idle, drive()], DT);
        if let Some(event) = state
            .events()
            .iter()
            .find(|event| matches!(event, RaceEvent::SpunOut { .. }))
        {
            hit = Some(*event);
            break;
        }
    }
    assert_eq!(hit, Some(RaceEvent::SpunOut { kart: 2, by: 0 }));
    assert!(state.hazards().is_empty(), "used up");
    assert!(state.kart_transforms()[2].spin_out_s.is_some());
    assert_eq!(
        state.kart_transforms()[0].spin_out_s,
        None,
        "the owner is untouched"
    );
    let [_, _, z0] = state.kart_transforms()[0].position;
    let [_, _, z2] = state.kart_transforms()[2].position;
    assert!(
        z2 - z0 > 2.0 * KART_HALF_LENGTH_M,
        "stopped short of kart 0: {} m apart",
        z2 - z0
    );
}

#[test]
fn a_bot_drops_a_held_hazard_within_the_tuned_time() {
    use sim::tuning::BOT_HAZARD_HOLD_S;
    // A lone bot on a box under its slot rolls a hazard, fires it on the
    // straight ahead and trails it for the tuned time before dropping it.
    let mut state = (0..)
        .map(|seed| {
            let state = started(State::with_bots(seed, 0, 1, oval_with_boxes_under_slots(1)));
            step(state, &[], DT)
        })
        .find(|state| {
            state.events().iter().any(|event| {
                matches!(
                    event,
                    RaceEvent::ItemPicked {
                        kart: 0,
                        item: Item::Hazard,
                        ..
                    }
                )
            })
        })
        .expect("a seed rolls the bot a hazard");
    let mut fired_at = None;
    for tick in 0..ticks(BOT_HAZARD_HOLD_S + 3.0) {
        state = step(state, &[], DT);
        if state.events().contains(&RaceEvent::ItemFired {
            kart: 0,
            item: Item::Hazard,
        }) {
            fired_at = Some(tick);
        }
        if let Some(fired_at) = fired_at {
            let [hazard] = state.hazards()[..] else {
                panic!("one hazard on the track: {:?}", state.hazards());
            };
            if !hazard.held {
                let held_for = tick - fired_at;
                assert!(
                    held_for <= ticks(BOT_HAZARD_HOLD_S) + 1,
                    "held for {held_for} ticks before the drop"
                );
                assert!(
                    held_for > ticks(0.5),
                    "trailed it a while, not {held_for} ticks"
                );
                return;
            }
        }
    }
    panic!("the bot never dropped its hazard: fired at {fired_at:?}");
}

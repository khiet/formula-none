//! The hazard at the sim's public seam: a fired hazard drops the tuned
//! distance behind the kart, trails it while the item button is held and
//! drops on release, spins out the kart that hits it for the tuned time
//! with its inputs ignored, never the kart that dropped it on release, and
//! is gone the tuned lifetime after the drop. Hazards are read from
//! `State::hazards` and spin-outs from `State::events` after the `step`
//! that produced them.

mod common;

use common::{brake, landed, oval_with_boxes_under, started, tap, throttle, ticks, DT};

use sim::tuning::{HAZARD_DROP_DISTANCE_M, HAZARD_LIFETIME_S, KART_HALF_LENGTH_M, SPIN_OUT_S};
use sim::{step, Item, ItemSlot, KartInput, RaceEvent, State};

/// A race of `humans` karts, every one on a box under its slot, with a
/// hazard rolled at Go landed in the slot of kart `holder`: the seeds
/// are tried in order until one rolls it.
fn holding_a_hazard(humans: usize, holder: usize) -> State {
    for seed in 0.. {
        let state = started(State::new(seed, humans, oval_with_boxes_under(0..humans)));
        let state = step(state, &vec![KartInput::default(); humans], DT);
        let rolled = state.events().iter().any(|event| {
            matches!(
                event,
                RaceEvent::ItemPicked { kart, item: Some(Item::Hazard), .. } if *kart == holder
            )
        });
        if rolled {
            return landed(state, humans, [holder]);
        }
    }
    unreachable!()
}

#[test]
fn a_fired_hazard_drops_the_tuned_distance_behind_the_kart() {
    let state = holding_a_hazard(1, 0);
    let [x, y, z] = state.kart_readouts()[0].position;
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
    assert_eq!(state.kart_readouts()[0].item_slot, ItemSlot::Empty);
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

/// The hazard's distance from the kart's centre on the ground, in metres.
fn distance_behind(state: &State, kart: usize) -> f32 {
    let [kx, _, kz] = state.kart_readouts()[kart].position;
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
    assert!(state.kart_readouts()[0].speed > 5.0, "driving on");

    // Let go: the hazard stays where it was and the kart drives away from it.
    let state = step(state, &[throttle()], DT);
    let dropped = state.hazards()[0];
    assert!(!dropped.held);
    let mut state = state;
    for _ in 0..ticks(0.5) {
        state = step(state, &[throttle()], DT);
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

/// A one-kart race the tick its kart is spun out by its own hazard: dropped
/// at Go, driven away from and backed into.
fn spun_out() -> State {
    let mut state = step(holding_a_hazard(1, 0), &[tap()], DT);
    for _ in 0..ticks(1.5) {
        state = step(state, &[throttle()], DT);
    }
    for _ in 0..ticks(8.0) {
        state = step(state, &[brake()], DT);
        if state.events().contains(&RaceEvent::SpunOut {
            kart: 0,
            by: Some(0),
        }) {
            return state;
        }
    }
    panic!("never backed into its own hazard");
}

#[test]
fn hitting_a_hazard_spins_the_kart_out_for_the_tuned_time_with_its_inputs_ignored() {
    let state = spun_out();
    assert!(state.hazards().is_empty(), "the hazard is used up");
    let hit = state.kart_readouts()[0];
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
    while let Some(left) = state.kart_readouts()[0].spin_out_s {
        assert!(left > 0.0);
        state = step(state, &[throttle()], DT);
        spun_ticks += 1;
        let [x, y, z] = state.kart_readouts()[0].position;
        let [hx, hy, hz] = hit.position;
        assert!(
            (x - hx).abs() < 1e-4 && (y - hy).abs() < 1e-4 && (z - hz).abs() < 1e-4,
            "moved while spun out: {:?} from {:?}",
            [x, y, z],
            hit.position
        );
        assert_eq!(state.kart_readouts()[0].rotation, hit.rotation);
        assert!(spun_ticks <= ticks(SPIN_OUT_S) + 2, "never recovered");
    }
    let expected = ticks(SPIN_OUT_S);
    assert!(
        (expected - 1..=expected + 1).contains(&spun_ticks),
        "spun out for {spun_ticks} ticks, expected about {expected}"
    );
    for _ in 0..ticks(0.5) {
        state = step(state, &[throttle()], DT);
    }
    assert!(
        state.kart_readouts()[0].speed > 3.0,
        "driving again: {} m/s",
        state.kart_readouts()[0].speed
    );
}

#[test]
fn a_dropped_hazard_is_gone_the_tuned_lifetime_after_the_drop() {
    // Dropped at Go and left alone: the kart drives off and parks.
    let mut state = step(holding_a_hazard(1, 0), &[tap()], DT);
    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle()], DT);
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
        state = step(state, &[hold, idle, throttle()], DT);
        if let Some(event) = state
            .events()
            .iter()
            .find(|event| matches!(event, RaceEvent::SpunOut { .. }))
        {
            hit = Some(*event);
            break;
        }
    }
    assert_eq!(
        hit,
        Some(RaceEvent::SpunOut {
            kart: 2,
            by: Some(0)
        })
    );
    assert!(state.hazards().is_empty(), "used up");
    assert!(state.kart_readouts()[2].spin_out_s.is_some());
    assert_eq!(
        state.kart_readouts()[0].spin_out_s,
        None,
        "the owner is untouched"
    );
    let [_, _, z0] = state.kart_readouts()[0].position;
    let [_, _, z2] = state.kart_readouts()[2].position;
    assert!(
        z2 - z0 > 2.0 * KART_HALF_LENGTH_M,
        "stopped short of kart 0: {} m apart",
        z2 - z0
    );

    // Backed into while spun out, kart 2 stands where it is and bounces
    // kart 0 off instead.
    let spun_at = state.kart_readouts()[2].position;
    let mut nearest = f32::MAX;
    for _ in 0..ticks(0.6) {
        state = step(state, &[brake(), idle, idle], DT);
        assert!(state.kart_readouts()[2].spin_out_s.is_some());
        let [x2, _, z2] = state.kart_readouts()[2].position;
        assert!(
            (x2 - spun_at[0]).abs() < 1e-3 && (z2 - spun_at[2]).abs() < 1e-3,
            "pushed from {spun_at:?} to {:?}",
            [x2, z2]
        );
        let [_, _, z0] = state.kart_readouts()[0].position;
        assert!(
            z2 - z0 >= 2.0 * KART_HALF_LENGTH_M - 0.1,
            "kart 0 backed through it: {} m apart",
            z2 - z0
        );
        nearest = nearest.min(z2 - z0);
    }
    assert!(
        nearest <= 2.0 * KART_HALF_LENGTH_M + 0.1,
        "kart 0 never reached it: {nearest} m apart at the nearest"
    );
}

#[test]
fn a_bot_drops_a_held_hazard_within_the_tuned_time() {
    use sim::tuning::BOT_HAZARD_HOLD_S;
    // A lone bot on a box under its slot rolls a hazard, fires it on the
    // straight ahead and trails it for the tuned time before dropping it.
    let mut state = (0..)
        .map(|seed| {
            let state = started(State::with_bots(seed, 0, 1, oval_with_boxes_under(0..1)));
            step(state, &[], DT)
        })
        .find(|state| {
            state.events().iter().any(|event| {
                matches!(
                    event,
                    RaceEvent::ItemPicked {
                        kart: 0,
                        item: Some(Item::Hazard),
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

//! Tyre life at the sim's public seam (ADR 0080): with Tyres On a kart's
//! life falls by how it is driven, read from `kart_readouts` after each
//! `step`, and with Off nothing changes. Worn tyres cost speed and grip.

mod common;

use common::{
    follow_centreline, landed, oval, oval_with_boxes_under, started, steer_toward, tap, throttle,
    ticks, track, with_slot_at, without_coins, DT, FIGURE_EIGHT, OVAL, SEED,
};

use sim::tuning::{
    TOP_SPEED_M_PER_S, TYRE_LIFE_CLIFF_PERCENT, TYRE_LIFE_NEW_PERCENT, TYRE_TOP_SPEED_FACTORS,
    TYRE_WEAR_SPIN_OUT_PERCENT,
};
use sim::{
    step, BotLevel, Item, KartInput, KartReadout, Prediction, RaceEvent, State, Track, Tyres,
};

fn on(track: Track, karts: usize) -> State {
    State::with_tyres(SEED, karts, 0, track, BotLevel::Basic, Tyres::On)
}

fn life(state: &State, kart: usize) -> f32 {
    state.kart_readouts()[kart].tyre_life
}

/// How sharply a driver must steer for the line to call it a corner.
const CORNER_STEER: f32 = 0.2;

/// The life left when the only kart first completes a lap, driven by `driver`.
fn life_after_a_lap(driver: impl Fn(&State, KartInput) -> KartInput) -> f32 {
    let mut state = started(on(oval(), 1));
    for _ in 0..ticks(120.0) {
        let input = driver(&state, follow_centreline(&state, 0));
        state = step(state, &[input], DT);
        let lapped = state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::LapCompleted { .. }));
        if lapped {
            return life(&state, 0);
        }
    }
    panic!("the driver never finished a lap");
}

#[test]
fn a_kart_that_drifts_a_lap_ends_it_on_less_life_than_one_that_lifts() {
    let lifted = life_after_a_lap(|_, line| KartInput {
        throttle: if line.steer.abs() > CORNER_STEER {
            0.5
        } else {
            1.0
        },
        ..line
    });
    let drifted = life_after_a_lap(|state, line| KartInput {
        // Pressed for each corner and let go on the straights, so a drift
        // starts on the landing and pays out as the road straightens.
        hop: line.steer.abs() > CORNER_STEER
            || state.kart_readouts()[0].drift.is_some() && line.steer.abs() > CORNER_STEER / 2.0,
        ..line
    });

    assert!(lifted < TYRE_LIFE_NEW_PERCENT, "rolling wears the tyres");
    assert!(drifted < lifted, "drifted to {drifted}, lifted to {lifted}");
}

/// Two karts on boxes on the oval with a hazard rolled for the front one,
/// dropped behind it in the path of the kart behind, so driving on spins
/// that kart out.
fn hazard_ahead_of_kart_one() -> State {
    for seed in 0.. {
        let state = started(State::with_tyres(
            seed,
            2,
            0,
            oval_with_boxes_under(0..2),
            BotLevel::Basic,
            Tyres::On,
        ));
        let state = step(state, &[KartInput::default(); 2], DT);
        let rolled = state.events().iter().any(|event| {
            matches!(
                event,
                RaceEvent::ItemPicked {
                    kart: 0,
                    item: Some(Item::Hazard),
                    ..
                }
            )
        });
        if rolled {
            let state = landed(state, 2, [0]);
            return step(state, &[tap(), KartInput::default()], DT);
        }
    }
    unreachable!()
}

#[test]
fn a_spin_out_takes_the_tuned_life_and_a_spun_out_kart_loses_no_more() {
    let mut state = hazard_ahead_of_kart_one();
    let [hazard] = state.hazards()[..] else {
        panic!("one hazard: {:?}", state.hazards());
    };
    let target = [hazard.position[0], hazard.position[2]];
    let mut left = ticks(20.0);
    let at_the_hit = loop {
        let before = life(&state, 1);
        let toward = steer_toward(&state, 1, target);
        state = step(state, &[KartInput::default(), toward], DT);
        let hit = state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::SpunOut { kart: 1, .. }));
        if hit {
            // The tick's own rolling comes off too, before the hit lands.
            let lost = before - life(&state, 1);
            assert!(
                (TYRE_WEAR_SPIN_OUT_PERCENT..TYRE_WEAR_SPIN_OUT_PERCENT + 0.01).contains(&lost),
                "lost {lost}"
            );
            break life(&state, 1);
        }
        left -= 1;
        assert!(left > 0, "kart 1 never met the hazard");
    };

    while state.kart_readouts()[1].spin_out_s.is_some() {
        assert_eq!(life(&state, 1), at_the_hit, "spun out, nothing wears it");
        state = step(state, &[KartInput::default(), throttle()], DT);
    }
}

#[test]
fn a_stopped_kart_loses_none() {
    let mut state = on(oval(), 1);
    for _ in 0..ticks(6.0) {
        state = step(state, &[KartInput::default()], DT);
    }
    assert_eq!(life(&state, 0), TYRE_LIFE_NEW_PERCENT);
}

#[test]
fn a_kart_brought_to_rest_loses_none() {
    let mut state = started(on(oval(), 1));
    for _ in 0..ticks(2.0) {
        state = step(state, &[throttle()], DT);
    }
    let lift = KartInput::default();
    while state.kart_readouts()[0].speed != 0.0 {
        state = step(state, &[lift], DT);
    }
    let at_rest = life(&state, 0);
    assert!(at_rest < TYRE_LIFE_NEW_PERCENT, "it wore on the way");
    for _ in 0..ticks(2.0) {
        state = step(state, &[lift], DT);
    }
    assert_eq!(life(&state, 0), at_rest);
}

#[test]
fn a_kart_in_the_air_loses_none() {
    let mut state = started(on(oval(), 1));
    for _ in 0..ticks(2.0) {
        state = step(state, &[throttle()], DT);
    }
    let hop = KartInput {
        hop: true,
        ..throttle()
    };
    state = step(state, &[hop], DT);
    assert!(state.kart_readouts()[0].airborne, "the hop took off");
    let at_take_off = life(&state, 0);
    while state.kart_readouts()[0].airborne {
        assert_eq!(life(&state, 0), at_take_off);
        state = step(state, &[hop], DT);
    }
    assert!(life(&state, 0) < at_take_off, "wears again on landing");
}

#[test]
fn life_never_goes_below_zero() {
    // A drift held round in circles on the open flat at the figure-eight's
    // crossing, long enough to wear out. The button is pressed afresh
    // whenever no drift holds, as a hop needs.
    let sidecar = with_slot_at(FIGURE_EIGHT, 0, [0.0, 0.0, 0.0], 0.0);
    let mut state = started(on(track(&sidecar), 1));
    for tick in 0..ticks(90.0) {
        let kart = state.kart_readouts()[0];
        let input = KartInput {
            steer: 1.0,
            hop: kart.drift.is_some() || kart.airborne || tick % 2 == 0,
            ..throttle()
        };
        state = step(state, &[input], DT);
        assert!(life(&state, 0) >= 0.0);
    }
    assert_eq!(life(&state, 0), 0.0, "worn right out");
}

/// A kart on the figure-eight's open flat, its tyres held to a drift round in
/// circles until they are gone.
fn worn_right_out() -> State {
    let sidecar = with_slot_at(FIGURE_EIGHT, 0, [0.0, 0.0, 0.0], 0.0);
    let mut state = started(on(track(&sidecar), 1));
    for tick in 0..ticks(120.0) {
        let kart = state.kart_readouts()[0];
        if kart.tyre_life == 0.0 {
            return state;
        }
        let input = KartInput {
            steer: 1.0,
            hop: kart.drift.is_some() || kart.airborne || tick % 2 == 0,
            ..throttle()
        };
        state = step(state, &[input], DT);
    }
    panic!("the tyres never wore out");
}

#[test]
fn a_kart_on_tyres_at_zero_finishes_a_lap_under_the_worn_top_speed() {
    let mut state = worn_right_out();
    let mut fastest = 0.0_f32;
    for _ in 0..ticks(180.0) {
        let input = follow_centreline(&state, 0);
        // A boost sets its own speed, and the circles end in a mini-turbo:
        // only a tick with no boost at either end answers to the cap.
        let free = state.kart_readouts()[0].boost_s.is_none();
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        if free && kart.boost_s.is_none() {
            fastest = fastest.max(kart.speed);
        }
        let lapped = state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::LapCompleted { .. }));
        if lapped {
            assert!(
                fastest <= TYRE_TOP_SPEED_FACTORS[3] * TOP_SPEED_M_PER_S + 0.01,
                "{fastest}"
            );
            assert!(
                fastest > TYRE_TOP_SPEED_FACTORS[3] * TOP_SPEED_M_PER_S - 2.0,
                "{fastest}"
            );
            return;
        }
    }
    panic!("the worn kart never finished a lap");
}

#[test]
fn a_kart_worn_by_a_race_on_the_same_inputs_falls_behind_one_with_tyres_off() {
    let mut worn = on(oval(), 1);
    let mut off = State::new(SEED, 1, oval());
    for _ in 0..ticks(20.0) {
        let input = follow_centreline(&worn, 0);
        worn = step(worn, &[input], DT);
        let input = follow_centreline(&off, 0);
        off = step(off, &[input], DT);
    }
    let (worn, off) = (worn.kart_readouts()[0], off.kart_readouts()[0]);
    assert!(worn.tyre_life < TYRE_LIFE_NEW_PERCENT);
    assert_eq!(off.tyre_life, TYRE_LIFE_NEW_PERCENT, "Off never wears");
    assert!(worn.spline_progress < off.spline_progress);
}

#[test]
fn tyre_life_enters_the_hash_only_with_tyres_on() {
    let run = |tyres: Tyres| {
        let mut state = State::with_tyres(SEED, 1, 0, oval(), BotLevel::Basic, tyres);
        for _ in 0..ticks(5.0) {
            state = step(state, &[throttle()], DT);
        }
        state.hash()
    };
    let mut plain = State::new(SEED, 1, oval());
    for _ in 0..ticks(5.0) {
        plain = step(plain, &[throttle()], DT);
    }
    assert_eq!(run(Tyres::Off), plain.hash());
    assert_ne!(run(Tyres::On), plain.hash());
}

/// A prediction of the only kart beside the server's race of it, past Go.
fn predicted_beside_the_server() -> (Prediction, State, u32) {
    let mut server = on(oval(), 1);
    let mut prediction = Prediction::with_tyres(SEED, 0, oval(), BotLevel::Basic, Tyres::On);
    let mut sequence = 0;
    for _ in 0..ticks(6.0) {
        sequence += 1;
        let input = if matches!(server.phase(), sim::Phase::Countdown { .. }) {
            KartInput::default()
        } else {
            throttle()
        };
        server = step(server, &[input], DT);
        prediction.tick(sequence, input, &[]);
    }
    (prediction, server, sequence)
}

#[test]
fn a_prediction_wears_its_own_tyres_as_the_server_does() {
    let (prediction, server, _) = predicted_beside_the_server();
    assert!(life(&server, 0) < TYRE_LIFE_NEW_PERCENT);
    assert_eq!(prediction.kart_readout().tyre_life, life(&server, 0));
}

#[test]
fn a_prediction_adopts_a_hits_wear_from_the_snapshot_and_lets_a_small_gap_stand() {
    let (mut prediction, mut server, mut sequence) = predicted_beside_the_server();
    // The first acknowledgement lays the inputs out, which always rewinds.
    prediction.reconcile(sequence, sequence, &server.kart_readouts()[0]);

    sequence += 1;
    server = step(server, &[throttle()], DT);
    prediction.tick(sequence, throttle(), &[]);
    let kart = server.kart_readouts()[0];
    let close = KartReadout {
        tyre_life: kart.tyre_life - 0.5,
        ..kart
    };
    prediction.reconcile(sequence, sequence, &close);
    assert_eq!(
        prediction.kart_readout().tyre_life,
        kart.tyre_life,
        "within the tolerance, no rewind"
    );

    sequence += 1;
    server = step(server, &[throttle()], DT);
    prediction.tick(sequence, throttle(), &[]);
    let hit = KartReadout {
        tyre_life: server.kart_readouts()[0].tyre_life - TYRE_WEAR_SPIN_OUT_PERCENT,
        ..server.kart_readouts()[0]
    };
    prediction.reconcile(sequence, sequence, &hit);
    assert_eq!(
        prediction.kart_readout().tyre_life,
        hit.tyre_life,
        "the server's word stands"
    );
}

#[test]
fn a_prediction_leaves_wear_from_another_kart_to_the_snapshots() {
    let (mut touching, _, mut sequence) = predicted_beside_the_server();
    let (mut alone, _, _) = predicted_beside_the_server();
    loop {
        sequence += 1;
        let own = touching.kart_readout();
        let mut ahead = own;
        ahead.position[2] -= 2.0;
        touching.tick(sequence, throttle(), &[own, ahead]);
        alone.tick(sequence, throttle(), &[]);
        if touching.kart_readout().bump.is_some() {
            break;
        }
        assert!(sequence < 3000, "the kart drawn ahead never touched it");
    }
    assert_eq!(
        touching.kart_readout().tyre_life,
        alone.kart_readout().tyre_life
    );
}

/// The fastest a lone kart on the oval goes over a lap under no boost, with
/// its tyres held at `life` every tick, as the speed readout shows it (ADR 0087).
fn speed_cap_at_life_km_per_h(life_percent: f32) -> u32 {
    let mut state = started(on(track(&without_coins(OVAL)), 1));
    let mut fastest = 0.0_f32;
    for _ in 0..ticks(180.0) {
        state.adopt_tyre_life(0, life_percent);
        let input = follow_centreline(&state, 0);
        let free = state.kart_readouts()[0].boost_s.is_none();
        state = step(state, &[input], DT);
        let kart = state.kart_readouts()[0];
        if free && kart.boost_s.is_none() {
            fastest = fastest.max(kart.speed);
        }
        let lapped = state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::LapCompleted { .. }));
        if lapped {
            return (fastest * 3.6).round() as u32;
        }
    }
    panic!("the kart never finished a lap");
}

#[test]
fn a_kart_at_the_cliff_caps_at_95_km_per_h_and_one_worn_out_at_76() {
    assert_eq!(speed_cap_at_life_km_per_h(TYRE_LIFE_CLIFF_PERCENT), 95);
    assert_eq!(speed_cap_at_life_km_per_h(0.0), 76);
}

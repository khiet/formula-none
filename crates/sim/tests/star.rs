//! The star at the sim's public seam: fired, it gives the kart a run of the
//! tuned time at a raised top speed cap, with a kick on the fire, immune to
//! hazards, projectiles and the runaway wheel, passing through every
//! ordinary kart and spinning it out, bumping another star kart as usual,
//! kept through a respawn and ended by the finish. A second star puts the
//! timer back to the start. Runs are read from `star_s` on the kart readout
//! and hits from `State::events` after the `step` that produced them.

mod common;

use common::{
    brake, follow_centreline, landed, started, tap, throttle, ticks, track, with_boxes_under,
    with_slot_at, without_coins, without_item_boxes, without_walls, without_wheel_crossings, DT,
    OVAL,
};

use sim::tuning::{
    BOOST_PAD_FACTOR, BOOST_PAD_S, BOT_ITEM_USE_DELAY_S, ITEM_BOOST_FACTOR, ITEM_ROLL_S,
    KART_HALF_LENGTH_M, MINI_TURBO_BOOST_FACTOR, OFF_TRACK_SPEED_FACTOR, STAR_KICK_S, STAR_S,
    STAR_TOP_SPEED_FACTOR, TOP_SPEED_M_PER_S, WHEEL_FIRST_PASS_S,
};
use sim::{
    step, Item, ItemSlot, KartInput, MiniTurbo, Phase, Prediction, RaceEvent, State, Surface, Track,
};

/// The star kart's top speed cap on a run, with no coins and no rubber band.
const STAR_TOP_SPEED: f32 = STAR_TOP_SPEED_FACTOR * TOP_SPEED_M_PER_S;

fn idle() -> KartInput {
    KartInput::default()
}

/// The spin-outs the last tick reported.
fn spun_out(state: &State) -> Vec<RaceEvent> {
    state
        .events()
        .iter()
        .copied()
        .filter(|event| matches!(event, RaceEvent::SpunOut { .. }))
        .collect()
}

fn star_s(state: &State, kart: usize) -> Option<f32> {
    state.kart_readouts()[kart].star_s
}

/// Whether the pickups the last tick reported include `kart` rolling `item`.
fn rolled(state: &State, kart: usize, item: Item) -> bool {
    state.events().iter().any(|event| {
        matches!(event, RaceEvent::ItemPicked { kart: by, item: Some(rolled), .. } if *by == kart && *rolled == item)
    })
}

/// A race of `humans` karts and `bots` bots on the track `build` makes for
/// the seed, stepped on until the items `rolls` names, rolled at Go by the
/// karts standing on boxes under their slots, have landed: the seeds are
/// tried in order until one rolls them all. The star never reaches the front
/// two places, so a kart that rolls one stands third or further back on the
/// grid.
fn rolling(
    build: impl Fn() -> Track,
    humans: usize,
    bots: usize,
    rolls: &[(usize, Item)],
) -> State {
    for seed in 0.. {
        let state = started(State::with_bots(seed, humans, bots, build()));
        let state = step(state, &vec![idle(); humans], DT);
        if rolls.iter().all(|&(slot, item)| rolled(&state, slot, item)) {
            return landed(state, humans, rolls.iter().map(|&(slot, _)| slot));
        }
    }
    unreachable!()
}

/// The oval with no coins and no item boxes but one under each of `slots`;
/// `common::oval_with_boxes_under` keeps the track's coins and boxes.
fn clean_oval_with_boxes_under(slots: impl IntoIterator<Item = usize>) -> Track {
    track(&with_boxes_under(
        &without_coins(&without_item_boxes(OVAL)),
        slots,
    ))
}

/// Six idle humans on the oval's grid, the last of them in slot 5 holding
/// a star: the holder is sixth, where the star is most likely. Its slot is
/// moved to `holder_at`, facing -Z, with a box under it; the holder stays
/// sixth while it is no further up the straight than the grid's last row.
/// The front straight runs along -Z at x = 30 with asphalt 6 m either side,
/// the grid in two columns at x = 28 and x = 32 from z = 4 to z = 21, and
/// the second boost pad centred on (33, -60), 3 m wide either side.
const HOLDER: usize = 5;
fn holding_a_star_at(holder_at: [f32; 3]) -> State {
    rolling(
        || {
            let sidecar = with_slot_at(
                &without_coins(&without_item_boxes(OVAL)),
                HOLDER,
                holder_at,
                0.0,
            );
            track(&with_boxes_under(&sidecar, [HOLDER]))
        },
        HOLDER + 1,
        0,
        &[(HOLDER, Item::Star)],
    )
}

/// The lane at x = 26 is clear of the grid's columns and the boost pad.
const CLEAR_LANE: [f32; 3] = [26.0, 0.0, 26.0];
/// The lane at x = 34 is clear of the grid's columns and over the pad.
const PAD_LANE: [f32; 3] = [34.0, 0.0, 26.0];

fn holding_a_star() -> State {
    holding_a_star_at(CLEAR_LANE)
}

/// `inputs` with `input` for the holder and every other kart idle.
fn holder(input: KartInput) -> Vec<KartInput> {
    let mut inputs = vec![idle(); HOLDER + 1];
    inputs[HOLDER] = input;
    inputs
}

#[test]
fn a_fired_star_kicks_the_kart_then_raises_its_top_speed_for_the_tuned_time() {
    // Up to speed in the clear lane, then followed along the centreline
    // once immune, since a run outlasts the straight.
    let mut state = holding_a_star();
    for _ in 0..ticks(2.5) {
        state = step(state, &holder(throttle()), DT);
    }
    assert_eq!(state.kart_readouts()[HOLDER].speed, TOP_SPEED_M_PER_S);

    let mut state = step(
        state,
        &holder(KartInput {
            use_item: true,
            ..throttle()
        }),
        DT,
    );
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: HOLDER,
            item: Item::Star
        }]
    );
    let fired = state.kart_readouts()[HOLDER];
    assert_eq!(fired.item_slot, ItemSlot::Empty);
    assert!((fired.star_s.expect("on a run") - (STAR_S - DT)).abs() < 1e-5);
    // The kick is the boost item's factor for its own short time.
    assert_eq!(fired.speed, ITEM_BOOST_FACTOR * TOP_SPEED_M_PER_S);
    assert!((fired.boost_s.expect("the kick") - (STAR_KICK_S - DT)).abs() < 1e-5);
    assert_eq!(fired.boost_tier, MiniTurbo::Red);

    // Once the kick is spent the kart settles to the raised cap and holds it
    // for the rest of the run, then falls back under the tuned top speed.
    let mut capped_ticks = 0;
    let mut run_ticks = 1;
    while let Some(left) = star_s(&state, HOLDER) {
        assert!(left > 0.0);
        let input = follow_centreline(&state, HOLDER);
        state = step(state, &holder(input), DT);
        run_ticks += 1;
        let kart = state.kart_readouts()[HOLDER];
        // The kick's last tick still sets its speed; the cap takes over
        // from the tick after, and only the pad on the way raises it.
        if kart.star_s.is_some() && run_ticks > ticks(STAR_KICK_S) + 1 {
            assert!(
                kart.speed > STAR_TOP_SPEED - 1e-3,
                "never under the star's cap: {} m/s",
                kart.speed
            );
            capped_ticks += usize::from((kart.speed - STAR_TOP_SPEED).abs() < 1e-3);
        }
        assert!(run_ticks <= ticks(STAR_S) + 2, "the run never ended");
    }
    let expected = ticks(STAR_S);
    assert!(
        (expected - 1..=expected + 1).contains(&run_ticks),
        "ran for {run_ticks} ticks, expected about {expected}"
    );
    // The run less the kick and the pad's boost, with a tick either end.
    assert!(
        capped_ticks >= ticks(STAR_S - STAR_KICK_S - BOOST_PAD_S) - 4,
        "held to the cap for {capped_ticks} ticks"
    );
    for _ in 0..ticks(0.5) {
        let input = follow_centreline(&state, HOLDER);
        state = step(state, &holder(input), DT);
    }
    let kart = state.kart_readouts()[HOLDER];
    assert!(
        kart.speed <= TOP_SPEED_M_PER_S || kart.boost_s.is_some(),
        "back under the tuned top speed: {} m/s",
        kart.speed
    );
}

#[test]
fn braking_slows_a_star_kart() {
    let mut state = step(holding_a_star(), &holder(tap()), DT);
    for _ in 0..ticks(1.5) {
        state = step(state, &holder(throttle()), DT);
    }
    let before = state.kart_readouts()[HOLDER].speed;
    assert!((before - STAR_TOP_SPEED).abs() < 1e-3);

    for _ in 0..ticks(0.5) {
        state = step(state, &holder(brake()), DT);
    }

    let kart = state.kart_readouts()[HOLDER];
    assert!(
        kart.speed < before - 5.0,
        "braked from {before} to {}",
        kart.speed
    );
    assert!(kart.star_s.is_some(), "the run goes on");
}

#[test]
fn a_mini_turbo_during_a_run_does_not_end_it() {
    let mut state = step(holding_a_star(), &holder(tap()), DT);
    for _ in 0..ticks(1.0) {
        state = step(state, &holder(throttle()), DT);
    }
    assert!(
        state.kart_readouts()[HOLDER].position[2] < 0.0,
        "past the grid"
    );
    // Hop steering left, then hold the hop with counter-steer so the drift
    // it lands into turns little and stays in the lane, until orange charges.
    let hop_left = KartInput {
        steer: -1.0,
        hop: true,
        ..throttle()
    };
    let hold_wide = KartInput {
        steer: 1.0,
        hop: true,
        ..throttle()
    };
    state = step(state, &holder(hop_left), DT);
    let mut drifted = false;
    for _ in 0..ticks(1.0) {
        state = step(state, &holder(hold_wide), DT);
        drifted |= state.kart_readouts()[HOLDER].drift.is_some();
        if state.kart_readouts()[HOLDER].mini_turbo == MiniTurbo::Orange {
            break;
        }
    }
    assert!(drifted, "the landing started a drift");
    assert_eq!(state.kart_readouts()[HOLDER].mini_turbo, MiniTurbo::Orange);

    let state = step(state, &holder(throttle()), DT);

    let kart = state.kart_readouts()[HOLDER];
    assert_eq!(kart.drift, None, "released");
    assert_eq!(kart.boost_tier, MiniTurbo::Orange);
    assert!(kart.boost_s.is_some(), "the release boosts");
    assert_eq!(kart.speed, MINI_TURBO_BOOST_FACTOR * TOP_SPEED_M_PER_S);
    let left = kart.star_s.expect("the run goes on under the boost");
    assert!(left > STAR_S - 2.0, "{left} s left");
}

#[test]
fn a_boost_pad_during_a_run_sets_its_own_speed_then_the_kart_settles_back_to_the_stars_cap() {
    // The holder's line down the straight, 2.5 m right of the centreline,
    // runs over the pad centred on (33, -60) and on round the first turn.
    let mut state = step(holding_a_star_at(PAD_LANE), &holder(tap()), DT);
    let pad_z = state.track().boost_pads()[1].position[2];
    assert_eq!(pad_z, -60.0, "the fixture's pad is where this test expects");
    let mut boosted = false;
    let mut settled = false;
    for _ in 0..ticks(STAR_S - 0.5) {
        let input = follow_centreline(&state, HOLDER);
        state = step(state, &holder(input), DT);
        let kart = state.kart_readouts()[HOLDER];
        assert!(kart.star_s.is_some(), "the run goes on");
        if kart.position[2] > pad_z + 2.0 || kart.boost_s.is_some_and(|_| kart.position[2] > pad_z)
        {
            continue;
        }
        let pad_speed = BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S;
        if kart.boost_s.is_some() {
            assert!(
                (kart.speed - pad_speed).abs() < 1e-3,
                "the pad sets its own speed: {}",
                kart.speed
            );
            boosted = true;
        } else if boosted {
            // The pad's last tick still sets its speed; from the next the
            // kart is held to the star's cap, never between.
            assert!(
                (kart.speed - STAR_TOP_SPEED).abs() < 1e-3 || (kart.speed - pad_speed).abs() < 1e-3,
                "settled back to the star's cap: {}",
                kart.speed
            );
            settled |= (kart.speed - STAR_TOP_SPEED).abs() < 1e-3;
        }
    }
    assert!(boosted && settled);
}

/// A quarter turn clockwise seen from above: from facing -Z to facing +X.
const FACING_PLUS_X: f32 = -std::f32::consts::FRAC_PI_2;

#[test]
fn a_star_kart_ignores_the_off_track_cap_and_a_respawn_keeps_the_timer() {
    // Three humans on the open oval, no walls: two parked ahead at the
    // left edge of the straight, so the holder in slot 2 stands third, and
    // the holder on its box turned to face +X. Fired, it drives across the
    // run-off and the grass and over the edge of the world at x = 70.
    const STAR: usize = 2;
    let build = || {
        let mut sidecar = without_coins(&without_walls(&without_item_boxes(OVAL)));
        sidecar = with_slot_at(&sidecar, 0, [25.5, 0.0, -20.0], 0.0);
        sidecar = with_slot_at(&sidecar, 1, [25.5, 0.0, -24.0], 0.0);
        sidecar = with_slot_at(&sidecar, STAR, [28.0, 0.0, 4.0], FACING_PLUS_X);
        track(&with_boxes_under(&sidecar, [STAR]))
    };
    let state = rolling(build, 3, 0, &[(STAR, Item::Star)]);
    let drive = [idle(), idle(), throttle()];
    let mut state = step(state, &[idle(), idle(), tap()], DT);
    let mut fastest_on_grass = 0.0_f32;
    let mut respawned_after = None;
    for tick in 1..ticks(STAR_S) {
        state = step(state, &drive, DT);
        let kart = state.kart_readouts()[STAR];
        if state.kart_surface(STAR) == Surface::Grass {
            fastest_on_grass = fastest_on_grass.max(kart.speed);
        }
        if state
            .events()
            .contains(&RaceEvent::Respawned { kart: STAR })
        {
            respawned_after = Some(tick);
            break;
        }
    }
    assert!(
        fastest_on_grass > OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S + 5.0,
        "the off-track cap does not apply on a run: {fastest_on_grass} m/s on the grass"
    );
    let respawned_after = respawned_after.expect("drove off the world and respawned");

    // The timer is untouched by the respawn and runs out when it would have.
    let left = star_s(&state, STAR).expect("the run survives the respawn");
    let expected = STAR_S - (respawned_after + 1) as f32 * DT;
    assert!(
        (left - expected).abs() < 1e-3,
        "{left} s left, expected {expected}"
    );
    let mut run_ticks = respawned_after + 1;
    while star_s(&state, STAR).is_some() {
        state = step(state, &drive, DT);
        run_ticks += 1;
        assert!(run_ticks <= ticks(STAR_S) + 2, "the run never ended");
    }
    assert!((ticks(STAR_S) - 1..=ticks(STAR_S) + 1).contains(&run_ticks));
}

#[test]
fn a_second_star_puts_the_timer_back_to_the_tuned_time_without_adding_and_kicks_again() {
    // Eight idle humans, each on a box under its slot, and a second box 2.5 m
    // ahead of the last slot: its kart rolls a star at Go, fires it as it
    // lands, is carried over the second box by the kick, still seventh, and
    // rolls another star, which it fires mid-run.
    let karts = sim::tuning::KARTS_PER_RACE;
    let slot = karts - 1;
    let [x, y, z] = common::slot_position(OVAL, slot);
    let boxes = || {
        track(&common::with_boxes_at(
            &with_boxes_under(&without_coins(&without_item_boxes(OVAL)), 0..karts),
            [[x, y, z - 2.5]],
        ))
    };
    let inputs = vec![idle(); karts];
    for seed in 0.. {
        let state = started(State::new(seed, karts, boxes()));
        let state = step(state, &inputs, DT);
        if !rolled(&state, slot, Item::Star) {
            continue;
        }
        let mut state = landed(state, karts, [slot]);
        let mut fire = inputs.clone();
        fire[slot] = tap();
        state = step(state, &fire, DT);
        assert!(star_s(&state, slot).is_some());
        let mut again = false;
        for _ in 0..ticks(STAR_KICK_S) {
            state = step(state, &inputs, DT);
            if rolled(&state, slot, Item::Star) {
                again = true;
                break;
            }
            if state
                .events()
                .iter()
                .any(|event| matches!(event, RaceEvent::ItemPicked { kart, .. } if *kart == slot))
            {
                break;
            }
        }
        if !again {
            continue;
        }
        state = landed(state, karts, [slot]);
        let before = star_s(&state, slot).expect("still on the first run");
        assert!(
            before < STAR_S - ITEM_ROLL_S,
            "{before} s left of the first run"
        );

        let state = step(state, &fire, DT);

        assert!(state.events().contains(&RaceEvent::ItemFired {
            kart: slot,
            item: Item::Star
        }));
        let kart = state.kart_readouts()[slot];
        let left = kart.star_s.expect("on a run");
        assert!(
            (left - (STAR_S - DT)).abs() < 1e-5,
            "back to the start, not added to: {left} s"
        );
        assert!((kart.boost_s.expect("the kick again") - (STAR_KICK_S - DT)).abs() < 1e-5);
        return;
    }
}

/// The first tick a kart driving with the throttle held loses speed to a
/// hit: more than a tick of acceleration could give back.
fn first_slowed(speeds: &[f32]) -> Option<usize> {
    speeds
        .windows(2)
        .position(|pair| pair[1] < pair[0] - 1.0)
        .map(|index| index + 1)
}

#[test]
fn a_star_kart_spins_out_the_kart_it_touches_and_passes_through_at_full_speed() {
    // The holder 10 m behind slot 1, idle at (32, 6), in its column; slot 3
    // at (32, 11) stands between them and is passed through first.
    const FIRST: usize = 3;
    const SECOND: usize = 1;
    let mut state = step(holding_a_star_at([32.0, 0.0, 16.0]), &holder(tap()), DT);
    let mut slowest_after_the_kick = f32::MAX;
    let mut hits = Vec::new();
    for _ in 0..ticks(2.5) {
        state = step(state, &holder(throttle()), DT);
        let kart = state.kart_readouts()[HOLDER];
        if kart.boost_s.is_none() {
            slowest_after_the_kick = slowest_after_the_kick.min(kart.speed);
        }
        hits.extend(spun_out(&state));
    }
    assert_eq!(
        hits,
        [
            RaceEvent::SpunOut {
                kart: FIRST,
                by: Some(HOLDER)
            },
            RaceEvent::SpunOut {
                kart: SECOND,
                by: Some(HOLDER)
            }
        ],
        "each kart in the column is spun out once, by the star kart"
    );
    assert!(
        (slowest_after_the_kick - STAR_TOP_SPEED).abs() < 1e-3,
        "the star kart was never slowed: {slowest_after_the_kick} m/s at the slowest"
    );
    assert_eq!(state.kart_readouts()[HOLDER].spin_out_s, None);
    let karts = state.kart_readouts();
    assert!(
        karts[HOLDER].position[2] < karts[SECOND].position[2] - 2.0 * KART_HALF_LENGTH_M,
        "passed through and out the other side: {:?} against {:?}",
        karts[HOLDER].position,
        karts[SECOND].position
    );
    // The karts hit stand where they were hit, bumped by nothing.
    for hit in [FIRST, SECOND] {
        assert_eq!(karts[hit].bump, None);
        assert!((karts[hit].position[0] - 32.0).abs() < 0.1);
    }
}

#[test]
fn a_kart_trailing_a_hazard_still_spins_out_and_the_hazard_is_left_where_it_is() {
    // Slot 1, second on the grid, rolls a hazard and trails it; the holder
    // 10 m behind it in its column drives through both.
    const VICTIM: usize = 1;
    let state = rolling(
        || {
            let sidecar = with_slot_at(
                &without_coins(&without_item_boxes(OVAL)),
                HOLDER,
                [32.0, 0.0, 16.0],
                0.0,
            );
            track(&with_boxes_under(&sidecar, [VICTIM, HOLDER]))
        },
        HOLDER + 1,
        0,
        &[(VICTIM, Item::Hazard), (HOLDER, Item::Star)],
    );
    let hold = KartInput {
        hold_item: true,
        ..idle()
    };
    let mut inputs = holder(tap());
    inputs[VICTIM] = hold;
    let mut state = step(state, &inputs, DT);
    assert!(state.hazards()[0].held);
    let mut inputs = holder(throttle());
    inputs[VICTIM] = hold;
    let mut hit = false;
    for _ in 0..ticks(2.5) {
        state = step(state, &inputs, DT);
        if state.events().contains(&RaceEvent::SpunOut {
            kart: VICTIM,
            by: Some(HOLDER),
        }) {
            hit = true;
            assert_eq!(
                state.kart_readouts()[VICTIM].spin_out_s,
                Some(sim::tuning::SPIN_OUT_S)
            );
            // The hit dropped the hazard where it trailed; the star kart,
            // on top of it, may then use it up as it does any dropped
            // hazard, to no effect.
            if let [hazard] = state.hazards()[..] {
                assert!(!hazard.held, "the spin-out ends the hold");
                assert_eq!(hazard.owner, VICTIM);
            }
        }
        if hit {
            assert!(
                state.hazards().iter().all(|hazard| !hazard.held),
                "the hazard trails nobody after the hit"
            );
        } else {
            // On the way to the kart the star kart passes the trailed
            // hazard without using it up.
            assert_eq!(state.hazards().len(), 1, "a held hazard is not used up");
        }
    }
    assert!(hit, "the trailed hazard shielded the kart");
    assert_eq!(state.kart_readouts()[HOLDER].spin_out_s, None);
}

#[test]
fn a_hazard_on_the_road_is_used_up_on_a_star_kart_to_no_effect() {
    // Slot 1 drops its hazard behind itself, in the holder's path.
    const DROPPER: usize = 1;
    let state = rolling(
        || {
            let sidecar = with_slot_at(
                &without_coins(&without_item_boxes(OVAL)),
                HOLDER,
                [32.0, 0.0, 16.0],
                0.0,
            );
            track(&with_boxes_under(&sidecar, [DROPPER, HOLDER]))
        },
        HOLDER + 1,
        0,
        &[(DROPPER, Item::Hazard), (HOLDER, Item::Star)],
    );
    let mut inputs = holder(tap());
    inputs[DROPPER] = tap();
    let mut state = step(state, &inputs, DT);
    assert_eq!(state.hazards().len(), 1);
    let mut gone_at = None;
    for tick in 0..ticks(2.0) {
        state = step(state, &holder(throttle()), DT);
        assert!(
            !spun_out(&state).contains(&RaceEvent::SpunOut {
                kart: HOLDER,
                by: Some(DROPPER)
            }),
            "the star kart is immune"
        );
        if state.hazards().is_empty() {
            gone_at = Some(tick);
            break;
        }
    }
    assert!(gone_at.is_some(), "the star kart used the hazard up");
    let kart = state.kart_readouts()[HOLDER];
    assert_eq!(kart.spin_out_s, None);
    assert!(kart.star_s.is_some());
    assert!(
        kart.speed > TOP_SPEED_M_PER_S,
        "drives on at {}",
        kart.speed
    );
}

#[test]
fn a_homing_projectile_is_used_up_on_a_star_kart_to_no_effect() {
    // Slot 6, seventh, holds a projectile; the holder, sixth, is the kart
    // next ahead of it and fires its star the tick before the projectile
    // flies.
    const FIRER: usize = 6;
    let humans = FIRER + 1;
    let state = rolling(
        || clean_oval_with_boxes_under([HOLDER, FIRER]),
        humans,
        0,
        &[(HOLDER, Item::Star), (FIRER, Item::Homing)],
    );
    let mut inputs = vec![idle(); humans];
    inputs[HOLDER] = tap();
    let mut state = step(state, &inputs, DT);
    inputs[HOLDER] = throttle();
    inputs[FIRER] = tap();
    state = step(state, &inputs, DT);
    let [projectile] = state.projectiles()[..] else {
        panic!("one projectile: {:?}", state.projectiles());
    };
    assert_eq!((projectile.owner, projectile.target), (FIRER, HOLDER));
    inputs[FIRER] = idle();
    let mut reached = false;
    for _ in 0..ticks(3.0) {
        state = step(state, &inputs, DT);
        assert!(
            !spun_out(&state)
                .iter()
                .any(|event| matches!(event, RaceEvent::SpunOut { kart, .. } if *kart == HOLDER)),
            "the star kart is immune"
        );
        if state.projectiles().is_empty() {
            reached = true;
            break;
        }
    }
    assert!(reached, "the projectile never reached the star kart");
    let kart = state.kart_readouts()[HOLDER];
    assert_eq!(kart.spin_out_s, None);
    assert!(kart.star_s.is_some());
    for _ in 0..ticks(0.5) {
        state = step(state, &inputs, DT);
        assert!(state.projectiles().is_empty(), "no new target");
    }
}

#[test]
fn two_star_karts_bump_as_usual_and_neither_spins_out() {
    // The holder and slot 6, 10 m behind it, in the clear lane; both roll stars.
    const BEHIND: usize = 6;
    let humans = BEHIND + 1;
    let state = rolling(
        || {
            let mut sidecar = without_coins(&without_item_boxes(OVAL));
            sidecar = with_slot_at(&sidecar, HOLDER, [26.0, 0.0, 16.0], 0.0);
            sidecar = with_slot_at(&sidecar, BEHIND, [26.0, 0.0, 26.0], 0.0);
            track(&with_boxes_under(&sidecar, [HOLDER, BEHIND]))
        },
        humans,
        0,
        &[(HOLDER, Item::Star), (BEHIND, Item::Star)],
    );
    let mut inputs = vec![idle(); humans];
    inputs[HOLDER] = tap();
    inputs[BEHIND] = tap();
    let mut state = step(state, &inputs, DT);
    inputs[HOLDER] = idle();
    inputs[BEHIND] = throttle();
    let mut speeds = Vec::new();
    let mut nudged = false;
    for _ in 0..ticks(2.0) {
        state = step(state, &inputs, DT);
        assert_eq!(spun_out(&state), [], "neither spins out");
        let karts = state.kart_readouts();
        speeds.push(karts[BEHIND].speed);
        nudged |= karts[HOLDER].velocity[2] < -0.5;
        assert!(
            karts[BEHIND].position[2] > karts[HOLDER].position[2] + 2.0 * KART_HALF_LENGTH_M - 0.2,
            "never through the star kart ahead"
        );
    }
    assert!(
        first_slowed(&speeds).is_some(),
        "the touch cost the driver speed"
    );
    assert!(nudged, "the kart ahead was bumped on up the column");
}

#[test]
fn the_runaway_wheel_passes_through_a_star_kart_and_rolls_on_to_hit_the_next() {
    // On Bell Deer: slot 0 parked well past the crossing, the holder in slot
    // 2 on the crossing's line by the left wall, where the first pass rolls
    // in, and slot 1 beside it, 2.5 m further from the wall. The holder
    // fires three seconds before the pass.
    use common::BELL_DEER;
    const STAR: usize = 2;
    const NEXT: usize = 1;
    let bell_deer = common::bell_deer();
    let crossing = bell_deer.wheel_crossings()[0];
    let (sin, cos) = crossing.yaw.sin_cos();
    let [x, y, z] = crossing.position;
    let on_crossing = |ahead: f32, right: f32| {
        [
            x - ahead * sin + right * cos,
            y,
            z - ahead * cos - right * sin,
        ]
    };
    let by_the_wall = -(crossing.half_width - 1.5);
    let state = rolling(
        || {
            let mut sidecar = without_coins(&without_item_boxes(BELL_DEER));
            sidecar = with_slot_at(&sidecar, 0, on_crossing(40.0, 0.0), crossing.yaw);
            sidecar = with_slot_at(
                &sidecar,
                NEXT,
                on_crossing(0.0, by_the_wall + 2.5),
                crossing.yaw,
            );
            sidecar = with_slot_at(&sidecar, STAR, on_crossing(0.0, by_the_wall), crossing.yaw);
            track(&with_boxes_under(&sidecar, [STAR]))
        },
        3,
        0,
        &[(STAR, Item::Star)],
    );
    let idle_all = [idle(), idle(), idle()];
    let mut state = state;
    loop {
        let Phase::Racing { elapsed_s, .. } = state.phase() else {
            panic!("{:?}", state.phase());
        };
        if elapsed_s >= WHEEL_FIRST_PASS_S - 3.0 {
            break;
        }
        state = step(state, &idle_all, DT);
    }
    state = step(state, &[idle(), idle(), tap()], DT);
    let mut hits = Vec::new();
    while star_s(&state, STAR).is_some() {
        state = step(state, &idle_all, DT);
        hits.extend(spun_out(&state));
        assert_eq!(state.kart_readouts()[STAR].spin_out_s, None, "immune");
    }
    assert_eq!(
        hits,
        [RaceEvent::SpunOut {
            kart: NEXT,
            by: None
        }],
        "the wheel rolled through the star kart and on into the next"
    );
}

#[test]
fn the_finish_ends_the_run() {
    // Three humans on an oval with no wheel: two parked ahead at the left
    // edge of the straight, so the holder stands third, and the holder
    // follows the centreline for the race's laps, firing its star 150 m
    // before the line on the last lap.
    const STAR: usize = 2;
    let build = || {
        let mut sidecar = without_coins(&without_wheel_crossings(&without_item_boxes(OVAL)));
        sidecar = with_slot_at(&sidecar, 0, [25.5, 0.0, -20.0], 0.0);
        sidecar = with_slot_at(&sidecar, 1, [25.5, 0.0, -24.0], 0.0);
        track(&with_boxes_under(&sidecar, [STAR]))
    };
    let mut state = rolling(build, 3, 0, &[(STAR, Item::Star)]);
    let lap_length = state.track().spline_length();
    let laps = sim::tuning::LAPS_PER_RACE;
    let mut fired = false;
    let mut starring_before_the_line = false;
    for _ in 0..ticks(200.0) {
        let standing = state.standings()[STAR];
        let to_the_line = lap_length
            - state
                .track()
                .lap_relative_progress(standing.checkpoint, standing.spline_progress);
        let mut input = follow_centreline(&state, STAR);
        if !fired && standing.lap == laps && to_the_line < 150.0 {
            input.use_item = true;
            fired = true;
        }
        state = step(state, &[idle(), idle(), input], DT);
        if fired && !state.kart_readouts()[STAR].item_slot.held().is_some() {
            starring_before_the_line |= star_s(&state, STAR).is_some();
        }
        if state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::Finished { kart, .. } if *kart == STAR))
        {
            assert!(starring_before_the_line, "reached the line on a run");
            assert_eq!(star_s(&state, STAR), None, "the finish ends the run");
            let state = step(state, &[idle(), idle(), idle()], DT);
            assert_eq!(star_s(&state, STAR), None);
            return;
        }
    }
    panic!("the kart never finished");
}

#[test]
fn a_bot_fires_a_star_within_the_tuned_delay_of_its_landing() {
    // Eight bots, the back three on boxes under their slots; one rolls a
    // star at Go and fires it on the straight.
    let karts = sim::tuning::KARTS_PER_RACE;
    for seed in 0.. {
        let state = started(State::with_bots(
            seed,
            0,
            karts,
            clean_oval_with_boxes_under(5..karts),
        ));
        let mut state = step(state, &[], DT);
        let Some(slot) = (5..karts).find(|&slot| rolled(&state, slot, Item::Star)) else {
            continue;
        };
        for tick in 0..ticks(ITEM_ROLL_S + BOT_ITEM_USE_DELAY_S) + 2 {
            state = step(state, &[], DT);
            if state.events().contains(&RaceEvent::ItemFired {
                kart: slot,
                item: Item::Star,
            }) {
                assert!(
                    tick >= ticks(ITEM_ROLL_S) - 1,
                    "fired before the roll landed"
                );
                assert!(star_s(&state, slot).is_some());
                return;
            }
        }
        panic!("seed {seed}: bot {slot} held its star past the delay");
    }
}

#[test]
fn the_prediction_passes_through_a_star_kart_the_snapshots_draw_and_adopts_the_snapshots_run() {
    use common::SEED;
    // The own kart at full throttle from slot 0; the snapshots draw a kart
    // on a run standing 10 m up its column, which the own kart meets as a
    // stand-in it passes through.
    let mut sidecar = with_slot_at(OVAL, 0, [28.0, 0.0, 4.0], 0.0);
    sidecar = with_slot_at(&sidecar, 1, [28.0, 0.0, -6.0], 0.0);
    let course = track(&sidecar);
    let mut prediction = Prediction::new(SEED, 0, course.clone());
    let mut standing = started(State::new(SEED, 2, course)).kart_readouts();
    standing[1].star_s = Some(5.0);
    let countdown = ticks(sim::tuning::COUNTDOWN_S) as u32;
    let mut sequence = 0;
    let mut slowest_once_going = f32::MAX;
    for _ in 0..countdown + ticks(2.0) as u32 {
        sequence += 1;
        let input = if sequence <= countdown {
            idle()
        } else {
            throttle()
        };
        prediction.tick(sequence, input, &standing);
        let kart = prediction.kart_readout();
        if kart.speed > 10.0 {
            slowest_once_going = slowest_once_going.min(kart.speed);
        }
    }
    let kart = prediction.kart_readout();
    assert!(
        kart.position[2] < standing[1].position[2] - 2.0 * KART_HALF_LENGTH_M,
        "through the star kart and out the other side: {:?}",
        kart.position
    );
    assert!(slowest_once_going > 10.0, "never slowed by a touch");

    // A snapshot that puts the own kart on a run is adopted on the rewind,
    // and the run runs down from there.
    let mut on_a_run = prediction.kart_readout();
    on_a_run.star_s = Some(4.0);
    prediction.reconcile(sequence, sequence, &on_a_run);
    let left = prediction
        .kart_readout()
        .star_s
        .expect("the run is adopted");
    assert!((left - 4.0).abs() < 1e-5, "{left} s left");
    for _ in 0..ticks(1.0) {
        sequence += 1;
        prediction.tick(sequence, throttle(), &standing);
    }
    let left = prediction.kart_readout().star_s.expect("still on the run");
    assert!((left - 3.0).abs() < 2.0 * DT, "{left} s left");
    assert!(
        prediction.kart_readout().speed > TOP_SPEED_M_PER_S + 1.0,
        "over the tuned top speed on the run: {} m/s",
        prediction.kart_readout().speed
    );
}

//! The Air Horn at the sim's public seam: fired, it lets off one blast the
//! tuned radius round the kart, within the tuned window of spline progress,
//! that spins out every ordinary kart in reach, uses up the hazards on the
//! road and the projectiles in flight within it and the hazard a kart in
//! reach trails in that kart's place, and leaves star karts, spun-out
//! karts, the firer's own trailed hazard, the wheel, the boxes and the
//! coins alone. Hits are read from `State::events` after the `step` that
//! produced them, and what the blast used up from `State::hazards` and
//! `State::projectiles`.

mod common;

use common::{
    rolling, tap, throttle, track, with_boxes_under, with_coins_at, with_slot_at, without_coins,
    without_item_boxes, DT, OVAL,
};

use sim::tuning::{COINS_LOST_PER_SPIN_OUT, HORN_RADIUS_M, SPIN_OUT_S};
use sim::{step, Item, ItemSlot, KartInput, RaceEvent, State, Track};

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

/// The oval's sidecar with no coins and no item boxes, its spawn slots
/// moved to `slots` facing -Z, and a box under each of `boxes`. The front
/// straight runs along -Z at x = 30 with asphalt 6 m either side; the start
/// line is at z = 0, so the grid at z = 4 to 21 stands a few metres short
/// of a lap.
fn oval_sidecar(slots: &[(usize, [f32; 3])], boxes: &[usize]) -> String {
    let mut sidecar = without_coins(&without_item_boxes(OVAL));
    for &(slot, at) in slots {
        sidecar = with_slot_at(&sidecar, slot, at, 0.0);
    }
    with_boxes_under(&sidecar, boxes.iter().copied())
}

/// The firer, first on the grid at (28, 4), holds a horn; the kart in slot
/// 1 stands 4.5 m from it, in reach, and the kart in slot 2 stands 9 m
/// behind it, out of reach.
const FIRER: usize = 0;
const NEAR: usize = 1;
const FAR: usize = 2;
const NEAR_AT: [f32; 3] = [32.0, 0.0, 6.0];
const FAR_AT: [f32; 3] = [28.0, 0.0, 13.0];

#[test]
fn a_blast_spins_out_the_karts_in_reach_and_costs_them_coins_and_leaves_the_rest() {
    // The near kart stands on three coins, taken at Go.
    let state = rolling(
        || {
            track(&with_coins_at(
                &oval_sidecar(&[(FAR, FAR_AT)], &[FIRER]),
                [NEAR_AT, NEAR_AT, NEAR_AT],
            ))
        },
        3,
        0,
        &[(FIRER, Item::Horn)],
    );
    assert_eq!(state.kart_readouts()[NEAR].coins, 3);

    let state = step(state, &[tap(), idle(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: NEAR,
                by: Some(FIRER)
            },
            RaceEvent::CoinsLost {
                kart: NEAR,
                count: COINS_LOST_PER_SPIN_OUT
            },
        ]
    );
    assert_eq!(state.kart_readouts()[FIRER].item_slot, ItemSlot::Empty);
    assert_eq!(state.kart_readouts()[FIRER].spin_out_s, None);
    // The blast is at the fire, before the tick's handling, so the hit kart
    // has stood a tick of its spin-out by the readout.
    let spin_out_s = state.kart_readouts()[NEAR].spin_out_s.expect("spun out");
    assert!(
        (spin_out_s - (SPIN_OUT_S - DT)).abs() < 1e-5,
        "{spin_out_s}"
    );
    assert_eq!(state.kart_readouts()[NEAR].coins, 0);
    assert_eq!(state.kart_readouts()[FAR].spin_out_s, None);
}

#[test]
fn a_kart_trailing_a_hazard_loses_the_hazard_in_its_place_and_drives_on() {
    let state = rolling(
        || track(&oval_sidecar(&[], &[FIRER, NEAR])),
        2,
        0,
        &[(FIRER, Item::Horn), (NEAR, Item::Hazard)],
    );
    let hold = KartInput {
        hold_item: true,
        ..idle()
    };
    let state = step(state, &[idle(), hold], DT);
    assert!(state.hazards()[0].held);

    let state = step(state, &[tap(), hold], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: FIRER,
            item: Item::Horn
        }]
    );
    assert!(state.hazards().is_empty(), "the trailed hazard is used up");
    assert_eq!(state.kart_readouts()[NEAR].spin_out_s, None);
}

#[test]
fn of_two_horns_on_one_tick_the_lower_slot_goes_first_and_a_kart_already_spun_out_is_unaffected() {
    // Slot 2 stands 6 m from the near kart and 8 m from the firer, in reach
    // of the one and not the other.
    const THIRD: usize = 2;
    const THIRD_AT: [f32; 3] = [32.0, 0.0, 12.0];
    let state = rolling(
        || track(&oval_sidecar(&[(THIRD, THIRD_AT)], &[FIRER, NEAR, THIRD])),
        3,
        0,
        &[(FIRER, Item::Horn), (NEAR, Item::Horn), (THIRD, Item::Horn)],
    );

    // The firer's blast spins the near kart out before its own press is
    // read, so the near kart's horn stays in its slot.
    let state = step(state, &[tap(), tap(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: NEAR,
                by: Some(FIRER)
            },
        ]
    );
    assert_eq!(
        state.kart_readouts()[NEAR].item_slot,
        ItemSlot::Held(Item::Horn)
    );
    let spin_out_before = state.kart_readouts()[NEAR].spin_out_s;

    // The third kart's blast a tick later reaches the near kart, already
    // spun out, which it neither spins out again nor restarts, and not the
    // firer; the near kart's press is still dead.
    let state = step(state, &[idle(), tap(), tap()], DT);
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: THIRD,
            item: Item::Horn
        }]
    );
    assert_eq!(spun_out(&state), []);
    assert!(state.kart_readouts()[NEAR].spin_out_s < spin_out_before);
    assert_eq!(state.kart_readouts()[FIRER].spin_out_s, None);
    assert_eq!(
        state.kart_readouts()[NEAR].item_slot,
        ItemSlot::Held(Item::Horn)
    );
}

/// The firer, holding a horn, driving up the straight at a kart on a
/// star's run trailing a hazard, and the input that drives it: the state
/// and the input a tick before the firer is in reach. The star kart,
/// third on the grid where a star can roll, rolls a hazard at Go and
/// trails it from the landing; its box comes back under it and rolls
/// again, and a seed whose second roll is a star is kept. It fires the
/// star with the hazard still trailing, and the kick carries it down the
/// straight, so the firer sets off after it along the centreline.
fn closing_on_a_star_kart_trailing_a_hazard() -> (State, KartInput) {
    const STAR: usize = 2;
    let star = |use_item: bool| KartInput {
        use_item,
        hold_item: true,
        ..idle()
    };
    for seed in 0.. {
        let state = common::started(State::new(
            seed,
            3,
            track(&oval_sidecar(&[], &[FIRER, STAR])),
        ));
        let mut state = step(state, &[idle(); 3], DT);
        if !(common::rolled(&state, FIRER, Item::Horn)
            && common::rolled(&state, STAR, Item::Hazard))
        {
            continue;
        }
        state = common::landed(state, 3, [FIRER, STAR]);
        state = step(state, &[idle(), idle(), star(true)], DT);
        assert!(state.hazards()[0].held);
        while state.kart_readouts()[STAR].item_slot.held().is_none() {
            state = step(state, &[idle(), idle(), star(false)], DT);
        }
        if state.kart_readouts()[STAR].item_slot != ItemSlot::Held(Item::Star) {
            continue;
        }
        state = step(state, &[idle(), idle(), star(true)], DT);
        assert!(state.kart_readouts()[STAR].star_s.is_some());
        // The kick carries the star kart clear of the grid before the
        // firer sets off, so the near kart is out of the blast's reach.
        for _ in 0..common::ticks(1.0) {
            state = step(state, &[idle(), idle(), star(false)], DT);
        }
        loop {
            let input = common::follow_centreline(&state, FIRER);
            let [firer, star_kart] = [FIRER, STAR].map(|kart| state.kart_readouts()[kart].position);
            let gap =
                ((firer[0] - star_kart[0]).powi(2) + (firer[2] - star_kart[2]).powi(2)).sqrt();
            if gap < HORN_RADIUS_M - 2.0 {
                assert!(
                    state.kart_readouts()[STAR].star_s.is_some(),
                    "the run ran out"
                );
                assert!(state.hazards()[0].held, "the hazard still trails");
                return (state, input);
            }
            state = step(state, &[input, idle(), star(false)], DT);
        }
    }
    unreachable!()
}

#[test]
fn a_star_kart_is_unaffected_and_the_hazard_it_trails_stays() {
    const STAR: usize = 2;
    let hold = KartInput {
        hold_item: true,
        ..idle()
    };
    let (state, driving) = closing_on_a_star_kart_trailing_a_hazard();
    let state = step(
        state,
        &[
            KartInput {
                use_item: true,
                ..driving
            },
            idle(),
            hold,
        ],
        DT,
    );
    assert_eq!(
        state.events(),
        &[RaceEvent::ItemFired {
            kart: FIRER,
            item: Item::Horn
        }]
    );
    assert_eq!(state.kart_readouts()[STAR].spin_out_s, None);
    assert!(state.kart_readouts()[STAR].star_s.is_some());
    let [hazard] = state.hazards()[..] else {
        panic!("the trailed hazard stays: {:?}", state.hazards());
    };
    assert!(hazard.held);
    assert_eq!(hazard.owner, STAR);
}

/// The firer with a hazard fired and a horn landed in its place: it rolls
/// a hazard at Go and fires it as it lands, tapped to drop it 1 m behind
/// or held to trail it there; its box comes back under it and rolls again,
/// and a seed whose second roll is a horn is kept. The near kart drops a
/// hazard of its own at Go, 1 m behind itself and in reach, and the far
/// kart one 10 m from the firer, out of reach.
fn hazards_on_the_road(firer_holds: bool) -> State {
    let firer = |use_item: bool| KartInput {
        use_item,
        hold_item: firer_holds,
        ..idle()
    };
    for seed in 0.. {
        let state = common::started(State::new(
            seed,
            3,
            track(&oval_sidecar(&[(FAR, FAR_AT)], &[FIRER, NEAR, FAR])),
        ));
        let mut state = step(state, &[idle(); 3], DT);
        if ![FIRER, NEAR, FAR]
            .iter()
            .all(|&kart| common::rolled(&state, kart, Item::Hazard))
        {
            continue;
        }
        state = common::landed(state, 3, [FIRER, NEAR, FAR]);
        state = step(state, &[firer(true), tap(), tap()], DT);
        assert_eq!(state.hazards().len(), 3);
        while state.kart_readouts()[FIRER].item_slot.held().is_none() {
            state = step(state, &[firer(false), idle(), idle()], DT);
        }
        if state.kart_readouts()[FIRER].item_slot != ItemSlot::Held(Item::Horn) {
            continue;
        }
        assert_eq!(
            state.hazards().len(),
            3,
            "all three hazards still lie there"
        );
        assert_eq!(state.hazards()[0].held, firer_holds);
        return state;
    }
    unreachable!()
}

#[test]
fn a_hazard_on_the_road_in_reach_is_used_up_whoever_dropped_it_the_firers_own_included() {
    let state = step(hazards_on_the_road(false), &[tap(), idle(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: NEAR,
                by: Some(FIRER)
            },
        ]
    );
    let [left] = state.hazards()[..] else {
        panic!("the far kart's hazard alone is left: {:?}", state.hazards());
    };
    assert_eq!(left.owner, FAR);
}

#[test]
fn the_firers_own_trailed_hazard_stays() {
    let hold = KartInput {
        use_item: true,
        hold_item: true,
        ..idle()
    };
    let state = step(hazards_on_the_road(true), &[hold, idle(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: NEAR,
                by: Some(FIRER)
            },
        ]
    );
    let owners: Vec<(usize, bool)> = state
        .hazards()
        .iter()
        .map(|hazard| (hazard.owner, hazard.held))
        .collect();
    assert_eq!(owners, [(FIRER, true), (FAR, false)]);
}

#[test]
fn a_homing_projectile_in_reach_is_used_up_whoever_it_chases() {
    // The target stands first, out of the firer's reach; the shooter and
    // the firer stand level, so the shooter, in the lower slot, is second
    // and its projectile chases the target past the firer 4 m beside it.
    const TARGET: usize = 0;
    const SHOOTER: usize = 1;
    const HOLDER: usize = 2;
    let state = rolling(
        || {
            track(&oval_sidecar(
                &[
                    (TARGET, [33.0, 0.0, 2.0]),
                    (SHOOTER, [32.0, 0.0, 10.0]),
                    (HOLDER, [28.0, 0.0, 10.0]),
                ],
                &[SHOOTER, HOLDER],
            ))
        },
        3,
        0,
        &[(SHOOTER, Item::Homing), (HOLDER, Item::Horn)],
    );
    let state = step(state, &[idle(), tap(), idle()], DT);
    let [projectile] = state.projectiles()[..] else {
        panic!("one projectile: {:?}", state.projectiles());
    };
    assert_eq!((projectile.owner, projectile.target), (SHOOTER, TARGET));

    let state = step(state, &[idle(), idle(), tap()], DT);
    assert!(state.projectiles().is_empty(), "the projectile is used up");
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: HOLDER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: SHOOTER,
                by: Some(HOLDER)
            },
        ]
    );
    let mut state = state;
    for _ in 0..common::ticks(1.0) {
        state = step(state, &[idle(); 3], DT);
        assert_eq!(state.kart_readouts()[TARGET].spin_out_s, None);
    }
}

#[test]
fn a_blast_with_nothing_in_reach_still_fires_and_leaves_the_boxes_coins_and_wheel_alone() {
    // A lone firer on Bell Deer, with two coins beside it and its box back
    // under it: a race pressed at the landing against one left idle.
    let race = || {
        let [x, y, z] = common::slot_position(common::BELL_DEER, FIRER);
        rolling(
            || {
                track(&with_coins_at(
                    &with_boxes_under(
                        &without_coins(&without_item_boxes(common::BELL_DEER)),
                        [FIRER],
                    ),
                    [[x + 3.0, y, z], [x - 3.0, y, z]],
                ))
            },
            1,
            0,
            &[(FIRER, Item::Horn)],
        )
    };
    let settle = |mut state: State, input: KartInput| {
        for _ in 0..common::ticks(1.0) {
            state = step(state, &[idle()], DT);
        }
        step(state, &[input], DT)
    };
    let pressed = settle(race(), tap());
    let idle_race = settle(race(), idle());
    assert_eq!(
        pressed.events(),
        &[RaceEvent::ItemFired {
            kart: FIRER,
            item: Item::Horn
        }]
    );
    assert_eq!(pressed.kart_readouts()[FIRER].item_slot, ItemSlot::Empty);
    assert_eq!(pressed.item_boxes_present(), idle_race.item_boxes_present());
    assert_eq!(pressed.coins_present(), idle_race.coins_present());
    assert_eq!(pressed.coins_present(), [true, true]);
    assert_eq!(pressed.wheels(), idle_race.wheels());
}

/// The spline progress of the top of Bell Deer's bridge, where one road
/// crosses 5 m over the other.
fn bridge_top(track: &Track) -> f32 {
    let steps = (track.spline_length() * 10.0) as usize;
    (0..steps)
        .map(|step| step as f32 / 10.0)
        .max_by(|&a, &b| track.point_at(a)[1].total_cmp(&track.point_at(b)[1]))
        .expect("a lap has length")
}

/// The spline progress of the point on the other road nearest in plan to
/// the point at `progress`: the road at least 50 m round the lap from it.
fn other_road_under(track: &Track, progress: f32) -> f32 {
    let lap = track.spline_length();
    let here = track.point_at(progress);
    let plan = |at: f32| {
        let there = track.point_at(at);
        ((there[0] - here[0]).powi(2) + (there[2] - here[2]).powi(2)).sqrt()
    };
    let steps = (lap * 10.0) as usize;
    (0..steps)
        .map(|step| step as f32 / 10.0)
        .filter(|&at| {
            let gap = (at - progress).rem_euclid(lap);
            gap.min(lap - gap) > 50.0
        })
        .min_by(|&a, &b| plan(a).total_cmp(&plan(b)))
        .expect("the other road passes under")
}

/// A spawn slot on the centreline at `progress`, facing along the lap.
fn on_the_centreline(track: &Track, progress: f32) -> ([f32; 3], f32) {
    let [x, z] = track.direction_at(progress);
    (track.point_at(progress), (-x).atan2(-z))
}

#[test]
fn a_blast_does_not_reach_a_kart_on_bell_deers_other_road_but_does_reach_one_along_its_own() {
    // The roads cross at right angles at the bridge's top. The firer
    // stands on the bridge 3 m short of the top, with one kart on the road
    // under at the crossing, 3 m away in plan, and another 5 m further
    // along the bridge.
    const UNDER: usize = 1;
    const BEHIND: usize = 2;
    let bell_deer = common::bell_deer();
    let top = bridge_top(&bell_deer);
    let firer_progress = top - 3.0;
    let under_progress = other_road_under(&bell_deer, firer_progress);
    let state = rolling(
        || {
            let mut sidecar = without_coins(&without_item_boxes(common::BELL_DEER));
            for (slot, progress) in [
                (FIRER, firer_progress),
                (UNDER, under_progress),
                (BEHIND, firer_progress + 5.0),
            ] {
                let (position, yaw) = on_the_centreline(&bell_deer, progress);
                sidecar = with_slot_at(&sidecar, slot, position, yaw);
            }
            track(&with_boxes_under(&sidecar, [FIRER]))
        },
        3,
        0,
        &[(FIRER, Item::Horn)],
    );
    let [firer, under, behind] = [FIRER, UNDER, BEHIND].map(|kart| state.kart_readouts()[kart]);
    let plan = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    assert!(
        plan(firer.position, under.position) < 4.0,
        "the kart under is in reach by distance"
    );
    assert!(
        firer.position[1] - under.position[1] > 4.0,
        "the bridge is 5 m up"
    );
    assert!(plan(firer.position, behind.position) < 6.0);
    let lap = bell_deer.spline_length();
    let gap = (firer.spline_progress - under.spline_progress).rem_euclid(lap);
    assert!(
        gap.min(lap - gap) > 50.0,
        "the two roads are far apart round the lap"
    );

    let state = step(state, &[tap(), idle(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: BEHIND,
                by: Some(FIRER)
            },
        ]
    );
    assert_eq!(state.kart_readouts()[UNDER].spin_out_s, None);
}

#[test]
fn the_progress_window_wraps_at_the_start_line() {
    // The firer stands 3 m short of the line and the near kart 3 m past it.
    let state = rolling(
        || {
            track(&oval_sidecar(
                &[(FIRER, [28.0, 0.0, 3.0]), (NEAR, [32.0, 0.0, -3.0])],
                &[FIRER],
            ))
        },
        2,
        0,
        &[(FIRER, Item::Horn)],
    );
    let lap = state.track().spline_length();
    let [firer, near] = [FIRER, NEAR].map(|kart| state.kart_readouts()[kart].spline_progress);
    assert!(firer > lap - 5.0, "the firer reads nearly a lap: {firer}");
    assert!(near < 5.0, "the near kart reads just past the line: {near}");

    let state = step(state, &[tap(), idle()], DT);
    assert_eq!(
        state.events(),
        &[
            RaceEvent::ItemFired {
                kart: FIRER,
                item: Item::Horn
            },
            RaceEvent::SpunOut {
                kart: NEAR,
                by: Some(FIRER)
            },
        ]
    );
}

/// The ticks at which `kart` picked up `item` and fired it, in a race
/// stepped with `inputs` for up to `seconds`.
fn picked_and_fired(
    mut state: State,
    inputs: &[KartInput],
    kart: usize,
    item: Item,
    seconds: f32,
) -> Option<(usize, usize, State)> {
    let mut picked = None;
    for tick in 0..common::ticks(seconds) {
        state = step(state, inputs, DT);
        if common::rolled(&state, kart, item) {
            picked = Some(tick);
        }
        if state
            .events()
            .contains(&RaceEvent::ItemFired { kart, item })
        {
            return picked.map(|picked| (picked, tick, state));
        }
    }
    None
}

#[test]
fn a_bot_fires_its_horn_as_soon_as_a_kart_is_in_reach() {
    use sim::tuning::ITEM_ROLL_S;
    // A human parked on the bot's line down the front straight; the bot
    // rolls at Go from the box under its slot and drives at the human.
    const HUMAN: usize = 0;
    const BOT: usize = 1;
    const PARKED: [f32; 3] = [32.5, 0.0, -45.0];
    for seed in 0.. {
        let state = common::started(State::with_bots(
            seed,
            1,
            1,
            track(&oval_sidecar(&[(HUMAN, PARKED)], &[BOT])),
        ));
        let Some((picked, fired, state)) = picked_and_fired(state, &[idle()], BOT, Item::Horn, 8.0)
        else {
            continue;
        };
        assert_eq!(picked, 0, "rolled at Go");
        assert!(
            state.events().contains(&RaceEvent::SpunOut {
                kart: HUMAN,
                by: Some(BOT)
            }),
            "fired with the human in reach: {:?}",
            state.events()
        );
        // Held past the landing, on the straight the first-straight rule
        // would have fired it on at once, until the human was in reach.
        assert!(
            fired > common::ticks(ITEM_ROLL_S) + 2,
            "fired at tick {fired}"
        );
        return;
    }
}

#[test]
fn a_bot_fires_its_horn_after_the_tuned_hold_with_no_kart_in_reach() {
    use sim::tuning::{BOT_HORN_HOLD_S, ITEM_ROLL_S};
    for seed in 0.. {
        let state = common::started(State::with_bots(
            seed,
            0,
            1,
            track(&oval_sidecar(&[], &[FIRER])),
        ));
        let Some((picked, fired, _)) = picked_and_fired(
            state,
            &[],
            FIRER,
            Item::Horn,
            ITEM_ROLL_S + BOT_HORN_HOLD_S + 1.0,
        ) else {
            continue;
        };
        assert_eq!(picked, 0, "rolled at Go");
        let expected = common::ticks(ITEM_ROLL_S + BOT_HORN_HOLD_S);
        assert!(
            (expected - 1..=expected + 2).contains(&fired),
            "fired {fired} ticks after the pickup, expected about {expected}"
        );
        return;
    }
}

#[test]
fn a_hazard_dropped_after_a_long_trail_is_judged_by_where_it_was_dropped() {
    // The near kart rolls a hazard 30 m behind the firer, trails it up the
    // straight past the firer and drops it 4 m beyond; the hazard's spline
    // progress is the drop's, not the fire's, so the blast reaches it.
    const START: [f32; 3] = [32.0, 0.0, 34.0];
    let state = rolling(
        || track(&oval_sidecar(&[(NEAR, START)], &[FIRER, NEAR])),
        2,
        0,
        &[(FIRER, Item::Horn), (NEAR, Item::Hazard)],
    );
    let trailing = KartInput {
        throttle: 1.0,
        hold_item: true,
        ..idle()
    };
    let mut state = step(
        state,
        &[
            idle(),
            KartInput {
                use_item: true,
                ..trailing
            },
        ],
        DT,
    );
    let firer_z = state.kart_readouts()[FIRER].position[2];
    while state.kart_readouts()[NEAR].position[2] > firer_z - 5.0 {
        state = step(state, &[idle(), trailing], DT);
        assert!(state.hazards()[0].held, "the hazard trails up the straight");
        assert_eq!(spun_out(&state), [], "the firer is not in the hazard's way");
    }
    let state = step(state, &[idle(), throttle()], DT);
    assert!(!state.hazards()[0].held, "dropped");

    let state = step(state, &[tap(), throttle()], DT);
    assert!(
        state.hazards().is_empty(),
        "the dropped hazard is in reach: {:?}",
        state.hazards()
    );
    assert!(state.events().contains(&RaceEvent::SpunOut {
        kart: NEAR,
        by: Some(FIRER)
    }));
}

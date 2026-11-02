//! The slipstream at the sim's public seam: a kart that holds another's
//! wake for the tuned charge is towed to a raised top speed cap for as long
//! as it stays there and for the tuned time after it pulls out. The wake
//! and the tow are read from `slipstream` on the kart readout after the
//! `step` that judged them.

mod common;

use common::{
    rolling, started, throttle, ticks, track, with_boxes_under, with_coins_at, with_slot_at,
    without_coins, without_item_boxes, without_walls, DT, OVAL, SEED,
};

use sim::tuning::{
    BOOST_PAD_FACTOR, BOOST_PAD_S, HOP_AIRTIME_S, MINI_TURBO_BOOST_FACTOR, MINI_TURBO_RED_BOOST_S,
    OFF_TRACK_SPEED_FACTOR, SLIPSTREAM_CHARGE_S, SLIPSTREAM_HALF_WIDTH_M,
    SLIPSTREAM_LEADER_SPEED_FRACTION, SLIPSTREAM_RANGE_M, SLIPSTREAM_TOP_SPEED_FACTOR,
    SLIPSTREAM_TOW_S, TOP_SPEED_M_PER_S,
};
use sim::{step, Item, KartInput, MiniTurbo, RaceEvent, Slipstream, State, Surface};

/// The towed cap with no coins and no rubber band.
const TOWED_TOP_SPEED: f32 = TOP_SPEED_M_PER_S * SLIPSTREAM_TOP_SPEED_FACTOR;

const LEADER: usize = 0;
const FOLLOWER: usize = 1;

/// Facing +X, across the oval's front straight.
const FACING_PLUS_X: f32 = -std::f32::consts::FRAC_PI_2;

fn idle() -> KartInput {
    KartInput::default()
}

fn brake() -> KartInput {
    KartInput {
        brake: 1.0,
        ..idle()
    }
}

/// Full throttle steering `steer`.
fn steering(steer: f32) -> KartInput {
    KartInput {
        steer,
        ..throttle()
    }
}

/// The oval's front straight runs along -Z at x = 30 from the line to
/// z = -100 with asphalt 6 m either side, a boost pad on its right half at
/// z = -60 and a tyre wall at x = 28 by the turn's entry. The lane at
/// x = 25 meets neither. No coins and no boxes, so nothing but the wake
/// changes a kart's speed.
const LANE_X: f32 = 25.0;

/// The clean oval with the first `at.len()` spawn slots moved to `at`,
/// each facing down the straight.
fn clean_oval_with_slots_at(at: &[[f32; 3]]) -> String {
    let mut sidecar = without_coins(&without_item_boxes(OVAL));
    for (slot, &position) in at.iter().enumerate() {
        sidecar = with_slot_at(&sidecar, slot, position, 0.0);
    }
    sidecar
}

/// Two humans on the straight, the leader at z = 4 and the follower
/// `gap_m` straight behind it, past the countdown.
fn nose_to_tail(gap_m: f32) -> State {
    let sidecar = clean_oval_with_slots_at(&[[LANE_X, 0.0, 4.0], [LANE_X, 0.0, 4.0 + gap_m]]);
    started(State::new(SEED, 2, track(&sidecar)))
}

fn slipstream(state: &State, kart: usize) -> Option<Slipstream> {
    state.kart_readouts()[kart].slipstream
}

fn speed(state: &State, kart: usize) -> f32 {
    state.kart_readouts()[kart].speed
}

/// Whether `kart` is towed, at any time left.
fn towed(state: &State, kart: usize) -> bool {
    matches!(slipstream(state, kart), Some(Slipstream::Tow { .. }))
}

/// Ticks a tow armed at the tuned time lasts out of the wake, run down a
/// tick at a time in f32 as the handling runs it.
fn tow_ticks() -> usize {
    let mut left = SLIPSTREAM_TOW_S;
    let mut count = 0;
    while left > 0.0 {
        left -= DT;
        count += 1;
    }
    count
}

/// Both karts at full throttle down the straight until the follower is
/// towed; fails the test when it never is.
fn towed_down_the_straight(mut state: State) -> State {
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle(), throttle()], DT);
        if towed(&state, FOLLOWER) {
            return state;
        }
    }
    panic!("never towed");
}

/// [`towed_down_the_straight`], then both straight on until the follower
/// has drawn up to the towed cap; fails the test when it never does.
fn at_the_towed_cap(state: State) -> State {
    let mut state = towed_down_the_straight(state);
    for _ in 0..ticks(1.5) {
        if speed(&state, FOLLOWER) == TOWED_TOP_SPEED {
            return state;
        }
        state = step(state, &[throttle(), throttle()], DT);
    }
    panic!(
        "never reached the towed cap: {} m/s",
        speed(&state, FOLLOWER)
    );
}

#[test]
fn a_kart_a_second_in_a_wake_is_towed_to_the_raised_cap_and_only_the_one_behind_is_in_a_wake() {
    // Both at full throttle from the grid, the follower 6 m behind: the
    // same speed every tick, so the gap holds until the tow takes.
    let mut state = nose_to_tail(6.0);
    let mut charged_for = 0;
    loop {
        state = step(state, &[throttle(), throttle()], DT);
        match slipstream(&state, FOLLOWER) {
            None => assert_eq!(charged_for, 0, "the charge never drops in the wake"),
            Some(Slipstream::Charging { s }) => {
                charged_for += 1;
                assert!((s - charged_for as f32 * DT).abs() < 1e-5, "{s} s charged");
            }
            Some(Slipstream::Tow { .. }) => break,
        }
        assert!(charged_for <= ticks(SLIPSTREAM_CHARGE_S), "never towed");
        assert_eq!(slipstream(&state, LEADER), None, "nobody tows the leader");
    }
    assert_eq!(charged_for, ticks(SLIPSTREAM_CHARGE_S) - 1);
    assert_eq!(slipstream(&state, LEADER), None, "nobody tows the leader");

    // Towed, the follower draws up to the raised cap and holds it while
    // the leader holds the tuned top speed; the tow stays at its full time
    // while the follower stays in the wake.
    let mut fastest = 0.0_f32;
    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle(), throttle()], DT);
        let follower = state.kart_readouts()[FOLLOWER];
        fastest = fastest.max(follower.speed);
        assert!(follower.speed <= TOWED_TOP_SPEED);
        assert_eq!(
            follower.slipstream,
            Some(Slipstream::Tow {
                left_s: SLIPSTREAM_TOW_S
            })
        );
    }
    assert_eq!(fastest, TOWED_TOP_SPEED);
    assert_eq!(speed(&state, LEADER), TOP_SPEED_M_PER_S);
}

#[test]
fn a_tick_short_of_the_charge_then_a_tick_out_of_the_wake_gives_nothing() {
    // The follower holds a horn, rolled at Go from a box under its slot
    // while both karts sit on the grid; both set off once it has landed.
    // A tick short of the charge the follower fires: the blast stops the
    // leader before the tick's wakes are judged, so that tick is the one
    // out of the wake.
    let build = || {
        let sidecar = clean_oval_with_slots_at(&[[LANE_X, 0.0, 4.0], [LANE_X, 0.0, 10.0]]);
        track(&with_boxes_under(&sidecar, [FOLLOWER]))
    };
    let mut state = rolling(build, 2, 0, &[(FOLLOWER, Item::Horn)]);
    let short = ticks(SLIPSTREAM_CHARGE_S) - 1;
    loop {
        state = step(state, &[throttle(), throttle()], DT);
        if let Some(Slipstream::Charging { s }) = slipstream(&state, FOLLOWER) {
            if (s - short as f32 * DT).abs() < 1e-5 {
                break;
            }
        }
        assert!(!towed(&state, FOLLOWER), "towed before the charge");
    }
    let fire = KartInput {
        use_item: true,
        ..throttle()
    };
    state = step(state, &[throttle(), fire], DT);
    assert!(state.events().contains(&RaceEvent::SpunOut {
        kart: LEADER,
        by: Some(FOLLOWER)
    }));
    assert_eq!(slipstream(&state, FOLLOWER), None);

    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle(), brake()], DT);
        assert_eq!(slipstream(&state, FOLLOWER), None);
    }
}

#[test]
fn the_tow_holds_for_the_tuned_time_after_pulling_out_and_then_settles() {
    // Towed at the cap, the follower pulls right a lane and straightens,
    // passing the leader beside it.
    let mut state = at_the_towed_cap(nose_to_tail(6.0));
    let mut pulled_out = None;
    let mut tick = 0;
    // Six ticks of lock right, a straight run across and six back.
    let steers = std::iter::repeat_n(1.0, 6)
        .chain(std::iter::repeat_n(0.0, 24))
        .chain(std::iter::repeat_n(-1.0, 6));
    for steer in steers {
        state = step(state, &[throttle(), steering(steer)], DT);
        tick += 1;
        if pulled_out.is_none() {
            if let Some(Slipstream::Tow { left_s }) = slipstream(&state, FOLLOWER) {
                if left_s < SLIPSTREAM_TOW_S {
                    pulled_out = Some(tick);
                }
            }
        }
    }
    let pulled_out = pulled_out.expect("left the wake");
    let follower = state.kart_readouts()[FOLLOWER];
    let leader = state.kart_readouts()[LEADER];
    assert!(
        follower.position[0] - leader.position[0] > SLIPSTREAM_HALF_WIDTH_M,
        "a lane across: {} m",
        follower.position[0] - leader.position[0]
    );

    // The cap holds through the tow's run-down and gives way the tick the
    // tow runs out: the speed drops to the tuned top speed at once.
    let mut towed_ticks = tick - pulled_out + 1;
    while towed(&state, FOLLOWER) {
        state = step(state, &[throttle(), throttle()], DT);
        towed_ticks += 1;
        assert!(towed_ticks <= tow_ticks() + 1, "the tow never ran out");
        if towed(&state, FOLLOWER) {
            assert_eq!(speed(&state, FOLLOWER), TOWED_TOP_SPEED);
        }
    }
    assert_eq!(towed_ticks, tow_ticks());
    assert_eq!(slipstream(&state, FOLLOWER), None);
    // The tow's last tick still holds the cap; the plain one takes over on
    // the next.
    state = step(state, &[throttle(), throttle()], DT);
    assert_eq!(speed(&state, FOLLOWER), TOP_SPEED_M_PER_S);
}

#[test]
fn a_leader_below_the_speed_fraction_tows_nobody() {
    // An idle leader has no wake: the follower rolls up behind it charging
    // nothing. Brakes behind it before touching it.
    let mut state = nose_to_tail(6.0);
    for _ in 0..ticks(0.6) {
        state = step(state, &[idle(), throttle()], DT);
        assert_eq!(slipstream(&state, FOLLOWER), None);
    }
    assert!(speed(&state, FOLLOWER) > 5.0);

    // A towed follower whose leader brakes below the fraction is out of
    // the wake from that tick: its tow runs down from there.
    let mut state = towed_down_the_straight(nose_to_tail(6.0));
    let fraction = SLIPSTREAM_LEADER_SPEED_FRACTION * TOP_SPEED_M_PER_S;
    let mut leader_was_at = speed(&state, LEADER);
    let mut left_the_wake = None;
    for _ in 0..ticks(1.0) {
        state = step(state, &[brake(), brake()], DT);
        let leader_is_at = speed(&state, LEADER);
        match slipstream(&state, FOLLOWER) {
            Some(Slipstream::Tow { left_s }) if left_s < SLIPSTREAM_TOW_S => {
                left_the_wake = Some((leader_was_at, leader_is_at));
                break;
            }
            Some(Slipstream::Tow { .. }) => {}
            other => panic!("{other:?} under the brake"),
        }
        leader_was_at = leader_is_at;
    }
    let (was_at, is_at) = left_the_wake.expect("the leader never slowed under the fraction");
    // The wake is judged from the leader's speed as the tick begins.
    assert!(
        was_at < fraction && is_at < was_at,
        "{was_at} then {is_at} m/s"
    );
}

#[test]
fn a_kart_beside_ahead_of_or_crossing_the_leaders_travel_is_not_in_the_wake() {
    // Beside: the grid's two columns, 4 m apart, neither in the other's
    // wake however fast they go.
    let mut state = started(State::new(
        SEED,
        2,
        track(&without_coins(&without_item_boxes(OVAL))),
    ));
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle(), throttle()], DT);
        assert_eq!(slipstream(&state, 0), None);
        assert_eq!(slipstream(&state, 1), None);
    }
    assert_eq!(speed(&state, 0), TOP_SPEED_M_PER_S);

    // Crossing: a kart set off diagonally from the straight's right edge
    // so that its line crosses the leader's some 9 m behind it as the
    // leader passes the fraction, well within the wake's reach and width,
    // but at more than the tuned angle to the leader's travel.
    let angle: f32 = 0.6;
    let crosses_after_m = 9.4;
    let crosser = [
        LANE_X + crosses_after_m * angle.sin(),
        0.0,
        4.0 + 6.0 + crosses_after_m * (1.0 - angle.cos()),
    ];
    let mut sidecar = clean_oval_with_slots_at(&[[LANE_X, 0.0, 4.0]]);
    sidecar = with_slot_at(&sidecar, FOLLOWER, crosser, angle);
    let mut state = started(State::new(SEED, 2, track(&sidecar)));
    let mut in_reach_while_fast = false;
    for _ in 0..ticks(2.0) {
        state = step(state, &[throttle(), throttle()], DT);
        let [leader, crosser] = [
            state.kart_readouts()[LEADER],
            state.kart_readouts()[FOLLOWER],
        ];
        let behind = crosser.position[2] - leader.position[2];
        let across = (crosser.position[0] - leader.position[0]).abs();
        in_reach_while_fast |= leader.speed > SLIPSTREAM_LEADER_SPEED_FRACTION * TOP_SPEED_M_PER_S
            && (0.0..=SLIPSTREAM_RANGE_M).contains(&behind)
            && across < SLIPSTREAM_HALF_WIDTH_M;
        assert_eq!(
            slipstream(&state, FOLLOWER),
            None,
            "crossing at {angle} rad"
        );
    }
    assert!(
        in_reach_while_fast,
        "the crosser never reached the wake's box"
    );
}

#[test]
fn off_the_asphalt_the_off_track_cap_wins_while_the_charge_and_the_tow_keep_counting() {
    // The leader on the asphalt's left edge, x = 24, the follower a metre
    // further out on the sand inside the oval, 6 m behind: in the wake but
    // held to the off-track cap, which the sand and the grass share.
    let sidecar = clean_oval_with_slots_at(&[[24.5, 0.0, 4.0], [23.5, 0.0, 10.0]]);
    let mut state = started(State::new(SEED, 2, track(&sidecar)));
    let off_track_cap = OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S;
    let mut towed_off_the_asphalt_for = 0;
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle(), throttle()], DT);
        assert_eq!(state.kart_surface(LEADER), Surface::Asphalt);
        assert_eq!(state.kart_surface(FOLLOWER), Surface::Sand);
        assert!(speed(&state, FOLLOWER) <= off_track_cap);
        if towed(&state, FOLLOWER) {
            towed_off_the_asphalt_for += 1;
            assert_eq!(speed(&state, FOLLOWER), off_track_cap);
        }
    }
    assert!(
        towed_off_the_asphalt_for > ticks(0.2),
        "towed for {towed_off_the_asphalt_for} ticks"
    );
}

#[test]
fn braking_slows_a_towed_kart() {
    let mut state = at_the_towed_cap(nose_to_tail(6.0));
    state = step(state, &[throttle(), brake()], DT);
    assert!(speed(&state, FOLLOWER) < TOWED_TOP_SPEED - 0.1);
    assert!(towed(&state, FOLLOWER));
}

#[test]
fn a_boost_pad_during_a_tow_fires_and_the_kart_settles_back_to_the_towed_cap() {
    // Nose to tail over the pad's centre line at x = 33: the leader's pad
    // boost pulls it away a little, the follower's keeps it in the wake,
    // and when the follower's boost ends it is still towed.
    let sidecar = clean_oval_with_slots_at(&[[33.0, 0.0, 4.0], [33.0, 0.0, 10.0]]);
    let mut state = towed_down_the_straight(started(State::new(SEED, 2, track(&sidecar))));
    let mut boosted_at = None;
    for tick in 1..ticks(4.0) {
        state = step(state, &[throttle(), throttle()], DT);
        let follower = state.kart_readouts()[FOLLOWER];
        assert!(towed(&state, FOLLOWER), "left the wake at tick {tick}");
        if let (None, Some(left)) = (boosted_at, follower.boost_s) {
            boosted_at = Some(tick);
            assert_eq!(follower.speed, BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S);
            assert!((left - (BOOST_PAD_S - DT)).abs() < 1e-5);
        }
        if boosted_at.is_some() && follower.boost_s.is_none() {
            // The boost's last tick still sets its speed; the cap takes
            // over on the next.
            state = step(state, &[throttle(), throttle()], DT);
            assert!(towed(&state, FOLLOWER));
            assert_eq!(
                speed(&state, FOLLOWER),
                TOWED_TOP_SPEED,
                "settled to the towed cap"
            );
            return;
        }
    }
    panic!("the follower never crossed the pad: boosted at {boosted_at:?}");
}

#[test]
fn a_red_mini_turbo_during_a_tow_fires_and_the_kart_settles_back_to_the_towed_cap() {
    // Three karts down the lane: a leader with a full purse, which keeps a
    // towed kart behind it from closing fast, then two towed karts 5 m
    // apart. At the turn's entry each hops and drifts left the same way at
    // the same point of the road, so the last one follows the middle one
    // round the same arc and stays in its wake through the drift and the
    // boost, while the middle one has left the leader's, which drives
    // straight on, and its tow runs out.
    const MIDDLE: usize = 1;
    const LAST: usize = 2;
    let positions = [[LANE_X, 0.0, 2.0], [LANE_X, 0.0, 8.0], [LANE_X, 0.0, 13.0]];
    let sidecar = clean_oval_with_slots_at(&positions);
    let sidecar = with_coins_at(&sidecar, [positions[LEADER]; 10]);
    let mut state = started(State::new(SEED, 3, track(&sidecar)));

    /// One kart's script: straight to the turn's entry, a hop left there,
    /// the drift held with a touch of counter-steer, which widens it to
    /// about the turn's radius, until it charges red, then the release
    /// with the wheel on full lock through the turn.
    #[derive(Default)]
    struct Script {
        hopped_at: Option<usize>,
        released: bool,
    }
    impl Script {
        fn input(&mut self, state: &State, kart: usize, tick: usize) -> KartInput {
            let readout = state.kart_readouts()[kart];
            if self.hopped_at.is_none() && readout.position[2] <= -100.0 {
                self.hopped_at = Some(tick);
            }
            let Some(hopped_at) = self.hopped_at else {
                return throttle();
            };
            if tick - hopped_at <= ticks(HOP_AIRTIME_S) + 1 {
                return KartInput {
                    hop: true,
                    ..steering(-1.0)
                };
            }
            if !self.released {
                if readout.mini_turbo == MiniTurbo::Red {
                    self.released = true;
                } else {
                    return KartInput {
                        hop: true,
                        ..steering(0.1)
                    };
                }
            }
            steering(-1.0)
        }
    }
    let mut scripts = [Script::default(), Script::default()];
    let mut released_at = None;
    let mut boost_ended_at = None;
    for tick in 0..ticks(10.0) {
        let was_released = scripts[1].released;
        let inputs = [
            throttle(),
            scripts[0].input(&state, MIDDLE, tick),
            scripts[1].input(&state, LAST, tick),
        ];
        state = step(state, &inputs, DT);
        let last = state.kart_readouts()[LAST];
        if scripts[1].hopped_at.is_none() {
            continue;
        }
        if scripts[1].released && !was_released {
            // The release fires its boost under the tow, which the boost
            // does not replace. The middle kart's own release a few ticks
            // before swung its travel by the slip angle, which can put the
            // last one out of its wake for a moment, so the tow may be
            // running down here; it is re-armed before the boost ends.
            released_at = Some(tick);
            assert_eq!(state.kart_readouts()[LEADER].coins, 10);
            assert_eq!(last.drift, None);
            assert_eq!(last.speed, MINI_TURBO_BOOST_FACTOR * TOP_SPEED_M_PER_S);
            let left = last.boost_s.expect("the boost");
            assert!((left - (MINI_TURBO_RED_BOOST_S - DT)).abs() < 1e-5);
            assert!(towed(&state, LAST));
        }
        if released_at.is_some() && boost_ended_at.is_none() && last.boost_s.is_none() {
            boost_ended_at = Some(tick);
            continue;
        }
        if let Some(ended) = boost_ended_at {
            // The boost's last tick still sets its speed; on the next the
            // last kart settles to the towed cap, where the middle one,
            // whose tow ran out behind the leader, is held to the plain one.
            assert_eq!(tick, ended + 1);
            assert_eq!(
                last.slipstream,
                Some(Slipstream::Tow {
                    left_s: SLIPSTREAM_TOW_S
                })
            );
            assert_eq!(last.speed, TOWED_TOP_SPEED);
            assert_eq!(slipstream(&state, MIDDLE), None);
            assert_eq!(speed(&state, MIDDLE), TOP_SPEED_M_PER_S);
            return;
        }
    }
    panic!("released at {released_at:?}, the boost ended at {boost_ended_at:?}");
}

#[test]
fn a_spin_out_clears_the_charge_and_the_tow() {
    // The leader holds a hazard, rolled at Go from a box under its slot;
    // towed by the time it lands, the follower runs into the drop.
    let build = || {
        let sidecar = clean_oval_with_slots_at(&[[LANE_X, 0.0, 4.0], [LANE_X, 0.0, 10.0]]);
        track(&with_boxes_under(&sidecar, [LEADER]))
    };
    let mut state = rolling(build, 2, 0, &[(LEADER, Item::Hazard)]);
    while !towed(&state, FOLLOWER) {
        state = step(state, &[throttle(), throttle()], DT);
    }
    let drop = KartInput {
        use_item: true,
        ..throttle()
    };
    state = step(state, &[drop, throttle()], DT);
    assert!(state.events().contains(&RaceEvent::ItemFired {
        kart: LEADER,
        item: Item::Hazard
    }));
    for _ in 0..ticks(1.0) {
        assert!(towed(&state, FOLLOWER));
        state = step(state, &[throttle(), throttle()], DT);
        if state
            .events()
            .iter()
            .any(|event| matches!(event, RaceEvent::SpunOut { kart: FOLLOWER, .. }))
        {
            break;
        }
    }
    let follower = state.kart_readouts()[FOLLOWER];
    assert!(follower.spin_out_s.is_some(), "never hit the hazard");
    // The hit is judged after the tick's handling; the freeze clears the
    // tow on the next.
    state = step(state, &[throttle(), throttle()], DT);
    assert_eq!(slipstream(&state, FOLLOWER), None);
    for _ in 0..ticks(1.5) {
        state = step(state, &[throttle(), throttle()], DT);
        assert_eq!(slipstream(&state, FOLLOWER), None);
    }
}

#[test]
fn a_respawn_clears_the_charge_and_the_tow() {
    // Both drive across the straight and over the edge of the world at
    // x = 70, nose to tail, towed on the grass; each falls for the respawn
    // delay and respawns. The leader respawns first, out of the follower's
    // reach, so the follower's tow is running down when its own respawn
    // clears it.
    let mut sidecar = without_walls(&without_coins(&without_item_boxes(OVAL)));
    sidecar = with_slot_at(&sidecar, LEADER, [30.0, 0.0, 4.0], FACING_PLUS_X);
    sidecar = with_slot_at(&sidecar, FOLLOWER, [24.5, 0.0, 4.0], FACING_PLUS_X);
    let mut state = started(State::new(SEED, 2, track(&sidecar)));
    let mut before = None;
    for _ in 0..ticks(8.0) {
        let was = slipstream(&state, FOLLOWER);
        state = step(state, &[throttle(), throttle()], DT);
        if state
            .events()
            .contains(&RaceEvent::Respawned { kart: FOLLOWER })
        {
            before = Some(was);
            break;
        }
    }
    let before = before.expect("the follower never respawned");
    assert!(
        matches!(before, Some(Slipstream::Tow { left_s }) if left_s < SLIPSTREAM_TOW_S),
        "towed and running down the tick before: {before:?}"
    );
    assert_eq!(slipstream(&state, FOLLOWER), None);
    assert_eq!(speed(&state, FOLLOWER), 0.0);
}

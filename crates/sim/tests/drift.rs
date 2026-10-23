//! Cornering technique at the sim's public seam: a hop, the drift it lands
//! into, and the mini-turbo the drift charges. Tests drive one kart on the
//! oval fixture, whose front straight is clear for 100 m past the start line
//! and runs along -Z, and assert what `kart_transforms` reports.

use sim::tuning::{
    ACCELERATION_TO_TOP_S, DRIFT_ENTRY_SPEED_FRACTION, DRIFT_SLIP_ANGLE_RAD, HOP_AIRTIME_S,
    MINI_TURBO_BLUE_BOOST_S, MINI_TURBO_BLUE_CHARGE_S, MINI_TURBO_BOOST_FACTOR,
    MINI_TURBO_ORANGE_BOOST_S, MINI_TURBO_ORANGE_CHARGE_S, TICKS_PER_SECOND, TOP_SPEED_M_PER_S,
};
use sim::{step, DriftDirection, KartInput, KartTransform, MiniTurbo, Phase, State, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

/// The race past its countdown, every kart idle on the grid until Go.
fn started(mut state: State) -> State {
    let inputs = vec![KartInput::default(); state.human_count()];
    while matches!(state.phase(), Phase::Countdown { .. }) {
        state = step(state, &inputs, DT);
    }
    state
}

fn race() -> State {
    started(State::new(
        SEED,
        1,
        Track::from_sidecar(OVAL).expect("the oval fixture loads"),
    ))
}

fn run(mut state: State, inputs: impl IntoIterator<Item = KartInput>) -> State {
    for input in inputs {
        state = step(state, &[input], DT);
    }
    state
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn only_kart(state: &State) -> KartTransform {
    let transforms = state.kart_transforms();
    assert_eq!(transforms.len(), 1);
    transforms[0]
}

fn accelerate() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// A kart at top speed on the front straight.
fn at_top_speed() -> State {
    run(race(), vec![accelerate(); ticks(ACCELERATION_TO_TOP_S) + 1])
}

#[test]
fn a_hop_lifts_the_kart_and_lands_it_after_the_tuned_airtime() {
    let hop = KartInput {
        hop: true,
        ..accelerate()
    };
    let ground = only_kart(&at_top_speed()).position[1];
    assert!(
        !only_kart(&at_top_speed()).airborne,
        "on the track before the hop"
    );

    let midair = run(at_top_speed(), vec![hop; ticks(HOP_AIRTIME_S / 2.0)]);
    assert!(
        only_kart(&midair).position[1] > ground + 0.05,
        "in the air halfway through the airtime"
    );
    assert!(
        only_kart(&midair).airborne,
        "reported in the air for the hop's tell"
    );

    let after_landing = run(midair, vec![hop; ticks(HOP_AIRTIME_S / 2.0) + 3]);
    assert!(
        (only_kart(&after_landing).position[1] - ground).abs() < 0.02,
        "back on the track just after the airtime"
    );
    assert!(!only_kart(&after_landing).airborne, "landed");
    assert!(
        only_kart(&after_landing).speed > 0.9 * TOP_SPEED_M_PER_S,
        "a hop does not cost speed"
    );
}

/// Hop and steer right with the throttle down: held through the landing,
/// the drift the technique is for.
fn hop_right() -> KartInput {
    KartInput {
        hop: true,
        steer: 1.0,
        ..accelerate()
    }
}

/// `state` one tick after the hop has landed, with `landing` held through
/// the airtime and the landing tick.
fn landed(state: State, landing: KartInput) -> State {
    run(state, vec![landing; ticks(HOP_AIRTIME_S) + 1])
}

#[test]
fn landing_while_steering_above_the_entry_speed_starts_a_drift_in_the_hop_direction() {
    let drifting = landed(at_top_speed(), hop_right());
    let drift = only_kart(&drifting).drift.expect("drifting");
    assert_eq!(drift.direction, DriftDirection::Right);
    assert_eq!(
        drift.mini_turbo(),
        MiniTurbo::None,
        "just landed, nothing charged"
    );
    assert!(drift.charge_s < 0.1);
}

#[test]
fn no_drift_without_a_press_or_in_the_air() {
    assert_eq!(only_kart(&at_top_speed()).drift, None);
    let midair = run(
        at_top_speed(),
        vec![hop_right(); ticks(HOP_AIRTIME_S / 2.0)],
    );
    assert_eq!(
        only_kart(&midair).drift,
        None,
        "a drift starts on landing, not at the hop"
    );
}

#[test]
fn no_drift_when_landing_below_the_entry_speed() {
    // Rolling, but under the entry fraction of top speed.
    let slow = run(race(), vec![accelerate(); ticks(0.5)]);
    let speed = only_kart(&slow).speed;
    assert!(
        speed > 1.0 && speed < DRIFT_ENTRY_SPEED_FRACTION * TOP_SPEED_M_PER_S,
        "{speed} m/s"
    );
    assert_eq!(only_kart(&landed(slow, hop_right())).drift, None);
}

#[test]
fn no_drift_when_the_kart_hopped_straight_or_lands_with_the_steer_centred() {
    let straight = KartInput {
        steer: 0.0,
        ..hop_right()
    };
    let hopped_straight = step(at_top_speed(), &[straight], DT);
    assert_eq!(
        only_kart(&landed(hopped_straight, hop_right())).drift,
        None,
        "steering after a straight hop is too late"
    );

    let hopped_right = step(at_top_speed(), &[hop_right()], DT);
    assert_eq!(only_kart(&landed(hopped_right, straight)).drift, None);
}

#[test]
fn no_drift_when_the_hop_button_is_let_go_before_landing() {
    let let_go = KartInput {
        hop: false,
        ..hop_right()
    };
    let hopped_right = step(at_top_speed(), &[hop_right()], DT);
    assert_eq!(only_kart(&landed(hopped_right, let_go)).drift, None);
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(transform: &KartTransform) -> f32 {
    let [x, y, z, w] = transform.rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

/// Yaw rate measured over `seconds` more of `input`, in radians per second.
fn measured_yaw_rate(state: State, input: KartInput, seconds: f32) -> f32 {
    let before = yaw(&only_kart(&state));
    let after = yaw(&only_kart(&run(state, vec![input; ticks(seconds)])));
    (after - before) / seconds
}

#[test]
fn the_drift_direction_is_fixed_at_the_hop() {
    let counter_steer = KartInput {
        steer: -1.0,
        ..hop_right()
    };
    let hopped_right = step(at_top_speed(), &[hop_right()], DT);
    let drifting = landed(hopped_right, counter_steer);
    assert_eq!(
        only_kart(&drifting).drift.map(|drift| drift.direction),
        Some(DriftDirection::Right)
    );
    // Clockwise from above is a negative rotation about +Y: the kart still
    // turns right while the player holds left.
    assert!(measured_yaw_rate(drifting, counter_steer, 0.5) < -0.1);
}

/// A right drift under way, one tick after landing, with the stick centred.
fn drifting_right() -> State {
    let hopped_right = step(at_top_speed(), &[hop_right()], DT);
    landed(hopped_right, hop_right())
}

#[test]
fn counter_steer_widens_the_arc_and_inward_steer_tightens_it() {
    let held = |steer: f32| KartInput {
        steer,
        ..hop_right()
    };
    let neutral = measured_yaw_rate(drifting_right(), held(0.0), 0.5);
    let inward = measured_yaw_rate(drifting_right(), held(1.0), 0.5);
    let counter = measured_yaw_rate(drifting_right(), held(-1.0), 0.5);

    // All clockwise (negative about +Y); the arc tightens with the rate.
    assert!(counter < 0.0, "counter-steer still turns right: {counter}");
    assert!(
        counter.abs() < neutral.abs() - 0.1,
        "wider: {counter} vs {neutral}"
    );
    assert!(
        inward.abs() > neutral.abs() + 0.1,
        "tighter: {inward} vs {neutral}"
    );
}

/// The mini-turbo tier `seconds` into a right drift held with the stick
/// centred.
fn tier_after(seconds: f32) -> MiniTurbo {
    let held = KartInput {
        steer: 0.0,
        ..hop_right()
    };
    let state = run(drifting_right(), vec![held; ticks(seconds)]);
    only_kart(&state)
        .drift
        .expect("still drifting")
        .mini_turbo()
}

#[test]
fn holding_the_drift_charges_blue_then_orange_at_the_tuned_times() {
    assert_eq!(tier_after(MINI_TURBO_BLUE_CHARGE_S - 0.1), MiniTurbo::None);
    assert_eq!(tier_after(MINI_TURBO_BLUE_CHARGE_S + 0.1), MiniTurbo::Blue);
    assert_eq!(
        tier_after(MINI_TURBO_ORANGE_CHARGE_S - 0.1),
        MiniTurbo::Blue
    );
    assert_eq!(
        tier_after(MINI_TURBO_ORANGE_CHARGE_S + 0.1),
        MiniTurbo::Orange
    );
    assert_eq!(
        tier_after(MINI_TURBO_ORANGE_CHARGE_S + 2.0),
        MiniTurbo::Orange,
        "orange is the top tier"
    );
}

/// The kart on the tick its hop lands into a right drift, before the first
/// tick of charge, so a hold from here is counted in whole ticks.
fn just_landed_right() -> State {
    let mut state = step(at_top_speed(), &[hop_right()], DT);
    while only_kart(&state).drift.is_none() {
        state = step(state, &[hop_right()], DT);
    }
    let drift = only_kart(&state).drift.expect("drifting");
    assert_eq!(drift.charge_s, 0.0, "the landing tick charges nothing");
    state
}

/// The boost the kart reports the tick a right drift is let go after exactly
/// `held` ticks of charge with the stick centred.
fn boost_after_ticks(held: usize) -> Option<f32> {
    let hold = KartInput {
        steer: 0.0,
        ..hop_right()
    };
    let charged = run(just_landed_right(), vec![hold; held]);
    only_kart(&step(charged, &[accelerate()], DT)).boost_s
}

fn assert_boost_of(reported: Option<f32>, boost_s: f32) {
    let reported = reported.expect("the release reports a boost");
    assert!(
        (reported - boost_s).abs() < 2.0 * DT,
        "a {boost_s} s boost, reported {reported}"
    );
}

#[test]
fn each_tier_pays_on_the_tick_its_charge_is_reached_and_the_tier_below_one_tick_earlier() {
    let blue = ticks(MINI_TURBO_BLUE_CHARGE_S);
    let orange = ticks(MINI_TURBO_ORANGE_CHARGE_S);
    assert_eq!(boost_after_ticks(blue - 1), None, "one tick short of blue");
    assert_boost_of(boost_after_ticks(blue), MINI_TURBO_BLUE_BOOST_S);
    assert_boost_of(boost_after_ticks(orange - 1), MINI_TURBO_BLUE_BOOST_S);
    assert_boost_of(boost_after_ticks(orange), MINI_TURBO_ORANGE_BOOST_S);
}

/// Speed between the last two ticks of `state`, read from positions so the
/// test never trusts the readout alone.
fn measured_speed(state: State, input: KartInput) -> f32 {
    let before = only_kart(&state);
    let after = only_kart(&step(state, &[input], DT));
    let dx = after.position[0] - before.position[0];
    let dz = after.position[2] - before.position[2];
    (dx * dx + dz * dz).sqrt() / DT
}

/// A right drift held with the stick centred for `charge_s`, then the hop
/// button let go with the throttle still down.
fn released_after(charge_s: f32) -> State {
    let held = KartInput {
        steer: 0.0,
        ..hop_right()
    };
    let charged = run(drifting_right(), vec![held; ticks(charge_s)]);
    step(charged, &[accelerate()], DT)
}

fn assert_boosts_for(charge_s: f32, boost_s: f32) {
    let released = released_after(charge_s);
    assert_eq!(
        only_kart(&released).drift,
        None,
        "the release ends the drift"
    );
    let reported = only_kart(&released)
        .boost_s
        .expect("the release reports the boost it starts");
    assert!(
        (reported - boost_s).abs() < 2.0 * DT,
        "the boost starts at the tuned {boost_s} s, reported {reported}"
    );
    let boosted = TOP_SPEED_M_PER_S * MINI_TURBO_BOOST_FACTOR;

    let early = run(released, vec![accelerate(); ticks(boost_s / 2.0)]);
    let late = run(early, vec![accelerate(); ticks(boost_s / 2.0) - 3]);
    let speed = measured_speed(late, accelerate());
    assert!(
        (speed - boosted).abs() < 0.5,
        "boosted to about {boosted} m/s until the boost ends, measured {speed}"
    );

    let over = run(
        released_after(charge_s),
        vec![accelerate(); ticks(boost_s) + 3],
    );
    assert_eq!(only_kart(&over).boost_s, None, "the boost has run out");
    let speed = measured_speed(over, accelerate());
    assert!(
        speed <= TOP_SPEED_M_PER_S + 0.01,
        "back under top speed after the boost, measured {speed}"
    );
}

#[test]
fn releasing_a_blue_charge_boosts_to_the_tuned_speed_for_the_blue_duration() {
    assert_boosts_for(MINI_TURBO_BLUE_CHARGE_S + 0.1, MINI_TURBO_BLUE_BOOST_S);
}

#[test]
fn releasing_an_orange_charge_boosts_for_the_longer_orange_duration() {
    assert_boosts_for(MINI_TURBO_ORANGE_CHARGE_S + 0.1, MINI_TURBO_ORANGE_BOOST_S);
}

#[test]
fn releasing_before_blue_ends_the_drift_with_no_boost() {
    let released = released_after(MINI_TURBO_BLUE_CHARGE_S - 0.1);
    assert_eq!(only_kart(&released).drift, None);
    let mut state = released;
    for _ in 0..ticks(MINI_TURBO_BLUE_BOOST_S) {
        let before = only_kart(&state);
        state = step(state, &[accelerate()], DT);
        let speed = (before.position[2] - only_kart(&state).position[2]) / DT;
        assert!(speed <= TOP_SPEED_M_PER_S + 0.01, "no boost: {speed} m/s");
    }
}

#[test]
fn a_drift_that_slows_below_the_entry_speed_ends_with_no_boost() {
    let brake_in_drift = KartInput {
        throttle: 0.0,
        brake: 1.0,
        steer: 0.0,
        ..hop_right()
    };
    // Long enough to charge blue, then to brake under the entry speed.
    let charged = run(
        drifting_right(),
        vec![hop_right(); ticks(MINI_TURBO_BLUE_CHARGE_S + 0.1)],
    );
    let slowed = run(charged, vec![brake_in_drift; ticks(2.0)]);
    let kart = only_kart(&slowed);
    assert!(kart.speed < DRIFT_ENTRY_SPEED_FRACTION * TOP_SPEED_M_PER_S);
    assert_eq!(kart.drift, None);

    let released = step(slowed, &[accelerate()], DT);
    assert!(measured_speed(released, accelerate()) < TOP_SPEED_M_PER_S);
}

#[test]
fn holding_the_hop_button_through_a_landing_does_not_hop_again() {
    let hold = KartInput {
        hop: true,
        ..accelerate()
    };
    let ground = only_kart(&at_top_speed()).position[1];
    // A straight hop lands into no drift, so a held button has nothing to
    // keep; it must not bounce the kart either.
    let mut state = landed(at_top_speed(), hold);
    for _ in 0..ticks(HOP_AIRTIME_S) {
        state = step(state, &[hold], DT);
        let height = only_kart(&state).position[1] - ground;
        assert!(height.abs() < 0.02, "stayed on the track: {height} m up");
    }

    let released = step(state, &[accelerate()], DT);
    let hopped_again = run(released, vec![hold; ticks(HOP_AIRTIME_S / 2.0)]);
    assert!(
        only_kart(&hopped_again).position[1] > ground + 0.05,
        "a fresh press hops"
    );
}

#[test]
fn a_drifting_kart_travels_outside_its_facing_by_the_tuned_slip_angle() {
    let held = KartInput {
        steer: 0.0,
        ..hop_right()
    };
    let before = only_kart(&drifting_right());
    let after = only_kart(&step(drifting_right(), &[held], DT));
    // The heading of the path, in the yaw convention: 0 is -Z, positive is
    // anticlockwise from above. A right drift points right of its path.
    let dx = after.position[0] - before.position[0];
    let dz = after.position[2] - before.position[2];
    let travel = (-dx).atan2(-dz);
    let facing = (yaw(&before) + yaw(&after)) / 2.0;
    let slip = travel - facing;
    assert!(
        (slip - DRIFT_SLIP_ANGLE_RAD).abs() < 0.05,
        "expected the path {DRIFT_SLIP_ANGLE_RAD} rad left of the nose, measured {slip}"
    );
}

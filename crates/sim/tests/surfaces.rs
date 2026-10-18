//! How the track pushes back, at the sim's public seam: drive a kart over
//! each surface of the oval fixture and read the transforms, surfaces and
//! standings the sim reports. The oval's front straight has its centreline
//! at x = 30 running along -Z, asphalt 6 m either side, sand for 6 m
//! beyond, grass to x = 70 and nothing past it (layouts/oval.py).

use sim::tuning::{KART_HALF_WIDTH_M, OFF_TRACK_SPEED_FACTOR, TICKS_PER_SECOND, TOP_SPEED_M_PER_S};
use sim::{step, KartInput, State, Surface, Track};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;
/// A quarter turn clockwise seen from above: from facing -Z to facing +X.
const FACING_PLUS_X: f32 = -std::f32::consts::FRAC_PI_2;

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn throttle() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// The track in `sidecar` with spawn slot 0 moved to `position` facing
/// `yaw`, so a kart can be started anywhere on or beside the track.
fn with_slot_0_at(sidecar: &str, position: [f32; 3], yaw: f32) -> Track {
    let mut sidecar: serde_json::Value = serde_json::from_str(sidecar).expect("valid JSON");
    sidecar["spawn_grid"][0] = serde_json::json!({ "position": position, "yaw": yaw });
    Track::from_sidecar(&sidecar.to_string()).expect("the track with a moved slot loads")
}

fn oval_with_slot_0_at(position: [f32; 3], yaw: f32) -> Track {
    with_slot_0_at(OVAL, position, yaw)
}

#[test]
fn sand_and_grass_cap_the_speed_at_the_off_track_fraction_of_top_speed() {
    // From the infield grass at x = 0 across the front straight: grass to
    // x = 18, sand to 24, asphalt to 36, sand to 42, grass beyond.
    let mut state = State::new(
        SEED,
        1,
        oval_with_slot_0_at([0.0, 0.0, -20.0], FACING_PLUS_X),
    );
    let cap = OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S;
    let mut fastest_off_track: f32 = 0.0;
    let mut fastest_on_asphalt: f32 = 0.0;
    let mut capped_on_the_second_sand = None;
    for _ in 0..ticks(6.0) {
        // The rules read the surface under the kart as the tick begins.
        let surface = state.kart_surface(0);
        state = step(state, &[throttle()], DT);
        let kart = state.kart_transforms()[0];
        match surface {
            Surface::Asphalt => fastest_on_asphalt = fastest_on_asphalt.max(kart.speed),
            Surface::Sand | Surface::Grass => {
                fastest_off_track = fastest_off_track.max(kart.speed);
                if kart.position[0] > 36.0 && capped_on_the_second_sand.is_none() {
                    capped_on_the_second_sand = Some(kart.speed);
                }
            }
            other => panic!("left the track: {other:?} at {kart:?}"),
        }
        if kart.position[0] > 41.0 {
            break;
        }
    }

    assert!(
        (fastest_off_track - cap).abs() < 0.01,
        "off track the kart reaches the cap and never passes it: {fastest_off_track} vs {cap}"
    );
    assert!(
        fastest_on_asphalt > cap + 2.0,
        "the asphalt lets it accelerate again: {fastest_on_asphalt}"
    );
    let second_sand = capped_on_the_second_sand.expect("the kart reached the far sand");
    assert!(
        second_sand <= cap + 0.01,
        "back on the sand the cap applies at once: {second_sand}"
    );
}

#[test]
fn a_boost_pad_gives_the_tuned_boost_for_the_tuned_time_from_the_tick_it_is_entered() {
    use sim::tuning::{BOOST_PAD_FACTOR, BOOST_PAD_HALF_LENGTH_M, BOOST_PAD_S};
    // Slot 1 at x = 32 drives up the front straight over the pad centred
    // on (33, -60), reaching top speed well before it, while pole idles.
    let mut state = State::new(SEED, 2, Track::from_sidecar(OVAL).expect("the oval loads"));
    let pad = state.track().boost_pads()[1].position;
    assert_eq!(
        pad,
        [33.0, 0.0, -60.0],
        "the fixture's pad is where this test expects"
    );
    let pad_z = pad[2];
    let mut boosted_z = Vec::new();
    let mut top_speed_before_the_pad = false;
    for _ in 0..ticks(6.0) {
        state = step(state, &[KartInput::default(), throttle()], DT);
        let kart = state.kart_transforms()[1];
        if kart.speed > TOP_SPEED_M_PER_S {
            assert!(
                (kart.speed - BOOST_PAD_FACTOR * TOP_SPEED_M_PER_S).abs() < 1e-3,
                "boosted to the pad's factor: {}",
                kart.speed
            );
            boosted_z.push(kart.position[2]);
        } else if kart.position[2] > pad_z - BOOST_PAD_HALF_LENGTH_M {
            // Short of the pad, or on it for the tick that reached it: the
            // rules read where the kart is as the tick begins.
            top_speed_before_the_pad |= kart.speed == TOP_SPEED_M_PER_S;
        } else {
            assert!(!boosted_z.is_empty(), "the pad boosted the kart");
            // Over as soon as the clock runs out: back to top speed, never
            // a tick of boost after a tick without.
            assert!(kart.speed <= TOP_SPEED_M_PER_S);
        }
    }

    assert!(top_speed_before_the_pad);
    let first = *boosted_z.first().expect("the pad boosted the kart");
    assert!(
        first < pad_z + BOOST_PAD_HALF_LENGTH_M && first > pad_z - BOOST_PAD_HALF_LENGTH_M - 1.0,
        "the boost starts the first tick begun on the pad, not at z {first}"
    );
    // The clock runs from entering the pad, however long the kart is on it.
    let expected = ticks(BOOST_PAD_S);
    assert!(
        (expected - 1..=expected + 1).contains(&boosted_z.len()),
        "boosted for {} ticks, expected about {expected}",
        boosted_z.len()
    );
}

/// A metre-high, half-metre-thick wall `half_length` either side of
/// `centre`, running along `along` with `normal` facing out of the side a
/// kart approaches, added to the oval's front straight as a wall mesh wound
/// outward like an exported one.
fn oval_with_a_wall(
    centre: [f32; 3],
    along: [f32; 3],
    normal: [f32; 3],
    half_length: f32,
) -> Track {
    const HEIGHT: f32 = 1.0;
    const HALF_THICKNESS: f32 = 0.25;
    let at = |run: f32, up: f32, out: f32| -> [f32; 3] {
        std::array::from_fn(|i| {
            centre[i] + along[i] * run + [0.0, 1.0, 0.0][i] * up + normal[i] * out
        })
    };
    let mut positions = Vec::new();
    for out in [-HALF_THICKNESS, HALF_THICKNESS] {
        for up in [0.0, HEIGHT] {
            for run in [-half_length, half_length] {
                positions.extend(at(run, up, out));
            }
        }
    }
    // Corner index: bit 0 is the end `along` points to, bit 1 the top, bit 2
    // the face `normal` points out of.
    let quads = [
        [0, 1, 3, 2],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 2, 6, 4],
        [1, 3, 7, 5],
    ];
    let box_centre = at(0.0, HEIGHT / 2.0, 0.0);
    let corner = |i: usize| [positions[3 * i], positions[3 * i + 1], positions[3 * i + 2]];
    let mut indices = Vec::new();
    for [a, b, c, d] in quads {
        let (pa, pb, pc) = (corner(a), corner(b), corner(c));
        let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let outward: f32 = (0..3).map(|i| n[i] * (pa[i] - box_centre[i])).sum();
        // Wind every face outward, as the exporter does.
        let [a, b, c, d] = if outward > 0.0 {
            [a, b, c, d]
        } else {
            [a, d, c, b]
        };
        indices.extend([a, b, c, a, c, d].map(|i| i as u32));
    }
    let mut sidecar: serde_json::Value = serde_json::from_str(OVAL).expect("valid JSON");
    sidecar["surfaces"]
        .as_array_mut()
        .expect("surfaces")
        .push(serde_json::json!({
            "name": "TestWall",
            "surface": "wall",
            "positions": positions,
            "indices": indices,
        }));
    Track::from_sidecar(&sidecar.to_string()).expect("the oval with a wall loads")
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Positions of slot 0 after each of `seconds` of full throttle, from the
/// one before the first tick, with the speed readout after each tick.
fn drive_straight(track: Track, seconds: f32) -> Vec<([f32; 3], f32)> {
    let mut state = State::new(SEED, 1, track);
    let mut trace = vec![(state.kart_transforms()[0].position, 0.0)];
    for _ in 0..ticks(seconds) {
        state = step(state, &[throttle()], DT);
        let kart = state.kart_transforms()[0];
        trace.push((kart.position, kart.speed));
    }
    trace
}

#[test]
fn a_wall_met_at_an_angle_stops_the_motion_into_it_and_keeps_the_motion_along_it() {
    // A wall across slot 0's column at 45 degrees, its face toward the
    // grid, so a kart driving straight up the column meets it at z = -40
    // moving 21 m/s into it and 21 m/s along it.
    let root_half = std::f32::consts::FRAC_1_SQRT_2;
    let normal = [root_half, 0.0, root_half];
    let along = [root_half, 0.0, -root_half];
    let centre = [28.0, 0.0, -40.0];
    let trace = drive_straight(oval_with_a_wall(centre, along, normal, 12.0), 4.0);
    let toward = |p: [f32; 3]| dot([p[0] - centre[0], 0.0, p[2] - centre[2]], normal);
    let beside = |p: [f32; 3]| dot([p[0] - centre[0], 0.0, p[2] - centre[2]], along);

    // The impact tick is the first whose approach is cut short.
    let full_tick = TOP_SPEED_M_PER_S * root_half * DT;
    let impact = (1..trace.len())
        .find(|&i| {
            let (before, after) = (trace[i - 1].0, trace[i].0);
            trace[i - 1].1 == TOP_SPEED_M_PER_S && toward(before) - toward(after) < full_tick - 0.02
        })
        .expect("the kart reaches the wall at top speed");
    let (before, after) = (trace[impact - 1].0, trace[impact].0);
    assert!(
        beside(after) - beside(before) > full_tick - 0.02,
        "the motion along the wall is kept through the impact tick: {} of {full_tick} m",
        beside(after) - beside(before)
    );
    let at_the_wall = toward(after);
    assert!(
        at_the_wall > 0.5 && at_the_wall < 1.3,
        "stopped with its corner at the wall, {at_the_wall} m from its centre"
    );
    // The corner that hit first swings the kart round along the wall, so
    // its centre may come as close as a kart lying flat against it, but
    // never closer, and never moves back out.
    let flat_against = 0.25 + KART_HALF_WIDTH_M;
    for pair in trace[impact..impact + ticks(1.0)].windows(2) {
        let [(previous, _), (position, speed)] = pair else {
            unreachable!()
        };
        assert!(
            toward(*position) > flat_against - 0.02,
            "pushed into the wall: {position:?}"
        );
        assert!(
            toward(*position) - toward(*previous) < 0.02,
            "bounced off the wall: {previous:?} then {position:?}"
        );
        assert!(
            *speed < TOP_SPEED_M_PER_S * 0.75,
            "the bump cost speed: {speed}"
        );
        assert!(
            (position[1] - 0.3).abs() < 0.1,
            "stayed on the ground: {position:?}"
        );
    }
    assert!(
        trace[impact + ticks(0.5)].1 < 1.0,
        "a kart still steering into the wall is stopped by it: {} m/s",
        trace[impact + ticks(0.5)].1
    );
}

#[test]
fn a_wall_met_head_on_stops_the_kart_dead_without_a_bounce() {
    let centre = [28.0, 0.0, -40.0];
    let trace = drive_straight(
        oval_with_a_wall(centre, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 6.0),
        4.0,
    );
    let impact = (1..trace.len())
        .find(|&i| trace[i - 1].1 == TOP_SPEED_M_PER_S && trace[i].1 < TOP_SPEED_M_PER_S)
        .expect("the kart reaches the wall at top speed");
    let stopped_z = trace[impact].0[2];
    assert!(
        stopped_z > centre[2] + 0.25 && stopped_z < centre[2] + 0.25 + 1.0,
        "nose at the wall's face: centre at z {stopped_z}"
    );
    assert!(
        trace[impact + 1].1 < 0.5,
        "stopped dead: {} m/s",
        trace[impact + 1].1
    );
    for (position, speed) in &trace[impact + 1..impact + ticks(1.0)] {
        assert!(
            (position[2] - stopped_z).abs() < 0.03,
            "moved after the stop: {position:?}"
        );
        assert!(
            (position[0] - 28.0).abs() < 0.03,
            "slid sideways: {position:?}"
        );
        assert!(
            *speed < 0.5,
            "a kart held by a wall reads as stopped: {speed}"
        );
    }
}

/// Yaw in radians recovered from the rotation quaternion `[x, y, z, w]`.
fn yaw(rotation: [f32; 4]) -> f32 {
    let [x, y, z, w] = rotation;
    (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (y * y + z * z))
}

#[test]
fn a_kart_out_of_the_world_for_the_tuned_time_respawns_at_its_last_checkpoint_facing_the_next() {
    use sim::tuning::{KART_HALF_HEIGHT_M, RESPAWN_DELAY_S};
    // Slot 0 turned to face +X drives off the front straight, across the
    // run-off and the grass, and over the edge of the world at x = 70.
    let mut state = State::new(
        SEED,
        1,
        oval_with_slot_0_at([28.0, 0.0, 4.0], FACING_PLUS_X),
    );
    let start = state.track().checkpoints()[0].position;
    let next = state.track().checkpoints()[1].position;
    assert!(
        next[0] == start[0] && next[2] < start[2],
        "the next gate is straight up -Z"
    );

    let mut out_since = None;
    let mut respawned_at = None;
    let mut last_seen_out = None;
    for tick in 0..ticks(8.0) {
        state = step(state, &[throttle()], DT);
        let kart = state.kart_transforms()[0];
        match (out_since, state.kart_surface(0)) {
            (None, Surface::OutOfWorld) => out_since = Some(tick),
            (Some(_), Surface::OutOfWorld) => last_seen_out = Some(kart),
            (Some(_), surface) => {
                respawned_at = Some(tick);
                assert_eq!(surface, Surface::Asphalt);
                break;
            }
            (None, _) => {}
        }
    }
    let out_since = out_since.expect("the kart drove off the edge of the world");
    let respawned_at = respawned_at.expect("the kart came back");
    let falling = last_seen_out.expect("the kart was out of the world for a while");
    assert!(
        falling.position[1] < -1.0,
        "fell while out of the world: {falling:?}"
    );
    let expected = out_since + ticks(RESPAWN_DELAY_S);
    assert!(
        (expected - 1..=expected + 1).contains(&respawned_at),
        "respawned {} ticks after leaving the world, expected {}",
        respawned_at - out_since,
        ticks(RESPAWN_DELAY_S)
    );
    let kart = state.kart_transforms()[0];
    assert_eq!(kart.position[0], start[0]);
    assert_eq!(kart.position[2], start[2]);
    assert!((kart.position[1] - start[1] - KART_HALF_HEIGHT_M).abs() < 1e-3);
    assert!(
        yaw(kart.rotation).abs() < 1e-3,
        "facing checkpoint 1: {kart:?}"
    );
    assert_eq!(kart.speed, 0.0, "respawned at rest");
    let standing = state.standings()[0];
    assert_eq!(standing.checkpoint, 0);
    assert_eq!(standing.laps_completed, 0);

    // Back in the race: it drives on through the gate it faces.
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle()], DT);
    }
    assert_eq!(state.standings()[0].checkpoint, 1);
    assert_eq!(state.kart_surface(0), Surface::Asphalt);
}

#[test]
fn backing_up_for_the_tuned_time_raises_the_wrong_way_warning_until_progress_increases() {
    use sim::tuning::WRONG_WAY_WARNING_S;
    let brake = KartInput {
        brake: 1.0,
        ..KartInput::default()
    };
    // Reversing from the grid moves the kart away from the line it faces:
    // spline progress falls from the first tick.
    let mut state = State::new(SEED, 1, Track::from_sidecar(OVAL).expect("the oval loads"));
    let mut warned_at = None;
    for tick in 0..ticks(WRONG_WAY_WARNING_S + 1.0) {
        state = step(state, &[brake], DT);
        if state.standings()[0].wrong_way {
            warned_at = Some(tick);
            break;
        }
    }
    let warned_at = warned_at.expect("the warning came on");
    let expected = ticks(WRONG_WAY_WARNING_S);
    assert!(
        (expected - 1..=expected + 1).contains(&(warned_at + 1)),
        "warned after {} ticks of reversing, expected {expected}",
        warned_at + 1
    );
    assert!(state.kart_transforms()[0].speed < 0.0, "still backing up");

    // Still warned while slowing down backwards; clear the tick it rolls forward.
    let mut cleared_at = None;
    for tick in 0..ticks(2.0) {
        state = step(state, &[throttle()], DT);
        let standing = state.standings()[0];
        let speed = state.kart_transforms()[0].speed;
        if speed <= 0.0 {
            assert!(
                standing.wrong_way,
                "warned until it moves forward: {speed} m/s"
            );
        } else {
            assert!(!standing.wrong_way, "cleared once progress increases");
            cleared_at = Some(tick);
            break;
        }
    }
    assert!(cleared_at.is_some(), "the kart got going forward again");
}

const BRANDS_PATCH: &str =
    include_str!("../../../assets/tracks/brands-patch/brands-patch.track.json");

/// Full throttle, steering toward `target` (x, z).
fn aim(state: &State, target: [f32; 2]) -> KartInput {
    const STEER_GAIN: f32 = 4.0;
    let transform = state.kart_transforms()[0];
    let (dx, dz) = (
        target[0] - transform.position[0],
        target[1] - transform.position[2],
    );
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    // Positive steer turns the kart right; right of a yaw is (cos, -sin) in x, z.
    let (sin, cos) = yaw(transform.rotation).sin_cos();
    let rightward = (dx * cos - dz * sin) / length;
    KartInput {
        steer: (rightward * STEER_GAIN).clamp(-1.0, 1.0),
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// Brands Patch with spawn slot 0 on the centreline `before_m` short of
/// the chicane, facing along it, and the spline progress of the chicane's
/// entry and exit (layouts/brands_patch.py: chicane-in through chicane-out).
fn brands_patch_before_the_chicane(before_m: f32) -> (Track, f32, f32) {
    let track = Track::from_sidecar(BRANDS_PATCH).expect("Brands Patch loads");
    let (entry, exit) = (371.11, 415.09);
    let spawn = track.point_at(entry - before_m);
    let ahead = track.point_at(entry - before_m + 1.0);
    assert!(
        ahead[0] < spawn[0] && (ahead[2] - spawn[2]).abs() < 1e-3,
        "the hill straight runs along -X"
    );
    (
        with_slot_0_at(BRANDS_PATCH, spawn, std::f32::consts::FRAC_PI_2),
        entry,
        exit,
    )
}

/// Drive slot 0 by `driver` until its spline progress passes `until`:
/// the ticks it took and the surfaces it crossed.
fn ticks_and_surfaces_to_pass(
    track: Track,
    until: f32,
    driver: impl Fn(&State) -> KartInput,
) -> (usize, Vec<Surface>) {
    let mut state = State::new(SEED, 1, track);
    let mut surfaces = Vec::new();
    for tick in 0..ticks(20.0) {
        let input = driver(&state);
        state = step(state, &[input], DT);
        let surface = state.kart_surface(0);
        if surfaces.last() != Some(&surface) {
            surfaces.push(surface);
        }
        if state.standings()[0].spline_progress >= until {
            return (tick + 1, surfaces);
        }
    }
    panic!("never reached {until} m: {:?}", state.standings()[0]);
}

#[test]
fn cutting_the_chicane_through_the_sand_is_slower_than_driving_it() {
    const APPROACH_M: f32 = 45.0;
    const LOOKAHEAD_M: f32 = 16.0;
    let (track, entry, exit) = brands_patch_before_the_chicane(APPROACH_M);
    let finish = exit + 5.0;
    // The chicane's entry and exit lie on one line along the hill straight;
    // the straight line between them crosses the sand on the inside of the
    // middle bend.
    let exit_point = track.point_at(exit);
    let (cut_ticks, cut_surfaces) = ticks_and_surfaces_to_pass(track.clone(), finish, |state| {
        aim(state, [exit_point[0], exit_point[2]])
    });
    let (driven_ticks, driven_surfaces) =
        ticks_and_surfaces_to_pass(track.clone(), finish, |state| {
            let ahead = state
                .track()
                .point_at(state.standings()[0].spline_progress + LOOKAHEAD_M);
            aim(state, [ahead[0], ahead[2]])
        });

    assert!(
        cut_surfaces.contains(&Surface::Sand),
        "the cut crosses the sand: {cut_surfaces:?}"
    );
    assert_eq!(
        driven_surfaces,
        [Surface::Asphalt],
        "the driven line stays on the asphalt"
    );
    assert!(
        cut_ticks > driven_ticks + ticks(0.3),
        "cut in {cut_ticks} ticks, driven in {driven_ticks}; from {} m before the {} m chicane",
        APPROACH_M,
        exit - entry
    );
}

#[test]
fn a_boost_pad_facing_the_other_way_boosts_a_kart_on_it_and_not_one_beside_it() {
    use sim::tuning::{BOOST_PAD_HALF_LENGTH_M, BOOST_PAD_HALF_WIDTH_M};
    // The back straight's pad at (-30, -55) faces +Z. One kart drives up
    // its centre from 45 m back; another drives the same way just outside
    // its width.
    let track = Track::from_sidecar(OVAL).expect("the oval loads");
    let pad = track.boost_pads()[0];
    assert_eq!(pad.position, [-30.0, 0.0, -55.0]);
    assert!(
        (pad.yaw.abs() - std::f32::consts::PI).abs() < 1e-3,
        "faces +Z"
    );
    let facing_plus_z = std::f32::consts::PI;
    let beside = BOOST_PAD_HALF_WIDTH_M + 0.6;

    let mut on_the_pad = State::new(
        SEED,
        1,
        oval_with_slot_0_at([-30.0, 0.0, -100.0], facing_plus_z),
    );
    let mut first_boost = None;
    for _ in 0..ticks(4.0) {
        on_the_pad = step(on_the_pad, &[throttle()], DT);
        let kart = on_the_pad.kart_transforms()[0];
        if kart.speed > TOP_SPEED_M_PER_S && first_boost.is_none() {
            first_boost = Some(kart.position);
        }
    }
    let first_boost = first_boost.expect("the pad boosted the kart on it");
    assert!(
        (first_boost[2] - pad.position[2]).abs() < BOOST_PAD_HALF_LENGTH_M + 1.0,
        "boosted on the pad, not at {first_boost:?}"
    );

    let mut beside_the_pad = State::new(
        SEED,
        1,
        oval_with_slot_0_at([-30.0 - beside, 0.0, -100.0], facing_plus_z),
    );
    for _ in 0..ticks(4.0) {
        beside_the_pad = step(beside_the_pad, &[throttle()], DT);
        let kart = beside_the_pad.kart_transforms()[0];
        assert!(
            kart.speed <= TOP_SPEED_M_PER_S,
            "boosted beside the pad: {kart:?}"
        );
    }
    assert!(
        beside_the_pad.kart_transforms()[0].position[2] > pad.position[2] + 10.0,
        "drove past the pad"
    );
}

#[test]
fn a_hop_over_the_sand_does_not_escape_the_cap() {
    use sim::tuning::KART_HALF_HEIGHT_M;
    // Slot 0 moved to the right of the front straight and turned a little
    // right, so it drifts toward the sand at x = 36 while getting up to
    // speed, and hops just before it reaches the edge.
    let mut state = State::new(SEED, 1, oval_with_slot_0_at([33.0, 0.0, 4.0], -0.08));
    let cap = OFF_TRACK_SPEED_FACTOR * TOP_SPEED_M_PER_S;
    let mut hop = false;
    let mut speed_at_the_hop = 0.0;
    let mut airborne_over_sand = 0;
    let mut landed = false;
    for _ in 0..ticks(6.0) {
        let surface = state.kart_surface(0);
        let before = state.kart_transforms()[0];
        if !hop && before.position[0] >= 35.8 {
            hop = true;
            speed_at_the_hop = before.speed;
        }
        state = step(state, &[KartInput { hop, ..throttle() }], DT);
        let kart = state.kart_transforms()[0];
        if surface != Surface::Sand {
            continue;
        }
        // The cap reads what is below the kart, in the air or on the ground.
        assert!(
            kart.speed <= cap + 0.01,
            "escaped the cap over the sand: {kart:?}"
        );
        if kart.position[1] > KART_HALF_HEIGHT_M + 0.05 {
            airborne_over_sand += 1;
        } else if airborne_over_sand > 0 {
            landed = true;
            break;
        }
    }
    assert!(
        speed_at_the_hop > cap + 2.0,
        "hopped at {speed_at_the_hop} m/s"
    );
    assert!(
        airborne_over_sand >= 5,
        "{airborne_over_sand} ticks in the air over the sand"
    );
    assert!(landed, "came down on the sand");
}

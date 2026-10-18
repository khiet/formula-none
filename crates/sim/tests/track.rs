//! Tracks through the public seam: load a sidecar, start a race on it, ask
//! what is under a point, drive into things. The exported fixtures under
//! `assets/tracks` are the inputs; their layouts document the numbers used.

use sim::tuning::{KART_HALF_EXTENT_M, TICKS_PER_SECOND};
use sim::{step, KartInput, State, Surface, Track, TrackError};

const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");
const BRANDS_PATCH: &str =
    include_str!("../../../assets/tracks/brands-patch/brands-patch.track.json");
const DT: f32 = 1.0 / TICKS_PER_SECOND as f32;
const SEED: u64 = 0x5EED;

fn oval() -> Track {
    Track::from_sidecar(OVAL).expect("the oval fixture loads")
}

fn brands_patch() -> Track {
    Track::from_sidecar(BRANDS_PATCH).expect("Brands Patch loads")
}

fn ticks(seconds: f32) -> usize {
    (seconds * TICKS_PER_SECOND as f32).round() as usize
}

fn throttle() -> KartInput {
    KartInput {
        throttle: 1.0,
        ..KartInput::default()
    }
}

/// Nearest centreline index to a point, for ordering things along the lap.
fn centreline_index(track: &Track, point: [f32; 3]) -> usize {
    let distance = |a: [f32; 3]| (0..3).map(|i| (a[i] - point[i]).powi(2)).sum::<f32>();
    (0..track.centreline().len())
        .min_by(|&a, &b| {
            distance(track.centreline()[a]).total_cmp(&distance(track.centreline()[b]))
        })
        .expect("a centreline has points")
}

#[test]
fn the_oval_fixture_has_eight_checkpoints_on_a_438_metre_lap() {
    let track = oval();
    // Two 125 m straights joined by 30 m radius turns (layouts/oval.py).
    let expected_length = 2.0 * 125.0 + 2.0 * std::f32::consts::PI * 30.0;

    assert_eq!(track.checkpoints().len(), 8);
    assert!(
        (track.spline_length() - expected_length).abs() < 0.5,
        "lap is {} m, expected about {expected_length}",
        track.spline_length()
    );
}

#[test]
fn brands_patch_carries_the_whole_layout() {
    let track = brands_patch();

    assert_eq!(track.name(), "brands-patch");
    assert_eq!(track.checkpoints().len(), 12);
    assert_eq!(track.item_boxes().len(), 8);
    assert_eq!(track.boost_pads().len(), 2);
    assert_eq!(track.spawn_grid().len(), 8);
    assert!(
        (760.0..=840.0).contains(&track.spline_length()),
        "about 800 m, got {}",
        track.spline_length()
    );
    for surface in [
        Surface::Asphalt,
        Surface::Sand,
        Surface::Grass,
        Surface::Wall,
    ] {
        assert!(
            track.surfaces().iter().any(|mesh| mesh.surface == surface),
            "no {surface:?} mesh"
        );
    }
}

#[test]
fn item_boxes_stand_in_two_rows_of_four() {
    let track = brands_patch();
    let mut rows: Vec<usize> = track
        .item_boxes()
        .iter()
        .map(|item| centreline_index(&track, item.position))
        .collect();
    rows.sort_unstable();
    rows.dedup();
    assert_eq!(
        rows.len(),
        2,
        "boxes sit at two points along the lap: {rows:?}"
    );
}

#[test]
fn checkpoints_run_in_lap_order_from_the_start_line() {
    for track in [oval(), brands_patch()] {
        let indices: Vec<usize> = track
            .checkpoints()
            .iter()
            .map(|checkpoint| centreline_index(&track, checkpoint.position))
            .collect();
        assert_eq!(
            indices[0],
            0,
            "{}: the first checkpoint is the start line",
            track.name()
        );
        assert!(
            indices.windows(2).all(|pair| pair[0] < pair[1]),
            "{}: checkpoints out of order along the centreline: {indices:?}",
            track.name()
        );
    }
}

#[test]
fn the_spawn_grid_sits_just_behind_the_start_line_facing_it() {
    let track = oval();
    let start = track.checkpoints()[0];
    for slot in track.spawn_grid() {
        let behind = slot.position[2] - start.position[2];
        assert!(
            (1.0..30.0).contains(&behind),
            "slot is {behind} m behind the line"
        );
        assert_eq!(slot.yaw, start.yaw);
    }
}

#[test]
fn a_sidecar_the_sim_cannot_race_on_is_rejected() {
    let other_version = OVAL.replacen("\"version\":1", "\"version\":2", 1);
    assert!(matches!(
        Track::from_sidecar(&other_version),
        Err(TrackError::UnsupportedVersion(2))
    ));

    assert!(matches!(
        Track::from_sidecar(&OVAL[..OVAL.len() / 2]),
        Err(TrackError::Json(_))
    ));

    let no_spawn = OVAL.replacen("\"spawn_grid\":[", "\"spawn_grid\":[],\"unused\":[", 1);
    assert!(matches!(
        Track::from_sidecar(&no_spawn),
        Err(TrackError::Invalid(_))
    ));

    let bad_index = OVAL.replacen("\"indices\":[", "\"indices\":[999999,0,1,", 1);
    assert!(matches!(
        Track::from_sidecar(&bad_index),
        Err(TrackError::Invalid(_))
    ));
}

#[test]
fn karts_start_on_the_spawn_grid_in_slot_order() {
    let track = oval();
    let state = State::new(SEED, 8, track.clone());
    let transforms = state.kart_transforms();

    assert_eq!(transforms.len(), 8);
    for (kart, slot) in transforms.iter().zip(track.spawn_grid()) {
        assert_eq!(kart.position[0], slot.position[0]);
        assert_eq!(kart.position[2], slot.position[2]);
        assert!((kart.position[1] - slot.position[1] - KART_HALF_EXTENT_M).abs() < 1e-5);
        assert_eq!(
            kart.rotation,
            [0.0, 0.0, 0.0, 1.0],
            "the oval grid faces -Z"
        );
    }
}

#[test]
fn a_spawn_slot_yaw_turns_the_kart_counter_clockwise_seen_from_above() {
    // Yaw is radians about +Y, so a quarter turn from -Z faces -X. The
    // exporter and the sim must agree on this or every grid faces wrong.
    let quarter = std::f32::consts::FRAC_PI_2;
    let sidecar = OVAL.replacen(
        "\"spawn_grid\":[{\"position\":[28.0,0.0,4.0],\"yaw\":0.0}",
        &format!("\"spawn_grid\":[{{\"position\":[28.0,0.0,4.0],\"yaw\":{quarter}}}"),
        1,
    );
    assert_ne!(
        sidecar, OVAL,
        "slot 0 of the oval is where this test expects it"
    );
    let mut state = State::new(SEED, 1, Track::from_sidecar(&sidecar).unwrap());
    let spawn = state.kart_transforms()[0];
    for _ in 0..ticks(1.0) {
        state = step(state, &[throttle()], DT);
    }
    let kart = state.kart_transforms()[0];

    assert!(
        kart.position[0] < spawn.position[0] - 1.0,
        "drove toward -X: {kart:?}"
    );
    assert!(
        (kart.position[2] - spawn.position[2]).abs() < 0.1,
        "held its line: {kart:?}"
    );
}

#[test]
#[should_panic(expected = "spawn slots")]
fn more_karts_than_spawn_slots_is_a_caller_error() {
    State::new(SEED, 9, oval());
}

#[test]
fn every_spawn_slot_is_on_asphalt_before_the_first_tick() {
    for track in [oval(), brands_patch()] {
        let state = State::new(SEED, 8, track);
        for kart in 0..8 {
            assert_eq!(
                state.kart_surface(kart),
                Surface::Asphalt,
                "{:?} slot {kart}",
                state.track()
            );
        }
    }
}

#[test]
fn the_surface_beside_the_track_is_sand_then_grass_then_nothing() {
    // The oval's front straight: centreline at x = 30, asphalt 6 m either
    // side, sand for 6 m beyond, grass for 40 m beyond the track's bounds.
    let state = State::new(SEED, 1, oval());
    let at = |x: f32| state.surface_at([x, 0.0, 4.0]);

    assert_eq!(at(30.0), Surface::Asphalt);
    assert_eq!(at(39.0), Surface::Sand);
    assert_eq!(at(50.0), Surface::Grass);
    assert_eq!(at(110.0), Surface::OutOfWorld);
    assert_eq!(state.surface_at([0.0, 0.0, 1000.0]), Surface::OutOfWorld);
}

#[test]
fn walls_and_pipes_read_as_wall_from_above() {
    let track = oval();
    let state = State::new(SEED, 1, track.clone());
    let walls: Vec<_> = track
        .surfaces()
        .iter()
        .filter(|mesh| mesh.surface == Surface::Wall)
        .collect();
    assert!(walls.len() >= 2, "the oval has walls and pipes");

    for mesh in walls {
        let vertices: Vec<[f32; 3]> = mesh.vertices().collect();
        // A triangle facing up is part of a top; probe above its centre.
        let top = mesh
            .triangles()
            .map(|[a, b, c]| {
                [
                    vertices[a as usize],
                    vertices[b as usize],
                    vertices[c as usize],
                ]
            })
            .find(|[a, b, c]| {
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let normal_y = u[2] * v[0] - u[0] * v[2];
                let area = (u[1] * v[2] - u[2] * v[1])
                    .hypot(normal_y)
                    .hypot(u[0] * v[1] - u[1] * v[0]);
                normal_y.abs() > 0.9 * area
            })
            .unwrap_or_else(|| panic!("{} has no top face", mesh.name));
        let centre = [
            (top[0][0] + top[1][0] + top[2][0]) / 3.0,
            (top[0][1] + top[1][1] + top[2][1]) / 3.0 + 1.0,
            (top[0][2] + top[1][2] + top[2][2]) / 3.0,
        ];
        assert_eq!(state.surface_at(centre), Surface::Wall, "{}", mesh.name);
    }
}

#[test]
fn a_kart_driving_into_a_pipe_stops_at_it() {
    // Oval slot 0 drives up the front straight, into the first turn and
    // head on into the pipe centred on its column at z = -107.67.
    let mut state = State::new(SEED, 1, oval());
    for _ in 0..ticks(7.0) {
        state = step(state, &[throttle()], DT);
        let kart = state.kart_transforms()[0];
        assert!(
            kart.position[2] > -107.67,
            "went through the pipe: {kart:?}"
        );
    }
    let kart = state.kart_transforms()[0];
    assert!(
        kart.position[2] < -102.0,
        "never reached the pipe: {kart:?}"
    );
    assert!(
        (kart.position[1] - KART_HALF_EXTENT_M).abs() < 0.2,
        "left the ground: {kart:?}"
    );
}

#[test]
fn a_kart_driving_into_a_wall_is_held_by_it() {
    // Oval slot 1 at x = 32 drives straight off the first turn's asphalt,
    // across the sand and grass, and into the wall that rings the turn 42 m
    // from its centre at (0, -100). The wall turns it rather than stopping it.
    let turn_centre = [0.0_f32, -100.0];
    let radius =
        |position: [f32; 3]| (position[0] - turn_centre[0]).hypot(position[2] - turn_centre[1]);
    let mut state = State::new(SEED, 2, oval());
    let idle = KartInput::default();
    for _ in 0..ticks(8.0) {
        state = step(state, &[idle, throttle()], DT);
        let kart = state.kart_transforms()[1];
        let in_the_turn = kart.position[2] < turn_centre[1];
        assert!(
            !in_the_turn || radius(kart.position) < 42.0,
            "went through the wall: {kart:?}"
        );
    }
    let kart = state.kart_transforms()[1];
    assert!(
        radius(kart.position) > 40.5,
        "never reached the wall: {kart:?}"
    );
    assert!(
        (kart.position[1] - KART_HALF_EXTENT_M + 0.05).abs() < 0.2,
        "left the ground: {kart:?}"
    );
}

#[test]
fn a_kart_drives_brands_patch_without_falling_through() {
    let mut state = State::new(SEED, 1, brands_patch());
    let spawn = state.kart_transforms()[0];
    for _ in 0..ticks(3.0) {
        state = step(state, &[throttle()], DT);
    }
    let kart = state.kart_transforms()[0];

    assert!(
        kart.position[2] < spawn.position[2] - 30.0,
        "drove up the straight: {kart:?}"
    );
    assert!(
        (kart.position[1] - KART_HALF_EXTENT_M).abs() < 0.1,
        "fell or flew: {kart:?}"
    );
    assert_eq!(state.kart_surface(0), Surface::Asphalt);
}

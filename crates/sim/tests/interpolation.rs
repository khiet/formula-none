//! The projectiles as the interpolation draws them: between the two
//! snapshots either side of its render clock, matched across them by owner
//! and target, held where the earlier snapshot had one the later no longer
//! carries, and not drawn before the snapshot that first carries one.

use sim::tuning::SNAPSHOTS_PER_SECOND;
use sim::tuning::TICKS_PER_SECOND;
use sim::{Interpolation, Projectile};

const TICKS_PER_SNAPSHOT: u32 = TICKS_PER_SECOND / SNAPSHOTS_PER_SECOND;

fn projectile(x: f32, owner: usize, target: usize) -> Projectile {
    Projectile {
        position: [x, 0.0, 0.0],
        owner,
        target,
    }
}

/// Snapshots at the snapshot rate from tick 0, `count` of them, each with
/// `at(tick)`'s projectiles, pushed as they would arrive, and the clock run
/// on until it draws between the second and third.
fn drawing_between_the_second_and_third(
    count: u32,
    at: impl Fn(u32) -> Vec<Projectile>,
) -> Interpolation {
    let mut interpolation = Interpolation::new();
    for snapshot in 0..count {
        let tick = snapshot * TICKS_PER_SNAPSHOT;
        interpolation.push(tick, Vec::new(), at(tick));
    }
    while interpolation
        .render_tick()
        .is_none_or(|render| render < TICKS_PER_SNAPSHOT as f32 + 0.5)
    {
        interpolation.advance();
    }
    interpolation
}

#[test]
fn a_projectile_is_drawn_between_the_snapshots_either_side_of_the_render_clock() {
    // The one projectile flies +X a metre a snapshot.
    let interpolation =
        drawing_between_the_second_and_third(6, |tick| vec![projectile(tick as f32, 3, 2)]);
    let render = interpolation.render_tick().unwrap();
    assert!(render > TICKS_PER_SNAPSHOT as f32 && render < 2.0 * TICKS_PER_SNAPSHOT as f32);
    let [drawn] = interpolation.projectiles()[..] else {
        panic!("one projectile: {:?}", interpolation.projectiles());
    };
    assert_eq!((drawn.owner, drawn.target), (3, 2));
    assert!(
        (drawn.position[0] - render).abs() < 1e-4,
        "drawn at x {} for a clock at {render}",
        drawn.position[0]
    );
}

#[test]
fn a_projectile_gone_from_the_later_snapshot_stays_where_the_earlier_had_it() {
    // Kart 1's projectile reaches its target between the second and third
    // snapshots; kart 5's, fired between them, flies on.
    let interpolation = drawing_between_the_second_and_third(6, |tick| {
        let mut flying = Vec::new();
        if tick < 2 * TICKS_PER_SNAPSHOT {
            flying.push(projectile(tick as f32, 1, 0));
        }
        if tick >= 2 * TICKS_PER_SNAPSHOT {
            flying.push(projectile(100.0 + tick as f32, 5, 4));
        }
        flying
    });
    let [drawn] = interpolation.projectiles()[..] else {
        panic!(
            "only the one the earlier snapshot carries: {:?}",
            interpolation.projectiles()
        );
    };
    assert_eq!(drawn, projectile(TICKS_PER_SNAPSHOT as f32, 1, 0));
}

#[test]
fn projectiles_are_matched_across_snapshots_in_fired_order() {
    // Two projectiles of the same owner at the same target: the first in
    // fired order follows the first, whatever their positions.
    let interpolation = drawing_between_the_second_and_third(6, |tick| {
        vec![
            projectile(tick as f32, 2, 1),
            projectile(50.0 + tick as f32, 2, 1),
        ]
    });
    let render = interpolation.render_tick().unwrap();
    let drawn = interpolation.projectiles();
    assert_eq!(drawn.len(), 2);
    assert!((drawn[0].position[0] - render).abs() < 1e-4);
    assert!((drawn[1].position[0] - (50.0 + render)).abs() < 1e-4);
}

#[test]
fn nothing_is_drawn_before_the_first_snapshot() {
    let mut interpolation = Interpolation::new();
    assert!(interpolation.projectiles().is_empty());
    interpolation.advance();
    assert!(interpolation.projectiles().is_empty());
}

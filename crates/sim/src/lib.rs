//! The rules of the game, run by the browser (through `bridge`) and the
//! server. Must stay free of I/O, async runtimes and anything else from this
//! repo so both sides run identical rules.
//!
//! The public seam is [`step`]: feed a [`State`] one tick of [`KartInput`]
//! per kart and get the next state back. Everything a renderer needs is read
//! through [`State::kart_transforms`]. A race runs on a [`Track`] loaded from
//! the sidecar exported next to its glTF.

pub mod track;
pub mod tuning;

pub use track::{Checkpoint, Placement, Surface, SurfaceMesh, Track, TrackError};

use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rapier3d::prelude::*;

/// One kart's controls for one tick, already mapped from whatever device
/// produced them. Bots and humans both speak this.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KartInput {
    /// -1 is full left, 1 is full right.
    pub steer: f32,
    /// 0 to 1.
    pub throttle: f32,
    /// 0 to 1.
    pub brake: f32,
}

/// What the renderer and the Instrument panel need per kart: where it is, in
/// metres and as a unit quaternion `[x, y, z, w]`, and how fast it is going.
/// Y is up and a kart with identity rotation faces -Z, matching Three.js.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KartTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    /// Metres per second along the kart's facing.
    pub speed: f32,
}

/// Everything the sim knows about a race. Owns the physics world and the
/// RNG, so a `State` plus an input sequence replays identically anywhere.
pub struct State {
    #[expect(
        dead_code,
        reason = "consumed by item rolls and bots in later milestones"
    )]
    rng: ChaCha8Rng,
    track: Track,
    karts: Vec<Kart>,
    physics: Physics,
}

struct Kart {
    body: RigidBodyHandle,
    /// Speed along the kart's facing, metres per second. Held outside Rapier
    /// so contacts and friction never leak into the handling model.
    speed: f32,
}

/// Rapier's working sets. One struct so `step` passes them around as a unit.
struct Physics {
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd: CCDSolver,
}

use tuning::{GRAVITY_M_PER_S2, KART_EDGE_RADIUS_M, KART_HALF_EXTENT_M};

/// A surface query starts this far above the asked point, so a point lying
/// exactly on a surface still finds it despite rounding.
const SURFACE_PROBE_RISE_M: f32 = 0.1;

impl State {
    /// A fresh race on `track` with `kart_count` karts on its spawn grid, in
    /// slot order. `kart_count` must not exceed the track's spawn slots.
    pub fn new(seed: u64, kart_count: usize, track: Track) -> Self {
        assert!(
            kart_count <= track.spawn_grid().len(),
            "{} karts but the track has {} spawn slots",
            kart_count,
            track.spawn_grid().len()
        );
        let mut physics = Physics::new();
        let ground = physics.bodies.insert(RigidBodyBuilder::fixed().build());
        for (surface, shape) in track.colliders() {
            physics.colliders.insert_with_parent(
                ColliderBuilder::new(shape.clone())
                    .friction(0.0)
                    .user_data(u128::from(surface.code()))
                    .build(),
                ground,
                &mut physics.bodies,
            );
        }

        let karts = track.spawn_grid()[..kart_count]
            .iter()
            .map(|slot| {
                let body = RigidBodyBuilder::dynamic()
                    .translation(Vec3::from(slot.position) + Vec3::Y * KART_HALF_EXTENT_M)
                    .rotation(Vec3::Y * slot.yaw)
                    // Steering is a yaw we set each tick; the box must never
                    // tip or roll from a contact.
                    .enabled_rotations(false, true, false)
                    .build();
                let body = physics.bodies.insert(body);
                let core = KART_HALF_EXTENT_M - KART_EDGE_RADIUS_M;
                physics.colliders.insert_with_parent(
                    ColliderBuilder::round_cuboid(core, core, core, KART_EDGE_RADIUS_M)
                        .friction(0.0)
                        .restitution(0.0)
                        .build(),
                    body,
                    &mut physics.bodies,
                );
                Kart { body, speed: 0.0 }
            })
            .collect();
        physics.index_colliders();

        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            track,
            karts,
            physics,
        }
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    /// What lies under `point`: the topmost tagged surface at or below it,
    /// or `OutOfWorld` when there is none. Karts are never an answer.
    pub fn surface_at(&self, point: [f32; 3]) -> Surface {
        let ray = Ray::new(
            Vec3::from(point) + Vec3::Y * SURFACE_PROBE_RISE_M,
            Vec3::NEG_Y,
        );
        let physics = &self.physics;
        let pipeline = physics.broad_phase.as_query_pipeline(
            physics.narrow_phase.query_dispatcher(),
            &physics.bodies,
            &physics.colliders,
            QueryFilter::only_fixed(),
        );
        pipeline
            .cast_ray(&ray, f32::MAX, true)
            .and_then(|(collider, _)| {
                Surface::from_code(physics.colliders[collider].user_data as u8)
            })
            .unwrap_or(Surface::OutOfWorld)
    }

    /// The surface under a kart's centre.
    pub fn kart_surface(&self, kart: usize) -> Surface {
        self.surface_at(self.kart_transforms()[kart].position)
    }

    /// One transform per kart, in grid order.
    pub fn kart_transforms(&self) -> Vec<KartTransform> {
        self.karts
            .iter()
            .map(|kart| {
                let body = &self.physics.bodies[kart.body];
                let position = body.translation();
                let rotation = body.rotation();
                KartTransform {
                    position: [position.x, position.y, position.z],
                    rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
                    speed: kart.speed,
                }
            })
            .collect()
    }
}

/// Advance the race by one tick of `dt` seconds. `inputs` holds one entry per
/// kart in grid order; the caller must supply exactly as many as there are karts.
pub fn step(mut state: State, inputs: &[KartInput], dt: f32) -> State {
    assert_eq!(inputs.len(), state.karts.len(), "one input per kart");
    for (kart, input) in state.karts.iter_mut().zip(inputs) {
        kart.speed = next_speed(kart.speed, input, dt);
        let body = &mut state.physics.bodies[kart.body];
        let forward = *body.rotation() * Vec3::NEG_Z;
        // Keep whatever vertical velocity gravity and the plane have agreed on.
        let vertical = body.linvel().y;
        body.set_linvel(forward * kart.speed + Vec3::Y * vertical, true);
        body.set_angvel(Vec3::Y * yaw_rate(kart.speed, input.steer), true);
    }
    state.physics.step(dt);
    state
}

/// Speed after one tick: throttle ramps toward top speed, brake and coasting
/// ramp toward zero, each at its tuned rate.
fn next_speed(speed: f32, input: &KartInput, dt: f32) -> f32 {
    use tuning::{ACCELERATION_TO_TOP_S, BRAKE_TO_STOP_S, COAST_TO_STOP_S, TOP_SPEED_M_PER_S};
    let throttle = input.throttle.clamp(0.0, 1.0);
    let brake = input.brake.clamp(0.0, 1.0);
    let accelerate = throttle * TOP_SPEED_M_PER_S / ACCELERATION_TO_TOP_S;
    let braking = brake * TOP_SPEED_M_PER_S / BRAKE_TO_STOP_S;
    let coast = if throttle == 0.0 && brake == 0.0 {
        TOP_SPEED_M_PER_S / COAST_TO_STOP_S
    } else {
        0.0
    };
    (speed + (accelerate - braking - coast) * dt).clamp(0.0, TOP_SPEED_M_PER_S)
}

/// Yaw rate for a steer input: positive steer turns right, which is a negative
/// rotation about +Y. Faster karts steer slower, and a near-stationary kart
/// barely turns at all.
fn yaw_rate(speed: f32, steer: f32) -> f32 {
    use tuning::{
        STEER_FULL_EFFECT_SPEED_M_PER_S, STEER_RATE_LOW_SPEED_RAD_PER_S,
        STEER_RATE_TOP_SPEED_RAD_PER_S, TOP_SPEED_M_PER_S,
    };
    let speed_fraction = (speed / TOP_SPEED_M_PER_S).clamp(0.0, 1.0);
    let rate = STEER_RATE_LOW_SPEED_RAD_PER_S
        + (STEER_RATE_TOP_SPEED_RAD_PER_S - STEER_RATE_LOW_SPEED_RAD_PER_S) * speed_fraction;
    let grip = (speed / STEER_FULL_EFFECT_SPEED_M_PER_S).clamp(0.0, 1.0);
    -steer.clamp(-1.0, 1.0) * rate * grip
}

impl Physics {
    fn new() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            integration: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd: CCDSolver::new(),
        }
    }

    /// Register newly inserted colliders with the broad phase so scene queries
    /// see the track before the first tick; `step` does this itself later.
    fn index_colliders(&mut self) {
        CollisionPipeline::new().step(
            self.integration.prediction_distance(),
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &(),
            &(),
        );
    }

    fn step(&mut self, dt: f32) {
        self.integration.dt = dt;
        self.pipeline.step(
            Vec3::new(0.0, GRAVITY_M_PER_S2, 0.0),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd,
            &(),
            &(),
        );
    }
}

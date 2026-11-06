//! Which items spin out which karts. `step` asks once a tick, after the
//! karts have moved, and the Air Horn asks at its fire; each answer is the
//! hits to apply, in order, with the items it uses up already gone from the
//! lists it was handed. Who an item spares is one rule, [`fate`]: a star
//! kart, a kart already spun out, a kart on the lane's safe ground (ADR
//! 0081) and a kart this pass of the runaway wheel has hit (ADRs 0075, 0076).

use super::{at_centreline_height, behind, FlyingProjectile, Kart, PlacedHazard, Track};
use crate::tuning::{
    HAZARD_RADIUS_M, HOMING_LOCK_RANGE_M, HOMING_LOOKAHEAD_M, HOMING_RADIUS_M, HOMING_SPEED_FACTOR,
    HORN_PROGRESS_WINDOW_M, HORN_RADIUS_M, KART_HALF_LENGTH_M, KART_HALF_WIDTH_M,
    STAR_HIT_RADIUS_M, TOP_SPEED_M_PER_S, WHEEL_RADIUS_M,
};
use rapier3d::prelude::{RigidBody, Rotation, Vec3};

/// A kart as an item judges it: where its box stands and what spares it.
/// A hit marks it spun out, so the items after it in a tick pass it by as
/// they would a kart spun out before the tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Target {
    translation: Vec3,
    rotation: Rotation,
    progress: f32,
    starring: bool,
    spun_out: bool,
    in_pit_lane: bool,
    wheel_pass_hit: u32,
}

impl Target {
    pub(crate) fn of(kart: &Kart, body: &RigidBody, in_pit_lane: bool) -> Self {
        Self {
            translation: body.translation(),
            rotation: *body.rotation(),
            progress: kart.spline_progress,
            starring: kart.starring(),
            spun_out: kart.spun_out(),
            in_pit_lane,
            wheel_pass_hit: kart.wheel_pass_hit,
        }
    }

    /// Whether the kart reaches a hazard or a projectile centred at
    /// `point`: the point lies within `radius` of its collision box, seen
    /// from above.
    fn reaches(&self, point: [f32; 3], radius: f32) -> bool {
        reaches(self.translation, &self.rotation, point, radius)
    }
}

/// What spun a kart out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Source {
    /// The star kart in this slot touched it.
    Star(usize),
    /// It reached a hazard the kart in this slot fired.
    Hazard(usize),
    /// This pass of the runaway wheel reached it.
    Wheel(u32),
    /// A projectile the kart in this slot fired reached it.
    Projectile(usize),
    /// The kart in this slot blasted its Air Horn.
    Horn(usize),
}

impl Source {
    /// The kart whose item or star it was; the runaway wheel is no one's.
    pub(crate) fn by(self) -> Option<usize> {
        match self {
            Self::Star(slot) | Self::Hazard(slot) | Self::Projectile(slot) | Self::Horn(slot) => {
                Some(slot)
            }
            Self::Wheel(_) => None,
        }
    }
}

/// A kart to spin out, and what spun it out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hit {
    pub(crate) kart: usize,
    pub(crate) source: Source,
}

/// An item that has reached a kart, as [`fate`] judges it.
#[derive(Debug, Clone, Copy)]
enum Reaching {
    Star,
    HeldHazard,
    DroppedHazard,
    Wheel {
        pass: u32,
    },
    /// A projectile is judged on every tick it chases the kart, since the
    /// lane makes it give up wherever it is.
    Projectile {
        reached: bool,
        shielded: bool,
    },
    Horn {
        shielded: bool,
    },
}

/// What becomes of the kart and the item.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fate {
    /// The kart spins out.
    Hit,
    /// The kart is left alone and the item carries on.
    Passes,
    /// The item is used up to no effect.
    UsedUp,
    /// The hazard the kart trails is used up in the kart's place.
    Shielded,
}

/// The immunity rule, every item's in one place. Each arm keeps its item's
/// order of tests: of a kart both spun out this tick and still trailing a
/// hazard, a projectile takes the hazard and a horn leaves both alone.
fn fate(target: &Target, item: Reaching) -> Fate {
    let Target {
        starring,
        spun_out,
        in_pit_lane,
        ..
    } = *target;
    match item {
        Reaching::Star | Reaching::HeldHazard if starring || spun_out || in_pit_lane => {
            Fate::Passes
        }
        Reaching::DroppedHazard if spun_out || in_pit_lane => Fate::Passes,
        // A star kart drives over a dropped hazard and uses it up.
        Reaching::DroppedHazard if starring => Fate::UsedUp,
        Reaching::Star | Reaching::HeldHazard | Reaching::DroppedHazard => Fate::Hit,
        // The wheel is no item, so the lane does not stop it: ADR 0081
        // spares lane karts from the items alone.
        Reaching::Wheel { pass } if starring || spun_out || target.wheel_pass_hit == pass => {
            Fate::Passes
        }
        Reaching::Wheel { .. } => Fate::Hit,
        Reaching::Projectile { .. } if in_pit_lane => Fate::UsedUp,
        Reaching::Projectile { reached: false, .. } => Fate::Passes,
        Reaching::Projectile { .. } if starring => Fate::UsedUp,
        Reaching::Projectile { shielded: true, .. } => Fate::Shielded,
        Reaching::Projectile { .. } if spun_out => Fate::UsedUp,
        Reaching::Projectile { .. } => Fate::Hit,
        Reaching::Horn { .. } if in_pit_lane || spun_out || starring => Fate::Passes,
        Reaching::Horn { shielded: true } => Fate::Shielded,
        Reaching::Horn { .. } => Fate::Hit,
    }
}

/// The index of the hazard the kart in `slot` trails, if any.
fn shield(hazards: &[PlacedHazard], slot: usize) -> Option<usize> {
    hazards
        .iter()
        .position(|placed| placed.hazard.held && placed.hazard.owner == slot)
}

/// The tick's hits from every item but the Air Horn, in the order to apply
/// them: the star karts', then the hazards', the runaway wheel's and the
/// projectiles'. `targets` are the karts where they ended the tick, `holds`
/// whether each holds the item button, `wheels` each rolling pass of the
/// runaway wheel and where it is. On the way the held hazards trail their
/// owners, the dropped ones age, the projectiles fly one tick of `dt`
/// seconds, and every item used up leaves its list.
pub(crate) fn resolve(
    track: &Track,
    mut targets: Vec<Target>,
    holds: &[bool],
    wheels: &[(u32, [f32; 3])],
    hazards: &mut Vec<PlacedHazard>,
    projectiles: &mut Vec<FlyingProjectile>,
    dt: f32,
) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut hit = |targets: &mut [Target], kart: usize, source: Source| {
        targets[kart].spun_out = true;
        if let Source::Wheel(pass) = source {
            targets[kart].wheel_pass_hit = pass;
        }
        hits.push(Hit { kart, source });
    };
    let mut holds = holds.to_vec();
    // A star kart spins out every kart whose box its centre reaches, and
    // passes through at full speed; another star kart it bumps as usual.
    // The spin-out ends a hold on a hazard, which is left where it is: the
    // hazard shields nobody from a star.
    for star in 0..targets.len() {
        let carrier = targets[star];
        if !carrier.starring || carrier.in_pit_lane {
            continue;
        }
        let centre = carrier.translation.to_array();
        for slot in 0..targets.len() {
            if slot == star || !targets[slot].reaches(centre, STAR_HIT_RADIUS_M) {
                continue;
            }
            if fate(&targets[slot], Reaching::Star) == Fate::Hit {
                hit(&mut targets, slot, Source::Star(star));
                holds[slot] = false;
            }
        }
    }
    // A held hazard trails its owner to where the owner ended the tick;
    // letting go of the button, or losing the controls, leaves it there.
    for placed in hazards.iter_mut().filter(|placed| placed.hazard.held) {
        let owner = &targets[placed.hazard.owner];
        if holds[placed.hazard.owner] {
            placed.hazard.position = behind(owner.translation, &owner.rotation);
            placed.progress = owner.progress;
        } else {
            placed.hazard.held = false;
        }
    }
    // A dropped hazard ages away; one still lying there spins out the first
    // kart to reach it, its owner included once the owner has got clear of
    // it, and is used up.
    hazards.retain_mut(|placed| {
        // No hazard lands in the lane: one let go over it is gone.
        if track.in_pit_lane(placed.hazard.position) {
            return false;
        }
        if !placed.hazard.held {
            placed.lifetime_left_s -= dt;
            if placed.lifetime_left_s <= 0.0 {
                return false;
            }
        }
        for slot in 0..targets.len() {
            let reached = targets[slot].reaches(placed.hazard.position, HAZARD_RADIUS_M);
            if slot == placed.hazard.owner && !placed.clear_of_owner {
                placed.clear_of_owner = !reached;
                continue;
            }
            if !reached {
                continue;
            }
            let item = if placed.hazard.held {
                Reaching::HeldHazard
            } else {
                Reaching::DroppedHazard
            };
            match fate(&targets[slot], item) {
                Fate::Passes => continue,
                Fate::Hit => hit(&mut targets, slot, Source::Hazard(placed.hazard.owner)),
                Fate::UsedUp | Fate::Shielded => {}
            }
            return false;
        }
        true
    });
    // A rolling wheel spins out every kart it reaches, once a pass.
    for &(pass, position) in wheels {
        for slot in 0..targets.len() {
            if targets[slot].reaches(position, WHEEL_RADIUS_M)
                && fate(&targets[slot], Reaching::Wheel { pass }) == Fate::Hit
            {
                hit(&mut targets, slot, Source::Wheel(pass));
            }
        }
    }
    // A projectile flies after its target and is used up on reaching the
    // kart: on a hazard the kart trails, which goes with it, else on the
    // kart itself.
    projectiles.retain_mut(|flying| {
        let slot = flying.projectile.target;
        fly(flying, track, targets[slot].translation, dt);
        let item = Reaching::Projectile {
            reached: targets[slot].reaches(flying.projectile.position, HOMING_RADIUS_M),
            shielded: shield(hazards, slot).is_some(),
        };
        match fate(&targets[slot], item) {
            Fate::Passes => return true,
            Fate::Shielded => {
                hazards.remove(shield(hazards, slot).expect("the shield was just found"));
            }
            Fate::Hit => hit(
                &mut targets,
                slot,
                Source::Projectile(flying.projectile.owner),
            ),
            Fate::UsedUp => {}
        }
        false
    });
    hits
}

/// The Air Horn's blast as its reach is judged (ADR 0076): from `centre`,
/// the firing kart's, at spline `progress` on a lap of `lap` metres. In
/// reach is within the tuned radius of the centre, seen from above, and
/// within the tuned window of spline progress the shorter way round the
/// lap, which keeps distance alone from reaching through a crossover or
/// across the barrier between two sections of road.
pub(crate) struct HornBlast {
    lap: f32,
    centre: Vec3,
    progress: f32,
}

impl HornBlast {
    pub(crate) fn new(track: &Track, centre: Vec3, progress: f32) -> Self {
        Self {
            lap: track.spline_length(),
            centre,
            progress,
        }
    }

    fn within_window(&self, progress: f32) -> bool {
        progress_gap(self.lap, self.progress, progress) <= HORN_PROGRESS_WINDOW_M
    }

    /// Whether the blast reaches a kart standing at `translation` facing
    /// `rotation` with spline `progress`: its collision box within the
    /// radius, as every hit is judged. The bot fires its horn by this test.
    pub(crate) fn reaches_kart(
        &self,
        translation: Vec3,
        rotation: &Rotation,
        progress: f32,
    ) -> bool {
        self.within_window(progress)
            && reaches(translation, rotation, self.centre.to_array(), HORN_RADIUS_M)
    }

    /// Whether the blast reaches a hazard or a projectile centred at
    /// `point` with spline `progress`.
    fn reaches_point(&self, point: [f32; 3], progress: f32) -> bool {
        let (dx, dz) = (point[0] - self.centre.x, point[2] - self.centre.z);
        self.within_window(progress) && dx * dx + dz * dz <= HORN_RADIUS_M * HORN_RADIUS_M
    }

    /// The blast from the kart in `firer` (ADR 0076). Every other kart in
    /// reach spins out, unless it trails a hazard, which is used up in its
    /// place, or [`fate`] spares it, trailed hazard and all. Every hazard
    /// lying on the road in reach is used up, the firer's own included; one
    /// the firer trails stays. Every projectile in reach is used up, whoever
    /// it chases. The runaway wheel, the item boxes and the coins are not
    /// touched.
    pub(crate) fn resolve(
        &self,
        firer: usize,
        targets: &[Target],
        hazards: &mut Vec<PlacedHazard>,
        projectiles: &mut Vec<FlyingProjectile>,
    ) -> Vec<Hit> {
        let mut hits = Vec::new();
        for (slot, target) in targets.iter().enumerate() {
            if slot == firer
                || !self.reaches_kart(target.translation, &target.rotation, target.progress)
            {
                continue;
            }
            let item = Reaching::Horn {
                shielded: shield(hazards, slot).is_some(),
            };
            match fate(target, item) {
                Fate::Hit => hits.push(Hit {
                    kart: slot,
                    source: Source::Horn(firer),
                }),
                Fate::Shielded => {
                    hazards.remove(shield(hazards, slot).expect("the shield was just found"));
                }
                Fate::Passes | Fate::UsedUp => {}
            }
        }
        hazards.retain(|placed| {
            placed.hazard.held || !self.reaches_point(placed.hazard.position, placed.progress)
        });
        projectiles
            .retain(|flying| !self.reaches_point(flying.projectile.position, flying.progress));
        hits
    }
}

/// Whether a kart standing at `translation` facing `rotation` reaches a
/// hazard or a projectile centred at `point`: the point lies within
/// `radius` of the kart's collision box, seen from above.
fn reaches(translation: Vec3, rotation: &Rotation, point: [f32; 3], radius: f32) -> bool {
    let local = rotation.inverse() * (Vec3::from(point) - translation);
    let nearest = Vec3::new(
        local.x.clamp(-KART_HALF_WIDTH_M, KART_HALF_WIDTH_M),
        0.0,
        local.z.clamp(-KART_HALF_LENGTH_M, KART_HALF_LENGTH_M),
    );
    let (dx, dz) = (local.x - nearest.x, local.z - nearest.z);
    dx * dx + dz * dz <= radius * radius
}

/// Fly the projectile one tick of `dt` seconds at the tuned speed: along
/// the centreline, steering for the point the tuned lookahead ahead of
/// where it is, until its target at `target` is within the tuned lock
/// range, then straight at the target. Seen from above; the height is the
/// centreline's where it ends up.
fn fly(flying: &mut FlyingProjectile, track: &Track, target: Vec3, dt: f32) {
    let here = Vec3::from(flying.projectile.position);
    let to_target = Vec3::new(target.x - here.x, 0.0, target.z - here.z);
    let aim = if to_target.length() <= HOMING_LOCK_RANGE_M {
        to_target
    } else {
        let ahead = track.point_at(flying.progress + HOMING_LOOKAHEAD_M);
        Vec3::new(ahead[0] - here.x, 0.0, ahead[2] - here.z)
    };
    let reach = HOMING_SPEED_FACTOR * TOP_SPEED_M_PER_S * dt;
    let moved = if aim.length() <= reach {
        aim
    } else {
        aim.normalize() * reach
    };
    (flying.projectile.position, flying.progress) =
        at_centreline_height(track, here + moved, flying.progress);
}

/// How far apart two spline progress readings are the shorter way round a
/// lap of `lap` metres.
fn progress_gap(lap: f32, a: f32, b: f32) -> f32 {
    let gap = (a - b).rem_euclid(lap);
    gap.min(lap - gap)
}

#[cfg(test)]
mod tests {
    //! The immunity rule through the module's interface: karts placed by
    //! hand with the items beside them, and no drive.
    use super::*;
    use crate::{Hazard, Projectile};

    const DT: f32 = 1.0 / 60.0;
    const PASS: u32 = 3;

    fn oval() -> Track {
        Track::from_sidecar(include_str!("../../../assets/tracks/oval/oval.track.json"))
            .expect("the oval loads")
    }

    /// An ordinary kart facing +Z at `x` along a line, each kart 20 m from
    /// the next so no item reaches two.
    fn kart(x: f32) -> Target {
        Target {
            translation: Vec3::new(x, 0.3, 0.0),
            rotation: Rotation::IDENTITY,
            progress: 0.0,
            starring: false,
            spun_out: false,
            in_pit_lane: false,
            wheel_pass_hit: 0,
        }
    }

    fn starring(target: Target) -> Target {
        Target {
            starring: true,
            ..target
        }
    }

    fn spun_out(target: Target) -> Target {
        Target {
            spun_out: true,
            ..target
        }
    }

    fn in_pit_lane(target: Target) -> Target {
        Target {
            in_pit_lane: true,
            ..target
        }
    }

    /// An ordinary kart in slot 0, then the three karts every item but
    /// the wheel spares: a star kart, a spun-out kart and a lane kart.
    fn spared_trio() -> Vec<Target> {
        vec![
            kart(0.0),
            starring(kart(20.0)),
            spun_out(kart(40.0)),
            in_pit_lane(kart(60.0)),
        ]
    }

    fn at(target: &Target) -> [f32; 3] {
        target.translation.to_array()
    }

    fn dropped(owner: usize, position: [f32; 3]) -> PlacedHazard {
        PlacedHazard {
            hazard: Hazard {
                position,
                owner,
                held: false,
            },
            progress: 0.0,
            lifetime_left_s: 10.0,
            clear_of_owner: true,
        }
    }

    fn trailed_by(owner: usize, targets: &[Target]) -> PlacedHazard {
        let target = &targets[owner];
        PlacedHazard {
            hazard: Hazard {
                position: behind(target.translation, &target.rotation),
                owner,
                held: true,
            },
            clear_of_owner: false,
            ..dropped(owner, [0.0; 3])
        }
    }

    fn chasing(target: usize, targets: &[Target], owner: usize) -> FlyingProjectile {
        FlyingProjectile {
            projectile: Projectile {
                position: at(&targets[target]),
                owner,
                target,
            },
            progress: 0.0,
        }
    }

    fn hit(kart: usize, source: Source) -> Hit {
        Hit { kart, source }
    }

    fn resolved(
        targets: Vec<Target>,
        hazards: &mut Vec<PlacedHazard>,
        projectiles: &mut Vec<FlyingProjectile>,
        wheels: &[(u32, [f32; 3])],
    ) -> Vec<Hit> {
        let holds = vec![true; targets.len()];
        resolve(&oval(), targets, &holds, wheels, hazards, projectiles, DT)
    }

    #[test]
    fn a_star_spins_out_only_the_ordinary_kart_it_touches() {
        let mut targets = spared_trio();
        // A star kart beside each of the four, in slots 4 to 7.
        for slot in 0..4 {
            let beside = targets[slot].translation + Vec3::X * 1.0;
            targets.push(starring(Target {
                translation: beside,
                ..kart(0.0)
            }));
        }
        let hits = resolved(targets, &mut Vec::new(), &mut Vec::new(), &[]);
        assert_eq!(hits, vec![hit(0, Source::Star(4))]);
    }

    #[test]
    fn a_star_kart_in_the_lane_spins_out_nobody() {
        let beside = Target {
            translation: Vec3::new(1.0, 0.3, 0.0),
            ..kart(0.0)
        };
        let targets = vec![kart(0.0), in_pit_lane(starring(beside))];
        assert!(resolved(targets, &mut Vec::new(), &mut Vec::new(), &[]).is_empty());
    }

    #[test]
    fn a_dropped_hazard_spins_out_an_ordinary_kart_and_is_used_up() {
        let targets = vec![kart(0.0), kart(20.0)];
        let mut hazards = vec![dropped(1, at(&targets[0]))];
        let hits = resolved(targets, &mut hazards, &mut Vec::new(), &[]);
        assert_eq!(hits, vec![hit(0, Source::Hazard(1))]);
        assert!(hazards.is_empty());
    }

    #[test]
    fn a_dropped_hazard_stays_under_a_spun_out_kart_and_a_lane_kart() {
        let targets = spared_trio();
        let mut hazards = vec![dropped(0, at(&targets[2])), dropped(0, at(&targets[3]))];
        assert!(resolved(targets, &mut hazards, &mut Vec::new(), &[]).is_empty());
        assert_eq!(hazards.len(), 2);
    }

    #[test]
    fn a_star_kart_uses_up_a_dropped_hazard_and_passes_a_held_one() {
        let mut targets = spared_trio();
        // Slot 4, just out of the star's reach ahead of the star kart in
        // slot 1, trails a hazard into it.
        targets.push(Target {
            translation: targets[1].translation - Vec3::Z * 2.2,
            ..kart(0.0)
        });
        let mut hazards = vec![dropped(0, at(&targets[1])), trailed_by(4, &targets)];
        assert!(resolved(targets, &mut hazards, &mut Vec::new(), &[]).is_empty());
        assert_eq!(hazards.len(), 1);
        assert!(hazards[0].hazard.held);
    }

    #[test]
    fn the_wheel_spins_out_a_kart_once_a_pass() {
        let mut targets = spared_trio();
        targets[0].wheel_pass_hit = PASS - 1;
        targets.push(Target {
            wheel_pass_hit: PASS,
            ..kart(80.0)
        });
        let wheels: Vec<_> = [0, 1, 2, 4].map(|slot| (PASS, at(&targets[slot]))).to_vec();
        let hits = resolved(targets, &mut Vec::new(), &mut Vec::new(), &wheels);
        assert_eq!(hits, vec![hit(0, Source::Wheel(PASS))]);
    }

    #[test]
    fn a_projectile_spins_out_an_ordinary_kart_and_is_used_up() {
        let targets = vec![kart(0.0), kart(20.0)];
        let mut projectiles = vec![chasing(0, &targets, 1)];
        let hits = resolved(targets, &mut Vec::new(), &mut projectiles, &[]);
        assert_eq!(hits, vec![hit(0, Source::Projectile(1))]);
        assert!(projectiles.is_empty());
    }

    #[test]
    fn a_projectile_is_used_up_on_a_spared_kart_and_gives_up_on_the_lane() {
        let targets = spared_trio();
        let mut projectiles: Vec<_> = (1..4).map(|slot| chasing(slot, &targets, 0)).collect();
        // Far from the lane kart, it gives up all the same.
        projectiles[2].projectile.position = at(&targets[0]);
        assert!(resolved(targets, &mut Vec::new(), &mut projectiles, &[]).is_empty());
        assert!(projectiles.is_empty());
    }

    #[test]
    fn a_trailed_hazard_takes_a_projectile_in_the_karts_place() {
        let targets = vec![kart(0.0), kart(20.0)];
        let mut hazards = vec![trailed_by(0, &targets)];
        let mut projectiles = vec![chasing(0, &targets, 1)];
        assert!(resolved(targets, &mut hazards, &mut projectiles, &[]).is_empty());
        assert!(hazards.is_empty());
        assert!(projectiles.is_empty());
    }

    #[test]
    fn a_kart_hit_once_in_a_tick_is_spared_by_the_items_after() {
        // A star kart touches slot 0, which also sits on a dropped hazard
        // out of the star kart's reach, under the wheel and in a
        // projectile's reach.
        let targets = vec![
            kart(0.0),
            starring(Target {
                translation: Vec3::new(1.5, 0.3, 0.0),
                ..kart(0.0)
            }),
            kart(20.0),
        ];
        let mut hazards = vec![dropped(2, [-0.5, 0.0, 0.0])];
        let mut projectiles = vec![chasing(0, &targets, 2)];
        let wheels = [(PASS, at(&targets[0]))];
        let hits = resolved(targets, &mut hazards, &mut projectiles, &wheels);
        assert_eq!(hits, vec![hit(0, Source::Star(1))]);
        assert_eq!(hazards.len(), 1, "the hazard stays under the spun-out kart");
        assert!(
            projectiles.is_empty(),
            "the projectile is used up to no effect"
        );
    }

    /// The horn from slot 0 at the origin, in reach of every kart placed
    /// within a few metres of it.
    fn blast() -> HornBlast {
        HornBlast::new(&oval(), Vec3::new(0.0, 0.3, 0.0), 0.0)
    }

    /// The firer in slot 0 with the karts the horn spares close round it.
    fn round_the_horn() -> Vec<Target> {
        let near = |x: f32| Target {
            translation: Vec3::new(x, 0.3, 0.0),
            ..kart(0.0)
        };
        vec![
            near(0.0),
            near(2.0),
            starring(near(-2.0)),
            spun_out(near(4.0)),
            in_pit_lane(near(-4.0)),
        ]
    }

    #[test]
    fn the_horn_spins_out_only_the_ordinary_karts_in_reach() {
        let targets = round_the_horn();
        let hits = blast().resolve(0, &targets, &mut Vec::new(), &mut Vec::new());
        assert_eq!(hits, vec![hit(1, Source::Horn(0))]);
    }

    #[test]
    fn a_trailed_hazard_takes_the_horn_and_a_spared_kart_keeps_its_own() {
        let targets = round_the_horn();
        let mut hazards = vec![trailed_by(1, &targets), trailed_by(2, &targets)];
        let hits = blast().resolve(0, &targets, &mut hazards, &mut Vec::new());
        assert!(hits.is_empty());
        assert_eq!(hazards.len(), 1);
        assert_eq!(hazards[0].hazard.owner, 2);
    }
}

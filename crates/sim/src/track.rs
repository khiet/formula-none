//! A track as the sim sees it: the sidecar exported from Blender next to the
//! glTF (ADR 0007). The sim reads this and never the mesh, so the picture can
//! change without touching a rule.
//!
//! Everything is in the glTF frame the renderer uses: metres, Y up, a kart
//! with yaw zero faces -Z, and yaw grows counter-clockwise seen from above.

use crate::tuning::{PROGRESS_WINDOW_M, WHEEL_PERIOD_S, WHEEL_WARNING_S};
use rapier3d::prelude::{SharedShape, TriMeshFlags, Vec3};
use serde::{Deserialize, Serialize};
use std::fmt;
use ts_rs::TS;

/// The sidecar layout this build reads; `assets/tracks/export.py` writes it.
pub const SIDECAR_VERSION: u32 = 1;

/// Metres of centreline a direction reading spans, and the distance between
/// the two directions a curvature reading compares. Wider than the export's
/// 1 m point spacing, so the reading is the turn's and not one vertex's;
/// narrower than any corner, so a corner reads at full strength.
const CURVATURE_WINDOW_M: f32 = 4.0;

/// What is under a point on the track. Five of these are exported as tagged
/// meshes; `OutOfWorld` is the answer when nothing lies below a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Asphalt,
    Sand,
    Grass,
    Wall,
    OutOfWorld,
    /// A tyre wall: it stands like a wall, and a kart that hits it bounces off.
    Tyres,
}

impl Surface {
    /// A stable small integer for crossing the wasm boundary; `from_code`
    /// inverts it.
    pub const fn code(self) -> u8 {
        match self {
            Surface::Asphalt => 0,
            Surface::Sand => 1,
            Surface::Grass => 2,
            Surface::Wall => 3,
            Surface::OutOfWorld => 4,
            Surface::Tyres => 5,
        }
    }

    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Surface::Asphalt,
            1 => Surface::Sand,
            2 => Surface::Grass,
            3 => Surface::Wall,
            4 => Surface::OutOfWorld,
            5 => Surface::Tyres,
            _ => return None,
        })
    }

    /// Whether a kart on this surface is held to the off-track speed cap:
    /// the run-off, not the track, a wall or tyre wall top, or thin air.
    pub const fn slows_karts(self) -> bool {
        matches!(self, Surface::Sand | Surface::Grass)
    }
}

/// Something placed on the track: a point on the surface and a facing.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Placement {
    pub position: [f32; 3],
    /// Radians about +Y; zero faces -Z.
    pub yaw: f32,
}

impl Placement {
    /// Whether `point` lies within `half_length` of this placement along
    /// the direction it faces and `half_width` across it. Height is
    /// ignored; what is placed sits on the surface under the point.
    pub fn covers(&self, point: [f32; 3], half_length: f32, half_width: f32) -> bool {
        let (along, across) = self.local(point);
        along.abs() <= half_length && across.abs() <= half_width
    }

    /// `point` as (metres ahead of this placement along the direction it
    /// faces, metres to its right), seen from above.
    pub fn local(&self, point: [f32; 3]) -> (f32, f32) {
        let (sin, cos) = libm::sincosf(self.yaw);
        let (dx, dz) = (point[0] - self.position[0], point[2] - self.position[2]);
        // Yaw zero faces -Z; the right of that facing, seen from above, is +X.
        (-dx * sin - dz * cos, dx * cos - dz * sin)
    }
}

/// An ordered gate across the track: a centre, the direction of travel
/// through it and how far it reaches to either side.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Checkpoint {
    pub position: [f32; 3],
    pub yaw: f32,
    pub half_width: f32,
}

impl Checkpoint {
    /// How far through a move from `before` to `after` the checkpoint was
    /// crossed, as a fraction of the move, or `None` when it was not: a
    /// crossing goes from behind the line to on or past it, in the
    /// direction of travel, within `half_width` of the centre where it met
    /// the line. Height is ignored; a checkpoint reaches up and down
    /// without limit. A move that starts on the line never counts, so a
    /// kart sitting on it crosses once, not every tick.
    pub(crate) fn crossed_between(&self, before: [f32; 3], after: [f32; 3]) -> Option<f32> {
        let (sin, cos) = libm::sincosf(self.yaw);
        // Yaw zero faces -Z; the direction of travel through the checkpoint
        // and its line, to the right of that direction, seen from above.
        let travel = [-sin, -cos];
        let line = [cos, -sin];
        let along = |point: [f32; 3]| {
            (point[0] - self.position[0]) * travel[0] + (point[2] - self.position[2]) * travel[1]
        };
        let (start, end) = (along(before), along(after));
        if !(start < 0.0 && end >= 0.0) {
            return None;
        }
        let t = start / (start - end);
        let met = [
            before[0] + (after[0] - before[0]) * t,
            before[2] + (after[2] - before[2]) * t,
        ];
        let across = (met[0] - self.position[0]) * line[0] + (met[1] - self.position[2]) * line[1];
        (across.abs() <= self.half_width).then_some(t)
    }
}

/// Where the runaway wheel crosses the track: square to it, through
/// `position`, from `half_width` to its left to as far to its right, wall
/// to wall. `yaw` is the direction of travel along the track there.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct WheelCrossing {
    pub position: [f32; 3],
    pub yaw: f32,
    pub half_width: f32,
    /// How far along the track either side of `position` the hazard zone
    /// (GLOSSARY) is painted: as far as the wheel can reach, its landing
    /// spread and its radius. Paint only; nothing in the sim reads it.
    pub hazard_half_depth_m: f32,
}

/// One tagged collision mesh: a flat position buffer and a triangle index
/// buffer, as exported.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SurfaceMesh {
    pub name: String,
    pub surface: Surface,
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
}

impl SurfaceMesh {
    pub fn vertices(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        self.positions.as_chunks::<3>().0.iter().copied()
    }

    pub fn triangles(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.indices.as_chunks::<3>().0.iter().copied()
    }
}

/// A pit lane as the sidecar has it (ADR 0081).
#[derive(Deserialize)]
struct PitLaneSidecar {
    path: Vec<[f32; 3]>,
    half_width: f32,
    entry: Checkpoint,
    exit: Checkpoint,
    boxes: Vec<Placement>,
}

/// A lane beside the course (ADR 0081): the road a kart takes from the fork to
/// the merge, and on it the stretch from the entry line to the exit line where
/// the limiter holds and nothing can hurt the kart. Whether a kart is in it is
/// read from where it stands and nowhere else, so nothing is locked in by
/// crossing a line and a prediction needs telling nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct PitLane {
    path: Vec<[f32; 3]>,
    half_width: f32,
    /// Metres along `path` at each of its points.
    metres_at_point: Vec<f32>,
    entry_m: f32,
    exit_m: f32,
    boxes: Vec<Placement>,
}

/// The pit boxes a lane has: one per grid slot, so karts never queue.
const PIT_BOXES: usize = 8;

impl PitLane {
    fn from_sidecar(sidecar: PitLaneSidecar) -> Result<Self, String> {
        if sidecar.path.len() < 2 {
            return Err("a pit lane's path needs at least two points".into());
        }
        if sidecar.boxes.len() != PIT_BOXES {
            return Err(format!(
                "a pit lane has {PIT_BOXES} pit boxes, not {}",
                sidecar.boxes.len()
            ));
        }
        let mut metres_at_point = vec![0.0];
        let mut metres = 0.0;
        for (a, b) in sidecar.path.iter().zip(sidecar.path.iter().skip(1)) {
            metres += Vec3::from(*a).distance(Vec3::from(*b));
            metres_at_point.push(metres);
        }
        let mut lane = Self {
            path: sidecar.path,
            half_width: sidecar.half_width,
            metres_at_point,
            entry_m: 0.0,
            exit_m: 0.0,
            boxes: sidecar.boxes,
        };
        let on_the_path = |lane: &Self, line: &Checkpoint| {
            lane.reading(line.position)
                .ok_or("a pit lane's lines must lie on its path")
        };
        lane.entry_m = on_the_path(&lane, &sidecar.entry)?;
        lane.exit_m = on_the_path(&lane, &sidecar.exit)?;
        if lane.entry_m >= lane.exit_m {
            return Err("a pit lane's entry line must come before its exit line".into());
        }
        Ok(lane)
    }

    /// The path from the fork to the merge, in the direction of travel.
    pub fn path(&self) -> &[[f32; 3]] {
        &self.path
    }

    /// Metres along the path to the entry line, where the limiter engages.
    pub fn entry_m(&self) -> f32 {
        self.entry_m
    }

    /// Metres along the path to the exit line, where the limiter lets go.
    pub fn exit_m(&self) -> f32 {
        self.exit_m
    }

    /// One per spawn slot, in slot order.
    pub fn boxes(&self) -> &[Placement] {
        &self.boxes
    }

    /// Metres along the path of the nearest point on it to `point`, while
    /// `point` is on the lane's road from fork to merge, and `None` off it.
    /// Seen from above; no course has a road over or under its lane.
    pub fn reading(&self, point: [f32; 3]) -> Option<f32> {
        let (px, pz) = (point[0], point[2]);
        let mut nearest = (f32::INFINITY, 0.0);
        for (index, (a, b)) in self.path.iter().zip(self.path.iter().skip(1)).enumerate() {
            let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
            let length_squared = dx * dx + dz * dz;
            let t = if length_squared > 0.0 {
                (((px - a[0]) * dx + (pz - a[2]) * dz) / length_squared).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (nx, nz) = (a[0] + dx * t, a[2] + dz * t);
            let distance_squared = (px - nx) * (px - nx) + (pz - nz) * (pz - nz);
            if distance_squared < nearest.0 {
                let start = self.metres_at_point[index];
                nearest = (
                    distance_squared,
                    start + (self.metres_at_point[index + 1] - start) * t,
                );
            }
        }
        (nearest.0 <= self.half_width * self.half_width).then_some(nearest.1)
    }

    /// Whether `point` lies from the entry line to the exit line of the lane:
    /// where the pit limiter holds and the lane is safe ground (ADR 0081).
    pub fn limits(&self, point: [f32; 3]) -> bool {
        self.reading(point)
            .is_some_and(|metres| (self.entry_m..=self.exit_m).contains(&metres))
    }

    /// The point `metres` along the path, held to its two ends.
    pub fn point_at(&self, metres: f32) -> [f32; 3] {
        let index = self
            .metres_at_point
            .partition_point(|&at| at <= metres)
            .saturating_sub(1)
            .min(self.path.len() - 2);
        let start = self.metres_at_point[index];
        let end = self.metres_at_point[index + 1];
        let t = if end > start {
            ((metres - start) / (end - start)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Vec3::from(self.path[index])
            .lerp(Vec3::from(self.path[index + 1]), t)
            .to_array()
    }
}

#[derive(Deserialize)]
struct Sidecar {
    version: u32,
    name: String,
    centreline: Vec<[f32; 3]>,
    checkpoints: Vec<Checkpoint>,
    item_boxes: Vec<Placement>,
    boost_pads: Vec<Placement>,
    coins: Vec<Placement>,
    kerbs: Vec<Placement>,
    wheel_crossings: Vec<WheelCrossing>,
    spawn_grid: Vec<Placement>,
    /// A course without one has no pits (ADR 0081).
    #[serde(default)]
    pit_lane: Option<PitLaneSidecar>,
    surfaces: Vec<SurfaceMesh>,
}

/// A loaded, validated track. Cloning shares the collision shapes.
#[derive(Clone)]
pub struct Track {
    name: String,
    centreline: Vec<[f32; 3]>,
    checkpoints: Vec<Checkpoint>,
    item_boxes: Vec<Placement>,
    boost_pads: Vec<Placement>,
    /// The coins a kart collects (GLOSSARY: coin).
    coins: Vec<Placement>,
    /// Sausage kerbs (GLOSSARY).
    kerbs: Vec<Placement>,
    /// Where the runaway wheel (GLOSSARY) crosses the track.
    wheel_crossings: Vec<WheelCrossing>,
    spawn_grid: Vec<Placement>,
    pit_lane: Option<PitLane>,
    surfaces: Vec<SurfaceMesh>,
    /// One collision shape per surface mesh, in `surfaces` order.
    shapes: Vec<SharedShape>,
    /// Spline progress at each centreline point, with the lap length as one
    /// more entry at the end so the closing segment has both ends.
    progress_at_point: Vec<f32>,
    checkpoint_progress: Vec<f32>,
    wheel_crossing_progress: Vec<f32>,
}

impl fmt::Debug for Track {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Track")
            .field("name", &self.name)
            .field("spline_length", &self.spline_length())
            .field("checkpoints", &self.checkpoints.len())
            .field("surfaces", &self.surfaces.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum TrackError {
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    Invalid(String),
}

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackError::Json(error) => write!(f, "sidecar is not valid JSON: {error}"),
            TrackError::UnsupportedVersion(version) => {
                write!(f, "sidecar version {version} is not {SIDECAR_VERSION}")
            }
            TrackError::Invalid(reason) => write!(f, "sidecar is invalid: {reason}"),
        }
    }
}

impl std::error::Error for TrackError {}

impl From<serde_json::Error> for TrackError {
    fn from(error: serde_json::Error) -> Self {
        TrackError::Json(error)
    }
}

impl Track {
    /// Parse and validate a sidecar; a `Track` that exists can be raced on.
    pub fn from_sidecar(json: &str) -> Result<Self, TrackError> {
        let sidecar: Sidecar = serde_json::from_str(json)?;
        if sidecar.version != SIDECAR_VERSION {
            return Err(TrackError::UnsupportedVersion(sidecar.version));
        }
        let invalid = |reason: String| Err(TrackError::Invalid(reason));
        if sidecar.centreline.len() < 3 {
            return invalid("the centreline needs at least three points".into());
        }
        if sidecar.checkpoints.is_empty() {
            return invalid("a track needs at least one checkpoint".into());
        }
        if sidecar.spawn_grid.is_empty() {
            return invalid("a track needs at least one spawn slot".into());
        }
        // The flag never waves for two passes at once.
        for crossing in &sidecar.wheel_crossings {
            let busy_s = WHEEL_WARNING_S + crate::wheel::longest_pass_s(crossing.half_width);
            if busy_s >= WHEEL_PERIOD_S {
                return invalid(format!(
                    "a wheel crossing {} m wide takes up to {busy_s} s of a {WHEEL_PERIOD_S} s period",
                    2.0 * crossing.half_width
                ));
            }
        }
        let pit_lane = sidecar
            .pit_lane
            .map(PitLane::from_sidecar)
            .transpose()
            .map_err(TrackError::Invalid)?;
        let mut shapes = Vec::with_capacity(sidecar.surfaces.len());
        for mesh in &sidecar.surfaces {
            if mesh.surface == Surface::OutOfWorld {
                return invalid(format!("{}: out of world is not a mesh tag", mesh.name));
            }
            if mesh.positions.len() % 3 != 0 || mesh.indices.len() % 3 != 0 {
                return invalid(format!(
                    "{}: positions and indices must come in threes",
                    mesh.name
                ));
            }
            let vertices: Vec<Vec3> = mesh.vertices().map(Vec3::from).collect();
            let triangles: Vec<[u32; 3]> = mesh.triangles().collect();
            if triangles.is_empty() {
                return invalid(format!("{}: no triangles", mesh.name));
            }
            if let Some(index) = triangles
                .iter()
                .flatten()
                .find(|&&index| index as usize >= vertices.len())
            {
                return invalid(format!(
                    "{}: index {index} is past the last vertex",
                    mesh.name
                ));
            }
            // Contact normals at a seam come from the adjacent triangles, so
            // a kart slides over a ribbon's edges instead of catching on them.
            // The flag also discards back-face contacts, so every exported
            // mesh must be wound outward.
            let shape = SharedShape::trimesh_with_flags(
                vertices,
                triangles,
                TriMeshFlags::FIX_INTERNAL_EDGES,
            )
            .map_err(|error| TrackError::Invalid(format!("{}: {error:?}", mesh.name)))?;
            shapes.push(shape);
        }
        let mut progress_at_point = Vec::with_capacity(sidecar.centreline.len() + 1);
        let mut progress = 0.0;
        for (a, b) in sidecar
            .centreline
            .iter()
            .zip(sidecar.centreline.iter().cycle().skip(1))
        {
            progress_at_point.push(progress);
            progress += Vec3::from(*a).distance(Vec3::from(*b));
        }
        progress_at_point.push(progress);
        let checkpoint_progress = sidecar
            .checkpoints
            .iter()
            .map(|checkpoint| {
                progress_along(
                    &sidecar.centreline,
                    &progress_at_point,
                    checkpoint.position,
                    None,
                )
            })
            .collect();
        let wheel_crossing_progress = sidecar
            .wheel_crossings
            .iter()
            .map(|crossing| {
                progress_along(
                    &sidecar.centreline,
                    &progress_at_point,
                    crossing.position,
                    None,
                )
            })
            .collect();
        Ok(Self {
            name: sidecar.name,
            centreline: sidecar.centreline,
            checkpoints: sidecar.checkpoints,
            item_boxes: sidecar.item_boxes,
            boost_pads: sidecar.boost_pads,
            coins: sidecar.coins,
            kerbs: sidecar.kerbs,
            wheel_crossings: sidecar.wheel_crossings,
            spawn_grid: sidecar.spawn_grid,
            pit_lane,
            surfaces: sidecar.surfaces,
            shapes,
            progress_at_point,
            checkpoint_progress,
            wheel_crossing_progress,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The centreline as a closed polyline in the direction of travel,
    /// starting at the start line. Spline progress is distance along it.
    pub fn centreline(&self) -> &[[f32; 3]] {
        &self.centreline
    }

    /// Length of one lap along the centreline, in metres.
    pub fn spline_length(&self) -> f32 {
        *self
            .progress_at_point
            .last()
            .expect("a centreline has points")
    }

    /// The spline progress of `point`: metres along the centreline from the
    /// start line to the nearest point on it, in `0..spline_length()`. Only
    /// x and z count, so a hopping kart reads the same as one on the
    /// asphalt beneath it. This searches the whole lap, so where the course
    /// crosses over itself it can answer for the other leg: use it only
    /// where there is no last reading, as at spawn and after a respawn.
    pub fn progress_at(&self, point: [f32; 3]) -> f32 {
        progress_along(&self.centreline, &self.progress_at_point, point, None)
    }

    /// The spline progress of `point`, as `progress_at` reads it, searching
    /// only the centreline within the tuned window either side of `last`,
    /// the reading before it (ADR 0046).
    pub fn progress_near(&self, point: [f32; 3], last: f32) -> f32 {
        progress_along(&self.centreline, &self.progress_at_point, point, Some(last))
    }

    /// Spline progress unwrapped around the start line for ordering karts
    /// past the same checkpoint: the one further along the lap reads
    /// higher. A kart on the grid behind the line is a few metres short of
    /// zero, not nearly a lap ahead, and a kart past the line that missed
    /// the start checkpoint is a little past the lap's length.
    pub fn lap_relative_progress(&self, checkpoint: usize, spline_progress: f32) -> f32 {
        let lap = self.spline_length();
        let anchor = self.checkpoint_progress[checkpoint];
        let mut ahead = spline_progress - anchor;
        if ahead > lap / 2.0 {
            ahead -= lap;
        } else if ahead <= -lap / 2.0 {
            ahead += lap;
        }
        anchor + ahead
    }

    /// The centreline point `progress` metres from the start line, wrapped
    /// around the lap; where a kart following the centreline would be.
    pub fn point_at(&self, progress: f32) -> [f32; 3] {
        let lap = self.spline_length();
        let progress = progress.rem_euclid(lap);
        let index = self
            .progress_at_point
            .partition_point(|&at| at <= progress)
            .saturating_sub(1)
            .min(self.centreline.len() - 1);
        let start = self.progress_at_point[index];
        let end = self.progress_at_point[index + 1];
        let t = if end > start {
            (progress - start) / (end - start)
        } else {
            0.0
        };
        let a = Vec3::from(self.centreline[index]);
        let b = Vec3::from(self.centreline[(index + 1) % self.centreline.len()]);
        a.lerp(b, t).to_array()
    }

    /// The unit direction of travel along the centreline at `progress`, as
    /// `[x, z]`: the chord across the curvature window centred there, so a
    /// polyline vertex does not read as a kink.
    pub fn direction_at(&self, progress: f32) -> [f32; 2] {
        let behind = self.point_at(progress - CURVATURE_WINDOW_M / 2.0);
        let ahead = self.point_at(progress + CURVATURE_WINDOW_M / 2.0);
        let (dx, dz) = (ahead[0] - behind[0], ahead[2] - behind[2]);
        let length = libm::sqrtf(dx * dx + dz * dz).max(f32::EPSILON);
        [dx / length, dz / length]
    }

    /// How sharply the centreline turns at `progress`, in radians per metre
    /// of travel: positive turning left (counter-clockwise seen from above),
    /// negative turning right, zero on a straight. The reciprocal is the
    /// turn's radius.
    pub fn curvature_at(&self, progress: f32) -> f32 {
        let [ax, az] = self.direction_at(progress - CURVATURE_WINDOW_M / 2.0);
        let [bx, bz] = self.direction_at(progress + CURVATURE_WINDOW_M / 2.0);
        // Facing -Z, left is -X, so a left turn swings the direction's cross
        // product negative.
        let cross = ax * bz - az * bx;
        let dot = ax * bx + az * bz;
        -libm::atan2f(cross, dot) / CURVATURE_WINDOW_M
    }

    /// Gates in lap order; the first is the start line.
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// The spline progress of each checkpoint's centre, in lap order.
    pub fn checkpoint_progress(&self) -> &[f32] {
        &self.checkpoint_progress
    }

    pub fn item_boxes(&self) -> &[Placement] {
        &self.item_boxes
    }

    pub fn boost_pads(&self) -> &[Placement] {
        &self.boost_pads
    }

    /// Whether `point` is on a boost pad: the tuned rectangle around a
    /// pad's placement.
    pub fn on_a_boost_pad(&self, point: [f32; 3]) -> bool {
        use crate::tuning::{BOOST_PAD_HALF_LENGTH_M, BOOST_PAD_HALF_WIDTH_M};
        self.boost_pads
            .iter()
            .any(|pad| pad.covers(point, BOOST_PAD_HALF_LENGTH_M, BOOST_PAD_HALF_WIDTH_M))
    }

    /// The coins a kart collects, in the sidecar's order.
    pub fn coins(&self) -> &[Placement] {
        &self.coins
    }

    pub fn kerbs(&self) -> &[Placement] {
        &self.kerbs
    }

    /// Whether `point` is on a sausage kerb: the tuned rectangle around a
    /// kerb's placement.
    pub fn on_a_kerb(&self, point: [f32; 3]) -> bool {
        use crate::tuning::{KERB_HALF_LENGTH_M, KERB_HALF_WIDTH_M};
        self.kerbs
            .iter()
            .any(|kerb| kerb.covers(point, KERB_HALF_LENGTH_M, KERB_HALF_WIDTH_M))
    }

    /// Where the runaway wheel crosses, in the sidecar's order; empty on a
    /// track without one.
    pub fn wheel_crossings(&self) -> &[WheelCrossing] {
        &self.wheel_crossings
    }

    /// The spline progress of each wheel crossing's centre, in
    /// `wheel_crossings` order.
    pub fn wheel_crossing_progress(&self) -> &[f32] {
        &self.wheel_crossing_progress
    }

    /// Start positions in grid order; slot 0 is pole.
    pub fn spawn_grid(&self) -> &[Placement] {
        &self.spawn_grid
    }

    /// The course's pit lane; `None` on a course without one (ADR 0081).
    pub fn pit_lane(&self) -> Option<&PitLane> {
        self.pit_lane.as_ref()
    }

    /// Whether `point` is where the pit limiter holds and the lane is safe
    /// ground: from entry line to exit line; never on a course without a lane.
    pub fn in_pit_lane(&self, point: [f32; 3]) -> bool {
        self.pit_lane().is_some_and(|lane| lane.limits(point))
    }

    pub fn surfaces(&self) -> &[SurfaceMesh] {
        &self.surfaces
    }

    /// Each surface mesh's tag and collision shape, in `surfaces` order.
    pub(crate) fn colliders(&self) -> impl Iterator<Item = (Surface, &SharedShape)> + '_ {
        self.surfaces
            .iter()
            .map(|mesh| mesh.surface)
            .zip(self.shapes.iter())
    }
}

/// `Track::progress_at`, or `Track::progress_near` given the `last`
/// reading, over the parts of a track it reads, so the checkpoints'
/// progress can be found while the track is still being built.
fn progress_along(
    centreline: &[[f32; 3]],
    progress_at_point: &[f32],
    point: [f32; 3],
    last: Option<f32>,
) -> f32 {
    let lap = progress_at_point[progress_at_point.len() - 1];
    // A segment is searched when any of it lies within the window of the
    // last reading, the shorter way round the lap.
    let in_window = |start: f32, end: f32| {
        let Some(last) = last else {
            return true;
        };
        let past_start = (last - start).rem_euclid(lap);
        let gap = if past_start <= end - start {
            0.0
        } else {
            (past_start - (end - start)).min(lap - past_start)
        };
        gap <= PROGRESS_WINDOW_M
    };
    let (px, pz) = (point[0], point[2]);
    let mut best = (f32::INFINITY, 0.0);
    for (index, (a, b)) in centreline
        .iter()
        .zip(centreline.iter().cycle().skip(1))
        .enumerate()
    {
        if !in_window(progress_at_point[index], progress_at_point[index + 1]) {
            continue;
        }
        let (ax, az) = (a[0], a[2]);
        let (dx, dz) = (b[0] - ax, b[2] - az);
        let length_squared = dx * dx + dz * dz;
        let mut t = if length_squared > 0.0 {
            (((px - ax) * dx + (pz - az) * dz) / length_squared).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let start = progress_at_point[index];
        let end = progress_at_point[index + 1];
        // A segment reaching past the window is searched only up to its
        // edge, so a long one cannot carry the reading onto another leg.
        if let Some(last) = last {
            if end > start {
                let ahead =
                    (start + (end - start) * t - last + lap / 2.0).rem_euclid(lap) - lap / 2.0;
                let over = ahead - ahead.clamp(-PROGRESS_WINDOW_M, PROGRESS_WINDOW_M);
                t = (t - over / (end - start)).clamp(0.0, 1.0);
            }
        }
        let (nx, nz) = (ax + dx * t, az + dz * t);
        let distance_squared = (px - nx) * (px - nx) + (pz - nz) * (pz - nz);
        if distance_squared < best.0 {
            best = (distance_squared, start + (end - start) * t);
        }
    }
    // The last point of the closing segment is the start line again.
    if best.1 >= lap {
        best.1 - lap
    } else {
        best.1
    }
}

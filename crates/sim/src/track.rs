//! A track as the sim sees it: the sidecar exported from Blender next to the
//! glTF (ADR 0007). The sim reads this and never the mesh, so the picture can
//! change without touching a rule.
//!
//! Everything is in the glTF frame the renderer uses: metres, Y up, a kart
//! with yaw zero faces -Z, and yaw grows counter-clockwise seen from above.

use rapier3d::prelude::{SharedShape, TriMeshFlags, Vec3};
use serde::Deserialize;
use std::fmt;

/// The sidecar layout this build reads; `assets/tracks/export.py` writes it.
pub const SIDECAR_VERSION: u32 = 1;

/// What is under a point on the track. Four of these are exported as tagged
/// meshes; `OutOfWorld` is the answer when nothing lies below a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Asphalt,
    Sand,
    Grass,
    Wall,
    OutOfWorld,
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
        }
    }

    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Surface::Asphalt,
            1 => Surface::Sand,
            2 => Surface::Grass,
            3 => Surface::Wall,
            4 => Surface::OutOfWorld,
            _ => return None,
        })
    }
}

/// Something placed on the track: a point on the surface and a facing.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Placement {
    pub position: [f32; 3],
    /// Radians about +Y; zero faces -Z.
    pub yaw: f32,
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
        let (sin, cos) = self.yaw.sin_cos();
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

#[derive(Deserialize)]
struct Sidecar {
    version: u32,
    name: String,
    centreline: Vec<[f32; 3]>,
    checkpoints: Vec<Checkpoint>,
    item_boxes: Vec<Placement>,
    boost_pads: Vec<Placement>,
    spawn_grid: Vec<Placement>,
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
    spawn_grid: Vec<Placement>,
    surfaces: Vec<SurfaceMesh>,
    /// One collision shape per surface mesh, in `surfaces` order.
    shapes: Vec<SharedShape>,
    /// Spline progress at each centreline point, with the lap length as one
    /// more entry at the end so the closing segment has both ends.
    progress_at_point: Vec<f32>,
    checkpoint_progress: Vec<f32>,
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
                progress_along(&sidecar.centreline, &progress_at_point, checkpoint.position)
            })
            .collect();
        Ok(Self {
            name: sidecar.name,
            centreline: sidecar.centreline,
            checkpoints: sidecar.checkpoints,
            item_boxes: sidecar.item_boxes,
            boost_pads: sidecar.boost_pads,
            spawn_grid: sidecar.spawn_grid,
            surfaces: sidecar.surfaces,
            shapes,
            progress_at_point,
            checkpoint_progress,
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
    /// asphalt beneath it.
    pub fn progress_at(&self, point: [f32; 3]) -> f32 {
        progress_along(&self.centreline, &self.progress_at_point, point)
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

    /// Start positions in grid order; slot 0 is pole.
    pub fn spawn_grid(&self) -> &[Placement] {
        &self.spawn_grid
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

/// `Track::progress_at` over the parts of a track it reads, so the
/// checkpoints' progress can be found while the track is still being built.
fn progress_along(centreline: &[[f32; 3]], progress_at_point: &[f32], point: [f32; 3]) -> f32 {
    let (px, pz) = (point[0], point[2]);
    let mut best = (f32::INFINITY, 0.0);
    for (index, (a, b)) in centreline
        .iter()
        .zip(centreline.iter().cycle().skip(1))
        .enumerate()
    {
        let (ax, az) = (a[0], a[2]);
        let (dx, dz) = (b[0] - ax, b[2] - az);
        let length_squared = dx * dx + dz * dz;
        let t = if length_squared > 0.0 {
            (((px - ax) * dx + (pz - az) * dz) / length_squared).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (nx, nz) = (ax + dx * t, az + dz * t);
        let distance_squared = (px - nx) * (px - nx) + (pz - nz) * (pz - nz);
        if distance_squared < best.0 {
            let start = progress_at_point[index];
            let end = progress_at_point[index + 1];
            best = (distance_squared, start + (end - start) * t);
        }
    }
    // The last point of the closing segment is the start line again.
    let lap = progress_at_point[progress_at_point.len() - 1];
    if best.1 >= lap {
        best.1 - lap
    } else {
        best.1
    }
}

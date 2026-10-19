//! The tracks the server can race: each sidecar is compiled into the
//! binary, so the container needs no asset volume and a room's track is
//! the same bytes the client fetched from the same commit (ADR 0007).

use sim::{Track, TrackError};

/// The track every room races until rooms pick one.
pub const DEFAULT: &str = "brands-patch";

const BRANDS_PATCH: &str =
    include_str!("../../../assets/tracks/brands-patch/brands-patch.track.json");
const OVAL: &str = include_str!("../../../assets/tracks/oval/oval.track.json");

/// The sidecar of the track called `name`, or `None` for a name the server
/// has no track for.
pub fn sidecar(name: &str) -> Option<&'static str> {
    match name {
        "brands-patch" => Some(BRANDS_PATCH),
        "oval" => Some(OVAL),
        _ => None,
    }
}

#[derive(Debug)]
pub enum LoadError {
    UnknownTrack(String),
    Unusable(TrackError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTrack(name) => write!(f, "no track called {name:?}"),
            Self::Unusable(error) => write!(f, "track unusable: {error}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// The track called `name`, parsed from its sidecar.
pub fn load(name: &str) -> Result<Track, LoadError> {
    let sidecar = sidecar(name).ok_or_else(|| LoadError::UnknownTrack(name.to_owned()))?;
    Track::from_sidecar(sidecar).map_err(LoadError::Unusable)
}

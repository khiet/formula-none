//! The tracks the server can race: each sidecar is compiled into the
//! binary, so the container needs no asset volume and a room's track is
//! the same bytes the client fetched from the same commit (ADR 0007).

use sim::{Track, TrackError};

/// The track every room races until rooms pick one.
pub const DEFAULT: &str = "bell-deer";

/// Every track by name with its sidecar; a track is added here and nowhere
/// else, so a room kept across a restart (ADR 0053) finds it by the same
/// name a Start does.
const TRACKS: &[(&str, &str)] = &[
    (
        "bell-deer",
        include_str!("../../../assets/tracks/bell-deer/bell-deer.track.json"),
    ),
    (
        "goldstone",
        include_str!("../../../assets/tracks/goldstone/goldstone.track.json"),
    ),
    (
        "oval",
        include_str!("../../../assets/tracks/oval/oval.track.json"),
    ),
];

/// The sidecar of the track called `name`, or `None` for a name the server
/// has no track for.
pub fn sidecar(name: &str) -> Option<&'static str> {
    TRACKS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, sidecar)| *sidecar)
}

/// The track called `name` as the server spells it, or `None` for a name
/// the server has no track for.
fn known(name: &str) -> Option<&'static str> {
    TRACKS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(known, _)| *known)
}

/// Old names of tracks still on the server, each with its new name. A room
/// kept across the deploy that renamed its track was written under the old
/// one; keep an entry for as long as such a file may still be on the volume
/// (ADR 0059).
const RENAMED: &[(&str, &str)] = &[("brands-patch", "bell-deer")];

/// The track a room kept under `name` races after a restart: the track of
/// that name, or the one it was renamed to; `None` for any other name.
pub fn for_kept_room(name: &str) -> Option<&'static str> {
    let name = RENAMED
        .iter()
        .find(|(old, _)| *old == name)
        .map_or(name, |(_, new)| *new);
    known(name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_track_loads_under_its_own_name() {
        for name in ["bell-deer", "goldstone", "oval"] {
            let track = load(name).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(track.name(), name);
        }
    }
}

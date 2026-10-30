//! Open rooms kept across a restart (ADR 0005, ADR 0012): one JSON file
//! per room under a directory on a volume, written whenever an Open room
//! changes and removed once it leaves Open or closes, and read back when
//! the next server starts. A room in Countdown, Racing or Results has no
//! file, so a race under way at a deploy is lost, as ADR 0015 allows.

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::room::KeptRoom;

/// The directory the rooms are kept in.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// Keep rooms under `dir`, created if need be.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Write `room`, replacing what was kept under its code. The file is
    /// written beside its name and renamed into place, so a server that
    /// dies mid-write leaves the last whole copy.
    pub fn save(&self, room: &KeptRoom) -> io::Result<()> {
        let path = self.path(&room.code);
        let partial = path.with_extension("json.partial");
        fs::write(&partial, serde_json::to_vec(room)?)?;
        fs::rename(partial, path)
    }

    /// Remove what is kept under `code`; nothing kept is not an error.
    pub fn forget(&self, code: &str) -> io::Result<()> {
        match fs::remove_file(self.path(code)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    /// Every room kept here, in no particular order. A file that is not a
    /// room is reported by path and left alone.
    pub fn load(&self) -> io::Result<Vec<Result<KeptRoom, Unreadable>>> {
        let mut rooms = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let room = fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
                .map_err(|reason| Unreadable {
                    path: path.clone(),
                    reason,
                });
            rooms.push(room);
        }
        Ok(rooms)
    }

    fn path(&self, code: &str) -> PathBuf {
        self.dir.join(format!("{code}.json"))
    }
}

/// A file under the directory that did not read as a room.
#[derive(Debug)]
pub struct Unreadable {
    pub path: PathBuf,
    pub reason: String,
}

//! Rerun replay fixtures natively and hold them to their recorded hashes, or
//! re-record the hashes after an intended change to the rules:
//!
//! ```sh
//! cargo run -p sim --example replay -- assets/replays/*.replay.json
//! cargo run -p sim --example replay -- --update assets/replays/*.replay.json
//! ```
//!
//! Without `--update` a fixture whose hashes differ is reported and the exit
//! status is non-zero. Tracks are resolved by name under `assets/tracks`.

use sim::{Replay, Track};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut update = false;
    let mut paths = Vec::new();
    for argument in std::env::args().skip(1) {
        if argument == "--update" {
            update = true;
        } else {
            paths.push(argument);
        }
    }
    if paths.is_empty() {
        eprintln!("usage: replay [--update] <fixture.replay.json>...");
        return ExitCode::FAILURE;
    }

    let mut diverged = false;
    for path in &paths {
        match check(path, update) {
            Ok(Outcome::Agrees) => {}
            Ok(Outcome::ReRecorded) => println!("{path}: hashes re-recorded"),
            Ok(Outcome::Differs) => {
                eprintln!("{path}: hashes differ from the recording");
                diverged = true;
            }
            Err(error) => {
                eprintln!("{path}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    if diverged {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

enum Outcome {
    Agrees,
    Differs,
    ReRecorded,
}

/// Rerun one fixture, print its hashes, and compare them with the recording;
/// with `update`, write differing hashes back instead of reporting them.
fn check(path: &str, update: bool) -> Result<Outcome, Box<dyn std::error::Error>> {
    let mut replay = Replay::from_json(&std::fs::read_to_string(path)?)?;
    let hashes = replay.run(track(&replay.track)?)?;
    let hex: Vec<String> = hashes.iter().map(|hash| format!("{hash:016x}")).collect();
    println!("{path}: {} ticks, {}", replay.tick_count(), hex.join(" "));
    if hashes == replay.hashes {
        return Ok(Outcome::Agrees);
    }
    if !update {
        return Ok(Outcome::Differs);
    }
    replay.hashes = hashes;
    std::fs::write(path, replay.to_json() + "\n")?;
    Ok(Outcome::ReRecorded)
}

fn track(name: &str) -> Result<Track, Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/tracks")
        .join(name)
        .join(format!("{name}.track.json"));
    Ok(Track::from_sidecar(&std::fs::read_to_string(path)?)?)
}

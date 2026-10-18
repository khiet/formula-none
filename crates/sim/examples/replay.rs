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
        let json = std::fs::read_to_string(path).unwrap_or_else(|error| {
            eprintln!("{path}: {error}");
            std::process::exit(1)
        });
        let mut replay = Replay::from_json(&json).unwrap_or_else(|error| {
            eprintln!("{path}: {error}");
            std::process::exit(1)
        });
        let hashes = replay.run(track(&replay.track)).unwrap_or_else(|error| {
            eprintln!("{path}: {error}");
            std::process::exit(1)
        });
        let hex: Vec<String> = hashes.iter().map(|hash| format!("{hash:016x}")).collect();
        println!("{path}: {} ticks, {}", replay.tick_count(), hex.join(" "));
        if hashes == replay.hashes {
            continue;
        }
        if update {
            replay.hashes = hashes;
            std::fs::write(path, replay.to_json() + "\n").expect("the fixture is writable");
            println!("{path}: hashes re-recorded");
        } else {
            eprintln!("{path}: hashes differ from the recording");
            diverged = true;
        }
    }
    if diverged {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn track(name: &str) -> Track {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/tracks")
        .join(name)
        .join(format!("{name}.track.json"));
    let sidecar = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Track::from_sidecar(&sidecar).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

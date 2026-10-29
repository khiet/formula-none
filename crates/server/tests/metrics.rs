//! The metrics the server exports for Prometheus (ADR 0018), read the way
//! Prometheus reads them: the text the scrape endpoint answers after rooms
//! and races have run over the fake transport.

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{drive_to_results, race_together};
use http_body_util::BodyExt;
use protocol::{ClientMessage, ServerMessage};
use server::fake_transport::{Conditions, Harness};
use sim::tuning::CONNECTION_PROBE_INTERVAL_S;
use tower::ServiceExt;

/// What `GET /metrics` answers for the harness's server.
async fn scrape(harness: &Harness) -> String {
    let (_stop, shutdown) = server::Shutdown::pair();
    let app = server::app("0123abcd", harness.rooms().clone(), shutdown);
    let response = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        content_type.starts_with("text/plain"),
        "Prometheus reads the text format, not {content_type}"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The value of the sample called `name` whose labels include every one of
/// `labels`, as `key="value"` pairs; `None` when no such sample is exposed.
fn sample(text: &str, name: &str, labels: &[(&str, &str)]) -> Option<f64> {
    text.lines().find_map(|line| {
        if line.starts_with('#') {
            return None;
        }
        let (series, value) = line.rsplit_once(' ')?;
        let (series_name, series_labels) = match series.split_once('{') {
            Some((series_name, rest)) => (series_name, rest.trim_end_matches('}')),
            None => (series, ""),
        };
        if series_name != name {
            return None;
        }
        let present = labels
            .iter()
            .all(|(key, value)| series_labels.contains(&format!("{key}=\"{value}\"")));
        present.then(|| value.parse().expect("a sample is a number"))
    })
}

/// Let the room task act on what the clients sent.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// The nonce of the next probe the server sends this client.
async fn next_ping(client: &mut server::fake_transport::Client) -> u32 {
    client
        .next_where(|message| match message {
            ServerMessage::Ping { nonce } => Some(nonce),
            _ => None,
        })
        .await
        .expect("the server probes a seated player")
}

#[tokio::test(start_paused = true)]
async fn a_race_records_every_tick_s_duration_and_the_room_its_players() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut players = vec![harness.connect(), harness.connect()];
    race_together(&code, &mut players).await;
    settle().await;
    let text = scrape(&harness).await;
    assert_eq!(
        sample(&text, "formula_none_room_players", &[("room", &code)]),
        Some(2.0),
        "two players sit in the room:\n{text}"
    );

    let raced = drive_to_results(&mut players, &[(0, 0.0), (1, 0.0)]).await;
    let text = scrape(&harness).await;
    let ticks = sample(&text, "formula_none_tick_duration_seconds_count", &[])
        .expect("the tick duration histogram");
    assert!(
        ticks >= f64::from(raced[0].results_tick),
        "every tick of the race is observed: {ticks} of at least {}",
        raced[0].results_tick
    );
    assert_eq!(
        sample(
            &text,
            "formula_none_tick_duration_seconds_bucket",
            &[("le", "+Inf")]
        ),
        Some(ticks)
    );
    // The buckets straddle the tick budget, so a tick over it shows.
    assert!(
        text.contains("formula_none_tick_duration_seconds_bucket{le=\"0.016"),
        "a bucket edge at the tick budget:\n{text}"
    );

    for player in &players {
        player.send(&ClientMessage::Leave);
    }
    settle().await;
    let text = scrape(&harness).await;
    assert_eq!(
        sample(&text, "formula_none_room_players", &[("room", &code)]),
        Some(0.0),
        "the room is empty once both have left:\n{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_server_probes_each_client_for_its_round_trip_time_and_loss() {
    const PROBES: usize = 5;
    let one_way = Duration::from_millis(50);
    let mut harness = Harness::new(Conditions {
        delay: one_way,
        ..Conditions::PERFECT
    });
    let code = harness.create_room();
    let mut silent = harness.connect();
    silent.join(&code, "bob").await.unwrap();
    // Seated last, so no probe waits in its queue while the other joins:
    // the round trip measured is the network's, not this test's.
    let mut answering = harness.connect();
    answering.join(&code, "ada").await.unwrap();

    let mut first_probe_at = None;
    for _ in 0..PROBES {
        let nonce = next_ping(&mut answering).await;
        first_probe_at.get_or_insert_with(tokio::time::Instant::now);
        answering.send(&ClientMessage::Pong { nonce });
    }
    // Probes come at the tuned interval, not in a burst.
    let probing = first_probe_at.unwrap().elapsed().as_secs_f32();
    let expected = (PROBES - 1) as f32 * CONNECTION_PROBE_INTERVAL_S;
    assert!(
        (probing - expected).abs() < 0.1,
        "{PROBES} probes spanned {probing} s, expected {expected} s"
    );
    // The last Pong is still on its way; let it land.
    tokio::time::sleep(one_way * 2).await;
    settle().await;

    let text = scrape(&harness).await;
    let unanswered = [("room", code.as_str()), ("slot", "0")];
    let answered = [("room", code.as_str()), ("slot", "1")];
    let count = sample(&text, "formula_none_client_rtt_seconds_count", &answered)
        .expect("the answering client's round trips");
    assert_eq!(count, PROBES as f64, "{text}");
    let sum = sample(&text, "formula_none_client_rtt_seconds_sum", &answered).unwrap();
    let mean = sum / count;
    let round_trip = one_way.as_secs_f64() * 2.0;
    assert!(
        (mean - round_trip).abs() < 1e-3,
        "the round trip is twice the one-way delay: {mean} s"
    );
    assert_eq!(
        sample(&text, "formula_none_client_rtt_seconds_count", &unanswered),
        None,
        "a client that never answers has no round trip:\n{text}"
    );
    assert_eq!(
        sample(&text, "formula_none_client_packet_loss", &answered),
        Some(0.0)
    );
    assert_eq!(
        sample(&text, "formula_none_client_packet_loss", &unanswered),
        Some(1.0),
        "every probe to the silent client is lost:\n{text}"
    );

    // An answer to a probe the next one has already superseded is no round
    // trip: that probe stays lost.
    let stale = next_ping(&mut silent).await;
    silent.send(&ClientMessage::Pong { nonce: stale });
    tokio::time::sleep(one_way * 2).await;
    settle().await;
    let text = scrape(&harness).await;
    assert_eq!(
        sample(&text, "formula_none_client_rtt_seconds_count", &unanswered),
        None,
        "a late Pong records nothing:\n{text}"
    );
    assert_eq!(
        sample(&text, "formula_none_client_packet_loss", &unanswered),
        Some(1.0)
    );
}

#[tokio::test(start_paused = true)]
async fn a_client_s_race_report_is_recorded_against_its_seat() {
    let mut harness = Harness::new(Conditions::PERFECT);
    let code = harness.create_room();
    let mut player = harness.connect();
    player.join(&code, "ada").await.unwrap();
    player.send(&ClientMessage::RaceReport {
        corrections_per_second: 2.5,
    });
    settle().await;
    let text = scrape(&harness).await;
    assert_eq!(
        sample(
            &text,
            "formula_none_client_corrections_per_second",
            &[("room", &code), ("slot", "0")]
        ),
        Some(2.5),
        "{text}"
    );
}

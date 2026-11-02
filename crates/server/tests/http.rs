//! The server's HTTP surface, driven through the router it exposes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use protocol::RoomState;
use serde_json::Value;
use sim::tuning::{
    ROOMS_MAX, ROOM_CLOSE_AFTER_EMPTY_S, ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH,
    ROOM_CREATES_PER_ADDRESS_MAX, ROOM_CREATE_WINDOW_S,
};
use std::time::Duration;
use tower::ServiceExt;

/// The router with its rooms; the stop sender comes along because dropping
/// it stops the server, and a room task that sees that exits.
fn app() -> (Router, server::Rooms, tokio::sync::watch::Sender<bool>) {
    let (stop, shutdown) = server::Shutdown::pair();
    let rooms = server::Rooms::new(shutdown.clone());
    (
        server::app("0123abcd", rooms.clone(), shutdown),
        rooms,
        stop,
    )
}

async fn send(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get(path: &str) -> (StatusCode, Value) {
    let (app, _, _stop) = app();
    send(app, Request::get(path).body(Body::empty()).unwrap()).await
}

#[tokio::test]
async fn health_answers_ok() {
    let (status, body) = get("/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "status": "ok" }));
}

#[tokio::test]
async fn version_reports_the_deployed_commit() {
    let (status, body) = get("/api/version").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "commit": "0123abcd" }));
}

#[tokio::test]
async fn creating_a_room_answers_with_its_code_and_opens_it() {
    let (app, rooms, _stop) = app();
    let (status, body) = send(
        app,
        Request::post("/api/rooms").body(Body::empty()).unwrap(),
    )
    .await;
    // Let the room's task run once: the room must still be open after it has.
    tokio::task::yield_now().await;
    assert_eq!(status, StatusCode::CREATED);
    let code = body["code"].as_str().expect("a code");
    assert_eq!(code.len(), ROOM_CODE_LENGTH);
    assert!(code
        .chars()
        .all(|letter| ROOM_CODE_ALPHABET.contains(letter)));
    let room = rooms.view(code).expect("the room is open");
    assert_eq!(room.state, RoomState::Open);
    assert!(room.players.is_empty());
}

async fn create(app: Router, body: Body) -> (StatusCode, Value) {
    send(
        app,
        Request::post("/api/rooms")
            .header("content-type", "application/json")
            .body(body)
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn a_room_created_on_a_track_races_that_track() {
    let (app, rooms, _stop) = app();
    let (status, body) = create(app, Body::from(r#"{ "track": "goldstone" }"#)).await;
    assert_eq!(status, StatusCode::CREATED);
    let room = rooms.view(body["code"].as_str().unwrap()).unwrap();
    assert_eq!(room.track, "goldstone");
}

#[tokio::test]
async fn a_room_created_without_a_body_races_bell_deer() {
    let (app, rooms, _stop) = app();
    let (_, body) = send(
        app,
        Request::post("/api/rooms").body(Body::empty()).unwrap(),
    )
    .await;
    let room = rooms.view(body["code"].as_str().unwrap()).unwrap();
    assert_eq!(room.track, "bell-deer");
}

/// A create as it arrives through Caddy, which names the address it saw.
async fn create_from(app: &Router, forwarded_for: &str) -> StatusCode {
    let request = Request::post("/api/rooms")
        .header("x-forwarded-for", forwarded_for)
        .body(Body::empty())
        .unwrap();
    send(app.clone(), request).await.0
}

async fn create_unforwarded(app: &Router) -> StatusCode {
    let request = Request::post("/api/rooms").body(Body::empty()).unwrap();
    send(app.clone(), request).await.0
}

/// A sleep past `seconds` under the paused clock, so room tasks and windows
/// see the time go by.
async fn after(seconds: f32) {
    tokio::time::sleep(Duration::from_secs_f32(seconds + 1.0)).await;
}

#[tokio::test(start_paused = true)]
async fn a_create_past_the_cap_is_refused_until_a_room_closes() {
    let (app, rooms, _stop) = app();
    for _ in 0..ROOMS_MAX {
        assert_eq!(create_unforwarded(&app).await, StatusCode::CREATED);
    }
    assert_eq!(
        create_unforwarded(&app).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(rooms.codes().len(), ROOMS_MAX);

    after(ROOM_CLOSE_AFTER_EMPTY_S).await;
    assert_eq!(create_unforwarded(&app).await, StatusCode::CREATED);
}

#[tokio::test(start_paused = true)]
async fn an_address_past_its_limit_is_refused_until_the_window_passes() {
    let (app, rooms, _stop) = app();
    for _ in 0..ROOM_CREATES_PER_ADDRESS_MAX {
        assert_eq!(create_from(&app, "203.0.113.7").await, StatusCode::CREATED);
    }
    assert_eq!(
        create_from(&app, "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(rooms.codes().len(), ROOM_CREATES_PER_ADDRESS_MAX);
    // Anyone else still gets a room.
    assert_eq!(create_from(&app, "203.0.113.8").await, StatusCode::CREATED);

    after(ROOM_CREATE_WINDOW_S).await;
    assert_eq!(create_from(&app, "203.0.113.7").await, StatusCode::CREATED);
}

#[tokio::test(start_paused = true)]
async fn an_address_is_the_last_one_forwarded_so_a_forged_entry_earns_nothing() {
    let (app, _, _stop) = app();
    for forged in 0..ROOM_CREATES_PER_ADDRESS_MAX {
        let header = format!("198.51.100.{forged}, 203.0.113.7");
        assert_eq!(create_from(&app, &header).await, StatusCode::CREATED);
    }
    assert_eq!(
        create_from(&app, "198.51.100.200, 203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test(start_paused = true)]
async fn a_create_refused_for_its_track_does_not_count_against_the_address() {
    let (app, _, _stop) = app();
    for _ in 0..=ROOM_CREATES_PER_ADDRESS_MAX {
        let request = Request::post("/api/rooms")
            .header("x-forwarded-for", "203.0.113.7")
            .body(Body::from(r#"{ "track": "monza" }"#))
            .unwrap();
        assert_eq!(send(app.clone(), request).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(create_from(&app, "203.0.113.7").await, StatusCode::CREATED);
}

#[tokio::test(start_paused = true)]
async fn rooms_brought_back_by_a_restart_count_toward_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let (_stop, shutdown) = server::Shutdown::pair();
    let before = server::Rooms::new(shutdown.clone())
        .kept_in(dir.path())
        .unwrap();
    for _ in 0..ROOMS_MAX {
        before.create().unwrap();
    }

    let restored = server::Rooms::new(shutdown).kept_in(dir.path()).unwrap();
    assert_eq!(restored.codes().len(), ROOMS_MAX);
    assert_eq!(restored.create(), Err(server::rooms::CreateRefused::Full));
}

#[tokio::test]
async fn a_room_on_an_unknown_track_is_refused_and_not_opened() {
    let (app, rooms, _stop) = app();
    let (status, _) = create(app, Body::from(r#"{ "track": "monza" }"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(rooms.codes().is_empty(), "{:?}", rooms.codes());
}

#[tokio::test]
async fn a_body_is_read_whatever_its_content_type_and_one_naming_no_track_races_bell_deer() {
    let (app, rooms, _stop) = app();
    let (status, _) = send(
        app.clone(),
        Request::post("/api/rooms")
            .body(Body::from(r#"{ "track": "monza" }"#))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(rooms.codes().is_empty(), "{:?}", rooms.codes());
    let (status, body) = create(app, Body::from("{}")).await;
    assert_eq!(status, StatusCode::CREATED);
    let room = rooms.view(body["code"].as_str().unwrap()).unwrap();
    assert_eq!(room.track, "bell-deer");
}

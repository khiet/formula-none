//! The server's HTTP surface, driven through the router it exposes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use protocol::RoomState;
use serde_json::Value;
use sim::tuning::{ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH};
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

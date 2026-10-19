//! The server's HTTP surface, driven through the router it exposes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use protocol::RoomState;
use serde_json::Value;
use sim::tuning::{ROOM_CODE_ALPHABET, ROOM_CODE_LENGTH};
use tower::ServiceExt;

fn app() -> (Router, server::Rooms) {
    let shutdown = server::Shutdown::pair().1;
    let rooms = server::Rooms::new(shutdown.clone());
    (server::app("0123abcd", rooms.clone(), shutdown), rooms)
}

async fn send(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get(path: &str) -> (StatusCode, Value) {
    send(app().0, Request::get(path).body(Body::empty()).unwrap()).await
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
    let (app, rooms) = app();
    let (status, body) = send(
        app,
        Request::post("/api/rooms").body(Body::empty()).unwrap(),
    )
    .await;
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

//! The server's HTTP surface, driven through the router it exposes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get(path: &str) -> (StatusCode, Value) {
    let app = server::app("0123abcd");
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
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
async fn unknown_paths_are_not_found() {
    let (status, _) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

//! The server's transports: the HTTP surface with the WebSocket fallback,
//! served by [`serve`], and WebTransport in [`webtransport`]. `main` binds
//! both; tests drive the router directly or through a socket.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rooms::CreateRefused;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tokio::net::TcpListener;
use tokio::sync::watch;

pub mod fake_transport;
pub mod kept;
pub mod lane;
pub mod log;
pub mod metrics;
pub mod probe;
pub mod race;
pub mod room;
pub mod rooms;
pub mod session;
pub mod tracks;
mod websocket;
pub mod webtransport;

pub use log::Log;
pub use metrics::Metrics;
pub use rooms::Rooms;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Serialize)]
struct Version {
    commit: &'static str,
}

/// The answer to a room create: the code the creator shares (ADR 0012).
#[derive(Serialize)]
struct CreatedRoom {
    code: String,
}

/// What a create may ask for: the track the room races, fixed for the
/// room (ADR 0061). A create with no body, or one naming no track, races
/// the server's default.
#[derive(Deserialize)]
struct CreateRoom {
    track: Option<String>,
}

/// The body is read as JSON whatever its content type says, so a create
/// that names a track always races it or is refused, never quietly opened
/// on the default.
async fn create_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<CreatedRoom>), StatusCode> {
    let request = if body.is_empty() {
        CreateRoom { track: None }
    } else {
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?
    };
    let track = match request.track {
        None => None,
        Some(track) => Some(tracks::known(&track).ok_or(StatusCode::BAD_REQUEST)?),
    };
    let code = state
        .rooms
        .create_on(track, forwarded_address(&headers))
        .map_err(|refused| match refused {
            // The client tells these two apart for the Title Scene's message.
            CreateRefused::Full => StatusCode::SERVICE_UNAVAILABLE,
            CreateRefused::TooManyFromAddress => StatusCode::TOO_MANY_REQUESTS,
        })?;
    Ok((StatusCode::CREATED, Json(CreatedRoom { code })))
}

/// The address Caddy saw this request come from. Caddy trusts no proxy, so it
/// replaces any `X-Forwarded-For` a client sends, and the server's HTTP port
/// is reachable only through it (ADR 0073); the last entry is taken so that
/// would hold even behind a proxy that appends. A request with no such header
/// did not come through Caddy: the Vite dev server, or the Compose network.
fn forwarded_address(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")?
        .to_str()
        .ok()?
        .rsplit(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// What Prometheus scrapes (ADR 0018). Caddy proxies only `/api/*`, so this
/// stays on the Compose network.
async fn scrape(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, Metrics::content_type())],
        state.rooms.metrics().scrape(),
    )
}

/// What every route shares: the rooms and the stop signal.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) rooms: Rooms,
    pub(crate) shutdown: Shutdown,
}

/// Every route the server answers; `commit` is what the version endpoint
/// reports and `rooms` is where a create puts a room and a session finds
/// one, and whose metrics the scrape serves. Sessions end when `shutdown`
/// resolves.
pub fn app(commit: &'static str, rooms: Rooms, shutdown: Shutdown) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route(
            "/api/version",
            get(move || async move { Json(Version { commit }) }),
        )
        .route("/api/rooms", post(create_room))
        .route("/api/ws", get(websocket::upgrade))
        .route("/metrics", get(scrape))
        .with_state(AppState { rooms, shutdown })
}

/// Serve `app` on `listener` until `shutdown` resolves, then finish the
/// requests in flight and return. Idle connections are closed rather than
/// waited on, so the return is prompt; a container restart depends on that.
pub async fn serve(listener: TcpListener, app: Router, shutdown: Shutdown) -> std::io::Result<()> {
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown.wait().await })
        .await
}

/// Tells every long-lived task that the server is stopping. Cheap to clone;
/// every clone resolves together.
#[derive(Clone, Debug)]
pub struct Shutdown {
    stopping: watch::Receiver<bool>,
}

impl Shutdown {
    /// A trigger and the handle it fires: send `true` to stop the server.
    pub fn pair() -> (watch::Sender<bool>, Self) {
        let (sender, stopping) = watch::channel(false);
        (sender, Self { stopping })
    }

    /// Resolves once the server is stopping, at once if it already is.
    pub async fn wait(&self) {
        let mut stopping = self.stopping.clone();
        // A dropped sender counts as stopping: nothing could ever fire it.
        let _ = stopping.wait_for(|stopping| *stopping).await;
    }
}

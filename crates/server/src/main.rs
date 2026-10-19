//! Authoritative server binary: the only place race outcomes are decided.
//!
//! Environment:
//! - `LISTEN_ADDR`: HTTP, health, version and the WebSocket fallback
//!   (default `0.0.0.0:8080`; Caddy proxies to it).
//! - `TLS_CERT` and `TLS_KEY`: the PEM files Caddy writes; set both and the
//!   server terminates QUIC for WebTransport on `WEBTRANSPORT_ADDR` (default
//!   `0.0.0.0:4433`, published as 443 UDP). Without both, WebTransport is
//!   off and only the fallback serves, as in local development.

use std::net::SocketAddr;
use std::time::Duration;

use server::webtransport::{WebTransportConfig, WebTransportServer};
use server::{Rooms, Shutdown};
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};

/// Set by the image build so the version endpoint names the deployed commit.
const COMMIT: &str = match option_env!("FORMULA_NONE_COMMIT") {
    Some(commit) => commit,
    None => "unknown",
};

/// How often the certificate files are re-read; Caddy renews weeks before
/// expiry, so a minute's delay is nothing.
const CERTIFICATE_POLL: Duration = Duration::from_secs(60);

/// Resolves on SIGTERM (Docker stop) or SIGINT (a terminal).
async fn shutdown_signal() {
    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut interrupt = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
}

fn webtransport_config() -> std::io::Result<Option<WebTransportConfig>> {
    let (Ok(certificate), Ok(private_key)) = (std::env::var("TLS_CERT"), std::env::var("TLS_KEY"))
    else {
        return Ok(None);
    };
    let bind = std::env::var("WEBTRANSPORT_ADDR").unwrap_or_else(|_| "0.0.0.0:4433".to_string());
    let bind: SocketAddr = bind
        .parse()
        .map_err(|error| std::io::Error::other(format!("WEBTRANSPORT_ADDR {bind:?}: {error}")))?;
    Ok(Some(WebTransportConfig {
        bind,
        certificate: certificate.into(),
        private_key: private_key.into(),
        certificate_poll: CERTIFICATE_POLL,
    }))
}

/// Bind once the certificate exists, then serve until shutdown. `main` runs
/// the HTTP side alongside, so waiting here does not hold up health.
async fn serve_webtransport(
    config: WebTransportConfig,
    rooms: Rooms,
    shutdown: Shutdown,
) -> std::io::Result<()> {
    let bound = tokio::select! {
        () = shutdown.wait() => return Ok(()),
        bound = WebTransportServer::bind(config, rooms) => bound?,
    };
    bound.run(shutdown).await
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr).await?;
    // The bound address, not the configured one: port 0 resolves here.
    println!("server {COMMIT} listening on {}", listener.local_addr()?);
    let webtransport = webtransport_config()?;
    if webtransport.is_none() {
        println!("webtransport off: TLS_CERT and TLS_KEY unset");
    }

    let (stop, shutdown) = Shutdown::pair();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = stop.send(true);
    });
    let rooms = Rooms::new(shutdown.clone());
    let http = server::serve(
        listener,
        server::app(COMMIT, rooms.clone(), shutdown.clone()),
        shutdown.clone(),
    );
    let quic = async {
        match webtransport {
            Some(config) => serve_webtransport(config, rooms, shutdown).await,
            None => Ok(()),
        }
    };
    tokio::try_join!(http, quic)?;
    Ok(())
}

//! One WebTransport handshake against a deployed server, for the deploy
//! smoke test (ADR 0016): connect through the system's trusted roots, send a
//! Ping datagram, print the round-trip time of its Pong. Exits non-zero when
//! a step fails or the budget runs out.
//!
//! ```sh
//! cargo run -p server --example webtransport-probe -- https://formulanone.sushimafia.com/wt
//! ```
//!
//! `WEBTRANSPORT_PROBE_CA` names a PEM file of extra roots to trust, for a
//! laptop stack whose certificate comes from Caddy's local authority
//! (deploy/README.md).

use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use server::probe::webtransport_round_trip;
use wtransport::config::{DnsLookupFuture, DnsResolver};
use wtransport::tls::client::build_default_tls_config;
use wtransport::tls::{build_native_cert_store, CertificateChain};
use wtransport::ClientConfig;

const BUDGET: Duration = Duration::from_secs(10);

/// Prefers an IPv4 address: the site has only an A record, and a laptop's
/// `localhost` resolves to `::1` first, where the local Compose stack was
/// seen not to answer QUIC.
struct Ipv4First;

impl DnsResolver for Ipv4First {
    fn resolve(&self, host: &str) -> Pin<Box<dyn DnsLookupFuture>> {
        let host = host.to_string();
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host(host).await?.collect();
            Ok(addresses
                .iter()
                .find(|address| address.is_ipv4())
                .or(addresses.first())
                .copied())
        })
    }
}

impl fmt::Debug for Ipv4First {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ipv4First")
    }
}

async fn client_config() -> Result<ClientConfig, Box<dyn std::error::Error>> {
    let mut roots = build_native_cert_store();
    if let Ok(path) = std::env::var("WEBTRANSPORT_PROBE_CA") {
        for certificate in CertificateChain::load_pemfile(&path).await?.as_slice() {
            roots.add(certificate.der().to_vec().into())?;
        }
    }
    Ok(ClientConfig::builder()
        .with_bind_default()
        .with_custom_tls(build_default_tls_config(Arc::new(roots), None))
        .dns_resolver(Ipv4First)
        .build())
}

#[tokio::main]
async fn main() -> ExitCode {
    let Some(url) = std::env::args().nth(1) else {
        eprintln!("usage: webtransport-probe <https://host[:port]/path>");
        return ExitCode::FAILURE;
    };
    let config = match client_config().await {
        Ok(config) => config,
        Err(error) => {
            eprintln!("webtransport probe: {error}");
            return ExitCode::FAILURE;
        }
    };
    match webtransport_round_trip(config, &url, BUDGET).await {
        Ok(rtt) => {
            println!(
                "webtransport ok: {url} answered a ping in {} ms",
                rtt.as_millis()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("webtransport failed: {url}: {error}");
            ExitCode::FAILURE
        }
    }
}

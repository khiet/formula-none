//! WebTransport over QUIC, terminated by the server itself (ADR 0011).
//!
//! The certificate is the one Caddy obtains (ADR 0015): its data volume is
//! mounted into the container and this module reads the PEM files Caddy
//! writes there. Caddy renews by rewriting the same files, so the server
//! polls them and reloads the endpoint's TLS configuration when their bytes
//! change; a renewal never needs a rebuild or a restart.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use wtransport::endpoint::endpoint_side::Server;
use wtransport::tls::rustls::crypto::ring::default_provider;
use wtransport::tls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use wtransport::tls::rustls::sign::CertifiedKey;
use wtransport::{Endpoint, Identity, RecvStream, SendStream, ServerConfig};

use tokio::sync::mpsc;

use sim::tuning::RELIABLE_FRAME_MAX_BYTES;

use crate::lane::{Lane, Outgoing};
use crate::rooms::Rooms;
use crate::session::{self, Channels};
use crate::Shutdown;

/// Where to listen and which files hold the certificate.
#[derive(Clone, Debug)]
pub struct WebTransportConfig {
    /// UDP address to terminate QUIC on.
    pub bind: SocketAddr,
    /// PEM certificate chain, as Caddy writes it.
    pub certificate: PathBuf,
    /// PEM private key, as Caddy writes it.
    pub private_key: PathBuf,
    /// How often the files are re-read for a renewal, and how often a
    /// missing certificate is looked for before the first bind.
    pub certificate_poll: Duration,
}

/// A bound QUIC endpoint; [`WebTransportServer::run`] accepts sessions.
pub struct WebTransportServer {
    endpoint: Endpoint<Server>,
    config: WebTransportConfig,
    loaded: Pems,
    rooms: Rooms,
}

/// The file bytes an identity was built from, so a rewrite is noticed by
/// content rather than by timestamp.
#[derive(PartialEq, Eq)]
struct Pems {
    certificate: Vec<u8>,
    private_key: Vec<u8>,
}

impl WebTransportServer {
    /// Bind once the certificate files can be read, polling until then: on
    /// a first deploy Caddy has not obtained the certificate yet.
    pub async fn bind(config: WebTransportConfig, rooms: Rooms) -> std::io::Result<Self> {
        let mut waiting = false;
        let (loaded, identity) = loop {
            match load(&config).await {
                Ok(loaded) => break loaded,
                Err(error) => {
                    if !waiting {
                        println!(
                            "webtransport waiting for certificate {}: {error}",
                            config.certificate.display()
                        );
                        waiting = true;
                    }
                    tokio::time::sleep(config.certificate_poll).await;
                }
            }
        };
        let endpoint = Endpoint::server(server_config(config.bind, identity))?;
        println!("webtransport listening on {}", endpoint.local_addr()?);
        Ok(Self {
            endpoint,
            config,
            loaded,
            rooms,
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.endpoint.local_addr()
    }

    /// Accept sessions, answer their datagrams and reload a renewed
    /// certificate until `shutdown` resolves; open sessions are closed then,
    /// so a deploy never waits on a player.
    pub async fn run(self, shutdown: Shutdown) -> std::io::Result<()> {
        let Self {
            endpoint,
            config,
            loaded,
            rooms,
        } = self;
        tokio::select! {
            () = shutdown.wait() => {}
            () = accept_sessions(&endpoint, rooms, shutdown.clone()) => {}
            () = reload_on_change(&endpoint, &config, loaded) => {}
        }
        endpoint.close(wtransport::VarInt::from_u32(0), b"server stopping");
        Ok(())
    }
}

async fn accept_sessions(endpoint: &Endpoint<Server>, rooms: Rooms, shutdown: Shutdown) {
    loop {
        let incoming = endpoint.accept().await;
        let shutdown = shutdown.clone();
        let rooms = rooms.clone();
        tokio::spawn(async move {
            let Ok(request) = incoming.await else {
                return;
            };
            let Ok(connection) = request.accept().await else {
                return;
            };
            run(connection, rooms, shutdown).await;
        });
    }
}

/// Pump the connection into a session and the session's answers back until
/// it ends or the server stops. Datagrams carry the datagram lane. The
/// client opens one bidirectional stream, which carries the reliable lane
/// both ways as length-prefixed frames (ADR 0011); a reliable message due
/// before the stream is open waits for it.
async fn run(connection: wtransport::Connection, rooms: Rooms, shutdown: Shutdown) {
    let (inbound_tx, inbound) = mpsc::unbounded_channel();
    let (outbound, mut outbound_rx) = mpsc::unbounded_channel();
    let session = tokio::spawn(session::run(
        rooms,
        Channels { inbound, outbound },
        shutdown.clone(),
    ));
    let mut reliable: Option<SendStream> = None;
    let mut waiting_for_stream: Vec<Vec<u8>> = Vec::new();
    let mut reading_stream = None;
    loop {
        tokio::select! {
            () = shutdown.wait() => break,
            sending = outbound_rx.recv() => match sending {
                // A failed send is the client's to notice: it resends an unanswered Ping.
                Some(Outgoing { lane: Lane::Datagram, bytes }) => {
                    let _ = connection.send_datagram(bytes);
                }
                Some(Outgoing { lane: Lane::Reliable, bytes }) => match reliable.as_mut() {
                    Some(stream) => {
                        if write_frame(stream, &bytes).await.is_err() {
                            break;
                        }
                    }
                    None => waiting_for_stream.push(bytes),
                },
                None => break,
            },
            received = connection.receive_datagram() => {
                let Ok(datagram) = received else { break };
                if inbound_tx.send(datagram.to_vec()).is_err() {
                    break;
                }
            }
            accepted = connection.accept_bi(), if reliable.is_none() => {
                let Ok((mut send, recv)) = accepted else { break };
                let mut failed = false;
                for bytes in waiting_for_stream.drain(..) {
                    if write_frame(&mut send, &bytes).await.is_err() {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    break;
                }
                reliable = Some(send);
                reading_stream = Some(tokio::spawn(read_frames(recv, inbound_tx.clone())));
            }
        }
    }
    session.abort();
    if let Some(reading) = reading_stream {
        reading.abort();
    }
}

/// One message on the reliable stream: its length as four little-endian
/// bytes, then its bytes.
async fn write_frame(stream: &mut SendStream, bytes: &[u8]) -> Result<(), FrameError> {
    let length = u32::try_from(bytes.len()).map_err(|_| FrameError::TooLong)?;
    stream
        .write_all(&length.to_le_bytes())
        .await
        .map_err(|_| FrameError::StreamEnded)?;
    stream
        .write_all(bytes)
        .await
        .map_err(|_| FrameError::StreamEnded)
}

/// Why a frame could not be written.
#[derive(Debug)]
enum FrameError {
    TooLong,
    StreamEnded,
}

/// Hand every frame on the stream to the session until the stream ends or
/// a frame claims more than `RELIABLE_FRAME_MAX_BYTES`.
async fn read_frames(mut stream: RecvStream, inbound: mpsc::UnboundedSender<Vec<u8>>) {
    loop {
        let mut length = [0u8; 4];
        if stream.read_exact(&mut length).await.is_err() {
            return;
        }
        let length = u32::from_le_bytes(length);
        if length > RELIABLE_FRAME_MAX_BYTES {
            return;
        }
        let mut bytes = vec![0u8; length as usize];
        if stream.read_exact(&mut bytes).await.is_err() {
            return;
        }
        if inbound.send(bytes).is_err() {
            return;
        }
    }
}

/// Re-read the files every poll and swap the identity in when they changed.
/// Half-written or unreadable files are skipped until the next poll.
async fn reload_on_change(
    endpoint: &Endpoint<Server>,
    config: &WebTransportConfig,
    mut loaded: Pems,
) {
    loop {
        tokio::time::sleep(config.certificate_poll).await;
        match load(config).await {
            Ok((pems, _)) if pems == loaded => {}
            Ok((pems, identity)) => {
                match endpoint.reload_config(server_config(config.bind, identity), false) {
                    Ok(()) => {
                        println!(
                            "webtransport reloaded certificate {}",
                            config.certificate.display()
                        );
                        loaded = pems;
                    }
                    Err(error) => println!("webtransport certificate reload failed: {error}"),
                }
            }
            Err(error) => {
                println!("webtransport certificate unreadable, keeping the loaded one: {error}")
            }
        }
    }
}

/// Read and validate the pair. Caddy rewrites the two files one after the
/// other, so a poll can land between them; a certificate that does not
/// match the key is reported as unreadable rather than built into a TLS
/// configuration, which would abort the server.
async fn load(config: &WebTransportConfig) -> std::io::Result<(Pems, Identity)> {
    let pems = Pems {
        certificate: tokio::fs::read(&config.certificate).await?,
        private_key: tokio::fs::read(&config.private_key).await?,
    };
    let identity = Identity::load_pemfiles(&config.certificate, &config.private_key)
        .await
        .map_err(std::io::Error::other)?;
    let chain: Vec<CertificateDer<'static>> = identity
        .certificate_chain()
        .as_slice()
        .iter()
        .map(|certificate| CertificateDer::from(certificate.der().to_vec()))
        .collect();
    let key = PrivateKeyDer::try_from(identity.private_key().secret_der().to_vec())
        .map_err(std::io::Error::other)?;
    CertifiedKey::from_der(chain, key, &default_provider()).map_err(std::io::Error::other)?;
    Ok((pems, identity))
}

fn server_config(bind: SocketAddr, identity: Identity) -> ServerConfig {
    ServerConfig::builder()
        .with_bind_address(bind)
        .with_identity(identity)
        .build()
}

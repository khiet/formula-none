//! The WebTransport handshake the deploy smoke test performs: connect, send
//! a Ping datagram, get its Pong back. Pings are repeated while waiting
//! because a datagram may be lost on the way.

use std::time::{Duration, Instant};

use protocol::{decode, encode, ClientMessage, ServerMessage};
use wtransport::{ClientConfig, Endpoint};

/// How often an unanswered Ping is sent again, as the client does it.
fn resend_interval() -> Duration {
    Duration::from_secs_f32(sim::tuning::PING_RESEND_S)
}

/// Connect to `url` with `config` and measure one Ping round trip, giving
/// up after `budget`.
pub async fn webtransport_round_trip(
    config: ClientConfig,
    url: &str,
    budget: Duration,
) -> Result<Duration, ProbeError> {
    let deadline = Instant::now() + budget;
    let connection = tokio::time::timeout_at(deadline.into(), async {
        Endpoint::client(config)?
            .connect(url)
            .await
            .map_err(ProbeError::from)
    })
    .await
    .map_err(|_| ProbeError::Timeout("connect"))??;

    let nonce = std::process::id();
    let ping = encode(&ClientMessage::Ping { nonce });
    let sent_at = Instant::now();
    connection.send_datagram(ping.clone())?;
    let resend = resend_interval();
    let mut resend = tokio::time::interval_at((sent_at + resend).into(), resend);
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline.into()) => return Err(ProbeError::Timeout("pong")),
            _ = resend.tick() => connection.send_datagram(ping.clone())?,
            received = connection.receive_datagram() => {
                let datagram = received?;
                if matches!(decode::<ServerMessage>(&datagram), Ok(ServerMessage::Pong { nonce: answered }) if answered == nonce) {
                    return Ok(sent_at.elapsed());
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ProbeError {
    Timeout(&'static str),
    Io(std::io::Error),
    Connecting(wtransport::error::ConnectingError),
    Connection(wtransport::error::ConnectionError),
    Datagram(wtransport::error::SendDatagramError),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout(stage) => write!(f, "timed out waiting for {stage}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Connecting(error) => write!(f, "{error}"),
            Self::Connection(error) => write!(f, "{error}"),
            Self::Datagram(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<std::io::Error> for ProbeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<wtransport::error::ConnectingError> for ProbeError {
    fn from(error: wtransport::error::ConnectingError) -> Self {
        Self::Connecting(error)
    }
}

impl From<wtransport::error::ConnectionError> for ProbeError {
    fn from(error: wtransport::error::ConnectionError) -> Self {
        Self::Connection(error)
    }
}

impl From<wtransport::error::SendDatagramError> for ProbeError {
    fn from(error: wtransport::error::SendDatagramError) -> Self {
        Self::Datagram(error)
    }
}

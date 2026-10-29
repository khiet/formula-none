//! What the developer measures the netcode by (ADR 0018): the five
//! instruments, recorded through the OpenTelemetry metrics SDK and exposed
//! in Prometheus's text format for the Prometheus in the Compose stack to
//! scrape. The rooms record; the HTTP surface serves [`Metrics::scrape`].
//!
//! Series are labelled by room code and, for the per-client ones, the
//! player's slot, so a series names one connection of one room. A gauge
//! keeps its last value after the room closes; the dashboard reads players
//! per room above zero, and the server's restart on every deploy clears the
//! rest.

use std::sync::Arc;
use std::time::Duration;

use opentelemetry::metrics::{Gauge, Histogram, MeterProvider};
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, TextEncoder};
use sim::tuning::TICK_S;

/// Every metric name starts with this.
const NAMESPACE: &str = "formula_none";

/// Where the tick duration histogram's buckets end, in seconds: fine below
/// the tick budget, which is an edge itself so a tick over it shows, and
/// coarse above.
const TICK_DURATION_BUCKETS_S: [f64; 10] = [
    0.0005,
    0.001,
    0.002,
    0.004,
    0.008,
    0.012,
    TICK_S as f64,
    0.025,
    0.05,
    0.1,
];

/// Where the round-trip time histogram's buckets end, in seconds: around
/// the 100 ms the reconciliation is tuned for (ADR 0009).
const RTT_BUCKETS_S: [f64; 12] = [
    0.005, 0.01, 0.02, 0.03, 0.05, 0.075, 0.1, 0.15, 0.2, 0.3, 0.5, 1.0,
];

/// The server's instruments. Cheap to clone; every clone records into the
/// same registry.
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<Inner>,
}

struct Inner {
    registry: prometheus::Registry,
    /// Owns the pipeline the instruments record into; dropped with the metrics.
    _provider: SdkMeterProvider,
    tick_duration: Histogram<f64>,
    room_players: Gauge<u64>,
    rtt: Histogram<f64>,
    packet_loss: Gauge<f64>,
    corrections_per_second: Gauge<f64>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// A fresh registry with the five instruments, nothing recorded yet.
    pub fn new() -> Self {
        let registry = prometheus::Registry::new();
        let exporter = opentelemetry_prometheus::exporter()
            .with_registry(registry.clone())
            .with_namespace(NAMESPACE)
            .without_target_info()
            .scope_info_enabled(false)
            .build()
            .expect("a fresh registry takes the exporter");
        let provider = SdkMeterProvider::builder().with_reader(exporter).build();
        let meter = provider.meter(NAMESPACE);
        let tick_duration = meter
            .f64_histogram("tick.duration")
            .with_unit("s")
            .with_description("How long one server tick took, from before the step until its broadcasts were queued")
            .with_boundaries(TICK_DURATION_BUCKETS_S.to_vec())
            .build();
        let room_players = meter
            .u64_gauge("room.players")
            .with_description("Players seated in the room; 0 once it has closed")
            .build();
        let rtt = meter
            .f64_histogram("client.rtt")
            .with_unit("s")
            .with_description(
                "Round-trip time of the server's probe to one client, on the datagram lane",
            )
            .with_boundaries(RTT_BUCKETS_S.to_vec())
            .build();
        let packet_loss = meter
            .f64_gauge("client.packet_loss")
            .with_description("Share of the server's recent probes to one client that came back late or not at all")
            .build();
        let corrections_per_second = meter
            .f64_gauge("client.corrections_per_second")
            .with_description(
                "Reconciliation corrections per second raced, as the client reported at race end",
            )
            .build();
        Self {
            inner: Arc::new(Inner {
                registry,
                _provider: provider,
                tick_duration,
                room_players,
                rtt,
                packet_loss,
                corrections_per_second,
            }),
        }
    }

    /// Every series in Prometheus's text format, as the scrape answers.
    pub fn scrape(&self) -> String {
        let mut text = Vec::new();
        TextEncoder::new()
            .encode(&self.inner.registry.gather(), &mut text)
            .expect("the text encoder writes any gathered family");
        String::from_utf8(text).expect("the text format is UTF-8")
    }

    /// The content type the scrape is served under.
    pub fn content_type() -> &'static str {
        prometheus::TEXT_FORMAT
    }

    /// One tick's work took `took`.
    pub(crate) fn tick(&self, took: Duration) {
        self.inner.tick_duration.record(took.as_secs_f64(), &[]);
    }

    /// The room with `code` now seats `players`.
    pub(crate) fn room_players(&self, code: &str, players: usize) {
        self.inner
            .room_players
            .record(players as u64, &[KeyValue::new("room", code.to_owned())]);
    }

    /// A probe to the player in `slot` of the room with `code` came back
    /// after `round_trip`.
    pub(crate) fn rtt(&self, code: &str, slot: u8, round_trip: Duration) {
        self.inner
            .rtt
            .record(round_trip.as_secs_f64(), &client(code, slot));
    }

    /// The share in 0..=1 of recent probes the player in `slot` of the room
    /// with `code` did not answer in time.
    pub(crate) fn packet_loss(&self, code: &str, slot: u8, share: f64) {
        self.inner.packet_loss.record(share, &client(code, slot));
    }

    /// The player in `slot` of the room with `code` reported this many
    /// reconciliation corrections per second over the race just ended.
    pub(crate) fn corrections_per_second(&self, code: &str, slot: u8, per_second: f32) {
        self.inner
            .corrections_per_second
            .record(f64::from(per_second), &client(code, slot));
    }
}

/// The labels that name one client: the room and the seat.
fn client(code: &str, slot: u8) -> [KeyValue; 2] {
    [
        KeyValue::new("room", code.to_owned()),
        KeyValue::new("slot", i64::from(slot)),
    ]
}

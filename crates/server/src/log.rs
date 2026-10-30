//! The server's log (ADR 0018): one JSON object per line, each with a
//! UTC timestamp and an event name, and the room code, player slot and
//! race tick wherever there is one, so a query by room code finds every
//! line about a room. On the instance the lines go to stdout, which the
//! awslogs driver ships to CloudWatch Logs; under the fake transport they
//! go into a buffer a test reads back.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};

/// Where the lines go. Cheap to clone; every clone writes to the same
/// sink, one whole line at a time.
#[derive(Clone)]
pub struct Log {
    sink: Arc<Mutex<dyn Write + Send>>,
}

/// What a [`Log::capture`] log has written so far.
#[derive(Clone, Default)]
pub struct Captured {
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl Captured {
    /// Everything written, as it would have reached stdout.
    pub fn text(&self) -> String {
        String::from_utf8(self.bytes.lock().expect("log lock").clone())
            .expect("the log is JSON text")
    }

    /// Every line parsed; the log writes nothing else.
    pub fn lines(&self) -> Vec<Value> {
        self.text()
            .lines()
            .map(|line| {
                serde_json::from_str(line).expect("the log writes one JSON object per line")
            })
            .collect()
    }
}

impl Log {
    /// A log to stdout, as the server runs.
    pub fn stdout() -> Self {
        Self {
            sink: Arc::new(Mutex::new(std::io::stdout())),
        }
    }

    /// A log into a buffer, and the buffer, for a test to read back.
    pub fn capture() -> (Self, Captured) {
        let captured = Captured::default();
        let bytes = Arc::clone(&captured.bytes);
        let log = Self {
            sink: Arc::new(Mutex::new(Writer(bytes))),
        };
        (log, captured)
    }

    /// Start a line about `event`; [`Line::emit`] writes it.
    pub fn event(&self, event: &str) -> Line<'_> {
        let mut fields = Map::new();
        fields.insert("ts".to_owned(), Value::String(timestamp(SystemTime::now())));
        fields.insert("event".to_owned(), Value::String(event.to_owned()));
        Line { log: self, fields }
    }

    fn write(&self, fields: &Map<String, Value>) {
        let mut line = serde_json::to_vec(fields).expect("the fields are JSON values");
        line.push(b'\n');
        // A sink that fails (a closed stdout) loses the line and nothing
        // else; the server does not stop for its log.
        let _ = self.sink.lock().expect("log lock").write_all(&line);
    }
}

/// `Vec<u8>` behind an `Arc` as a writer, for [`Log::capture`].
struct Writer(Arc<Mutex<Vec<u8>>>);

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One line being built: the fields after the timestamp and the event, in
/// the order they are added. The named fields are the ones every query
/// filters on, spelt the same on every line.
#[must_use = "a line is written by emit"]
pub struct Line<'a> {
    log: &'a Log,
    fields: Map<String, Value>,
}

impl Line<'_> {
    pub fn room(self, code: &str) -> Self {
        self.with("room", code)
    }

    pub fn slot(self, slot: u8) -> Self {
        self.with("slot", slot)
    }

    pub fn tick(self, tick: u32) -> Self {
        self.with("tick", tick)
    }

    /// Any other field; a value that is not JSON (a NaN) is written as null.
    pub fn with(mut self, key: &str, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        self.fields.insert(key.to_owned(), value);
        self
    }

    /// Write the line.
    pub fn emit(self) {
        self.log.write(&self.fields);
    }
}

/// `now` as RFC 3339 UTC to the millisecond, as in
/// `2026-10-30T12:34:56.789Z`; a clock before the epoch reads as the epoch.
fn timestamp(now: SystemTime) -> String {
    let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = since_epoch.as_secs();
    let millis = since_epoch.subsec_millis();
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let of_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01, by Howard
/// Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_timestamp_is_rfc_3339_utc_to_the_millisecond() {
        // 2026-10-30T12:34:56.789Z
        let at = UNIX_EPOCH + Duration::from_millis(1_793_363_696_789);
        assert_eq!(timestamp(at), "2026-10-30T12:34:56.789Z");
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // A leap day.
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(timestamp(leap), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn a_line_is_one_json_object_with_its_fields_in_order() {
        let (log, captured) = Log::capture();
        log.event("player_joined")
            .room("ABCD")
            .slot(2)
            .with("nickname", "ada")
            .emit();
        log.event("race_go").room("ABCD").tick(300).emit();
        let text = captured.text();
        assert_eq!(text.lines().count(), 2);
        let lines = captured.lines();
        assert_eq!(lines[0]["event"], "player_joined");
        assert_eq!(lines[0]["room"], "ABCD");
        assert_eq!(lines[0]["slot"], 2);
        assert_eq!(lines[0]["nickname"], "ada");
        assert_eq!(lines[1]["tick"], 300);
        let keys: Vec<&String> = lines[0].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["ts", "event", "room", "slot", "nickname"]);
    }
}

//! The provider clock: the only source of protocol-visible time.
//!
//! Grant expiry, `recorded_at` and every later deadline read the time from
//! here, never from `SystemTime` directly, so a conformance run can move time
//! without waiting for it and a production run cannot be told a different time
//! over the protocol.
//!
//! Three sources, chosen once at launch:
//!
//! - **System**, the default and the only production source.
//! - **Fixed**, from the conformance configuration's `clock.fixed`.
//! - **File**, the `clock.file` test control (Protocol decision 007). The runner
//!   replaces the file atomically; the provider reads it on every call.
//!
//! The file source has two rules, both pinned by
//! `core.grants.test-clock-never-moves-backward`, and both there so that a
//! grant which has expired **stays** expired:
//!
//! - a backward write is ignored, so time never runs back and revives a grant;
//! - a malformed write is ignored, keeping the last good instant, so a torn or
//!   garbage file cannot reset time to something earlier either.
//!
//! A file that is malformed **at launch** is different: there is no last good
//! instant to keep, so the provider refuses to start rather than invent one.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::grants::is_instant;

#[derive(Debug, Clone)]
pub enum Source {
    System,
    Fixed(String),
    File(PathBuf),
}

#[derive(Debug)]
pub struct Clock {
    source: Source,
    /// The last good instant read from the file source. Never moves backward.
    /// Behind a mutex because every session on a shared transport reads the
    /// same clock, so one session can never see time run back that another
    /// has already seen move forward.
    last: Mutex<Option<String>>,
}

impl Clock {
    /// Open a clock. A file source must hold a valid instant now, or there is
    /// no honest time to start from.
    pub fn open(source: Source) -> Result<Self, String> {
        let clock = Self {
            source,
            last: Mutex::new(None),
        };
        if let Source::File(path) = &clock.source {
            match read_instant(path) {
                Some(instant) => *clock.lock() = Some(instant),
                None => {
                    return Err(format!(
                        "the clock file {} does not hold a UTC instant; refusing to start \
                         rather than guess the time",
                        path.display()
                    ));
                }
            }
        }
        Ok(clock)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The current instant, `YYYY-MM-DDTHH:MM:SSZ`.
    pub fn now(&self) -> String {
        match &self.source {
            Source::System => system_now(),
            Source::Fixed(instant) => instant.clone(),
            Source::File(path) => {
                let mut last = self.lock();
                if let Some(read) = read_instant(path) {
                    // Fixed-width UTC, so string order is time order.
                    if last.as_ref().is_none_or(|previous| read > *previous) {
                        *last = Some(read);
                    }
                }
                last.clone()
                    .expect("a file clock holds an instant from launch onward")
            }
        }
    }
}

/// The file's contents if they are exactly one instant, and nothing otherwise.
/// Exact, not trimmed: the runner writes the instant's bytes and nothing else,
/// and a reader that tolerated surrounding bytes would accept files the
/// control never produces.
fn read_instant(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    is_instant(&text).then_some(text)
}

/// Wall-clock UTC at one-second resolution.
pub fn system_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    format_unix(seconds)
}

/// Format seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// The civil-from-days conversion is Howard Hinnant's algorithm, which is exact
/// over the proleptic Gregorian calendar and needs no date library.
pub fn format_unix(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rem = seconds % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Seconds since the Unix epoch, from an instant [`format_unix`] wrote.
///
/// The inverse of [`format_unix`], and the reason it exists: a model call
/// put on the work pool needs the request's deadline as a **duration from
/// now**, and the pool measures in monotonic time while the protocol
/// measures in wall-clock instants. One conversion, at the moment the
/// call is asked about, is a great deal better than two clocks running
/// beside each other and disagreeing about the same request.
///
/// `None` for anything that is not the format this module writes. A
/// deadline CBR cannot read is not a deadline it guesses at.
pub fn unix_of(instant: &str) -> Option<u64> {
    let bytes = instant.as_bytes();
    if bytes.len() != 20 || bytes[19] != b'Z' {
        return None;
    }
    let number =
        |from: usize, to: usize| -> Option<i64> { instant.get(from..to)?.parse::<i64>().ok() };
    if &instant[4..5] != "-"
        || &instant[7..8] != "-"
        || &instant[10..11] != "T"
        || &instant[13..14] != ":"
        || &instant[16..17] != ":"
    {
        return None;
    }
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // Howard Hinnant's days-from-civil, the inverse of the algorithm
    // `format_unix` uses, so the two cannot drift apart.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    u64::try_from(seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instant_round_trips_through_seconds() {
        // **The two halves cannot drift apart**, which is the whole
        // reason the inverse is written here beside the original rather
        // than wherever it was first needed.
        for seconds in [
            0,
            1,
            86_399,
            86_400,
            951_782_400,   // 2000-02-29, a leap day in a leap century
            1_709_164_800, // 2024-02-29
            1_789_100_000,
            4_102_444_800,  // 2100-01-01, not a leap year
            32_503_680_000, // 3000-01-01
        ] {
            let written = format_unix(seconds);
            assert_eq!(unix_of(&written), Some(seconds), "{written}");
        }
    }

    #[test]
    fn anything_that_is_not_an_instant_is_refused_rather_than_guessed_at() {
        for text in [
            "",
            "2026-09-21",
            "2026-09-21T00:00:00",
            "2026-09-21T00:00:00z",
            "2026-09-21 00:00:00Z",
            "2026/09/21T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-09-00T00:00:00Z",
            "2026-09-21T24:00:00Z",
            "2026-09-21T00:60:00Z",
            "202x-09-21T00:00:00Z",
            "1960-01-01T00:00:00Z",
        ] {
            assert_eq!(unix_of(text), None, "{text} was read as an instant");
        }
    }

    fn clock_file(content: &str) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("clock");
        std::fs::write(&path, content).expect("writes clock");
        (directory, path)
    }

    #[test]
    fn a_file_clock_ignores_backward_and_malformed_writes() {
        let (_directory, path) = clock_file("2030-01-01T00:00:00Z");
        let clock = Clock::open(Source::File(path.clone())).expect("opens");
        assert_eq!(clock.now(), "2030-01-01T00:00:00Z");

        std::fs::write(&path, "2030-01-01T02:00:00Z").expect("writes");
        assert_eq!(clock.now(), "2030-01-01T02:00:00Z", "forward is followed");

        std::fs::write(&path, "2030-01-01T00:00:00Z").expect("writes");
        assert_eq!(clock.now(), "2030-01-01T02:00:00Z", "backward is ignored");

        std::fs::write(&path, "not-an-instant").expect("writes");
        assert_eq!(clock.now(), "2030-01-01T02:00:00Z", "malformed is ignored");

        std::fs::remove_file(&path).expect("removes");
        assert_eq!(clock.now(), "2030-01-01T02:00:00Z", "a missing file is too");
    }

    #[test]
    fn a_malformed_clock_file_at_launch_refuses_to_start() {
        let (_directory, path) = clock_file("2030-01-01T00:00:00Z\n");
        assert!(
            Clock::open(Source::File(path)).is_err(),
            "there is no last good instant to fall back on"
        );
    }

    #[test]
    fn unix_seconds_format_as_utc_instants() {
        assert_eq!(format_unix(0), "1970-01-01T00:00:00Z");
        // 2000-02-29 exercises the century leap-year rule.
        assert_eq!(format_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_unix(1_893_456_000), "2030-01-01T00:00:00Z");
        assert_eq!(format_unix(4_102_444_799), "2099-12-31T23:59:59Z");
    }
}

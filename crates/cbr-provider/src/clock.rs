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

use std::cell::RefCell;
use std::path::PathBuf;

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
    last: RefCell<Option<String>>,
}

impl Clock {
    /// Open a clock. A file source must hold a valid instant now, or there is
    /// no honest time to start from.
    pub fn open(source: Source) -> Result<Self, String> {
        let clock = Self {
            source,
            last: RefCell::new(None),
        };
        if let Source::File(path) = &clock.source {
            match read_instant(path) {
                Some(instant) => *clock.last.borrow_mut() = Some(instant),
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

    /// The current instant, `YYYY-MM-DDTHH:MM:SSZ`.
    pub fn now(&self) -> String {
        match &self.source {
            Source::System => system_now(),
            Source::Fixed(instant) => instant.clone(),
            Source::File(path) => {
                let mut last = self.last.borrow_mut();
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

#[cfg(test)]
mod tests {
    use super::*;

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

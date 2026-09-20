//! The gate for m4a, written and run before the module beneath it existed.
//!
//! Each test names the rule it holds. Nothing here opens a socket, reads a
//! credential or waits on a clock: time is moved, never slept through.

use std::collections::BTreeMap;

use super::*;

/// A non-Latin text in the corpus on purpose. A character-count estimate
/// under-counts here — Devanagari and CJK take three bytes per character in
/// UTF-8 and a byte-level BPE may emit a token per byte — which is exactly
/// the trap a byte bound does not fall into.
const NON_LATIN: &str = "\
एक प्रदाता की प्रतिक्रिया में क्रेडेंशियल हो सकते हैं, इसलिए रिकॉर्डिंग की सीमा पर ही उन्हें हटाया जाता है।
プロバイダーの応答には第三者の資格情報が含まれることがあるため、記録の境界で削除する。
提供方的响应中可能包含第三方凭据，因此在记录边界处将其删除。";

/// The corpus: CBR's own sources, plus the text above. Fixed, and read from
/// the crate it is testing, so it grows with the repository rather than
/// being a snapshot that stops being representative.
fn corpus() -> Vec<(String, Vec<u8>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && let Ok(bytes) = std::fs::read(&path)
            {
                files.push((path.display().to_string(), bytes));
            }
        }
    }
    files.sort();
    assert!(files.len() > 10, "the corpus is the crate's own sources");
    files.push(("non-latin".into(), NON_LATIN.as_bytes().to_vec()));
    files
}

#[test]
fn the_estimate_never_falls_below_what_any_byte_level_tokenizer_could_emit() {
    // One-sided: over-estimating is the safe direction and under-estimating
    // admits a request the provider charges for anyway. The reference is the
    // worst case a byte-level BPE can produce, which is one token per byte —
    // obtained by arithmetic rather than measured, because no tokenizer is
    // available offline and fetching one is not this milestone's to do.
    for (name, bytes) in corpus() {
        let reference = worst_case_tokens(&bytes);
        let estimated = estimate(&bytes, 1);
        assert!(
            estimated >= reference,
            "{name}: estimate {estimated} < worst case {reference}"
        );
    }
}

#[test]
fn a_character_count_would_under_estimate_the_non_latin_text_and_a_byte_count_does_not() {
    // The test that says why the bound is bytes. If this ever stops holding,
    // the corpus has lost the case it exists for.
    let bytes = NON_LATIN.as_bytes();
    let characters = NON_LATIN.chars().count() as u64;
    assert!(
        characters < worst_case_tokens(bytes),
        "the non-Latin text has more bytes than characters: {characters}"
    );
    assert!(estimate(bytes, 1) >= worst_case_tokens(bytes));
}

#[test]
fn the_estimate_counts_the_whole_body_and_not_only_its_messages() {
    // The defect this catches: an estimate that walks `messages` and forgets
    // the tool schemas, the system instructions or the JSON framing around
    // them. The serialized body is what is sent, so the serialized body is
    // what is counted.
    let small = estimate(b"{\"messages\":[]}", 0);
    let large = estimate(
        b"{\"messages\":[],\"tools\":[{\"name\":\"search\",\"schema\":{\"a\":1}}]}",
        0,
    );
    assert!(large > small, "{large} is not more than {small}");
    assert_eq!(large - small, 45, "every added byte is counted");
}

#[test]
fn the_estimate_reserves_generation_and_margin_above_the_input_bound() {
    // **What nothing asserted until m4b.** The estimate's parts were named
    // in constants, documented, and never read by a test: removing the
    // margin from it left the whole workspace green. An estimate that is
    // only the input bound admits a request whose generation it has not
    // accounted for, which is the same defect the completion's reservation
    // had, one layer down.
    for (name, bytes) in corpus() {
        let headroom = estimate(&bytes, 0) - worst_case_tokens(&bytes);
        assert!(
            headroom >= RESERVED_GENERATION_TOKENS + SAFETY_MARGIN_TOKENS,
            "{name}: the estimate leaves {headroom} above the input bound, which is less \
             than the generation and margin it is supposed to reserve"
        );
    }
}

#[test]
fn the_estimate_grows_with_the_messages_it_frames() {
    // The provider frames each message, and the serialized body does not
    // obviously show that framing. Dropping the term left every test green.
    let body = b"{\"messages\":[]}";
    // Strictly greater, not merely equal to four framings: a framing term
    // of zero satisfies the equality and is not a framing at all, which is
    // exactly what the surviving mutant did.
    assert!(
        estimate(body, 4) > estimate(body, 0),
        "four messages cost more than none"
    );
    assert_eq!(
        estimate(body, 4) - estimate(body, 0),
        4 * MESSAGE_OVERHEAD_TOKENS,
        "and cost four framings, not some other number"
    );
}

#[test]
fn an_instant_reads_as_seconds_and_an_unreadable_one_reads_as_nothing() {
    assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(epoch_seconds("1970-01-01T00:00:01Z"), Some(1));
    assert_eq!(epoch_seconds("2026-09-20T09:56:46Z"), Some(1_789_898_206));
    // A leap year, and the day after the leap day.
    assert_eq!(epoch_seconds("2024-03-01T00:00:00Z"), Some(1_709_251_200));
    assert_eq!(epoch_seconds("not an instant"), None);
    assert_eq!(epoch_seconds("2026-13-01T00:00:00Z"), None);
}

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    Ledger::migrate(&connection).expect("migrates");
    connection
}

const T0: &str = "2026-09-20T12:00:00Z";

fn later(instant: &str, seconds: i64) -> String {
    let base = epoch_seconds(instant).expect("an instant");
    let total = base + seconds;
    // Only used to move the clock in tests, so a simple round trip is enough.
    let days = total.div_euclid(86_400);
    let rest = total.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Howard Hinnant's civil-from-days, for the test's own clock arithmetic.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[test]
fn a_reservation_is_written_before_the_send_and_counts_until_it_is_settled() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "request", 1_000)
        .expect("admits")
        .expect("within the envelope");
    // Counted the moment it is written, which is what makes a crash between
    // the reservation and the send safe.
    assert_eq!(ledger.spend(T0, "job").expect("spend").window, 1_000);
    ledger
        .settle(T0, &reservation, Settlement::Usage(400))
        .expect("settles");
    assert_eq!(
        ledger.spend(T0, "job").expect("spend").window,
        400,
        "settling replaces the estimate with what was spent"
    );
}

#[test]
fn an_unsettled_reservation_is_still_counted_after_a_restart() {
    // The durability half: the ledger is in the store, so the counters are
    // what the store says and not what a process remembers.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("ledger.sqlite");
    {
        let connection = Connection::open(&path).expect("opens");
        Ledger::migrate(&connection).expect("migrates");
        Ledger::new(&connection)
            .admit(T0, "job", "request", 5_000)
            .expect("admits")
            .expect("within the envelope");
    }
    let connection = Connection::open(&path).expect("reopens");
    let spend = Ledger::new(&connection).spend(T0, "job").expect("spend");
    assert_eq!(spend.window, 5_000, "the spend survived the process");
}

#[test]
fn the_window_rolls_and_the_month_does_not() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "r1", 1_000)
        .expect("admits")
        .expect("admitted");
    ledger
        .settle(T0, &reservation, Settlement::Usage(1_000))
        .expect("settles");

    // Time is moved, never slept through.
    let inside = later(T0, WINDOW_SECONDS - 60);
    let outside = later(T0, WINDOW_SECONDS + 60);
    assert_eq!(ledger.spend(&inside, "job").expect("spend").window, 1_000);
    assert_eq!(
        ledger.spend(&outside, "job").expect("spend").window,
        0,
        "the rolling window forgot it"
    );
    assert_eq!(
        ledger.spend(&outside, "job").expect("spend").month,
        1_000,
        "the calendar month did not"
    );
}

#[test]
fn each_counter_refuses_on_its_own() {
    // Passing one is not passing. A month can be almost untouched while the
    // window is exhausted, and the reverse holds across a month boundary.
    let connection = database();
    let ledger = Ledger::new(&connection);
    // Filled the way it would really be filled: under both ceilings, which
    // is why this takes many jobs rather than one large request. A single
    // reservation the size of the window is refused by the per-request
    // ceiling long before any counter is consulted.
    let mut spent = 0u64;
    let mut job = 0u64;
    while spent + PER_REQUEST_TOKENS <= WINDOW_TOKENS {
        let name = format!("job-{}", job);
        for _ in 0..(PER_JOB_TOKENS / PER_REQUEST_TOKENS) {
            if spent + PER_REQUEST_TOKENS > WINDOW_TOKENS {
                break;
            }
            let reservation = ledger
                .admit(T0, &name, "r", PER_REQUEST_TOKENS)
                .expect("admits")
                .expect("admitted");
            ledger
                .settle(T0, &reservation, Settlement::Usage(PER_REQUEST_TOKENS))
                .expect("settles");
            spent += PER_REQUEST_TOKENS;
        }
        job += 1;
    }
    assert_eq!(spent, WINDOW_TOKENS, "the window is exactly full");
    assert_eq!(
        ledger.admit(T0, "fresh", "r", 100).expect("admits"),
        Err(Refusal::WindowExhausted),
        "the window is spent although the month has room and the job is new"
    );
    // Past the window, the same spend still counts against the month.
    let next = later(T0, WINDOW_SECONDS + 60);
    assert!(
        ledger
            .admit(&next, "fresh", "r", 100)
            .expect("admits")
            .is_ok(),
        "the window rolled"
    );
    assert_eq!(
        ledger.spend(&next, "fresh").expect("spend").month,
        WINDOW_TOKENS + 100,
        "and the month kept every one of them"
    );
}

#[test]
fn a_refusal_writes_no_reservation_and_is_itself_recorded() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    assert_eq!(
        ledger
            .admit(T0, "job", "huge", PER_REQUEST_TOKENS + 1)
            .expect("admits"),
        Err(Refusal::PerRequest)
    );
    let rows = ledger.rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "refusal"),
        "the refusal is a recorded event: {rows:?}"
    );
    assert!(
        !rows.iter().any(|(kind, _, _)| kind == "reservation"),
        "and nothing was reserved: {rows:?}"
    );
    assert_eq!(ledger.spend(T0, "job").expect("spend").window, 0);
}

#[test]
fn a_job_ceiling_refuses_while_the_envelope_has_room() {
    let connection = database();
    let ledger = Ledger::new(&connection);
    let mut spent = 0;
    while spent + PER_REQUEST_TOKENS <= PER_JOB_TOKENS {
        let reservation = ledger
            .admit(T0, "job", "r", PER_REQUEST_TOKENS)
            .expect("admits")
            .expect("admitted");
        ledger
            .settle(T0, &reservation, Settlement::Usage(PER_REQUEST_TOKENS))
            .expect("settles");
        spent += PER_REQUEST_TOKENS;
    }
    assert_eq!(
        ledger.admit(T0, "job", "one more", 1_000).expect("admits"),
        Err(Refusal::PerJob),
        "the job is spent"
    );
    assert!(
        ledger
            .admit(T0, "other", "r", 1_000)
            .expect("admits")
            .is_ok(),
        "another job still has room, so this was a ceiling and not the envelope"
    );
}

#[test]
fn provider_exhaustion_is_a_different_outcome_from_an_exhausted_envelope() {
    // The quota is shared with the owner's other tools, so the provider can
    // refuse while CBR's ledger has room. Reporting that as
    // `budget_exhausted` would name the wrong limit.
    assert_eq!(Refusal::WindowExhausted.reason(), "budget_exhausted");
    assert_eq!(Refusal::MonthExhausted.reason(), "budget_exhausted");
    assert_eq!(
        Settlement::ProviderExhausted.reason(),
        Some("provider_quota_exhausted")
    );
    assert_ne!(
        Settlement::ProviderExhausted.reason(),
        Some(Refusal::WindowExhausted.reason())
    );

    let connection = database();
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "r", 1_000)
        .expect("admits")
        .expect("admitted");
    ledger
        .settle(T0, &reservation, Settlement::ProviderExhausted)
        .expect("settles");
    // The ledger is not corrupted by it: the reservation is reconciled to
    // what was actually spent, which for a refusal is nothing.
    assert_eq!(ledger.spend(T0, "job").expect("spend").window, 0);
    assert!(
        ledger
            .rows()
            .expect("rows")
            .iter()
            .any(|(kind, _, _)| kind == "provider_exhausted"),
        "and it is recorded as what it was"
    );
}

#[test]
fn a_usage_that_differs_from_the_estimate_is_reconciled_and_the_divergence_kept() {
    // A systematically low estimate is how an envelope leaks, so the
    // difference is a thing the ledger holds rather than a thing it discards.
    let connection = database();
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "r", 10_000)
        .expect("admits")
        .expect("admitted");
    ledger
        .settle(T0, &reservation, Settlement::Usage(12_000))
        .expect("settles");
    assert_eq!(
        ledger.spend(T0, "job").expect("spend").window,
        12_000,
        "the actual spend wins, even when it is larger than the estimate"
    );
    let rows = ledger.rows().expect("rows");
    assert!(
        rows.iter().any(|(kind, detail, tokens)| kind == "usage"
            && *tokens == 12_000
            && detail == "10000"),
        "the estimate it diverged from is kept beside it: {rows:?}"
    );
}

#[test]
fn the_crate_has_no_network_dependency_in_its_tree() {
    // The envelope exists before any transport, and this is what says so:
    // nothing that can open a socket is reachable from this crate.
    let lock = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Cargo.lock")
        .canonicalize()
        .expect("the workspace lock file");
    let text = std::fs::read_to_string(lock).expect("reads");
    let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut name = String::new();
    let mut collecting = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            name.clear();
            collecting = false;
        } else if let Some(rest) = line.strip_prefix("name = ") {
            name = rest.trim_matches('"').to_string();
            dependencies.entry(name.clone()).or_default();
        } else if line == "dependencies = [" {
            collecting = true;
        } else if collecting {
            if line == "]" {
                collecting = false;
            } else {
                let entry = line.trim_end_matches(',').trim_matches('"');
                let first = entry.split_whitespace().next().unwrap_or_default();
                if !first.is_empty() {
                    dependencies
                        .entry(name.clone())
                        .or_default()
                        .push(first.into());
                }
            }
        }
    }
    let mut reached: BTreeMap<String, ()> = BTreeMap::new();
    let mut stack = vec!["cbr-provider".to_string()];
    while let Some(package) = stack.pop() {
        if reached.insert(package.clone(), ()).is_some() {
            continue;
        }
        for next in dependencies.get(&package).cloned().unwrap_or_default() {
            stack.push(next);
        }
    }
    assert!(reached.len() > 5, "the lock file parsed: {}", reached.len());
    for forbidden in [
        "reqwest",
        "hyper",
        "tokio",
        "rustls",
        "native-tls",
        "openssl",
        "curl",
        "ureq",
        "attohttpc",
        "isahc",
        "surf",
        "h2",
        "quinn",
        "async-std",
    ] {
        assert!(
            !reached.contains_key(forbidden),
            "{forbidden} is reachable from cbr-provider; m4a adds no transport"
        );
    }
}

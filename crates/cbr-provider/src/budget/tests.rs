//! The gate for m4a, written and run before the module beneath it existed.
//!
//! Each test names the rule it holds. Nothing here opens a socket, reads a
//! credential or waits on a clock: time is moved, never slept through.

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
fn the_bound_never_falls_below_what_any_byte_level_tokenizer_could_emit() {
    // One-sided: over-estimating is the safe direction and under-estimating
    // admits a request the provider charges for anyway. The reference is the
    // worst case a byte-level BPE can produce, which is one token per byte —
    // obtained by arithmetic rather than measured, because no tokenizer is
    // available offline and fetching one is not this milestone's to do.
    for (name, bytes) in corpus() {
        let reference = worst_case_tokens(&bytes);
        let estimated = input_bound(&bytes, 1);
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
    assert!(input_bound(bytes, 1) >= worst_case_tokens(bytes));
}

#[test]
fn the_bound_counts_the_whole_body_and_not_only_its_messages() {
    // The defect this catches: an estimate that walks `messages` and forgets
    // the tool schemas, the system instructions or the JSON framing around
    // them. The serialized body is what is sent, so the serialized body is
    // what is counted.
    let small = input_bound(b"{\"messages\":[]}", 0);
    let large = input_bound(
        b"{\"messages\":[],\"tools\":[{\"name\":\"search\",\"schema\":{\"a\":1}}]}",
        0,
    );
    assert!(large > small, "{large} is not more than {small}");
    assert_eq!(large - small, 45, "every added byte is counted");
}

#[test]
fn the_bound_is_the_input_and_the_framing_and_nothing_else() {
    // **What nothing asserted until m4b.** The estimate's parts were named
    // in constants, documented, and never read by a test: removing the
    // margin from it left the whole workspace green. An estimate that is
    // only the input bound admits a request whose generation it has not
    // accounted for, which is the same defect the completion's reservation
    // had, one layer down.
    // **What changed, and why this test did.** m4a's `estimate` added a
    // fixed 4,096-token generation reserve because admission happened
    // before the request's own limit was known. Every call site knows it
    // now, so the completion reserves the real figure and this function is
    // the input alone — which is also what stops the calibration's ratio
    // column measuring the reservation instead of the bound.
    //
    // The property that reserve carried has not gone: it moved to
    // `model::tests::the_completions_reservation_covers_the_margin_as_well_as_the_generation`,
    // where the figure is the one the request actually asked for.
    for (name, bytes) in corpus() {
        assert_eq!(
            input_bound(&bytes, 0),
            worst_case_tokens(&bytes),
            "{name}: the bound with no messages to frame is the byte count"
        );
    }
}

#[test]
fn the_bound_grows_with_the_messages_it_frames() {
    // The provider frames each message, and the serialized body does not
    // obviously show that framing. Dropping the term left every test green.
    let body = b"{\"messages\":[]}";
    // Strictly greater, not merely equal to four framings: a framing term
    // of zero satisfies the equality and is not a framing at all, which is
    // exactly what the surviving mutant did.
    assert!(
        input_bound(body, 4) > input_bound(body, 0),
        "four messages cost more than none"
    );
    assert_eq!(
        input_bound(body, 4) - input_bound(body, 0),
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

/// Every row as (job, request, kind, tokens, estimate): what `rows` leaves
/// out is who a row is about, which is what m5-settle's rows must name.
fn attributed(connection: &Connection) -> Vec<(String, String, String, u64, u64)> {
    let mut statement = connection
        .prepare("SELECT job, request, kind, tokens, estimate FROM model_ledger ORDER BY id")
        .expect("the ledger table exists");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?.max(0) as u64,
                row.get::<_, i64>(4)?.max(0) as u64,
            ))
        })
        .expect("queries")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows")
}

#[test]
fn a_settlement_above_its_reservation_is_charged_whole_and_is_an_overrun_naming_its_job_and_request()
 {
    // A systematically low estimate is how an envelope leaks, so the
    // difference is a thing the ledger holds rather than a thing it
    // discards. **The counter holds the bill**, which is what the shared
    // quota was charged; and the row saying it passed its reservation
    // names the call, because an unattributed row stops nobody.
    let connection = database();
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "r", 10_000)
        .expect("admits")
        .expect("admitted");
    ledger
        .settle(T0, &reservation, Settlement::Usage(12_000))
        .expect("settles");
    let rows = attributed(&connection);
    assert!(
        rows.contains(&(
            "job".to_string(),
            "r".to_string(),
            "overrun".to_string(),
            12_000,
            10_000
        )),
        "no overrun row names the call that passed its reservation: {rows:?}"
    );
    assert_eq!(
        ledger.spend(T0, "job").expect("spend").window,
        12_000,
        "the actual spend wins, even when it is larger than the estimate"
    );
    assert!(
        rows.contains(&(
            "job".to_string(),
            "r".to_string(),
            "usage".to_string(),
            12_000,
            10_000
        )),
        "the estimate it passed is kept beside the bill: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.2 == "divergence"),
        "the old unattributed kind is still written: {rows:?}"
    );
}

#[test]
fn a_store_that_recorded_an_overrun_admits_nothing_again_even_after_a_restart() {
    // **The stop is the store's, not the process's**: an overrun is a
    // fact about what a provider did, and forgetting it at a restart would
    // admit the next call on the same assumption that just failed.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("ledger.sqlite");
    {
        let connection = Connection::open(&path).expect("opens");
        Ledger::migrate(&connection).expect("migrates");
        let ledger = Ledger::new(&connection);
        let reservation = ledger
            .admit(T0, "job", "r", 10_000)
            .expect("admits")
            .expect("admitted");
        ledger
            .settle(T0, &reservation, Settlement::Usage(10_001))
            .expect("settles");
    }
    let connection = Connection::open(&path).expect("reopens");
    let ledger = Ledger::new(&connection);
    assert_eq!(
        ledger.admit(T0, "another", "next", 1).expect("admits"),
        Err(Refusal::Overrun),
        "a store that overran admitted again after a restart"
    );
    // **Before every ceiling**, so the reason a caller reads is the stop
    // and not whichever limit it happened to meet first.
    assert_eq!(
        ledger
            .admit(T0, "another", "huge", PER_REQUEST_TOKENS + 1)
            .expect("admits"),
        Err(Refusal::Overrun),
        "the stop was read after a ceiling"
    );
    assert!(
        !Refusal::Overrun.a_tighter_figure_could_admit(),
        "no figure admits anything on a stopped store, so no count is worth making"
    );
}

#[test]
fn an_old_divergence_row_stops_nothing() {
    // **A guard.** Stores written before m5-settle hold unattributed
    // `divergence` rows. They are history, not a stop: keying the stop on
    // them would stop every store that ever settled above an estimate
    // under the old rule.
    let connection = database();
    let ledger = Ledger::new(&connection);
    ledger
        .note(T0, "", "", "divergence", 12_000, 10_000)
        .expect("notes");
    let admitted = ledger.admit(T0, "job", "r", 1).expect("admits");
    assert!(
        admitted.is_ok(),
        "an old divergence row stopped the store: {admitted:?}"
    );
}

#[test]
fn a_settlement_whose_overrun_row_cannot_be_written_leaves_its_reservation_standing() {
    // **The settlement and its overrun row are one transaction.** Written
    // apart, a failure between them would leave the bill counted and no
    // stop recorded, and the next call admitted on a store that had
    // overrun. Here the overrun row cannot be written, and the whole
    // settlement is refused: the reservation stands at its estimate,
    // which over-counts rather than forgets.
    let connection = database();
    connection
        .execute_batch(
            "CREATE TRIGGER no_overrun BEFORE INSERT ON model_ledger
             WHEN NEW.kind = 'overrun'
             BEGIN SELECT RAISE(ABORT, 'no overrun row'); END;",
        )
        .expect("trigger");
    let ledger = Ledger::new(&connection);
    let reservation = ledger
        .admit(T0, "job", "r", 10_000)
        .expect("admits")
        .expect("admitted");
    let settled = ledger.settle(T0, &reservation, Settlement::Usage(12_000));
    let rows = attributed(&connection);
    assert!(
        settled.is_err(),
        "a settlement whose overrun row was not written succeeded: {rows:?}"
    );
    assert_eq!(
        rows,
        vec![(
            "job".to_string(),
            "r".to_string(),
            "reservation".to_string(),
            10_000,
            10_000
        )],
        "half a settlement landed"
    );
}

#[test]
fn after_an_overrun_nothing_is_admitted_and_no_counter_passes_its_ceiling_but_by_overruns() {
    // A walk over three jobs under a run ceiling: admissions of any size,
    // settlements of any figure, many far above what was reserved. After
    // every step, the run and every job hold no more than their ceilings
    // and what settlements billed above their reservations; and once the
    // first overrun has settled, nothing is admitted.
    let connection = database();
    let ceiling = 300_000;
    let ledger = Ledger::new(&connection).with_run_ceiling(Some(ceiling));
    let mut seed: u64 = 0x5eed;
    let mut next = |bound: u64| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % bound
    };
    let mut open: Vec<Reservation> = Vec::new();
    let mut excess = 0;
    let mut overran_at = None;
    for step in 0..2_000 {
        if next(2) == 0 || open.is_empty() {
            let job = format!("job{}", next(3));
            let admitted = ledger
                .admit(T0, &job, "r", 1 + next(60_000))
                .expect("admits");
            if let Some(at) = overran_at {
                assert!(
                    matches!(admitted, Err(Refusal::Overrun)),
                    "step {step}: {admitted:?} after the overrun at step {at}"
                );
            }
            if let Ok(reservation) = admitted {
                open.push(reservation);
            }
        } else {
            let reservation = open.remove(next(open.len() as u64) as usize);
            let settlement = match next(4) {
                0 => Settlement::UsageUnknown,
                1 => Settlement::NothingSpent,
                _ => Settlement::Usage(next(reservation.estimate * 3 + 1)),
            };
            ledger
                .settle(T0, &reservation, settlement)
                .expect("settles");
            if let Settlement::Usage(billed) = settlement
                && billed > reservation.estimate
            {
                excess += billed - reservation.estimate;
                overran_at.get_or_insert(step);
            }
        }
        let run = ledger.spend(T0, "job0").expect("spend").window;
        assert!(
            run <= ceiling + excess,
            "step {step}: the run holds {run}, past {ceiling} and {excess} overrun"
        );
        for job in ["job0", "job1", "job2"] {
            let held = ledger.spend(T0, job).expect("spend").job;
            assert!(
                held <= PER_JOB_TOKENS + excess,
                "step {step}: {job} holds {held}, past its ceiling and {excess} overrun"
            );
        }
    }
    assert!(
        overran_at.is_some(),
        "the walk never overran, so it tested nothing"
    );
}

#[test]
fn a_stops_refusal_row_is_a_refusal() {
    // A refusal is an event and not a spend, and a report finds refusals
    // under one kind. The stop's refusal is written under it, with the
    // call it refused.
    for stop in ["overrun", "bound_unsound"] {
        let connection = database();
        let ledger = Ledger::new(&connection);
        ledger
            .note(T0, "other", "x", stop, 12_000, 10_000)
            .expect("notes");
        let _ = ledger.admit(T0, "job", "r", 5).expect("admits");
        let rows = attributed(&connection);
        assert_eq!(
            rows.last(),
            Some(&(
                "job".to_string(),
                "r".to_string(),
                "refusal".to_string(),
                0,
                5
            )),
            "after {stop}, the refused call is not a refusal row: {rows:?}"
        );
    }
}

// m4a's `the_crate_has_no_network_dependency_in_its_tree` moved to
// `wire::net::tests` when m4b gave the crate one, and became two tests
// there: the four crates that do not need a network client still have
// none, and the one that does names exactly what it added.

#[test]
fn two_admissions_racing_for_the_last_of_a_window_admit_exactly_one() {
    // **The property m4a claimed and m4c needs.** `BEGIN IMMEDIATE` takes
    // the write lock before the read, so a check-then-write race cannot
    // have both readers see the same room and both write against it.
    // Until m4c there was no concurrency to test it under; there is now.
    //
    // Two connections to one file, two threads, one barrier, and exactly
    // enough window left for one of the two requests.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("ledger.sqlite");
    let open = || {
        let connection = Connection::open(&path).expect("opens");
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .expect("wal");
        connection
            .busy_timeout(std::time::Duration::from_secs(10))
            .expect("busy timeout");
        connection
    };
    let setup = open();
    Ledger::migrate(&setup).expect("migrates");

    // Leave room for exactly one request of `each`.
    let each = 10_000;
    let ledger = Ledger::new(&setup);
    let mut spent = 0;
    let mut job = 0;
    while spent + PER_REQUEST_TOKENS <= WINDOW_TOKENS - each {
        ledger
            .admit(T0, &format!("filler-{job}"), "r", PER_REQUEST_TOKENS)
            .expect("admits")
            .expect("room");
        spent += PER_REQUEST_TOKENS;
        job += 1;
    }
    let remaining = WINDOW_TOKENS - each - spent;
    if remaining > 0 {
        ledger
            .admit(T0, "filler-top", "r", remaining)
            .expect("admits")
            .expect("room");
    }
    assert_eq!(
        ledger.spend(T0, "none").expect("spend").window,
        WINDOW_TOKENS - each,
        "exactly one request's worth is left"
    );
    drop(setup);

    let start = std::sync::Barrier::new(2);
    let admitted = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for racer in 0..2 {
            let start = &start;
            let admitted = &admitted;
            let open = &open;
            scope.spawn(move || {
                let connection = open();
                let ledger = Ledger::new(&connection);
                start.wait();
                if let Ok(Ok(_)) = ledger.admit(T0, &format!("racer-{racer}"), "r", each) {
                    admitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            });
        }
    });

    assert_eq!(
        admitted.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "exactly one of the two was admitted"
    );
    let after = open();
    assert!(
        Ledger::new(&after).spend(T0, "none").expect("spend").window <= WINDOW_TOKENS,
        "and the window was never over-committed"
    );
}

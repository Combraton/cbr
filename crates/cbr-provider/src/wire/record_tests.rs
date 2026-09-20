//! The gate for the recording boundary.

use rusqlite::Connection;

use super::Dialect;
use super::record::*;
use super::redact::{REDACTED, redact};
use crate::budget::Ledger;
use crate::model::{Answer, Attempt, Recorder, Runtime, no_barrier};

const T0: &str = "2026-09-20T12:00:00Z";

/// A response carrying somebody else's live credential, in the place a
/// provider actually put one: the query string of a presigned URL.
const PLANTED: &str = "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\
\"https://oss-cn.example.com/speech.mp3?OSSAccessKeyId=LTAI5tSTOREKEYCANARY\
&Expires=1789000000&Signature=STORESIGCANARY%3D\"},\"finish_reason\":\"stop\"}],\
\"usage\":{\"total_tokens\":40}}";

fn body() -> Vec<u8> {
    br#"{"max_tokens":64,"messages":[]}"#.to_vec()
}

#[test]
fn a_credential_shaped_string_in_a_response_never_reaches_the_store() {
    // **The test the boundary exists for.** Not "the log looks clean": the
    // store is opened as a file, every byte of it is read back, and the
    // planted strings are looked for in it.
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("store.sqlite");
    {
        let connection = Connection::open(&path).expect("opens");
        Ledger::migrate(&connection).expect("ledger");
        migrate(&connection).expect("records");
        let transport = Recorder::answering_with_bytes(vec![
            (Answer::Counted(30), br#"{"total_tokens":30}"#.to_vec()),
            (
                Answer::Completed {
                    body: PLANTED.as_bytes().to_vec(),
                    usage: Some(40),
                },
                PLANTED.as_bytes().to_vec(),
            ),
        ]);
        let recording = Recording {
            inner: &transport,
            store: &connection,
            now: T0,
            job: "job",
            request: "r",
            model: "MiniMax-M2.7",
            dialect: Dialect::OpenAi,
            scrubber: None,
        };
        let runtime = Runtime {
            ledger: Ledger::new(&connection),
            transport: &recording,
        };
        runtime.call(
            T0,
            &Attempt {
                job: "job",
                request: "r",
                body: &body(),
                messages: 1,
                generation: 64,
                dialect: Dialect::OpenAi,
            },
            &no_barrier,
        );
        // Both calls were recorded, and what was recorded says so.
        let recorded = rows(&connection).expect("rows");
        assert_eq!(recorded.len(), 2, "the count and the completion");
        let received = String::from_utf8_lossy(&recorded[1].2).to_string();
        assert!(
            received.contains(REDACTED),
            "the response was recorded: {received}"
        );
        assert!(
            received.contains("https://oss-cn.example.com/speech.mp3"),
            "and is still a record of something: {received}"
        );
    }
    // Every file the store left behind, including the write-ahead log.
    let mut scanned = 0usize;
    for entry in std::fs::read_dir(directory.path()).expect("reads") {
        let path = entry.expect("entry").path();
        let bytes = std::fs::read(&path).expect("reads");
        scanned += bytes.len();
        let text = String::from_utf8_lossy(&bytes);
        for canary in ["STOREKEYCANARY", "STORESIGCANARY"] {
            assert!(
                !text.contains(canary),
                "{canary} reached {}",
                path.display()
            );
        }
    }
    assert!(
        scanned > 1024,
        "the store was actually read: {scanned} bytes"
    );
}

#[test]
fn the_store_only_accepts_bytes_that_went_through_the_boundary() {
    // There is no constructor for `Redacted` but `redact`, so a call site
    // cannot hand the store bytes that did not pass. This test is what
    // says the type is load-bearing rather than decorative: it is the
    // compiler that enforces it, and this asserts the shape the compiler
    // is enforcing.
    let connection = Connection::open_in_memory().expect("opens");
    migrate(&connection).expect("records");
    record(
        &connection,
        T0,
        "job",
        "r",
        crate::model::Call::Completion,
        Dialect::OpenAi,
        "MiniMax-M2.7",
        &redact(b"sent", None),
        &redact(br#"{"api_key":"INLINECANARY"}"#, None),
    )
    .expect("records");
    let recorded = rows(&connection).expect("rows");
    assert_eq!(recorded.len(), 1);
    assert!(!String::from_utf8_lossy(&recorded[0].2).contains("INLINECANARY"));
}

#[test]
fn what_was_sent_is_recorded_too_and_is_redacted_the_same_way() {
    // A request body holds repository excerpts, and a repository can hold
    // a credential as easily as a response can.
    let connection = Connection::open_in_memory().expect("opens");
    migrate(&connection).expect("records");
    let transport = Recorder::new(vec![Answer::Counted(10)]);
    let recording = Recording {
        inner: &transport,
        store: &connection,
        now: T0,
        job: "job",
        request: "r",
        model: "MiniMax-M2.7",
        dialect: Dialect::OpenAi,
        scrubber: None,
    };
    let sent = br#"{"messages":[{"content":"found sk-REQUESTCANARY0123456789 in config"}]}"#;
    crate::model::Transport::send(&recording, crate::model::Call::Count, sent);
    let recorded = rows(&connection).expect("rows");
    assert_eq!(recorded.len(), 1);
    let stored = String::from_utf8_lossy(&recorded[0].1).to_string();
    assert!(!stored.contains("REQUESTCANARY"), "{stored}");
}

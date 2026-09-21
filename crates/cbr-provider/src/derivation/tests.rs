use super::*;
use crate::context::{int, list, text};
use crate::selection::Candidate;

fn candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            id: "c1".into(),
            path: "queue.md".into(),
            start_line: 1,
            end_line: 20,
            text: "the queue drains on shutdown".into(),
        },
        Candidate {
            id: "c2".into(),
            path: "queue.md".into(),
            start_line: 21,
            end_line: 40,
            text: "the drain is ordered".into(),
        },
    ]
}

fn question(offered: &[Offered]) -> Question<'_> {
    Question {
        model: "MiniMax-M3",
        dialect: "responses",
        task: "how does the queue drain?",
        selector: "queue drain",
        offered,
    }
}

fn made<'a>(question: &'a Question<'a>, answer: Answer) -> Made<'a> {
    Made {
        question,
        answer,
        spend: Spend {
            admission: crate::model::ADMITTED_COUNT,
            usage: Some(1234),
            input_usage: Some(1000),
            counted: Some(1100),
            repairs: 1,
            latency_ms: 4200,
        },
        job: "j1",
        request: "r1",
        item: "i1",
        made_at: "2026-09-21T00:00:00Z",
    }
}

#[test]
fn the_same_question_has_the_same_digest() {
    let offered = offered(&candidates());
    assert_eq!(question(&offered).digest(), question(&offered).digest());
}

#[test]
fn a_candidate_set_with_one_fewer_is_a_different_question() {
    // **This is the whole safety of replay.** A retained answer is an id
    // from the set it was offered; reusing it for a different set would
    // be choosing from a list the model never saw.
    let all = offered(&candidates());
    let fewer = offered(&candidates()[..1]);
    assert_ne!(question(&all).digest(), question(&fewer).digest());
}

#[test]
fn the_order_the_candidates_were_offered_in_is_part_of_the_question() {
    let mut reversed = candidates();
    reversed.reverse();
    assert_ne!(
        question(&offered(&candidates())).digest(),
        question(&offered(&reversed)).digest()
    );
}

#[test]
fn a_candidate_whose_text_changed_is_a_different_question() {
    // The record keeps digests rather than the text itself, so this is
    // the only thing standing between a file that was edited and an
    // answer chosen before it was.
    let mut edited = candidates();
    edited[1].text = "the drain is not ordered after all".into();
    assert_ne!(
        question(&offered(&candidates())).digest(),
        question(&offered(&edited)).digest()
    );
}

#[test]
fn a_different_selector_or_task_or_model_is_a_different_question() {
    let offered = offered(&candidates());
    let base = question(&offered).digest();
    let mut other = question(&offered);
    other.selector = "something else";
    assert_ne!(base, other.digest(), "the selector");
    let mut other = question(&offered);
    other.task = "a different question entirely";
    assert_ne!(base, other.digest(), "the task");
    let mut other = question(&offered);
    other.model = "MiniMax-M2.7-highspeed";
    assert_ne!(base, other.digest(), "the model");
}

#[test]
fn the_record_holds_no_repository_text() {
    // A derivation record is an audit record, not a second copy of the
    // repository. What was shown is identified by digest; the bytes
    // themselves are the source artifact the packet already cites.
    let offered = offered(&candidates());
    let value = record(&made(&question(&offered), Answer::Chose("c2".into())));
    let canonical = crate::context::canonical(&value);
    for quoted in ["the queue drains on shutdown", "the drain is ordered"] {
        assert!(!canonical.contains(quoted), "the record quotes {quoted}");
    }
    assert!(canonical.contains("queue.md"), "but it says where");
}

#[test]
fn a_record_carries_the_model_the_admission_the_cost_and_the_choice() {
    let offered = offered(&candidates());
    let question = question(&offered);
    let value = record(&made(&question, Answer::Chose("c2".into())));
    assert_eq!(text(&value, &["model"]), "MiniMax-M3");
    assert_eq!(text(&value, &["admission"]), crate::model::ADMITTED_COUNT);
    assert_eq!(int(&value, &["usage", "tokens"]), 1234);
    assert_eq!(int(&value, &["usage", "input_tokens"]), 1000);
    assert_eq!(int(&value, &["usage", "counted_tokens"]), 1100);
    assert_eq!(int(&value, &["usage", "repairs"]), 1);
    assert_eq!(int(&value, &["latency_ms"]), 4200);
    assert_eq!(list(&value, &["question", "offered"]).len(), 2);
    assert_eq!(text(&value, &["answer", "chose"]), "c2");
    assert_eq!(text(&value, &["question", "digest"]), question.digest());
}

#[test]
fn the_artifact_id_is_the_records_own_digest() {
    let offered = offered(&candidates());
    let first = record(&made(&question(&offered), Answer::Chose("c1".into())));
    let second = record(&made(&question(&offered), Answer::Chose("c2".into())));
    assert_ne!(
        artifact_id(&digest_of(&first)),
        artifact_id(&digest_of(&second)),
        "two answers to one question are two records"
    );
    assert_eq!(
        artifact_id(&digest_of(&first)),
        artifact_id(&digest_of(&first))
    );
    assert!(artifact_id(&digest_of(&first)).starts_with("der."));
}

#[test]
fn a_chosen_id_reads_back_as_the_id_that_was_chosen() {
    let offered = offered(&candidates());
    let value = record(&made(&question(&offered), Answer::Chose("c2".into())));
    assert!(matches!(answer_of(&value), Some(Answer::Chose(id)) if id == "c2"));
}

#[test]
fn a_failure_reads_back_as_the_typed_reason_it_failed_for() {
    let offered = offered(&candidates());
    let value = record(&made(
        &question(&offered),
        Answer::Unmet(crate::selection::NOT_OFFERED),
    ));
    assert!(
        matches!(answer_of(&value), Some(Answer::Unmet(reason)) if reason == crate::selection::NOT_OFFERED)
    );
}

#[test]
fn a_reason_this_build_does_not_know_is_not_guessed_at() {
    // A record written by a later build can name a reason this one has
    // never heard of. It is not mapped to the nearest thing that sounds
    // like it: an unreadable record is its own typed failure.
    assert_eq!(reason("model_ate_the_homework"), None);
    assert_eq!(reason(""), None);
}

#[test]
fn every_reason_the_runtime_can_produce_is_one_a_record_can_carry() {
    // The table is the thing a later build reads records through, so a
    // reason added elsewhere and not added here would make its own
    // records unreadable. Every refusal, every settlement and every
    // unusable response is constructed here rather than remembered.
    use crate::budget::{Refusal, Settlement};
    use crate::wire::response::Unusable;
    for known in REASONS {
        assert_eq!(reason(known), Some(*known), "{known}");
    }
    let mut produced: Vec<&'static str> = vec![
        crate::selection::NOT_OFFERED,
        crate::selection::NOT_STRUCTURED,
        // The four the call site itself names, which belong to no enum.
        "model_call_timed_out",
        "store_unavailable",
        "index_unavailable",
        "work_panicked",
        "model_network_not_permitted",
    ];
    for refusal in [
        Refusal::WindowExhausted,
        Refusal::MonthExhausted,
        Refusal::PerRequest,
        Refusal::PerJob,
        Refusal::RunCeiling,
    ] {
        produced.push(refusal.reason());
    }
    for settlement in [
        Settlement::UsageUnknown,
        Settlement::NothingSpent,
        Settlement::ProviderExhausted,
    ] {
        if let Some(reason) = settlement.reason() {
            produced.push(reason);
        }
    }
    for unusable in [
        Unusable::Malformed,
        Unusable::ProviderError,
        Unusable::ProviderExhausted,
        Unusable::ReasoningLeaked,
        Unusable::Truncated,
        Unusable::NotStructured,
        Unusable::NoToolCall,
    ] {
        produced.push(unusable.reason());
    }
    for reason in produced {
        assert!(
            REASONS.contains(&reason),
            "the table lost {reason}, which the runtime produces"
        );
    }
}

#[test]
fn a_reader_with_the_same_view_reads_a_derivation() {
    let under = readable_under(&["app".to_string()], &["adapter".to_string()]);
    assert!(covers(
        &under,
        &["app".to_string()],
        &["adapter".to_string()]
    ));
}

#[test]
fn a_reader_missing_a_repository_or_a_claim_does_not() {
    let under = readable_under(
        &["app".to_string(), "outside".to_string()],
        &["adapter".to_string()],
    );
    assert!(
        !covers(&under, &["app".to_string()], &["adapter".to_string()]),
        "a repository the job could read and the reader cannot"
    );
    assert!(
        !covers(
            &under,
            &["app".to_string(), "outside".to_string()],
            &[".no-claims".to_string()]
        ),
        "a claim the job could read and the reader cannot"
    );
}

#[test]
fn a_reader_who_can_read_more_than_the_job_could_reads_it() {
    // The rule is that the derivation is no wider than the reader, not
    // that the two are equal: an authority reads everything.
    let under = readable_under(&["app".to_string()], &[]);
    assert!(covers(
        &under,
        &["app".to_string(), "outside".to_string()],
        &["adapter".to_string()]
    ));
}

//! The gate for the launch decision.
//!
//! **Every row is decided here, without starting a process.** Before this
//! existed, "a careless launch returns before the credential read" was a
//! property of the *order of statements in `main`*, checked only by
//! process-level tests that asserted the message did not mention a
//! keychain — an assertion that would not have noticed a successful,
//! silent read. Now it is a property of the decision: a `Refuse` does not
//! read a credential, and the compiler and the table below say so.

use super::*;

/// Every combination of the four inputs that matters, and what it decides.
fn rows() -> Vec<(Launch, Decision)> {
    let refuse = |reason: &str| Decision::Refuse(reason.to_string());
    vec![
        // Nothing configured: the ordinary launch, and the one every
        // conformance run and every developer is.
        (
            Launch {
                model_configured: false,
                permit_model_network: false,
                calibrate: false,
                replay: false,
                model_run_ceiling: None,
            },
            Decision::Serve,
        ),
        // A ceiling without a model is not a mistake: it lowers the
        // envelope for a run that makes no model calls, which is a no-op
        // but an honest one.
        (
            Launch {
                model_configured: false,
                permit_model_network: false,
                calibrate: false,
                replay: false,
                model_run_ceiling: Some(1_000),
            },
            Decision::Serve,
        ),
        // Permitting calls to nothing.
        (
            Launch {
                model_configured: false,
                permit_model_network: true,
                calibrate: false,
                replay: false,
                model_run_ceiling: None,
            },
            refuse("--permit-model-network needs a configured model"),
        ),
        // A model with no permission would serve and fail every call.
        (
            Launch {
                model_configured: true,
                permit_model_network: false,
                calibrate: false,
                replay: false,
                model_run_ceiling: None,
            },
            refuse("--permit-model-network was not given"),
        ),
        // The live serving launch.
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: false,
                replay: false,
                model_run_ceiling: None,
            },
            if SERVING_CALLS_A_MODEL {
                Decision::ReadCredentialThenServe
            } else {
                refuse("no call site")
            },
        ),
        // The calibration, and the three ways of asking for it carelessly.
        (
            Launch {
                model_configured: false,
                permit_model_network: true,
                calibrate: true,
                replay: false,
                model_run_ceiling: Some(100_000),
            },
            refuse("--calibrate needs a configured model"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: false,
                calibrate: true,
                replay: false,
                model_run_ceiling: Some(100_000),
            },
            refuse("--calibrate makes live calls and needs --permit-model-network"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: true,
                replay: false,
                model_run_ceiling: None,
            },
            refuse("--calibrate needs --model-run-ceiling"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: true,
                replay: false,
                model_run_ceiling: Some(100_001),
            },
            refuse("is above the calibration's cap"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: true,
                replay: false,
                model_run_ceiling: Some(100_000),
            },
            Decision::ReadCredentialThenCalibrate,
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: true,
                replay: false,
                model_run_ceiling: Some(1),
            },
            Decision::ReadCredentialThenCalibrate,
        ),
        // The offline rebuild, and the three ways of asking for one that
        // would not be offline.
        (
            Launch {
                model_configured: true,
                permit_model_network: false,
                calibrate: false,
                replay: true,
                model_run_ceiling: None,
            },
            Decision::ServeFromRecords,
        ),
        (
            Launch {
                model_configured: false,
                permit_model_network: false,
                calibrate: false,
                replay: true,
                model_run_ceiling: None,
            },
            refuse("--replay-model needs a configured model"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: false,
                replay: true,
                model_run_ceiling: None,
            },
            refuse("is not an offline rebuild"),
        ),
        (
            Launch {
                model_configured: true,
                permit_model_network: true,
                calibrate: true,
                replay: true,
                model_run_ceiling: Some(100_000),
            },
            refuse("a launch is one or the other"),
        ),
    ]
}

#[test]
fn a_replay_never_reads_a_credential_and_never_opens_the_network() {
    // Stated over **every** input rather than over the table. The whole
    // claim of an offline rebuild is that it cannot call anything, and
    // the two things that would let it are a credential and the permit.
    for model_configured in [false, true] {
        for permit_model_network in [false, true] {
            for calibrate in [false, true] {
                for model_run_ceiling in [None, Some(1), Some(100_000), Some(100_001)] {
                    let launch = Launch {
                        model_configured,
                        permit_model_network,
                        calibrate,
                        replay: true,
                        model_run_ceiling,
                    };
                    let decided = decide(&launch);
                    assert!(
                        !decided.reads_credential(),
                        "{launch:?} reads a credential: {decided:?}"
                    );
                    if decided == Decision::ServeFromRecords {
                        assert!(
                            !permit_model_network && !calibrate && model_configured,
                            "{launch:?} was allowed to replay"
                        );
                    } else {
                        assert!(matches!(decided, Decision::Refuse(_)), "{decided:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn every_row_decides_what_it_should() {
    for (launch, expected) in rows() {
        let decided = decide(&launch);
        match (&decided, &expected) {
            // A refusal is matched on the part of the reason that carries
            // the meaning, so the wording can improve without the table
            // becoming a transcript of it.
            (Decision::Refuse(said), Decision::Refuse(wanted)) => assert!(
                said.contains(wanted.as_str()),
                "{launch:?}\n  said:   {said}\n  wanted: {wanted}"
            ),
            _ => assert_eq!(decided, expected, "{launch:?}"),
        }
    }
}

#[test]
fn a_refusal_never_reads_a_credential() {
    // **The rule that used to rest on statement order.** Nothing here
    // depends on where a check sits in `main`: a decision either reads the
    // credential or it does not, and no refusal does.
    for (launch, _) in rows() {
        let decided = decide(&launch);
        if matches!(decided, Decision::Refuse(_) | Decision::Serve) {
            assert!(
                !decided.reads_credential(),
                "{launch:?} would read the credential: {decided:?}"
            );
        }
    }
}

#[test]
fn a_serving_launch_reads_no_credential_while_nothing_serving_can_use_one() {
    // **READINESS §2: a Keychain read is a prompt, an audit entry and a
    // secret in a process that had no use for one.** Until m4c connects
    // selection to the transport, a *serving* launch is exactly such a
    // process: it would read the owner's key and have no call site to
    // spend it at.
    //
    // I found this by reading it myself — a careless probe of the binary
    // with a valid model configuration and `--permit-model-network`, which
    // read the owner's Keychain entry for a process that could not use it.
    // Nothing was sent, recorded or printed, and the bytes were zeroed on
    // drop; it was still a read that should not have been possible.
    //
    // So while `SERVING_CALLS_A_MODEL` is false, **the calibration is the
    // only launch that reads a credential**. m4c sets it true in the commit
    // that gives serving a call site, and this test then stops constraining
    // that row — which is why it is written as an implication rather than
    // as a flat assertion about today.
    if SERVING_CALLS_A_MODEL {
        return;
    }
    for model_run_ceiling in [None, Some(100_000)] {
        let launch = Launch {
            model_configured: true,
            permit_model_network: true,
            calibrate: false,
            replay: false,
            model_run_ceiling,
        };
        let decided = decide(&launch);
        assert!(
            !decided.reads_credential(),
            "{launch:?} read a credential with nowhere to spend it: {decided:?}"
        );
        assert!(matches!(decided, Decision::Refuse(_)), "{decided:?}");
    }
}

#[test]
fn the_only_launch_that_reads_a_credential_today_is_the_calibration() {
    // The consequence, stated over every input: one decision reads a key,
    // and it is the one that needs three flags and a ceiling.
    if SERVING_CALLS_A_MODEL {
        return;
    }
    for model_configured in [false, true] {
        for permit_model_network in [false, true] {
            for calibrate in [false, true] {
                for model_run_ceiling in [None, Some(1), Some(100_000), Some(100_001)] {
                    let launch = Launch {
                        model_configured,
                        permit_model_network,
                        calibrate,
                        replay: false,
                        model_run_ceiling,
                    };
                    let decided = decide(&launch);
                    if decided.reads_credential() {
                        assert_eq!(decided, Decision::ReadCredentialThenCalibrate, "{launch:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn both_live_decisions_are_reachable() {
    // The other half: the rule above cannot be satisfied by nothing ever
    // reading a credential. Asserted as *reachability of each decision*
    // rather than as a count of rows — an earlier version counted, and
    // adding one more table row to cover a smaller ceiling broke it
    // without anything having gone wrong.
    let decided: Vec<_> = rows()
        .into_iter()
        .map(|(launch, _)| (launch, decide(&launch)))
        .collect();
    let mut wanted = vec![Decision::ReadCredentialThenCalibrate];
    if SERVING_CALLS_A_MODEL {
        wanted.push(Decision::ReadCredentialThenServe);
    }
    for wanted in wanted {
        assert!(
            decided.iter().any(|(_, got)| *got == wanted),
            "no row reaches {wanted:?}"
        );
    }
    for (launch, got) in &decided {
        if got.reads_credential() {
            assert!(
                launch.model_configured && launch.permit_model_network,
                "{launch:?} reads a credential without both"
            );
        }
    }
}

#[test]
fn a_credential_is_read_only_when_a_model_is_configured_and_permitted() {
    // Stated over **every** input rather than over the table, so a
    // combination nobody thought to tabulate cannot slip through.
    for model_configured in [false, true] {
        for permit_model_network in [false, true] {
            for calibrate in [false, true] {
                for model_run_ceiling in [None, Some(1), Some(100_000), Some(100_001)] {
                    for replay in [false, true] {
                        let launch = Launch {
                            model_configured,
                            permit_model_network,
                            calibrate,
                            replay,
                            model_run_ceiling,
                        };
                        if decide(&launch).reads_credential() {
                            assert!(
                                model_configured && permit_model_network && !replay,
                                "{launch:?} reads a credential"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_calibration_is_never_decided_without_its_ceiling_inside_the_cap() {
    for model_run_ceiling in [None, Some(100_001), Some(u64::MAX)] {
        let launch = Launch {
            model_configured: true,
            permit_model_network: true,
            calibrate: true,
            replay: false,
            model_run_ceiling,
        };
        assert!(
            matches!(decide(&launch), Decision::Refuse(_)),
            "{launch:?} was allowed to calibrate"
        );
    }
}

#[test]
fn a_serving_launch_with_a_model_and_the_permit_reads_a_credential_and_serves() {
    // **What the constant asserts, asserted.** Every other test about
    // `SERVING_CALLS_A_MODEL` is an implication — `if the constant, skip`
    // — which is right for rules that stop applying when it flips, and
    // leaves nothing at all constraining the flipped value. Setting it
    // back to false broke no test in the suite, which means the fact it
    // stands for was not held by anything.
    //
    // The fact is this: serving has a call site now, so a launch given a
    // model and the permit has somewhere to spend a credential and is not
    // refused. A change that removes the call site has to change this
    // test, which is the point of it.
    assert_eq!(
        decide(&Launch {
            model_configured: true,
            permit_model_network: true,
            calibrate: false,
            replay: false,
            model_run_ceiling: None,
        }),
        Decision::ReadCredentialThenServe,
        "serving calls a model, so this launch is not refused"
    );
}

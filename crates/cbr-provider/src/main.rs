//! CBR's protocol provider.
//!
//! Serves `core/1` over the stdio form of the stream binding, plus the
//! conformance-only `core-test/1` under a conformance launch configuration.
//! Frames go to standard output and nothing else does: diagnostics go to
//! standard error, because a stray byte on standard output would corrupt or
//! inject a frame.

mod barriers;
mod budget;
mod calibration;
mod clock;
mod compiler;
mod config;
mod context;
mod credentials;
mod derivation;
mod effects;
mod envelope;
mod errors;
mod evidence;
mod frames;
mod grants;
mod jsonrpc;
mod keychain;
mod knowledge;
mod launch;
mod model;
mod outbox;
mod peer;
mod provider;
mod repositories;
mod selection;
mod session;
mod socket;
mod store;
mod wire;
mod work;

use std::path::PathBuf;

use crate::provider::Provider;

struct Args {
    data_dir: Option<PathBuf>,
    config: Option<PathBuf>,
    /// Serve the Unix-socket form at this path instead of stdio.
    socket: Option<PathBuf>,
    /// Administration: issue a new credential for the configured principal,
    /// revoking the old one, write its handoff file, and exit.
    rotate_credential: bool,
    /// Administration: revoke every credential of this principal and exit.
    revoke_credential: Option<String>,
    /// Administration: issue a credential for another principal — a second
    /// person, or a model producer that will act under a grant — revoking any
    /// it held, write its handoff file, and exit.
    issue_credential: Option<String>,
    /// Register a repository this process may read, as `<id>=<path>`. A
    /// checkout path is local configuration and never crosses the wire, so
    /// it is given to the process that reads it rather than to a client.
    register_repository: Vec<repositories::Registration>,
    /// A ceiling for this whole run, which can only lower the envelope.
    model_run_ceiling: Option<u64>,
    /// **Open the network gate.** Without it no socket can be opened at
    /// all, because the transport cannot be constructed without the permit
    /// this produces. A configured model is not enough: calling a provider
    /// spends the owner's quota and sends repository text to a third
    /// party, so it is a deliberate act at the launch rather than a
    /// consequence of having configured one.
    permit_model_network: bool,
    /// **Run the calibration** of [READINESS §10] and write its table here,
    /// then exit. Needs `--permit-model-network` and a run ceiling, and
    /// serves nothing.
    ///
    /// [READINESS §10]: ../../docs/work/m4/READINESS.md
    calibrate: Option<PathBuf>,
    /// **Rebuild offline.** Every model question is answered from a
    /// retained derivation record; the transport is one that panics if
    /// it is ever reached. No credential is read and no socket to a
    /// provider can be opened, which is what INTERNALS section 5 means
    /// by *reproducible from the retained records without calling the
    /// model again*.
    replay_model: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        data_dir: None,
        config: None,
        socket: None,
        rotate_credential: false,
        revoke_credential: None,
        issue_credential: None,
        register_repository: Vec::new(),
        model_run_ceiling: None,
        permit_model_network: false,
        calibrate: None,
        replay_model: false,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        match flag.as_str() {
            "--data-dir" => {
                args.data_dir = Some(PathBuf::from(
                    argv.next().ok_or("--data-dir needs a value")?,
                ));
            }
            "--config" => {
                args.config = Some(PathBuf::from(argv.next().ok_or("--config needs a value")?));
            }
            "--socket" => {
                args.socket = Some(PathBuf::from(argv.next().ok_or("--socket needs a value")?));
            }
            "--permit-model-network" => args.permit_model_network = true,
            "--replay-model" => args.replay_model = true,
            "--calibrate" => {
                args.calibrate = Some(PathBuf::from(
                    argv.next()
                        .ok_or("--calibrate needs a path to write its table to")?,
                ));
            }
            "--rotate-credential" => args.rotate_credential = true,
            "--revoke-credential" => {
                args.revoke_credential =
                    Some(argv.next().ok_or("--revoke-credential needs a principal")?);
            }
            "--issue-credential" => {
                args.issue_credential =
                    Some(argv.next().ok_or("--issue-credential needs a principal")?);
            }
            "--model-run-ceiling" => {
                let value = argv
                    .next()
                    .ok_or("--model-run-ceiling needs a number of tokens")?;
                args.model_run_ceiling = Some(
                    value
                        .parse()
                        .map_err(|_| "--model-run-ceiling takes a number of tokens".to_string())?,
                );
            }
            "--register-repository" => {
                let value = argv
                    .next()
                    .ok_or("--register-repository needs <id>=<path>")?;
                args.register_repository.push(repositories::parse(&value)?);
            }
            "--schemas" => {
                // Accepted and ignored: this provider validates against its own
                // code rather than by loading schemas at runtime.
                let _ = argv.next();
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let mut config = config::Config::load(args.config.as_deref())?;
    // A launch-set ceiling for this whole run. It is checked after the two
    // counters, so it can only lower the owner's envelope.
    config.model_run_ceiling = args.model_run_ceiling;
    let data_dir = args.data_dir.clone().unwrap_or_else(|| PathBuf::from("."));
    // Opened before the store, and before any socket is bound, so a malformed
    // clock file stops the launch before anything is written or listened on.
    let clock = clock::Clock::open(config.clock.clone())?;
    // Credential administration touches only the store and the handoff file,
    // then exits: it serves nothing.
    if args.rotate_credential || args.revoke_credential.is_some() || args.issue_credential.is_some()
    {
        let mut store = store::Store::open(&data_dir)
            .map_err(|error| format!("opening the store at {}: {error}", data_dir.display()))?;
        if let Some(principal) = &args.revoke_credential {
            let revoked = store
                .revoke_credentials(principal)
                .map_err(|e| e.to_string())?;
            eprintln!("cbr-provider: revoked {revoked} credential(s) of {principal}");
        }
        let issue_for = args
            .issue_credential
            .clone()
            .or_else(|| args.rotate_credential.then(|| config.principal.clone()));
        if let Some(principal) = issue_for {
            let path = credentials::issue(&mut store, &data_dir, &principal)?;
            eprintln!(
                "cbr-provider: issued a credential for {principal}; handoff file {}",
                path.display()
            );
        }
        return Ok(());
    }
    // **The launch decision is taken once, purely, and acted on here.**
    // Whether a credential is read is a property of the decision rather
    // than of the order of the statements below; `launch::tests` states
    // every row of it without starting a process.
    let decision = launch::decide(&launch::Launch {
        model_configured: config.model_runtime.is_some(),
        permit_model_network: args.permit_model_network,
        calibrate: args.calibrate.is_some(),
        model_run_ceiling: args.model_run_ceiling,
        replay: args.replay_model,
    });
    if let launch::Decision::Refuse(reason) = &decision {
        return Err(reason.clone());
    }
    if decision.reads_credential() {
        // The gate to a socket, and the only thing that opens it. A
        // transport cannot be constructed without the permit this
        // produces, which is what makes "CI opens no socket" a property of
        // the build rather than of the test suite's manners.
        wire::net::permit_network();
    }

    // **The one credential read, at process start, and only when the
    // decision says so.** It happens before the store is opened and before
    // anything is listened on, so a launch that cannot read the key it was
    // told to use refuses rather than serving and failing every model call
    // one at a time.
    let credential =
        keychain::for_launch(decision.reads_credential(), keychain::read).map_err(|refused| {
            format!(
                "a model is configured and its credential could not be read: {}",
                refused.reason()
            )
        })?;

    // The calibration serves nothing: it runs, writes its table and exits.
    if let Some(table) = args.calibrate {
        // The decision above already refused every launch without
        // these; what is left is the acting on it.
        let runtime = config
            .model_runtime
            .clone()
            .ok_or("--calibrate needs a configured model")?;
        let credential = credential.ok_or("--calibrate needs a credential")?;
        let permit = wire::net::permit().ok_or("--calibrate needs --permit-model-network")?;
        let store = store::Store::open(&data_dir)
            .map_err(|error| format!("opening the store at {}: {error}", data_dir.display()))?;
        let transport = wire::http::Http::new(
            permit,
            runtime.dialect,
            &credential,
            std::time::Duration::from_secs(120),
        );
        let now = clock.now();
        let scrubber = credential.scrubber();
        // Recorded and redacted like any other call: a measurement is not
        // exempt from READINESS section 6.
        let recording = wire::record::Recording {
            inner: &transport,
            store: store.connection(),
            now: &now,
            job: "calibration",
            request: "calibration",
            model: &runtime.model,
            dialect: runtime.dialect,
            scrubber: Some(&scrubber),
        };
        let report = calibration::run(
            &now,
            budget::Ledger::new(store.connection()).with_run_ceiling(config.model_run_ceiling),
            &recording,
            runtime.dialect,
            &runtime.model,
        );
        std::fs::write(&table, report.table())
            .map_err(|error| format!("writing {}: {error}", table.display()))?;
        eprintln!(
            "cbr-provider: calibration wrote {} ({} files, {} tokens estimated locally)",
            table.display(),
            report.rows.len(),
            report.local_total()
        );
        if let Some(stopped) = report.stopped {
            return Err(format!("calibration stopped: {stopped}"));
        }
        return Ok(());
    }

    // An unsafe socket directory refuses the start before the store is opened,
    // so a refused start writes nothing.
    if let Some(socket) = &args.socket {
        socket::check_directory(socket)?;
    }
    if let Some((directory, enabled)) = &config.test_barriers {
        barriers::init(directory, enabled);
    }
    // One serving process per data directory, held until this process ends.
    let _held = store::lock_data_dir(&data_dir)?;
    // Start-time effects — epoch, retention, capabilities, the generation —
    // happen once, here, whichever binding follows.
    let provider = Provider::open(config.clone(), clock, &data_dir)
        .map_err(|error| format!("opening the store at {}: {error}", data_dir.display()))?;

    // **The model this launch may call, if it may call one.** Two sources
    // and never both: the `model.fake` control, which carries its own
    // identity and needs no credential, or a configured `model_runtime`
    // with the permit and the credential `launch::decide` already
    // insisted on. Absent is the ordinary case, and then preparation is
    // the deterministic path M3 shipped.
    // **An offline rebuild, when this launch is one.** It answers from
    // retained derivation records and its transport panics if reached,
    // so it takes the model identity from the configuration and nothing
    // else: no fake, no credential, no permit.
    if args.replay_model && config.model.is_some() {
        return Err(
            "`model.fake` and --replay-model are two different transports and a launch \
                    has one; a rebuild that could be answered by a fake is not a rebuild"
                .into(),
        );
    }
    let serving = if args.replay_model {
        let runtime = config
            .model_runtime
            .clone()
            .ok_or("--replay-model needs a configured model")?;
        Some(std::sync::Arc::new(provider::Serving {
            dialect: runtime.dialect,
            model: runtime.model.clone(),
            counting: model::Counting::WhenItCouldAdmit,
            run_ceiling: config.model_run_ceiling,
            wire: provider::Wire::Replay,
        }))
    } else {
        match (
            config.model.clone(),
            config.model_runtime.clone(),
            credential,
        ) {
            (Some(fake), _, _) => Some(std::sync::Arc::new(provider::Serving {
                dialect: fake.dialect,
                model: fake.model.clone(),
                counting: fake.counting,
                run_ceiling: config.model_run_ceiling,
                wire: provider::Wire::Fake(model::Fake::new(
                    fake.dialect,
                    fake.answers,
                    fake.usage,
                )),
            })),
            (None, Some(runtime), Some(credential)) => {
                Some(std::sync::Arc::new(provider::Serving {
                    dialect: runtime.dialect,
                    model: runtime.model.clone(),
                    counting: model::Counting::WhenItCouldAdmit,
                    run_ceiling: config.model_run_ceiling,
                    wire: provider::Wire::Live(credential),
                }))
            }
            _ => None,
        }
    };
    let mut provider = provider.with_model(serving);

    // Registration is a launch-time act: a checkout that cannot be read is
    // refused here rather than becoming a repository that answers nothing.
    provider.register_repositories(&args.register_repository)?;

    if let Some(socket) = &args.socket {
        // A production socket needs a credential someone can hold. A
        // conformance launch is given its credentials by the runner instead.
        if config.mode == config::Mode::Production {
            let mut store = store::Store::open(&data_dir)
                .map_err(|error| format!("opening the store: {error}"))?;
            credentials::ensure(&mut store, &data_dir, &config.principal)?;
        }
        // Each session connects to the store this start already prepared.
        let clock = provider.shared_clock();
        // One pool for the process, shared by every connection's provider:
        // an index build started on one poll has to be the same build the
        // next poll finds running.
        let work = provider.shared_work();
        let model = provider.shared_model();
        drop(provider);
        return socket::serve(socket, config, clock, work, model, data_dir);
    }

    let stdin = std::io::stdin();
    match session::serve(stdin.lock(), std::io::stdout(), &mut provider, None)? {
        session::Ended::Normally => Ok(()),
        // The connection is closed: returning ends the process, which on the
        // stdio binding is what closing the connection means. A writer thread
        // still blocked on the stalled consumer ends with it and writes
        // nothing more.
        session::Ended::TooSlow(record) => {
            eprintln!("cbr-provider: {}", record.record());
            Ok(())
        }
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("cbr-provider: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}

//! CBR's protocol provider.
//!
//! Serves `core/1` over the stdio form of the stream binding, plus the
//! conformance-only `core-test/1` under a conformance launch configuration.
//! Frames go to standard output and nothing else does: diagnostics go to
//! standard error, because a stray byte on standard output would corrupt or
//! inject a frame.

mod barriers;
mod clock;
mod config;
mod effects;
mod envelope;
mod errors;
mod evidence;
mod frames;
mod grants;
mod jsonrpc;
mod outbox;
mod provider;
mod session;
mod socket;
mod store;

use std::path::PathBuf;

use crate::provider::Provider;

struct Args {
    data_dir: Option<PathBuf>,
    config: Option<PathBuf>,
    /// Serve the Unix-socket form at this path instead of stdio.
    socket: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        data_dir: None,
        config: None,
        socket: None,
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
    let config = config::Config::load(args.config.as_deref())?;
    let data_dir = args.data_dir.clone().unwrap_or_else(|| PathBuf::from("."));
    // Opened before the store, and before any socket is bound, so a malformed
    // clock file stops the launch before anything is written or listened on.
    let clock = clock::Clock::open(config.clock.clone())?;
    // An unsafe socket directory refuses the start before the store is opened,
    // so a refused start writes nothing.
    if let Some(socket) = &args.socket {
        socket::check_directory(socket)?;
    }
    if let Some((directory, enabled)) = &config.test_barriers {
        barriers::init(directory, enabled);
    }
    // Start-time effects — epoch, retention, capabilities, the generation —
    // happen once, here, whichever binding follows.
    let mut provider = Provider::open(config.clone(), clock, &data_dir)
        .map_err(|error| format!("opening the store at {}: {error}", data_dir.display()))?;

    if let Some(socket) = &args.socket {
        // Each session connects to the store this start already prepared.
        let clock = provider.shared_clock();
        drop(provider);
        return socket::serve(socket, config, clock, data_dir);
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

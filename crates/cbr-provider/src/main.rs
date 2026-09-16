//! CBR's protocol provider.
//!
//! Serves `core/1` over the stdio form of the stream binding, plus the
//! conformance-only `core-test/1`. Frames go to standard output and nothing
//! else does: diagnostics go to standard error, because a stray byte on
//! standard output would corrupt or inject a frame.
//!
//! Scope at this stage is the stream binding and the minimum command path the
//! `stream` fixtures exercise. The store is in memory (see `store.rs`), no
//! feature is implemented, and no test control is declared.

mod config;
mod envelope;
mod errors;
mod frames;
mod jsonrpc;
mod provider;
mod store;

use std::io::Write;
use std::path::PathBuf;

use cbr_encoding::Value;

use crate::errors::FrameFailure;
use crate::frames::{Frame, FrameReader};
use crate::provider::Provider;

struct Args {
    data_dir: Option<PathBuf>,
    config: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        data_dir: None,
        config: None,
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

/// Write one frame: canonical bytes, then the terminator. Never a line feed
/// inside, which canonical encoding guarantees by escaping control characters.
fn write_frame(out: &mut impl Write, value: &Value) -> std::io::Result<()> {
    let bytes = cbr_encoding::to_canonical(value);
    debug_assert!(
        !bytes.contains(&b'\n'),
        "a frame must not contain a line feed"
    );
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// A frame-level failure: answer once with a null id, flush, and close without
/// reading further input (STREAM section 2).
fn frame_failure(out: &mut impl Write, failure: FrameFailure) -> std::io::Result<()> {
    let error = Value::Object(vec![
        ("code".into(), Value::Int(failure.jsonrpc_code())),
        ("message".into(), Value::String(failure.code().into())),
        (
            "data".into(),
            Value::Object(vec![
                ("code".into(), Value::String(failure.code().into())),
                ("retry".into(), Value::String("no".into())),
                ("details".into(), Value::Object(vec![])),
            ]),
        ),
    ]);
    write_frame(out, &jsonrpc::error_response(Value::Null, error))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if let Some(dir) = &args.data_dir {
        // The directory is the provider's own state location. Creating it is
        // the only thing done with it at this stage, because the store is not
        // yet durable.
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    let config = config::Config::load(args.config.as_deref())?;
    let mut provider = Provider::new(config);

    let stdin = std::io::stdin();
    let mut reader = FrameReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    loop {
        let frame = match reader.next_frame(provider.frame_limit()) {
            Ok(frame) => frame,
            Err(error) => return Err(format!("reading standard input: {error}")),
        };
        match frame {
            // End of input: the provider exits 0 after draining.
            Frame::Eof => return Ok(()),
            Frame::Blank => continue,
            Frame::TooLarge => {
                let _ = frame_failure(&mut out, FrameFailure::TooLarge);
                return Ok(());
            }
            Frame::Bytes(bytes) => {
                let value = match frames::parse_frame(&bytes) {
                    Ok(value) => value,
                    Err(failure) => {
                        let _ = frame_failure(&mut out, failure);
                        return Ok(());
                    }
                };
                match jsonrpc::classify(&value) {
                    // A notification is neither processed nor answered.
                    jsonrpc::Incoming::Notification => continue,
                    jsonrpc::Incoming::Invalid { id } => {
                        let response =
                            jsonrpc::error_response(id, jsonrpc::invalid_request_error());
                        write_frame(&mut out, &response)
                            .map_err(|e| format!("writing response: {e}"))?;
                    }
                    jsonrpc::Incoming::Request { id, method, params } => {
                        let response = match provider.handle(&method, &params) {
                            Ok(result) => jsonrpc::response(id, result),
                            Err(error) => {
                                jsonrpc::error_response(id, error.to_error_object(error.code))
                            }
                        };
                        write_frame(&mut out, &response)
                            .map_err(|e| format!("writing response: {e}"))?;
                    }
                }
            }
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

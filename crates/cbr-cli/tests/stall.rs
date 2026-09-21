//! **How long the preparation tick is held, measured the way M3 measured it.**
//!
//! Not a gate. It is `#[ignore]`d, and it does nothing at all unless
//! `CBR_STALL_CHECKOUT` names a git checkout for it to index, so neither
//! `cargo test` nor CI ever runs it.
//!
//! It exists because M3's pilots recorded a stall and VERIFICATION named
//! it a known limit: *the index build holds the preparation tick
//! throughout, so every other job on that provider waits those 12.6
//! seconds.* A claim that m4c removed it is worth only what a measurement
//! of the same shape says, before and after, on the same machine and the
//! same trees.
//!
//! The instrument is two clients on one provider:
//!
//! * one submits a context request over the named checkout and polls for
//!   its packet, timing every poll;
//! * the other, on its own connection, keeps making an unrelated read and
//!   records the worst latency it sees.
//!
//! `tick_context` runs at the start of every request under the processing
//! lock, so the **bystander's worst latency is what "every other job
//! waits" means**, in seconds. The first client's worst poll is the same
//! stall seen from the inside. Time to first packet is printed beside
//! them because it is the number that should *not* improve: the build
//! costs what it costs, and m4c moves where it is paid, not how much.
//!
//! Run it as:
//!
//! ```text
//! CBR_STALL_CHECKOUT=<a git checkout> CBR_STALL_LABEL=<a name> \
//!   cargo test -p cbr-cli --test stall -- --ignored --nocapture
//! ```
//!
//! No model runs and no credential is read: the provider is started with
//! no model configured.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cbr_encoding::Value;

fn provider_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let binary = path.join("cbr-provider");
    assert!(
        binary.exists(),
        "{} is not built; build the workspace first",
        binary.display()
    );
    binary
}

fn ok(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "cbr failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    cbr_encoding::parse(String::from_utf8_lossy(&output.stdout).trim().as_bytes())
        .expect("canonical JSON on stdout")
}

/// Every `cbr` call is a fresh process on a fresh connection, so a
/// measured latency carries the client's own startup. `floor` measures
/// that startup against an idle provider, and every figure below is
/// reported beside it rather than corrected for it.
struct Client {
    binary: PathBuf,
    socket: PathBuf,
    credential: PathBuf,
}

impl Client {
    fn call(&self, arguments: &[&str]) -> (Duration, Output) {
        let started = Instant::now();
        let output = Command::new(&self.binary)
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(&self.credential)
            .output()
            .expect("cbr runs");
        (started.elapsed(), output)
    }
}

fn seconds(duration: Duration) -> String {
    format!("{:.3}s", duration.as_secs_f64())
}

#[test]
#[ignore = "measures a stall against a checkout named by CBR_STALL_CHECKOUT"]
fn the_preparation_tick_is_not_held_by_an_index_build() {
    let Ok(checkout) = std::env::var("CBR_STALL_CHECKOUT") else {
        eprintln!("CBR_STALL_CHECKOUT is unset: nothing measured");
        return;
    };
    let checkout = PathBuf::from(checkout);
    let label = std::env::var("CBR_STALL_LABEL").unwrap_or_else(|_| "unnamed".into());
    assert!(
        checkout.join(".git").exists(),
        "{} is not a git checkout",
        checkout.display()
    );

    let directory = tempfile::tempdir().expect("temp dir");
    let sockets = directory.path().join("s");
    std::fs::create_dir(&sockets).expect("socket dir");
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
    std::fs::write(
        directory.path().join("cbr.json"),
        r#"{"format":"cbr-config/1","principal":"owner"}"#,
    )
    .expect("config");
    let socket = sockets.join("cbr.sock");
    let data = directory.path().join("data");

    let mut provider: Child = Command::new(provider_binary())
        .arg("--data-dir")
        .arg(&data)
        .arg("--config")
        .arg(directory.path().join("cbr.json"))
        .arg("--socket")
        .arg(&socket)
        .arg("--register-repository")
        .arg(format!("app={}", checkout.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("provider starts");
    let started = Instant::now();
    while UnixStream::connect(&socket).is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "never listened"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let client = Client {
        binary: PathBuf::from(env!("CARGO_BIN_EXE_cbr")),
        socket: socket.clone(),
        credential: data.join("credentials").join("owner"),
    };

    // The client's own startup, against a provider with nothing to do.
    let mut floor = Duration::MAX;
    for _ in 0..5 {
        let (took, output) = client.call(&["request", "not-a-request"]);
        // Unknown or not: `tick_context` runs before the method is even
        // looked up, so this call pays the tick whatever it answers.
        drop(output);
        floor = floor.min(took);
    }

    let listed = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .args(["ls-files"])
            .output()
            .expect("git runs")
            .stdout,
    )
    .to_string();
    let mut tracked: Vec<String> = listed.lines().map(str::to_string).collect();
    let blobs = tracked.len();
    if let Some(position) = tracked.iter().position(|name| name == "README.md") {
        tracked.swap(0, position);
    }

    // The bystander: an unrelated read, over and over, on its own
    // connection, for as long as the build lasts.
    let stop = Arc::new(AtomicBool::new(false));
    let bystander = {
        let stop = Arc::clone(&stop);
        let client = Client {
            binary: client.binary.clone(),
            socket: client.socket.clone(),
            credential: client.credential.clone(),
        };
        std::thread::spawn(move || {
            let mut worst = Duration::ZERO;
            let mut calls = 0_u32;
            while !stop.load(Ordering::Relaxed) {
                let (took, _) = client.call(&["request", "not-a-request"]);
                worst = worst.max(took);
                calls += 1;
                // A bystander, not a load generator: without this pause it
                // spawns thousands of processes during the build and the
                // time to first packet measures the contention rather than
                // the build.
                std::thread::sleep(Duration::from_millis(100));
            }
            (worst, calls)
        })
    };

    // A request names at least one item. Which file it is does not
    // matter to a measurement of the build: the whole tree is indexed
    // whatever the item asks for. `README.md` when the tree has one, and
    // otherwise the first tracked file, so the harness needs to know
    // nothing about the repository it is pointed at.
    let want = format!("readme=source:{}", tracked.first().expect("a tracked file"));
    let repository = checkout.to_str().expect("utf-8");
    let (submitted_in, submitted) = client.call(&[
        "context",
        "stall",
        "--repo",
        repository,
        "--repo-id",
        "app",
        "--selector",
        "readme",
        "--task",
        "what does this repository do",
        "--capacity",
        "65536",
        "--want",
        &want,
    ]);
    assert!(
        submitted.status.success(),
        "submit: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );

    let polling = Instant::now();
    let mut polls = 0_u32;
    let mut worst_poll = Duration::ZERO;
    loop {
        let (took, output) = client.call(&["request", "stall"]);
        polls += 1;
        worst_poll = worst_poll.max(took);
        let inspected = ok(&output);
        let done = !inspected
            .get("packets")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .is_empty();
        if done {
            break;
        }
        assert!(
            polling.elapsed() < Duration::from_secs(300),
            "no packet for {label} after five minutes"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let to_first_packet = polling.elapsed();

    stop.store(true, Ordering::Relaxed);
    let (bystander_worst, bystander_calls) = bystander.join().expect("bystander");

    println!();
    println!("stall measurement: {label}");
    println!("  tracked files            {blobs}");
    println!("  client startup (floor)   {}", seconds(floor));
    println!("  submit                   {}", seconds(submitted_in));
    println!("  polls                    {polls}");
    println!("  worst poll               {}", seconds(worst_poll));
    println!("  bystander calls          {bystander_calls}");
    println!("  bystander worst latency  {}", seconds(bystander_worst));
    println!("  time to first packet     {}", seconds(to_first_packet));
    println!();

    let _ = provider.kill();
    let _ = provider.wait();
}

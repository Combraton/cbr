//! The `cbr` command against the real provider over its real socket.
//!
//! Issue #3's observable acceptance: bytes ingested with `cbr ingest` come back
//! from `cbr fetch` byte-identical, checked against the digest the provider
//! sealed, after the provider was killed with `SIGKILL` and started again. The
//! CLI is a separate binary that speaks only the public protocol, so nothing
//! here can pass through a shortcut into the store.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn cbr() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cbr"))
}

fn provider_binary() -> PathBuf {
    // Another package's binary: built by `cargo test --workspace`, and found
    // beside this test's own target directory.
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    let binary = path.join("cbr-provider");
    assert!(
        binary.exists(),
        "{} is not built; run the tests with `cargo test --workspace`",
        binary.display()
    );
    binary
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

/// Deterministic, incompressible-looking bytes large enough to need several
/// chunks in both directions.
fn content(size: usize) -> Vec<u8> {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

struct Fixture {
    _directory: tempfile::TempDir,
    data: PathBuf,
    socket: PathBuf,
    config: PathBuf,
}

impl Fixture {
    fn new(config: &str) -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let data = directory.path().join("data");
        let sockets = directory.path().join("sockets");
        std::fs::create_dir(&sockets).expect("socket dir");
        std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).expect("0700");
        let path = directory.path().join("config.json");
        std::fs::write(&path, config).expect("config");
        Self {
            data,
            socket: sockets.join("cbr.sock"),
            config: path,
            _directory: directory,
        }
    }

    fn start(&self) -> Child {
        let child = Command::new(provider_binary())
            .arg("--data-dir")
            .arg(&self.data)
            .arg("--config")
            .arg(&self.config)
            .arg("--socket")
            .arg(&self.socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("provider starts");
        // A killed provider leaves its socket file behind, so the file existing
        // proves nothing; a connection being accepted does.
        let started = Instant::now();
        while UnixStream::connect(&self.socket).is_err() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the provider never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    fn cbr(&self, credential_file: &Path, arguments: &[&str]) -> Output {
        Command::new(cbr())
            .args(arguments)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--credential-file")
            .arg(credential_file)
            .output()
            .expect("cbr runs")
    }
}

fn kill(mut child: Child) -> String {
    child.kill().expect("SIGKILL");
    child.wait().expect("reaped");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("stderr reads");
    stderr
}

fn line<'a>(stdout: &'a str, name: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name} ")))
        .unwrap_or_else(|| panic!("no `{name}` line in {stdout}"))
}

#[test]
fn ingested_bytes_fetch_identically_after_sigkill_and_restart() {
    let fixture = Fixture::new(r#"{"format":"cbr-config/1","principal":"owner"}"#);
    let provider = fixture.start();

    // The production start issued a credential and handed it off privately.
    let handoff = fixture.data.join("credentials").join("owner");
    assert_eq!(mode(handoff.parent().expect("dir")), 0o700);
    assert_eq!(mode(&handoff), 0o600);
    assert_eq!(mode(&fixture.socket), 0o600);
    let credential = std::fs::read_to_string(&handoff).expect("credential");
    let credential = credential.trim_end().to_string();

    let original = content(3 * 1024 * 1024 + 123);
    let input = fixture.data.join("..").join("input.bin");
    std::fs::write(&input, &original).expect("input");
    let expected = cbr_encoding::digest_bytes(&original);

    let ingested = fixture.cbr(&handoff, &["ingest", input.to_str().expect("utf-8")]);
    let stdout = String::from_utf8(ingested.stdout).expect("utf-8");
    let stderr = String::from_utf8(ingested.stderr).expect("utf-8");
    assert!(ingested.status.success(), "ingest failed: {stderr}");
    let artifact = line(&stdout, "artifact").to_string();
    assert_eq!(line(&stdout, "digest"), expected, "sealed digest");
    assert_eq!(line(&stdout, "size"), original.len().to_string());

    // SIGKILL, not a polite stop: the seal must already be durable.
    let first_stderr = kill(provider);
    let provider = fixture.start();
    assert_eq!(
        std::fs::read_to_string(&handoff)
            .expect("credential")
            .trim_end(),
        credential,
        "a restart keeps a live handed-off credential rather than rotating it"
    );

    let out = fixture.data.join("..").join("fetched.bin");
    let fetched = fixture.cbr(
        &handoff,
        &[
            "fetch",
            &artifact,
            "--digest",
            &expected,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    let fetch_stderr = String::from_utf8(fetched.stderr.clone()).expect("utf-8");
    assert!(fetched.status.success(), "fetch failed: {fetch_stderr}");
    let bytes = std::fs::read(&out).expect("fetched file");
    assert!(
        bytes == original,
        "fetched bytes differ from the ingested bytes"
    );
    assert_eq!(cbr_encoding::digest_bytes(&bytes), expected);

    // Naming a digest the artifact was not sealed with fetches nothing.
    let wrong = cbr_encoding::digest_bytes(b"something else");
    let refused_out = fixture.data.join("..").join("refused.bin");
    let refused = fixture.cbr(
        &handoff,
        &[
            "fetch",
            &artifact,
            "--digest",
            &wrong,
            "--out",
            refused_out.to_str().expect("utf-8"),
        ],
    );
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("artifact_digest_mismatch"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(!refused_out.exists(), "a refused fetch writes nothing");

    let second_stderr = kill(provider);
    // CORE section 18.1: the credential never reaches any output.
    for (name, text) in [
        ("ingest stdout", stdout.as_str()),
        ("ingest stderr", stderr.as_str()),
        ("fetch stderr", fetch_stderr.as_str()),
        ("provider stderr", first_stderr.as_str()),
        ("provider stderr after restart", second_stderr.as_str()),
    ] {
        assert!(
            !text.contains(&credential),
            "the credential appeared in {name}"
        );
    }
}

#[test]
fn fetch_refuses_bytes_that_do_not_match_the_sealed_digest() {
    // The provider verifies before it serves; this checks that `cbr` does not
    // rely on that. A conformance control makes the provider serve altered
    // bytes behind honest metadata, which only the reader's own check catches.
    let credential = format!("ccred1.owner.{}", "B".repeat(43));
    let credential_line = format!(r#""credentials":[{{"credential":"{credential}"}}]"#);
    let base = format!(
        r#""format":"combraton-conformance-config/1","principal":"owner","authority_principals":["owner"],{credential_line}"#
    );
    let fixture = Fixture::new(&format!("{{{base}}}"));
    let credential_file = fixture.config.with_file_name("credential");
    std::fs::write(&credential_file, format!("{credential}\n")).expect("credential file");

    let provider = fixture.start();
    let original = content(64 * 1024);
    let input = fixture.config.with_file_name("input.bin");
    std::fs::write(&input, &original).expect("input");
    let ingested = fixture.cbr(
        &credential_file,
        &["ingest", input.to_str().expect("utf-8")],
    );
    assert!(
        ingested.status.success(),
        "{}",
        String::from_utf8_lossy(&ingested.stderr)
    );
    let stdout = String::from_utf8(ingested.stdout).expect("utf-8");
    let artifact = line(&stdout, "artifact").to_string();
    let digest = line(&stdout, "digest").to_string();
    kill(provider);

    std::fs::write(
        &fixture.config,
        format!(r#"{{{base},"evidence_store":{{"serve_altered_bytes":["{artifact}"]}}}}"#),
    )
    .expect("config");
    let provider = fixture.start();
    let out = fixture.config.with_file_name("altered.bin");
    let fetched = fixture.cbr(
        &credential_file,
        &[
            "fetch",
            &artifact,
            "--digest",
            &digest,
            "--out",
            out.to_str().expect("utf-8"),
        ],
    );
    kill(provider);
    assert!(!fetched.status.success(), "altered bytes must be refused");
    let stderr = String::from_utf8_lossy(&fetched.stderr);
    assert!(stderr.contains("nothing written"), "{stderr}");
    assert!(
        !out.exists(),
        "altered bytes are never written as the artifact"
    );
    assert!(
        !out.with_extension("cbr-partial").exists(),
        "nor left behind partially"
    );
}

/// CORE section 18.1: a provider on a shared transport must support rotation
/// and revocation. Both are administration commands run beside the serving
/// provider, and both apply to the next authentication without a restart.
#[test]
fn a_rotated_or_revoked_credential_no_longer_authenticates() {
    let fixture = Fixture::new(r#"{"format":"cbr-config/1","principal":"owner"}"#);
    let provider = fixture.start();
    let handoff = fixture.data.join("credentials").join("owner");
    let first = std::fs::read_to_string(&handoff).expect("credential");
    let kept = fixture.config.with_file_name("first-credential");
    std::fs::write(&kept, &first).expect("copy");
    let input = fixture.config.with_file_name("input.bin");
    std::fs::write(&input, content(4096)).expect("input");
    let ingest =
        |credential: &Path| fixture.cbr(credential, &["ingest", input.to_str().expect("utf-8")]);
    let admin = |flag: &[&str]| {
        Command::new(provider_binary())
            .arg("--data-dir")
            .arg(&fixture.data)
            .arg("--config")
            .arg(&fixture.config)
            .args(flag)
            .output()
            .expect("administration runs")
    };
    let refused = |output: &Output| {
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("authentication_failed")
    };
    assert!(
        ingest(&handoff).status.success(),
        "the issued credential works"
    );

    let rotated = admin(&["--rotate-credential"]);
    assert!(
        rotated.status.success(),
        "{}",
        String::from_utf8_lossy(&rotated.stderr)
    );
    let second = std::fs::read_to_string(&handoff).expect("credential");
    assert_ne!(first, second, "rotation replaces the handoff file");
    assert!(
        !String::from_utf8_lossy(&rotated.stderr).contains(second.trim_end()),
        "rotation never prints the credential"
    );
    assert!(
        refused(&ingest(&kept)),
        "the rotated-out credential is refused"
    );
    assert!(
        ingest(&handoff).status.success(),
        "the new credential works"
    );

    let revoked = admin(&["--revoke-credential", "owner"]);
    assert!(
        revoked.status.success(),
        "{}",
        String::from_utf8_lossy(&revoked.stderr)
    );
    assert!(
        refused(&ingest(&handoff)),
        "a revoked credential is refused"
    );

    // A restart finds the handed-off credential revoked and issues a new one,
    // so a production socket always has a credential someone can hold.
    kill(provider);
    let provider = fixture.start();
    let third = std::fs::read_to_string(&handoff).expect("credential");
    assert_ne!(third, second);
    assert!(
        ingest(&handoff).status.success(),
        "the reissued credential works"
    );
    assert!(refused(&ingest(&kept)), "and nothing earlier came back");
    kill(provider);
}

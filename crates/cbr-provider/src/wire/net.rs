//! The gate between CBR and a socket.
//!
//! **Shut by default.** A launch opens it only when the owner passes
//! `--permit-model-network`, and until then the transport cannot be built
//! at all: [`Http`](super::http::Http) takes a [`Permit`], and the only way
//! to obtain one is [`permit`], which answers `None` unless the gate was
//! opened.
//!
//! That is what makes "CI opens no socket" and "m4b makes no live call"
//! enforceable rather than promised. A test cannot reach the network by
//! forgetting something; it would have to open the gate on purpose, and
//! [`tests::the_gate_has_exactly_one_caller_and_it_is_the_launch`] is what
//! says no test does.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static PERMITTED: AtomicBool = AtomicBool::new(false);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);

/// Proof that this launch permitted network access.
///
/// Zero-sized, and its field is private to this module, so it cannot be
/// conjured anywhere else — the same shape as
/// [`Redacted`](super::redact::Redacted), for the same reason.
#[derive(Debug)]
pub struct Permit(());

/// Open the gate. **One caller, and it is the launch.**
pub fn permit_network() {
    PERMITTED.store(true, Ordering::Release);
}

/// A permit, if this launch has one.
pub fn permit() -> Option<Permit> {
    permitted().then_some(Permit(()))
}

pub fn permitted() -> bool {
    PERMITTED.load(Ordering::Acquire)
}

/// How many requests have left this process. Zero in every test run.
pub fn requests() -> usize {
    REQUESTS.load(Ordering::Acquire)
}

/// Counted at the one place a request is made.
pub(super) fn note_request() {
    REQUESTS.fetch_add(1, Ordering::AcqRel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_is_shut_and_no_request_has_been_made() {
        // If this ever fails, something in the suite opened it.
        assert!(!permitted(), "the network gate was opened by a test");
        assert!(permit().is_none(), "a permit was available to a test");
        assert_eq!(requests(), 0, "a request left the process during a test");
    }

    #[test]
    fn the_gate_has_exactly_one_caller_and_it_is_the_launch() {
        // **The assertion behind "no test in the suite resolves a host".**
        // The transport cannot be constructed without a permit, a permit
        // cannot be had without the gate, and the gate has one caller: the
        // launch path, behind a flag the owner passes deliberately.
        //
        // A source scan, because the claim is about what the code *says*
        // rather than about what one run of it did.
        let mut callers = Vec::new();
        for path in sources() {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let here = path.ends_with("wire/net.rs");
            for (number, line) in text.lines().enumerate() {
                if !line.contains("permit_network()") {
                    continue;
                }
                // Its own definition and this test's own text.
                if here {
                    continue;
                }
                callers.push(format!("{}:{}", path.display(), number + 1));
            }
        }
        assert_eq!(
            callers.len(),
            1,
            "the network gate should have exactly one caller: {callers:?}"
        );
        assert!(
            callers[0].contains("src/main.rs"),
            "and it should be the launch: {callers:?}"
        );
    }

    /// Network clients and TLS stacks, whatever brings them in. If one of
    /// these appears where it should not, something reached for a socket.
    const NETWORK_CLIENTS: [&str; 14] = [
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
    ];

    /// **Everything the transport added**, named here as well as in
    /// [STACK §8.2](../../../docs/work/readiness/STACK.md), where each one
    /// carries its licence and the reason it is there. Fifteen of these are
    /// Windows target shims that build on no platform CBR supports.
    const ADDED_BY_THE_TRANSPORT: [&str; 26] = [
        "base64",
        "getrandom",
        "http",
        "httparse",
        "ring",
        "rustls",
        "rustls-pki-types",
        "rustls-webpki",
        "subtle",
        "untrusted",
        "ureq",
        "ureq-proto",
        "utf8-zero",
        "wasi",
        "webpki-roots",
        "windows-sys",
        "windows-targets",
        "windows_aarch64_gnullvm",
        "windows_aarch64_msvc",
        "windows_i686_gnu",
        "windows_i686_gnullvm",
        "windows_i686_msvc",
        "windows_x86_64_gnu",
        "windows_x86_64_gnullvm",
        "windows_x86_64_msvc",
        "zeroize",
    ];

    #[test]
    fn no_crate_but_the_provider_reaches_a_network_client() {
        // A dependency is kept out of every crate that does not need one.
        // The memory engine indexes a tree, the identity crate reads a git
        // object, the encoding crate canonicalises JSON and the CLI speaks
        // to a Unix socket: none of them has any business opening one.
        for crate_name in ["cbr-encoding", "cbr-identity", "cbr-memory", "cbr-cli"] {
            let reached = reachable(crate_name);
            for client in NETWORK_CLIENTS {
                assert!(
                    !reached.contains(client),
                    "{client} is reachable from {crate_name}, which needs no network"
                );
            }
        }
    }

    #[test]
    fn the_transport_added_exactly_the_crates_that_were_named() {
        // Two halves. Everything named above is really there, so the list
        // cannot rot into a list of things CBR used to depend on; and the
        // only network clients reachable at all are the two that were
        // chosen, so a third cannot arrive as somebody else's transitive
        // dependency without this failing.
        let reached = reachable("cbr-provider");
        for added in ADDED_BY_THE_TRANSPORT {
            assert!(
                reached.contains(added),
                "{added} is named as a dependency of the transport and is not in the tree"
            );
        }
        let clients: Vec<&str> = NETWORK_CLIENTS
            .into_iter()
            .filter(|client| reached.contains(*client))
            .collect();
        assert_eq!(
            clients,
            ["rustls", "ureq"],
            "the only network clients in the tree are the ones that were chosen"
        );
    }

    /// Every package reachable from `root` in the workspace lock file.
    fn reachable(root: &str) -> std::collections::BTreeSet<String> {
        use std::collections::{BTreeMap, BTreeSet};
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
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut stack = vec![root.to_string()];
        while let Some(package) = stack.pop() {
            if !reached.insert(package.clone()) {
                continue;
            }
            for next in dependencies.get(&package).cloned().unwrap_or_default() {
                stack.push(next);
            }
        }
        assert!(reached.len() > 5, "the lock file parsed: {}", reached.len());
        reached
    }

    /// Every Rust source in the workspace, tests included.
    fn sources() -> Vec<std::path::PathBuf> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the workspace root");
        let mut found = Vec::new();
        let mut stack = vec![root.join("crates")];
        while let Some(directory) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    found.push(path);
                }
            }
        }
        assert!(
            found.len() > 20,
            "the workspace was walked: {}",
            found.len()
        );
        found
    }
}

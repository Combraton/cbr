//! Source identity (PROTOCOL-PIN section 5).
//!
//! Protocol 0.1 compares `tree`, `dirty.snapshot_digest` and `environment` as
//! opaque strings by exact equality. Equality is only useful if the producer
//! computes them deterministically, so CBR fixes its own definitions here and
//! every one of them has a property test with a negative control: a change
//! that must alter the value and a change that must not.
//!
//! - **`tree`** is the git **root tree object id** of a commit, not the commit
//!   id: two commits with identical content are one content basis. The commit
//!   is returned beside it, so a producer records it in the evidence
//!   descriptor's capture anchors (ADR 001, question 8).
//! - **`dirty.snapshot_digest`** is `sha256` over the canonical JSON of
//!   `{ format, base_tree, entries }`, where each entry is
//!   `[path, status, mode, sha256-of-bytes | null]`, sorted by path, covering
//!   modified, staged, deleted and untracked-not-ignored files. A deletion is
//!   present with an explicit status, so "removed" is never "absent from the
//!   listing". Modification time is never an input.
//! - **`environment`** is `sha256` over the canonical JSON of an explicitly
//!   declared fact set. **Build facts are part of it** (ADR 001, question 4),
//!   and the record says so in `covers`, because Protocol 0.1 has no
//!   `build_digest` condition (G1). Nothing undeclared is ever captured.
//!
//! Git is read through `gix`, in this process: no subprocess, no shell, and no
//! runtime dependency on an installed `git`. ADR 001 question 11 chose the
//! `git` binary while identity needed one `rev-parse`, and named the trigger
//! that would change it — retrieval reading a tree's blobs in bulk. M3 does
//! that, so this moved, behind the same functions and with the same property
//! tests as its acceptance. This crate reads; it never changes a repository.

use std::path::Path;

use cbr_encoding::Value;
use gix::bstr::ByteSlice;

pub const SNAPSHOT_FORMAT: &str = "cbr-dirty-snapshot/1";
pub const ENVIRONMENT_FORMAT: &str = "cbr-environment/1";

/// Why an identity could not be computed. Never a partial answer.
#[derive(Debug)]
pub enum IdentityError {
    Git(String),
    Io(std::io::Error),
    Facts(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityError::Git(message) => write!(f, "git: {message}"),
            IdentityError::Io(error) => write!(f, "io: {error}"),
            IdentityError::Facts(message) => write!(f, "environment facts: {message}"),
        }
    }
}

impl From<std::io::Error> for IdentityError {
    fn from(error: std::io::Error) -> Self {
        IdentityError::Io(error)
    }
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn open(repository: &Path) -> Result<gix::Repository, IdentityError> {
    gix::open(repository).map_err(|error| IdentityError::Git(format!("opening: {error}")))
}

/// A commit and the root tree object it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBasis {
    pub commit: String,
    pub tree: String,
}

/// The commit a revision names, and its root tree object id.
pub fn git_basis(repository: &Path, revision: &str) -> Result<GitBasis, IdentityError> {
    let repository = open(repository)?;
    basis(&repository, revision)
}

fn basis(repository: &gix::Repository, revision: &str) -> Result<GitBasis, IdentityError> {
    let object = repository
        .rev_parse_single(revision)
        .map_err(|error| IdentityError::Git(format!("resolving {revision}: {error}")))?;
    let commit = object
        .object()
        .map_err(|error| IdentityError::Git(format!("reading {revision}: {error}")))?
        .peel_to_commit()
        .map_err(|error| IdentityError::Git(format!("{revision} is not a commit: {error}")))?;
    let tree = commit
        .tree_id()
        .map_err(|error| IdentityError::Git(format!("reading the tree of {revision}: {error}")))?;
    Ok(GitBasis {
        commit: commit.id().to_string(),
        tree: tree.to_string(),
    })
}

/// One path in a tree: where it is, what it is, and the blob it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: String,
    /// Git's six-digit mode, as the tree records it.
    pub mode: String,
    pub blob: String,
}

/// Every blob in a tree, recursively, in the tree's own order. Submodules are
/// reported as gitlinks with no blob: their contents are their own
/// repository's identity, not this one's.
pub fn tree_entries(repository: &Path, tree: &str) -> Result<Vec<TreeEntry>, IdentityError> {
    let repository = open(repository)?;
    let tree = repository
        .rev_parse_single(tree)
        .map_err(|error| IdentityError::Git(format!("resolving the tree: {error}")))?
        .object()
        .map_err(|error| IdentityError::Git(format!("reading the tree: {error}")))?
        .peel_to_tree()
        .map_err(|error| IdentityError::Git(format!("not a tree: {error}")))?;
    let mut recorder = gix::traverse::tree::Recorder::default();
    tree.traverse()
        .breadthfirst(&mut recorder)
        .map_err(|error| IdentityError::Git(format!("walking the tree: {error}")))?;
    let mut entries = Vec::new();
    for record in recorder.records {
        if record.mode.is_tree() {
            continue;
        }
        entries.push(TreeEntry {
            path: record.filepath.to_str_lossy().into_owned(),
            mode: format!("{:06o}", record.mode.value()),
            blob: record.oid.to_string(),
        });
    }
    Ok(entries)
}

/// One blob's bytes, by object id.
pub fn read_blob(repository: &Path, blob: &str) -> Result<Vec<u8>, IdentityError> {
    let repository = open(repository)?;
    let object = repository
        .rev_parse_single(blob)
        .map_err(|error| IdentityError::Git(format!("resolving blob {blob}: {error}")))?
        .object()
        .map_err(|error| IdentityError::Git(format!("reading blob {blob}: {error}")))?;
    Ok(object.detach().data)
}

/// A dirty working tree's snapshot: the digest, and the document it is the
/// digest of, so a reader can check one against the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub digest: String,
    pub document: Value,
}

/// The snapshot of everything that differs from `HEAD`, or `None` when the
/// working tree is clean.
pub fn dirty_snapshot(path: &Path) -> Result<Option<Snapshot>, IdentityError> {
    let repository = open(path)?;
    let base = basis(&repository, "HEAD")?;
    // Both halves of "dirty": what the working tree has that the index does
    // not, and what the index has that `HEAD` does not. Untracked files are
    // reported one by one rather than collapsed into their directory, because
    // an entry names a file. Ignored files are not reported at all.
    let status = repository
        .status(gix::progress::Discard)
        .map_err(|error| IdentityError::Git(format!("status: {error}")))?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .index_worktree_submodules(gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::All,
            check_dirty: false,
        })
        .into_iter(None)
        .map_err(|error| IdentityError::Git(format!("status: {error}")))?;
    let mut changed: Vec<(String, bool)> = Vec::new();
    for item in status {
        let item = item.map_err(|error| IdentityError::Git(format!("status: {error}")))?;
        match item {
            gix::status::Item::IndexWorktree(item) => {
                let untracked = matches!(
                    item,
                    gix::status::index_worktree::Item::DirectoryContents { .. }
                );
                changed.push((item.rela_path().to_str_lossy().into_owned(), untracked));
            }
            gix::status::Item::TreeIndex(change) => {
                changed.push((location(&change).to_str_lossy().into_owned(), false));
            }
        }
    }
    changed.sort();
    changed.dedup_by(|a, b| a.0 == b.0);
    let mut entries: Vec<(String, Value)> = Vec::new();
    for (path_in_tree, untracked) in changed {
        let value = entry(path, &path_in_tree, untracked)?;
        entries.push((path_in_tree, value));
    }
    if entries.is_empty() {
        return Ok(None);
    }
    entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let document = object(vec![
        ("format", Value::String(SNAPSHOT_FORMAT.into())),
        ("base_tree", Value::String(base.tree)),
        (
            "entries",
            Value::Array(entries.into_iter().map(|(_, e)| e).collect()),
        ),
    ]);
    Ok(Some(Snapshot {
        digest: cbr_encoding::digest_canonical(&document),
        document,
    }))
}

/// The path a staged change names.
fn location(change: &gix::diff::index::Change) -> &gix::bstr::BStr {
    match change {
        gix::diff::index::Change::Addition { location, .. }
        | gix::diff::index::Change::Deletion { location, .. }
        | gix::diff::index::Change::Modification { location, .. }
        | gix::diff::index::Change::Rewrite { location, .. } => location.as_ref(),
    }
}

/// `[path, status, mode, sha256 | null]` for one changed path, from what is
/// on disk now. The mode and bytes are read, never the modification time.
fn entry(repository: &Path, path: &str, untracked: bool) -> Result<Value, IdentityError> {
    let on_disk = repository.join(path);
    let row = |status: &str, mode: &str, digest: Value| {
        Value::Array(vec![
            Value::String(path.to_string()),
            Value::String(status.to_string()),
            Value::String(mode.to_string()),
            digest,
        ])
    };
    let metadata = match std::fs::symlink_metadata(&on_disk) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(row("deleted", "000000", Value::Null));
        }
        Err(error) => return Err(error.into()),
    };
    let status = if untracked { "untracked" } else { "modified" };
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(&on_disk)?;
        let bytes = target.as_os_str().as_encoded_bytes();
        return Ok(row(
            status,
            "120000",
            Value::String(cbr_encoding::digest_bytes(bytes)),
        ));
    }
    if metadata.is_dir() {
        // A submodule's working tree: recorded as a gitlink, its contents are
        // the submodule's own identity, not this repository's.
        return Ok(row(status, "160000", Value::Null));
    }
    use std::os::unix::fs::PermissionsExt;
    let mode = if metadata.permissions().mode() & 0o111 != 0 {
        "100755"
    } else {
        "100644"
    };
    let bytes = std::fs::read(&on_disk)?;
    Ok(row(
        status,
        mode,
        Value::String(cbr_encoding::digest_bytes(&bytes)),
    ))
}

/// One declared fact. `class` says whether it describes the environment or
/// the build; both are covered by the one environment digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub name: String,
    pub class: FactClass,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactClass {
    Environment,
    Build,
}

impl FactClass {
    fn wire(self) -> &'static str {
        match self {
            FactClass::Environment => "environment",
            FactClass::Build => "build",
        }
    }
}

/// Read a declared fact set: `{ "format": "cbr-environment-facts/1",
/// "facts": [ { "name", "class": "environment" | "build", "value" } ] }`.
pub fn parse_facts(bytes: &[u8]) -> Result<Vec<Fact>, IdentityError> {
    let value =
        cbr_encoding::parse(bytes).map_err(|error| IdentityError::Facts(error.to_string()))?;
    if value.get("format").and_then(Value::as_str) != Some("cbr-environment-facts/1") {
        return Err(IdentityError::Facts(
            "format must be cbr-environment-facts/1".into(),
        ));
    }
    let mut facts = Vec::new();
    for item in value
        .get("facts")
        .and_then(Value::as_array)
        .ok_or_else(|| IdentityError::Facts("facts must be an array".into()))?
    {
        let text = |name: &str| {
            item.get(name)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| IdentityError::Facts(format!("each fact needs a string `{name}`")))
        };
        let class = match text("class")?.as_str() {
            "environment" => FactClass::Environment,
            "build" => FactClass::Build,
            other => return Err(IdentityError::Facts(format!("unknown class {other}"))),
        };
        facts.push(Fact {
            name: text("name")?,
            class,
            value: text("value")?,
        });
    }
    Ok(facts)
}

/// The environment record and its digest. Facts are ordered by name, so the
/// order a file lists them in is not identity; a name declared twice is
/// refused rather than resolved.
pub fn environment(facts: &[Fact]) -> Result<(String, Value), IdentityError> {
    let mut sorted = facts.to_vec();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(pair) = sorted.windows(2).find(|w| w[0].name == w[1].name) {
        return Err(IdentityError::Facts(format!(
            "fact {} is declared twice",
            pair[0].name
        )));
    }
    let record = object(vec![
        ("format", Value::String(ENVIRONMENT_FORMAT.into())),
        // Said in the record, not left for a reader to infer: this digest
        // covers build facts too, so a build difference is an environment
        // difference until Protocol has a build condition (G1).
        (
            "covers",
            Value::Array(vec![
                Value::String("environment".into()),
                Value::String("build".into()),
            ]),
        ),
        (
            "facts",
            Value::Array(
                sorted
                    .iter()
                    .map(|fact| {
                        object(vec![
                            ("name", Value::String(fact.name.clone())),
                            ("class", Value::String(fact.class.wire().into())),
                            ("value", Value::String(fact.value.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    Ok((cbr_encoding::digest_canonical(&record), record))
}

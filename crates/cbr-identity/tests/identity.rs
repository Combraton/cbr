//! Property tests for source identity (PROTOCOL-PIN section 5), each with a
//! negative control: a change that must alter the identity, and a change that
//! must not. They run against real git repositories in temporary directories.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use cbr_identity::{
    Fact, FactClass, dirty_snapshot, environment, git_basis, read_blob, tree_entries,
};

fn git(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args([
            "-c",
            "user.name=cbr-test",
            "-c",
            "user.email=cbr-test@invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.fileMode=true"])
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path();
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "core.fileMode", "true"]);
    std::fs::write(path.join("a.txt"), "alpha\n").unwrap();
    std::fs::write(path.join("b.txt"), "beta\n").unwrap();
    std::fs::write(path.join(".gitignore"), "ignored.log\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "first"]);
    directory
}

#[test]
fn the_tree_is_the_content_not_the_commit() {
    let directory = repository();
    let path = directory.path();
    let first = git_basis(path, "HEAD").expect("basis");
    assert_eq!(first.tree, git(path, &["rev-parse", "HEAD^{tree}"]));
    assert_eq!(first.commit, git(path, &["rev-parse", "HEAD"]));
    assert_ne!(first.tree, first.commit, "the tree id is not the commit id");

    // A second commit with identical content: a different commit, the same
    // content basis. This is the negative control.
    git(
        path,
        &["commit", "-q", "--allow-empty", "-m", "no content change"],
    );
    let second = git_basis(path, "HEAD").expect("basis");
    assert_ne!(second.commit, first.commit);
    assert_eq!(second.tree, first.tree, "identical content is one tree");

    // A content change must change the tree.
    std::fs::write(path.join("a.txt"), "alpha, changed\n").unwrap();
    git(path, &["commit", "-q", "-am", "content"]);
    assert_ne!(git_basis(path, "HEAD").expect("basis").tree, first.tree);
}

#[test]
fn a_clean_checkout_has_no_snapshot() {
    let directory = repository();
    assert_eq!(dirty_snapshot(directory.path()).expect("snapshot"), None);
    // An ignored file does not make a checkout dirty.
    std::fs::write(directory.path().join("ignored.log"), "noise").unwrap();
    assert_eq!(dirty_snapshot(directory.path()).expect("snapshot"), None);
}

#[test]
fn the_snapshot_changes_on_a_deletion_and_a_mode_change_but_not_on_mtime_alone() {
    let directory = repository();
    let path = directory.path();
    std::fs::write(path.join("a.txt"), "alpha, edited\n").unwrap();
    let edited = dirty_snapshot(path).expect("snapshot").expect("dirty");

    // Negative control: rewriting the same bytes moves the modification time
    // and nothing else. The snapshot must not change.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join("a.txt"))
        .unwrap();
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
        .unwrap();
    drop(file);
    assert_eq!(
        dirty_snapshot(path)
            .expect("snapshot")
            .expect("dirty")
            .digest,
        edited.digest,
        "modification time alone is not identity"
    );
    // The same for a clean tracked file: a new modification time does not
    // make it part of the snapshot.
    let clean = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join("b.txt"))
        .unwrap();
    clean
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(7200))
        .unwrap();
    drop(clean);
    assert_eq!(
        dirty_snapshot(path)
            .expect("snapshot")
            .expect("dirty")
            .digest,
        edited.digest,
        "a clean file's modification time is not identity"
    );
    // Nor does staging the same change: the working tree is what it describes.
    git(path, &["add", "a.txt"]);
    assert_eq!(
        dirty_snapshot(path)
            .expect("snapshot")
            .expect("dirty")
            .digest,
        edited.digest
    );
    // Nor an ignored file.
    std::fs::write(path.join("ignored.log"), "noise").unwrap();
    assert_eq!(
        dirty_snapshot(path)
            .expect("snapshot")
            .expect("dirty")
            .digest,
        edited.digest
    );

    // A mode change must change it.
    let mut permissions = std::fs::metadata(path.join("a.txt")).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path.join("a.txt"), permissions).unwrap();
    let executable = dirty_snapshot(path).expect("snapshot").expect("dirty");
    assert_ne!(
        executable.digest, edited.digest,
        "a mode change is identity"
    );

    // A deletion must change it, and be present as a deletion.
    std::fs::remove_file(path.join("b.txt")).unwrap();
    let deleted = dirty_snapshot(path).expect("snapshot").expect("dirty");
    assert_ne!(deleted.digest, executable.digest, "a deletion is identity");
    let entries = deleted.document.get("entries").unwrap().as_array().unwrap();
    assert!(
        entries.iter().any(|entry| {
            let row = entry.as_array().unwrap();
            row[0].as_str() == Some("b.txt") && row[1].as_str() == Some("deleted")
        }),
        "a deletion is recorded with an explicit status: {entries:?}"
    );

    // An untracked, not ignored, file must change it.
    std::fs::write(path.join("new.txt"), "new\n").unwrap();
    assert_ne!(
        dirty_snapshot(path)
            .expect("snapshot")
            .expect("dirty")
            .digest,
        deleted.digest
    );

    // And the digest is the digest of the document it comes with.
    assert_eq!(
        deleted.digest,
        cbr_encoding::digest_canonical(&deleted.document)
    );
}

fn fact(name: &str, class: FactClass, value: &str) -> Fact {
    Fact {
        name: name.into(),
        class,
        value: value.into(),
    }
}

#[test]
fn the_environment_digest_covers_declared_build_facts_and_says_so() {
    let facts = vec![
        fact("os", FactClass::Environment, "linux"),
        fact("rustc", FactClass::Build, "1.97.1"),
    ];
    let (digest, record) = environment(&facts).expect("environment");
    let covers: Vec<&str> = record
        .get("covers")
        .and_then(|c| c.as_array())
        .unwrap()
        .iter()
        .filter_map(|c| c.as_str())
        .collect();
    assert_eq!(
        covers,
        ["environment", "build"],
        "the record says it covers build"
    );

    // A build fact is identity.
    let rebuilt = vec![
        fact("os", FactClass::Environment, "linux"),
        fact("rustc", FactClass::Build, "1.98.0"),
    ];
    assert_ne!(environment(&rebuilt).unwrap().0, digest);

    // Negative control: the order a file lists facts in is not identity.
    let reordered = vec![facts[1].clone(), facts[0].clone()];
    assert_eq!(environment(&reordered).unwrap().0, digest);

    // A fact declared twice is refused rather than resolved.
    let twice = vec![facts[0].clone(), facts[0].clone()];
    assert!(environment(&twice).is_err());
}

/// Reading a tree's blobs in bulk is what M3's retrieval needs, and what ADR
/// 001 question 11 named as the trigger for this crate moving to `gix`. A
/// tree answers for its own content and no other: an anchor taken at one tree
/// is detectably stale at another.
#[test]
fn a_tree_lists_its_own_blobs_and_each_one_reads_back_exactly() {
    let directory = repository();
    let path = directory.path();
    std::fs::create_dir(path.join("dir")).unwrap();
    std::fs::write(path.join("dir/c.txt"), "gamma\n").unwrap();
    let mut permissions = std::fs::metadata(path.join("b.txt")).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path.join("b.txt"), permissions).unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "second"]);

    let second = git_basis(path, "HEAD").expect("basis");
    let first = git_basis(path, "HEAD~1").expect("basis");

    let entries = tree_entries(path, &second.tree).expect("entries");
    let named = |name: &str| entries.iter().find(|entry| entry.path == name);
    assert!(named(".gitignore").is_some(), "{entries:#?}");
    assert_eq!(named("a.txt").expect("a.txt").mode, "100644");
    assert_eq!(named("b.txt").expect("b.txt").mode, "100755");
    let nested = named("dir/c.txt").expect("a path inside a directory, not the directory");
    assert!(
        !entries.iter().any(|entry| entry.path == "dir"),
        "a tree is not a blob: {entries:#?}"
    );

    assert_eq!(read_blob(path, &nested.blob).expect("blob"), b"gamma\n");
    assert_eq!(
        read_blob(path, &named("a.txt").expect("a.txt").blob).expect("blob"),
        b"alpha\n"
    );

    // The earlier tree does not know about the later file.
    let earlier = tree_entries(path, &first.tree).expect("entries");
    assert!(
        !earlier.iter().any(|entry| entry.path == "dir/c.txt"),
        "a tree answers for its own content only: {earlier:#?}"
    );
    assert_eq!(earlier.len(), 3, "{earlier:#?}");

    // A tree that does not exist is an error, never an empty listing that
    // would read as "this tree has no files".
    assert!(tree_entries(path, &"0".repeat(40)).is_err());
}

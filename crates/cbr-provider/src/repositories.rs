//! Registered repositories, and the view a grant makes of them.
//!
//! **Registration is provider configuration, not a protocol operation.** The
//! protocol has no operation for it, and the one fact registration needs — a
//! checkout path — identifies the operator's own filesystem. A path must
//! therefore never cross the wire, so it is given to the process that will
//! read it, at launch, with `--register-repository <id>=<path>`. What the
//! protocol sees is a `cbr.repository` subject holding the id and the instant
//! it was registered, and nothing else.
//!
//! **A registered repository is not a readable one.** Reading is a grant:
//! a session sees exactly the repositories its grant covers with the
//! `context.read` right, and a repository outside that view is never
//! searched and never named, so a principal cannot tell "no match" from "not
//! allowed" (INTERNALS section 5, CORE section 15.5).

use std::path::{Path, PathBuf};

use crate::grants::Grant;
use crate::store::{REPOSITORY, Store, StoreError, SubjectKey};

/// The right a grant needs over a `cbr.repository` subject before a session
/// may retrieve anything from it.
pub const READ_RIGHT: &str = "context.read";

/// One registration as the launch names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub id: String,
    pub checkout: PathBuf,
}

/// Parse `<id>=<path>`. The path must be absolute, because a relative one
/// means something different to every process that reads it.
pub fn parse(argument: &str) -> Result<Registration, String> {
    let (id, path) = argument
        .split_once('=')
        .ok_or("--register-repository takes <id>=<path>")?;
    if id.is_empty() || id.len() > 128 {
        return Err(format!("{id:?} is not a repository id"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!("{id:?} is not a repository id"));
    }
    let checkout = PathBuf::from(path);
    if !checkout.is_absolute() {
        return Err(format!("the checkout of {id} must be an absolute path"));
    }
    Ok(Registration {
        id: id.into(),
        checkout,
    })
}

/// Register one repository, refusing a path that is not a git checkout this
/// process can read. Refusing at launch is the point: a registration that
/// cannot be indexed would otherwise look like a repository with nothing in
/// it, which is the false absence everything downstream is built to avoid.
pub fn register(
    store: &mut Store,
    registration: &Registration,
    recorded_at: &str,
) -> Result<i64, String> {
    cbr_identity::git_basis(&registration.checkout, "HEAD").map_err(|error| {
        format!(
            "registering {}: {} is not a readable git checkout: {error}",
            registration.id,
            registration.checkout.display()
        )
    })?;
    store
        .register_repository(&registration.id, &registration.checkout, recorded_at)
        .map_err(|error| format!("registering {}: {error}", registration.id))
}

/// One repository a session may read: its id and where it is checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visible {
    pub id: String,
    pub checkout: PathBuf,
}

/// The repositories this session may read, in id order.
///
/// `grant` is `None` for the provider's own authority principal, which reads
/// every registered repository; for anyone else it is the grant the command
/// named, and only what that grant covers is visible.
pub fn view(store: &Store, grant: Option<&Grant>) -> Result<Vec<Visible>, StoreError> {
    let mut visible = Vec::new();
    for (id, _) in store.subjects_of_kind(REPOSITORY)? {
        let permitted = match grant {
            None => true,
            Some(grant) => grant.may_read(
                &SubjectKey {
                    kind: REPOSITORY.to_string(),
                    id: id.clone(),
                },
                READ_RIGHT,
            ),
        };
        if !permitted {
            continue;
        }
        // A registration whose checkout this process cannot resolve is not
        // visible: an unreadable repository must not become an empty one.
        if let Some(checkout) = store.repository_checkout(&id)? {
            visible.push(Visible { id, checkout });
        }
    }
    visible.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(visible)
}

/// Where a repository is checked out, if this session may read it.
pub fn checkout(
    store: &Store,
    grant: Option<&Grant>,
    id: &str,
) -> Result<Option<PathBuf>, StoreError> {
    Ok(view(store, grant)?
        .into_iter()
        .find(|visible| visible.id == id)
        .map(|visible| visible.checkout))
}

/// Whether `path` is inside `root`, for the rule that nothing reads a
/// checkout outside a granted scope: a registered repository's own directory
/// is the whole of what registering it permitted.
pub fn inside(root: &Path, path: &Path) -> bool {
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(path)) => path.starts_with(root),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbr_encoding::Value;

    fn object(fields: Vec<(&str, Value)>) -> Value {
        Value::Object(
            fields
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        )
    }

    fn string(text: &str) -> Value {
        Value::String(text.to_string())
    }

    fn checkout_at(directory: &Path) -> PathBuf {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(["init", "-q", "-b", "main"])
            .output()
            .expect("git runs");
        assert!(output.status.success());
        std::fs::write(directory.join("one.txt"), "one\n").expect("writes");
        for arguments in [
            vec!["add", "."],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "one",
            ],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(directory)
                .args(&arguments)
                .output()
                .expect("git runs");
            assert!(output.status.success(), "{arguments:?}");
        }
        directory.to_path_buf()
    }

    fn grant_over(resources: Vec<Value>, rights: Vec<Value>) -> Grant {
        let payload = object(vec![
            ("holder", string("reader")),
            ("audience", string("cbr")),
            ("rights", Value::Array(rights)),
            ("resources", Value::Array(resources)),
            (
                "delegation",
                object(vec![
                    ("allowed", Value::Bool(false)),
                    ("max_depth", Value::Int(0)),
                ]),
            ),
        ]);
        crate::grants::parse_issue("g", "owner", &payload).expect("a grant")
    }

    #[test]
    fn a_registration_names_an_absolute_path_and_a_usable_id() {
        assert_eq!(
            parse("app=/tmp/app").expect("parses"),
            Registration {
                id: "app".into(),
                checkout: PathBuf::from("/tmp/app"),
            }
        );
        assert!(parse("app").is_err(), "no path");
        assert!(parse("app=relative/path").is_err(), "not absolute");
        assert!(parse("=/tmp/app").is_err(), "no id");
        assert!(parse("a b=/tmp/app").is_err(), "a space is not an id");
    }

    #[test]
    fn a_registration_refuses_a_path_that_is_not_a_readable_checkout() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut store = Store::open(&directory.path().join("data")).expect("store");
        let registration = Registration {
            id: "app".into(),
            checkout: directory.path().join("not-a-repository"),
        };
        let error = register(&mut store, &registration, "2026-09-19T00:00:00Z")
            .expect_err("refuses a path with no git in it");
        assert!(error.contains("not a readable git checkout"), "{error}");
        assert!(
            store
                .subjects_of_kind(REPOSITORY)
                .expect("subjects")
                .is_empty(),
            "a refused registration writes no subject"
        );
    }

    #[test]
    fn a_registered_repository_records_its_id_and_never_its_path() {
        let directory = tempfile::tempdir().expect("temp dir");
        let checkout = directory.path().join("app");
        std::fs::create_dir_all(&checkout).expect("dir");
        let checkout = checkout_at(&checkout);
        let mut store = Store::open(&directory.path().join("data")).expect("store");
        let registration = Registration {
            id: "app".into(),
            checkout: checkout.clone(),
        };
        assert_eq!(
            register(&mut store, &registration, "2026-09-19T00:00:00Z").expect("registers"),
            1
        );
        // Registering the same checkout again is not a change.
        assert_eq!(
            register(&mut store, &registration, "2026-09-19T00:00:01Z").expect("registers"),
            1,
            "an unchanged registration is not a new revision"
        );

        let subjects = store.subjects_of_kind(REPOSITORY).expect("subjects");
        assert_eq!(subjects.len(), 1);
        let (id, value) = &subjects[0];
        assert_eq!(id, "app");
        let text = checkout.to_string_lossy().into_owned();
        assert!(
            !value.contains(&text),
            "a subject must not carry the operator's path: {value}"
        );
        assert!(value.contains("registered_at"), "{value}");
        // The path is still resolvable inside this process, which is the
        // only place it exists.
        assert_eq!(
            store.repository_checkout("app").expect("checkout"),
            Some(checkout)
        );
    }

    #[test]
    fn a_view_holds_exactly_what_the_grant_covers() {
        let directory = tempfile::tempdir().expect("temp dir");
        let open = checkout_at(&{
            let path = directory.path().join("open");
            std::fs::create_dir_all(&path).expect("dir");
            path
        });
        let closed = checkout_at(&{
            let path = directory.path().join("closed");
            std::fs::create_dir_all(&path).expect("dir");
            path
        });
        let mut store = Store::open(&directory.path().join("data")).expect("store");
        for (id, checkout) in [("open", &open), ("closed", &closed)] {
            register(
                &mut store,
                &Registration {
                    id: id.into(),
                    checkout: checkout.clone(),
                },
                "2026-09-19T00:00:00Z",
            )
            .expect("registers");
        }

        // The authority principal holds no grant and sees every registration.
        let all = view(&store, None).expect("view");
        assert_eq!(
            all.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            ["closed", "open"]
        );

        // A grant narrowed to one repository sees one.
        let narrow = grant_over(
            vec![object(vec![
                ("kind", string(REPOSITORY)),
                ("id", string("open")),
            ])],
            vec![string(READ_RIGHT)],
        );
        assert_eq!(
            view(&store, Some(&narrow))
                .expect("view")
                .iter()
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>(),
            ["open"]
        );

        // The right matters as much as the resource: covering the subject
        // without holding the read right is not a view.
        let wrong_right = grant_over(
            vec![object(vec![("kind", string(REPOSITORY))])],
            vec![string("knowledge.read")],
        );
        assert!(
            view(&store, Some(&wrong_right)).expect("view").is_empty(),
            "covering a repository is not reading it"
        );

        // And a grant over another kind entirely sees nothing.
        let elsewhere = grant_over(
            vec![object(vec![("kind", string("knowledge.claim"))])],
            vec![string(READ_RIGHT)],
        );
        assert!(view(&store, Some(&elsewhere)).expect("view").is_empty());
    }

    #[test]
    fn a_path_outside_a_registered_checkout_is_outside_the_grant() {
        let directory = tempfile::tempdir().expect("temp dir");
        let inside_path = directory.path().join("app/src");
        std::fs::create_dir_all(&inside_path).expect("dir");
        let root = directory.path().join("app");
        assert!(inside(&root, &inside_path));
        assert!(!inside(&root, directory.path()));
        assert!(!inside(&root, &directory.path().join("other")));
    }
}

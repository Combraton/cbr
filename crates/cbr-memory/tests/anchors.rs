//! What a code anchor may and may not claim (STACK section 6, INTERNALS §2).
//!
//! Tags-level anchoring finds *where a name is defined*. It does not resolve
//! *which* definition a call refers to: two methods with one name, generics,
//! macros, re-exports and shadowing all collapse to several candidates. The
//! tests below hold CBR to the honest version of that — **every candidate is
//! kept and the ambiguity is marked** — because the failure that matters is a
//! citation that silently points at the wrong `get`.
//!
//! An anchor is also recorded at the tree and blob it was taken from, so a
//! stale anchor is detectable rather than merely wrong.

use cbr_memory::anchors::{self, Language};
use rusqlite::Connection;

fn database() -> Connection {
    let connection = Connection::open_in_memory().expect("opens");
    anchors::migrate(&connection).expect("migrates");
    connection
}

const TWO_GETS: &str = r#"
struct Cache;
struct Store;

impl Cache {
    fn get(&self, key: &str) -> Option<String> { None }
}

impl Store {
    fn get(&self, key: &str) -> Option<String> { None }
}

fn only_here() -> u8 { 7 }

fn caller(cache: &Cache) {
    cache.get("k");
    only_here();
}
"#;

#[test]
fn a_definition_is_anchored_to_its_exact_span() {
    let found = anchors::anchors(Language::Rust, TWO_GETS.as_bytes()).expect("parses");
    let unique: Vec<_> = found
        .iter()
        .filter(|anchor| anchor.name == "only_here" && anchor.is_definition)
        .collect();
    assert_eq!(unique.len(), 1, "{found:#?}");
    let anchor = unique[0];
    let span = &TWO_GETS[anchor.start_byte as usize..anchor.end_byte as usize];
    assert!(span.contains("only_here"), "{span:?}");
    assert!(anchor.kind.contains("function"), "{anchor:?}");
    assert!(
        anchor.start_line >= 1 && anchor.end_line >= anchor.start_line,
        "{anchor:?}"
    );
}

/// The property that matters: a name defined twice resolves to both, marked
/// ambiguous. Picking one silently is the failure this exists to prevent.
#[test]
fn a_name_defined_twice_resolves_to_both_and_is_marked_ambiguous() {
    let connection = database();
    let found = anchors::anchors(Language::Rust, TWO_GETS.as_bytes()).expect("parses");
    anchors::record(&connection, "t1", "src/lib.rs", "sha256:blob", &found).expect("records");

    let resolved = anchors::resolve(&connection, "t1", "get").expect("resolves");
    assert_eq!(resolved.candidates.len(), 2, "{resolved:#?}");
    assert!(
        resolved.ambiguous,
        "two definitions of one name are ambiguous"
    );

    let single = anchors::resolve(&connection, "t1", "only_here").expect("resolves");
    assert_eq!(single.candidates.len(), 1);
    assert!(!single.ambiguous);

    let absent = anchors::resolve(&connection, "t1", "never_defined").expect("resolves");
    assert!(absent.candidates.is_empty());
    assert!(!absent.ambiguous, "nothing found is not ambiguity");

    // The source calls both names. A call is a reference, not a definition,
    // and resolving a name must not offer call sites as places it is defined.
    assert!(
        found
            .iter()
            .any(|anchor| anchor.name == "get" && !anchor.is_definition),
        "the calls are tagged as references: {found:#?}"
    );
    assert!(
        resolved
            .candidates
            .iter()
            .all(|candidate| candidate.kind.contains("method")
                || candidate.kind.contains("function")),
        "{resolved:#?}"
    );
}

/// An anchor belongs to the tree it was taken at. Asking at another tree is
/// answered with nothing, not with the old tree's spans.
#[test]
fn an_anchor_never_answers_for_a_tree_it_was_not_taken_at() {
    let connection = database();
    let found = anchors::anchors(Language::Rust, TWO_GETS.as_bytes()).expect("parses");
    anchors::record(&connection, "t1", "src/lib.rs", "sha256:blob-1", &found).expect("records");

    assert_eq!(
        anchors::resolve(&connection, "t2", "only_here")
            .expect("resolves")
            .candidates
            .len(),
        0,
        "a different tree has no anchors until it is indexed"
    );
    let candidate = &anchors::resolve(&connection, "t1", "only_here")
        .expect("resolves")
        .candidates[0];
    assert_eq!(candidate.blob, "sha256:blob-1");
    assert_eq!(candidate.path, "src/lib.rs");

    anchors::remove_tree(&connection, "t1").expect("removes");
    assert!(
        anchors::resolve(&connection, "t1", "only_here")
            .expect("resolves")
            .candidates
            .is_empty()
    );
}

#[test]
fn every_claimed_language_yields_its_definitions() {
    let cases = [
        (Language::Rust, "fn alpha() {}\n", "alpha"),
        (Language::Python, "def beta():\n    return 1\n", "beta"),
        (
            Language::JavaScript,
            "function gamma() { return 1; }\n",
            "gamma",
        ),
        (
            Language::TypeScript,
            "export function delta(x: number): number { return x; }\n",
            "delta",
        ),
        (
            Language::Tsx,
            "export function Epsilon(): JSX.Element { return <div/>; }\n",
            "Epsilon",
        ),
    ];
    for (language, source, name) in cases {
        let found = anchors::anchors(language, source.as_bytes()).expect("parses");
        assert!(
            found
                .iter()
                .any(|anchor| anchor.name == name && anchor.is_definition),
            "{language:?} did not define {name}: {found:#?}"
        );
    }
}

#[test]
fn only_the_claimed_languages_are_anchored() {
    assert_eq!(
        anchors::language_for_path("src/main.rs"),
        Some(Language::Rust)
    );
    assert_eq!(anchors::language_for_path("a/b.py"), Some(Language::Python));
    assert_eq!(
        anchors::language_for_path("a/b.mjs"),
        Some(Language::JavaScript)
    );
    assert_eq!(
        anchors::language_for_path("a/b.cjs"),
        Some(Language::JavaScript)
    );
    assert_eq!(
        anchors::language_for_path("a/b.js"),
        Some(Language::JavaScript)
    );
    assert_eq!(
        anchors::language_for_path("a/b.ts"),
        Some(Language::TypeScript)
    );
    assert_eq!(anchors::language_for_path("a/b.tsx"), Some(Language::Tsx));
    // Claiming a language CBR has not pinned would anchor it wrongly and
    // silently, so these have no anchors at all.
    assert_eq!(anchors::language_for_path("README.md"), None);
    assert_eq!(anchors::language_for_path("a/b.go"), None);
    assert_eq!(anchors::language_for_path("Makefile"), None);
}

/// Source that does not parse is normal in a working tree. Anchoring yields
/// what it can rather than failing, and says that it was incomplete.
#[test]
fn source_that_does_not_parse_yields_what_it_can() {
    let broken = "fn good() {}\nfn broken( {\n";
    let found = anchors::anchors(Language::Rust, broken.as_bytes()).expect("does not fail");
    assert!(
        found.iter().any(|anchor| anchor.name == "good"),
        "{found:#?}"
    );
}

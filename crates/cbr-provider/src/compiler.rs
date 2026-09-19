//! The deterministic packet compiler (INTERNALS section 5).
//!
//! It is the production answer to the question m3a left open: where does a
//! packet's content come from when there is no test script? Here, from the
//! journal and from registered repositories, by rules with no model in them.
//! `context.script` remains a test control; a request that has no script is
//! compiled instead of waiting for its deadline.
//!
//! **Its provenance is its own.** A packet compiled here names
//! [`COMPILER`], never [`crate::context::SCRIPT_COMPILER`], so a scripted
//! packet and a compiled one can never be mistaken for one another, in a
//! record or in a review.
//!
//! **What "deterministic" claims, and what it does not.** No model is
//! consulted, and nothing consults the wall clock except the capture instant
//! the provider already stamps. Given the same request record, the same
//! repository trees and the same claim revisions, the steps below are the
//! same steps, so the packet's bytes are the same bytes — which is the
//! property TALK section 3 asks for and
//! `a_sealed_packet_rebuilds_to_the_same_digest` measures. It does not claim
//! that what was selected is what the task needed; INTERNALS section 4 is
//! explicit that a build being structurally consistent is not a claim about
//! usefulness, and J1's oracle is what speaks to that.
//!
//! The six steps of INTERNALS section 5 map onto this module and its caller:
//! the caller resolves the view and the basis (1) and chooses the frontier
//! (2) by building or reusing an index; retrieval runs inside the view (3);
//! applicability is the claim and condition machinery Knowledge already owns
//! (4); the budget is allocated by `context::inclusion`, mandatory first (5);
//! and the packet is sealed by `context::compile_packet` (6). What lives
//! here is the shaping in between: which selection becomes which section,
//! what each section cites, which items go unmet and why, and what coverage
//! the answer has to admit to.

use cbr_encoding::Value;

use crate::context::{object, string};

/// The provenance a compiled packet carries. Distinct from the scripted
/// compiler by construction, and version-bearing because a change to the
/// rules below is a change to what a packet means.
pub const COMPILER: &str = "cbr-context-compiler/1";

/// The media type a cited source file is sealed under. It is the file's own
/// bytes, so it is a captured observation of the repository, not a
/// derivation: the citation resolves to exactly what is in the tree.
pub const SOURCE_MEDIA_TYPE: &str = "application/octet-stream";

/// The evidence `source.kind` a cited source file is sealed under.
pub const SOURCE_KIND: &str = "repository_blob";

/// The artifact id a cited blob takes. Content-addressed, so citing the same
/// blob twice — from two items, two requests or two repositories — is one
/// artifact, and a citation is stable across recompilations.
pub fn artifact_id(blob: &str) -> String {
    format!("src.{blob}")
}

/// The descriptor a cited source file is sealed with. The anchors are what
/// make a citation checkable: the tree and the path say where in the
/// repository these bytes are, so a reader can fetch the artifact and
/// compare it with the blob at that path in that tree.
pub fn source_descriptor(
    repository: &str,
    tree: &str,
    path: &str,
    digest: &str,
    size: usize,
    captured_at: &str,
) -> Value {
    object(vec![
        ("digest", string(digest)),
        ("size", Value::Int(size as i64)),
        ("media_type", string(SOURCE_MEDIA_TYPE)),
        ("producer", object(vec![("producer_id", string(COMPILER))])),
        (
            "source",
            object(vec![("kind", string(SOURCE_KIND)), ("id", string(path))]),
        ),
        ("scope", string(repository)),
        (
            "capture",
            object(vec![
                ("captured_at", string(captured_at)),
                (
                    "anchors",
                    Value::Array(vec![
                        object(vec![("kind", string("git_tree")), ("id", string(tree))]),
                        object(vec![
                            ("kind", string("repository")),
                            ("id", string(repository)),
                        ]),
                        object(vec![("kind", string("path")), ("id", string(path))]),
                    ]),
                ),
            ]),
        ),
        (
            "coverage",
            object(vec![("completeness", string("complete"))]),
        ),
        ("retention_class", string("context-source")),
    ])
}

/// One span the compiler selected for one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub item: String,
    pub repository: String,
    pub tree: String,
    pub path: String,
    pub blob: String,
    pub digest: String,
    pub size: usize,
    pub start_byte: i64,
    pub end_byte: i64,
    pub start_line: u32,
    pub end_line: u32,
    /// `index` or `canonical`: which projection answered.
    pub origin: &'static str,
}

impl Selection {
    pub fn artifact(&self) -> String {
        artifact_id(&self.blob)
    }

    /// The line a section carries as its content: what was selected, where it
    /// is and how it was found, in one line a reader can act on. The bytes
    /// themselves are the citation, not the content, because a packet is a
    /// bounded thing and a file is not.
    pub fn content(&self) -> String {
        format!(
            "{}:{} lines {}-{} at tree {} (found in the {} projection)",
            self.repository, self.path, self.start_line, self.end_line, self.tree, self.origin
        )
    }
}

/// What one repository in the view contributed, as the packet's coverage
/// records it. `gaps` is the honest half: a lagging projection, a binary
/// file nobody indexed, a language with no anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    pub repository: String,
    pub frontier: String,
    pub state: String,
    pub gaps: Vec<String>,
}

impl Reach {
    fn to_coverage(&self) -> Value {
        let mut gaps: Vec<Value> = Vec::new();
        if self.state != "complete" {
            gaps.push(string(&format!("projection {}", self.state)));
        }
        gaps.extend(self.gaps.iter().map(|gap| string(gap)));
        object(vec![
            (
                "producer",
                string(&format!("{COMPILER} over {}", self.repository)),
            ),
            ("frontier", string(&self.frontier)),
            ("gaps", Value::Array(gaps)),
        ])
    }
}

/// One item the compiler could not satisfy, with the reason it will report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmet {
    pub item: String,
    pub reason: String,
}

/// A claim the compiler resolved for a `claim_included` item.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimSection {
    pub item: String,
    pub reference: Value,
    /// The claim record as read, or the error that reading it gave.
    pub read: Result<Value, &'static str>,
}

impl ClaimSection {
    /// A claim is `binding` only when an authority decided it so. A claim
    /// that was merely proposed is a hypothesis, and saying otherwise would
    /// be the compiler deciding something it has only read.
    fn label(&self) -> &'static str {
        let binding = self.read.as_ref().is_ok_and(|record| {
            record
                .get("reliance")
                .and_then(|reliance| reliance.get("permitted_use"))
                .and_then(Value::as_str)
                == Some("binding")
        });
        if binding { "binding" } else { "hypothesis" }
    }
}

/// An evidence artifact an `evidence_included` item named and the compiler
/// confirmed.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceSection {
    pub item: String,
    pub evidence: Value,
    pub summary: String,
}

/// Authority content the request supplied for an item.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoritySection {
    pub item: String,
    pub revision: i64,
    pub evidence: Value,
}

/// Everything the compiler decided, before it becomes a script.
#[derive(Debug, Clone, Default)]
pub struct Decided {
    pub selections: Vec<Selection>,
    pub claims: Vec<ClaimSection>,
    pub evidence: Vec<EvidenceSection>,
    pub authority: Vec<AuthoritySection>,
    pub reach: Vec<Reach>,
    pub unmet: Vec<Unmet>,
}

/// Turn what the compiler decided into the steps the job runs.
///
/// The order is fixed and does not depend on iteration order anywhere: the
/// sections come in item order, then coverage in repository order, then the
/// unmet reasons in item order, then `publish`. Two compilations of the same
/// request therefore produce the same script, byte for byte, which is what
/// makes the sealed packet reproducible.
pub fn steps(decided: &Decided) -> Vec<Value> {
    let mut steps: Vec<Value> = Vec::new();

    let mut sections: Vec<(String, Value)> = Vec::new();
    for selection in &decided.selections {
        sections.push((
            format!("{}:source", selection.item),
            object(vec![
                ("section_id", string(&format!("s-{}", selection.item))),
                ("item_id", string(&selection.item)),
                ("label", string("source_inspected")),
                ("content", string(&selection.content())),
                (
                    "source",
                    object(vec![
                        ("repository", string(&selection.repository)),
                        ("path", string(&selection.path)),
                    ]),
                ),
                (
                    "citations",
                    Value::Array(vec![object(vec![
                        ("citation_id", string(&format!("c-{}", selection.item))),
                        (
                            "evidence",
                            object(vec![
                                ("provider", string("cbr")),
                                (
                                    "artifact",
                                    object(vec![
                                        ("kind", string(crate::evidence::ARTIFACT)),
                                        ("id", string(&selection.artifact())),
                                    ]),
                                ),
                                ("digest", string(&selection.digest)),
                            ]),
                        ),
                    ])]),
                ),
                (
                    "span",
                    object(vec![
                        ("start_byte", Value::Int(selection.start_byte)),
                        ("end_byte", Value::Int(selection.end_byte)),
                        ("start_line", Value::Int(i64::from(selection.start_line))),
                        ("end_line", Value::Int(i64::from(selection.end_line))),
                    ]),
                ),
            ]),
        ));
    }
    for claim in &decided.claims {
        sections.push((
            format!("{}:claim", claim.item),
            object(vec![
                ("section_id", string(&format!("s-{}", claim.item))),
                ("item_id", string(&claim.item)),
                ("label", string(claim.label())),
                (
                    "content",
                    string(&format!("claim {}", reference_id(&claim.reference))),
                ),
                ("citations", Value::Array(Vec::new())),
                ("claim", claim.reference.clone()),
            ]),
        ));
    }
    for evidence in &decided.evidence {
        sections.push((
            format!("{}:evidence", evidence.item),
            object(vec![
                ("section_id", string(&format!("s-{}", evidence.item))),
                ("item_id", string(&evidence.item)),
                ("label", string("observation")),
                ("content", string(&evidence.summary)),
                (
                    "citations",
                    Value::Array(vec![object(vec![
                        ("citation_id", string(&format!("c-{}", evidence.item))),
                        ("evidence", evidence.evidence.clone()),
                    ])]),
                ),
            ]),
        ));
    }
    for authority in &decided.authority {
        sections.push((
            format!("{}:authority", authority.item),
            object(vec![
                ("section_id", string(&format!("s-{}", authority.item))),
                ("item_id", string(&authority.item)),
                ("label", string("declared_requirement")),
                (
                    "content",
                    string(&format!(
                        "authority content at revision {}",
                        authority.revision
                    )),
                ),
                ("authority_revision", Value::Int(authority.revision)),
                (
                    "citations",
                    Value::Array(vec![object(vec![
                        ("citation_id", string(&format!("c-{}", authority.item))),
                        ("evidence", authority.evidence.clone()),
                    ])]),
                ),
            ]),
        ));
    }
    sections.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, section) in sections {
        steps.push(object(vec![("section", section)]));
    }

    let mut reach: Vec<&Reach> = decided.reach.iter().collect();
    reach.sort_by(|left, right| left.repository.cmp(&right.repository));
    for one in reach {
        steps.push(object(vec![("coverage", one.to_coverage())]));
    }

    let mut unmet: Vec<&Unmet> = decided.unmet.iter().collect();
    unmet.sort_by(|left, right| left.item.cmp(&right.item));
    for one in unmet {
        steps.push(object(vec![(
            "unmet",
            object(vec![
                ("item_id", string(&one.item)),
                ("reason", string(&one.reason)),
            ]),
        )]));
    }

    steps.push(object(vec![("publish", Value::Object(Vec::new()))]));
    steps
}

fn reference_id(reference: &Value) -> String {
    reference
        .get("claim")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(item: &str, path: &str) -> Selection {
        Selection {
            item: item.into(),
            repository: "app".into(),
            tree: "t".repeat(40),
            path: path.into(),
            blob: "b".repeat(40),
            digest: format!("sha256:{}", "c".repeat(64)),
            size: 12,
            start_byte: 0,
            end_byte: 12,
            start_line: 1,
            end_line: 2,
            origin: "index",
        }
    }

    #[test]
    fn the_script_is_ordered_by_item_whatever_order_the_compiler_decided_in() {
        let forwards = Decided {
            selections: vec![selection("b", "two.rs"), selection("a", "one.rs")],
            reach: vec![
                Reach {
                    repository: "z".into(),
                    frontier: "t".into(),
                    state: "complete".into(),
                    gaps: vec![],
                },
                Reach {
                    repository: "a".into(),
                    frontier: "t".into(),
                    state: "lagging".into(),
                    gaps: vec![],
                },
            ],
            unmet: vec![
                Unmet {
                    item: "d".into(),
                    reason: "no_source".into(),
                },
                Unmet {
                    item: "c".into(),
                    reason: "no_source".into(),
                },
            ],
            ..Decided::default()
        };
        let backwards = Decided {
            selections: forwards.selections.iter().rev().cloned().collect(),
            reach: forwards.reach.iter().rev().cloned().collect(),
            unmet: forwards.unmet.iter().rev().cloned().collect(),
            ..Decided::default()
        };
        assert_eq!(
            steps(&forwards),
            steps(&backwards),
            "the script does not depend on the order things were found in"
        );
        let names: Vec<String> = steps(&forwards)
            .iter()
            .map(|step| crate::context::step(step).0.to_string())
            .collect();
        assert_eq!(
            names,
            [
                "section", "section", "coverage", "coverage", "unmet", "unmet", "publish"
            ]
        );
    }

    #[test]
    fn a_lagging_projection_puts_its_state_in_the_coverage_gaps() {
        let reach = Reach {
            repository: "app".into(),
            frontier: "t".repeat(40),
            state: "lagging".into(),
            gaps: vec!["2 blobs were not text".into()],
        };
        let coverage = reach.to_coverage();
        let gaps: Vec<String> = coverage
            .get("gaps")
            .and_then(Value::as_array)
            .expect("gaps")
            .iter()
            .filter_map(|gap| gap.as_str().map(str::to_string))
            .collect();
        assert_eq!(gaps, ["projection lagging", "2 blobs were not text"]);
        assert_eq!(
            coverage.get("frontier").and_then(Value::as_str),
            Some(reach.frontier.as_str())
        );
    }

    #[test]
    fn a_complete_projection_admits_only_the_gaps_it_really_has() {
        let reach = Reach {
            repository: "app".into(),
            frontier: "t".repeat(40),
            state: "complete".into(),
            gaps: vec![],
        };
        assert_eq!(
            reach.to_coverage().get("gaps"),
            Some(&Value::Array(Vec::new()))
        );
    }

    #[test]
    fn a_citation_names_the_blob_it_came_from_and_the_tree_it_is_in() {
        let selection = selection("a", "src/one.rs");
        let descriptor = source_descriptor(
            &selection.repository,
            &selection.tree,
            &selection.path,
            &selection.digest,
            selection.size,
            "2026-09-19T00:00:00Z",
        );
        let anchors: Vec<(String, String)> = descriptor
            .get("capture")
            .and_then(|capture| capture.get("anchors"))
            .and_then(Value::as_array)
            .expect("anchors")
            .iter()
            .map(|anchor| {
                (
                    anchor
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    anchor
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                )
            })
            .collect();
        assert_eq!(
            anchors,
            [
                ("git_tree".to_string(), selection.tree.clone()),
                ("repository".to_string(), "app".to_string()),
                ("path".to_string(), "src/one.rs".to_string()),
            ]
        );
        assert_eq!(selection.artifact(), format!("src.{}", selection.blob));
        assert_ne!(COMPILER, crate::context::SCRIPT_COMPILER);
    }
}

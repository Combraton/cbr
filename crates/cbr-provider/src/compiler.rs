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

use crate::context::{object, set, string};

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

/// How much of a cited span a section carries as its content.
///
/// A section that carried only a locator would make the output capacity
/// measure nothing a consumer reads: the budget would be spent on pointers
/// and the packet would look small while the work of reading it was entirely
/// ahead of the reader. So a section carries a bounded excerpt of the span
/// it cites, and that excerpt is what the capacity counts.
pub const EXCERPT_BYTES: usize = 2048;

/// A bounded window of an artifact, and where in the artifact it starts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Excerpt {
    pub text: String,
    pub start_byte: i64,
    pub end_byte: i64,
}

/// The excerpt a section carries for a span: a **contiguous byte range of
/// the cited artifact**, at most [`EXCERPT_BYTES`] long, centred on the
/// first query term that occurs inside the span.
///
/// It was the span's first bytes, which is wrong whenever a chunk is larger
/// than the cap: the decision record's twenty table rows run to 14 KB, so a
/// 2 KB prefix stopped several rows before the row that answered the
/// question. A reader saw a citation that resolved and an excerpt that did
/// not show the answer.
///
/// Nothing is elided from the middle and nothing is reflowed, so a reader
/// checks it by fetching the citation and taking `start_byte..end_byte` —
/// which the locator states.
pub fn excerpt(bytes: &[u8], start_byte: i64, end_byte: i64, terms: &[String]) -> Excerpt {
    let start = usize::try_from(start_byte).unwrap_or(0).min(bytes.len());
    let end = usize::try_from(end_byte)
        .unwrap_or(0)
        .clamp(start, bytes.len());
    let span = &bytes[start..end];
    if span.len() <= EXCERPT_BYTES {
        return Excerpt {
            text: String::from_utf8_lossy(span).into_owned(),
            start_byte: start as i64,
            end_byte: end as i64,
        };
    }

    // A chunk is bounded in bytes as well as lines, so this path is the
    // rare one: a span only exceeds the cap when a single line does. Then
    // the window is centred on the first term that occurs in it.
    //
    // Three cleverer rules were tried first and all three were fitted to one
    // example. The defect they were chasing was upstream: a twenty-line
    // chunk of a Markdown table was 8.7 KB, so no window over it could be
    // both bounded and representative. `lexical::CHUNK_BYTES` fixed that,
    // and this is what is left.
    let haystack = String::from_utf8_lossy(span).to_lowercase();
    let found = terms
        .iter()
        .filter_map(|term| haystack.find(term.as_str()))
        .min()
        .unwrap_or(0);

    let mut from = found.saturating_sub(EXCERPT_BYTES / 2);
    if from + EXCERPT_BYTES > span.len() {
        from = span.len() - EXCERPT_BYTES;
    }
    let mut to = from + EXCERPT_BYTES;
    // Snap both ends back to character boundaries: a continuation byte is
    // 0b10xxxxxx.
    while from > 0 && (span[from] & 0b1100_0000) == 0b1000_0000 {
        from -= 1;
    }
    while to > from && to < span.len() && (span[to] & 0b1100_0000) == 0b1000_0000 {
        to -= 1;
    }
    Excerpt {
        text: String::from_utf8_lossy(&span[from..to]).into_owned(),
        start_byte: (start + from) as i64,
        end_byte: (start + to) as i64,
    }
}

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
    /// `index`, `canonical`, or `first-chunk` when no term was found in the
    /// file at all and its opening chunk was cited instead.
    pub origin: &'static str,
    /// A bounded window of the cited span, carried as the section's content.
    pub excerpt: Excerpt,
}

impl Selection {
    pub fn artifact(&self) -> String {
        artifact_id(&self.blob)
    }

    /// A locator line, then the excerpt it locates. The locator says where
    /// the bytes are and how they were found; the excerpt is the bytes, so
    /// the output capacity measures what a consumer actually reads.
    pub fn content(&self) -> String {
        format!("{}\n{}", self.locator(), self.excerpt.text)
    }

    pub fn locator(&self) -> String {
        let how = match self.origin {
            "first-chunk" => "no term matched in this file; its opening chunk".to_string(),
            projection => format!("found in the {projection} projection"),
        };
        format!(
            "{}:{} lines {}-{} at tree {} ({how}); excerpt is bytes {}-{} of the artifact",
            self.repository,
            self.path,
            self.start_line,
            self.end_line,
            self.tree,
            self.excerpt.start_byte,
            self.excerpt.end_byte
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

/// How many discovered spans one packet may carry. A discovery section is
/// advisory, so capacity drops the surplus anyway; this bounds the work of
/// producing them.
pub const DISCOVERED_SPANS: usize = 8;

/// How many discovered spans may come from one file.
///
/// Without this, one file takes the whole budget: ranking by score alone
/// gave three of six spans to a single source file and two to the test that
/// quoted the question, and the decision record that answered it never
/// appeared. Six spans from two files have covered less ground than six
/// spans from six.
pub const DISCOVERED_PER_PATH: usize = 2;

/// Where a discovered section sits when capacity runs out.
///
/// INTERNALS section 5 step 5 orders a packet: mandatory constraints and
/// material unknowns first, then task evidence, then optional context. Item
/// sections are the mandatory part and `context::inclusion` reserves for
/// them; this orders everything after them, so that under pressure a packet
/// loses what it can most afford to.
///
/// Sorting by section id, which is what this did, drops by **path name**:
/// an anchor for `binary` outlived a binding decision because `a` sorts
/// before `c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// A decision an authority made binding. Losing this loses the answer.
    BindingClaim,
    /// Another claim that is current at this basis.
    CurrentClaim,
    /// A span the question found, in the order retrieval ranked it.
    Span,
    /// Where a name is defined and used: useful, and reconstructible from
    /// the spans by a reader who has them.
    Anchor,
    /// Rejected alternatives and claims not applicable here. They belong in
    /// the packet (INTERNALS section 5 step 3) and they are the first thing
    /// to go when it will not fit.
    Historical,
}

/// A section no item asked for.
///
/// CONTEXT section 6 lets a packet carry content beyond what its items name,
/// and INTERNALS section 5 step 3 says what that content is for: current
/// decisions, applicable observations, rejected alternatives and unresolved
/// hypotheses, each distinguishable. A request that names its answer by path
/// is not asking a question; these are what makes the answer findable when
/// it does not.
///
/// All of them are advisory: they carry no `item_id`, so
/// `context::inclusion` reserves nothing for them and drops them with reason
/// `output_capacity` before it drops anything an item required.
#[derive(Debug, Clone, PartialEq)]
pub struct Discovered {
    /// Names the section. Ties within a rank are broken by it, so a packet
    /// is still reproducible.
    pub id: String,
    pub rank: Rank,
    /// Position within the rank: retrieval's own order for a span.
    pub order: usize,
    pub label: &'static str,
    /// A rejected alternative or a claim not applicable here: present, and
    /// never current (CONTEXT section 14: a historical section is never
    /// current for an item, and a read reports it as invalidated).
    pub historical: bool,
    pub content: String,
    pub citation: Option<Value>,
    /// The claim this section carries, as a `claim_included` item's section
    /// carries it, so `context::claim_snapshot` and the read-time facts of
    /// CONTEXT section 14 see a discovered claim exactly as they see an
    /// asked-for one.
    pub claim: Option<Value>,
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
    pub discovered: Vec<Discovered>,
    /// What the compiler looked at and left out, with a typed reason, so a
    /// caller can count what it did not get.
    pub omitted: Vec<Value>,
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
    // Item-bound sections first, discovered ones after: `context::inclusion`
    // walks sections in order, and what an item required must be reserved
    // before anything nobody asked for is considered.
    for (key, section) in &mut sections {
        *key = format!("1:{key}");
        let _ = &section;
    }
    let mut ordered: Vec<&Discovered> = decided.discovered.iter().collect();
    ordered.sort_by(|left, right| {
        (left.rank, left.order, &left.id).cmp(&(right.rank, right.order, &right.id))
    });
    for (position, found) in ordered.into_iter().enumerate() {
        let mut section = object(vec![
            ("section_id", string(&format!("d-{}", found.id))),
            ("label", string(found.label)),
            ("content", string(&found.content)),
            ("historical", Value::Bool(found.historical)),
            (
                "citations",
                Value::Array(match &found.citation {
                    Some(evidence) => vec![object(vec![
                        ("citation_id", string(&format!("dc-{}", found.id))),
                        ("evidence", evidence.clone()),
                    ])],
                    None => Vec::new(),
                }),
            ),
        ]);
        if let Some(claim) = &found.claim {
            set(&mut section, "claim", claim.clone());
        }
        if found.historical {
            set_label(&mut section, "stale");
        }
        // `inclusion` walks sections in order and keeps them while capacity
        // lasts, so the position here *is* the drop order.
        sections.push((format!("2:{position:04}"), section));
    }
    for omitted in &decided.omitted {
        steps.push(object(vec![("omit", omitted.clone())]));
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

fn set_label(section: &mut Value, label: &str) {
    if let Value::Object(members) = section {
        for (name, value) in members.iter_mut() {
            if name == "label" {
                *value = string(label);
            }
        }
    }
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
            excerpt: Excerpt {
                text: "fn one() {}\n".into(),
                start_byte: 0,
                end_byte: 12,
            },
        }
    }

    #[test]
    fn an_excerpt_is_a_byte_range_of_the_artifact_centred_on_what_matched() {
        let terms = vec!["answer".to_string()];
        let bytes = b"line one\nline two\nline three\n";
        // A span that fits is carried whole, and says which bytes it is.
        let whole = excerpt(bytes, 9, 18, &terms);
        assert_eq!(whole.text, "line two\n");
        assert_eq!((whole.start_byte, whole.end_byte), (9, 18));
        // Out of range is clamped rather than panicking: a span comes from
        // an index that may have been built from an older blob.
        assert_eq!(
            excerpt(bytes, 9, 9_000, &terms).text,
            "line two\nline three\n"
        );
        assert_eq!(excerpt(bytes, 9_000, 9_001, &terms).text, "");

        // The failure this fixes: a span far larger than the cap, whose
        // answer is past the first `EXCERPT_BYTES`. The old rule cut the
        // prefix and missed it.
        let mut long = vec![b'x'; EXCERPT_BYTES * 3];
        long.splice(
            EXCERPT_BYTES * 2..EXCERPT_BYTES * 2 + 6,
            b"answer".iter().copied(),
        );
        let window = excerpt(&long, 0, long.len() as i64, &terms);
        assert!(
            window.text.contains("answer"),
            "the window is centred on the match, not on the start"
        );
        assert!(window.text.len() <= EXCERPT_BYTES);
        // And it is exactly those bytes of the artifact.
        let from = usize::try_from(window.start_byte).expect("in range");
        let to = usize::try_from(window.end_byte).expect("in range");
        assert_eq!(&long[from..to], window.text.as_bytes());

        // No term in the span: the opening of it, as before.
        let quiet = excerpt(&long, 0, long.len() as i64, &["absent".to_string()]);
        assert_eq!(quiet.start_byte, 0);
        // A window never lands inside a character.
        let wide: Vec<u8> = "\u{4e00}".repeat(EXCERPT_BYTES).into_bytes();
        let cut = excerpt(&wide, 0, wide.len() as i64, &terms);
        assert!(cut.text.len() <= EXCERPT_BYTES);
        let from = usize::try_from(cut.start_byte).expect("in range");
        let to = usize::try_from(cut.end_byte).expect("in range");
        assert_eq!(&wide[from..to], cut.text.as_bytes());
    }

    #[test]
    fn a_section_carries_its_locator_and_then_the_bytes() {
        let selection = selection("a", "src/one.rs");
        let content = selection.content();
        assert!(content.starts_with(&selection.locator()), "{content}");
        assert!(content.ends_with(&selection.excerpt.text), "{content}");
        assert!(
            selection
                .locator()
                .contains("excerpt is bytes 0-12 of the artifact"),
            "the locator names the byte range the excerpt is: {}",
            selection.locator()
        );
        let unmatched = Selection {
            origin: "first-chunk",
            ..selection
        };
        assert!(
            unmatched.locator().contains("no term matched"),
            "a first chunk says it is one: {}",
            unmatched.locator()
        );
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
        // And ascending, not merely stable: a script that reversed every
        // list would be just as independent of the order things were found
        // in, and would still be a different packet.
        let field = |name: &str, field: &str| -> Vec<String> {
            steps(&forwards)
                .iter()
                .filter(|step| crate::context::step(step).0 == name)
                .filter_map(|step| {
                    crate::context::step(step)
                        .1
                        .get(field)
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        };
        assert_eq!(field("section", "item_id"), ["a", "b"]);
        assert_eq!(field("unmet", "item_id"), ["c", "d"]);
        let producers = field("coverage", "producer");
        assert!(
            producers[0].ends_with(" a") && producers[1].ends_with(" z"),
            "coverage is in repository order: {producers:?}"
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

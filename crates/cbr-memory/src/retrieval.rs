//! The rules that govern a retrieval answer (INTERNALS sections 2 and 5).
//!
//! [`index`](crate::index) can build an index and say what it did not reach.
//! That is a mechanism. This module is the set of rules a *caller* is entitled
//! to, and every one of them exists because a retrieval answer is easy to
//! misread:
//!
//! - **A build manifest** (INTERNALS section 2) names the source frontier an
//!   index was built to and the compiler that built it, and distinguishes a
//!   `complete` projection from a `lagging` or `unavailable` one. Without it
//!   an index that stopped a thousand commits ago answers exactly like a
//!   current one.
//! - **Nothing found is never the answer** when the projection is not
//!   complete (INTERNALS section 5: *empty search results are not proof of
//!   absence*). A lagging or unavailable index either falls back to the
//!   canonical records — the repository's own objects, bounded — or says its
//!   coverage is incomplete and why. It never returns silence.
//! - **A search answers inside a view.** A repository the principal may not
//!   read is not searched and is not named, so "no match" and "not allowed"
//!   are the same answer, because the answer describes the caller's view and
//!   nothing else.
//! - **Every answer is bounded** in three separate ways: rows per page with a
//!   cursor that walks the rest, bytes per span, and bytes per batch. One
//!   `LIMIT` bounds a row count, which is not a response size.
//!
//! A manifest is derived, like everything else here: it records what was
//! built, and is replaced when the index is rebuilt.
//!
//! **A lagging projection is not consulted at all.** Its rows describe a tree
//! the caller did not ask about, so a hit in it would cite the wrong bytes.
//! The fallback reads the tree the caller *did* ask about, which is slower
//! and correct.

use std::collections::BTreeSet;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::index::{self, CHUNK_LINES, Coverage, IndexError, MAX_BLOB_BYTES};
use crate::lexical;

/// The compiler identity this build writes into every manifest it records.
/// A change to how an index is built is a change to this string, because a
/// manifest that claims a build it did not get is worse than none.
pub const COMPILER: &str = "cbr-index/1";

/// How much source the canonical fallback will read before it stops and says
/// so. The fallback exists to keep an incomplete projection honest, not to
/// become an unbounded grep.
pub const CANONICAL_READ_BYTES: usize = 8 << 20;

/// How current a projection is (INTERNALS section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Built to exactly the basis the caller asked about.
    Complete,
    /// Built, but to a different frontier than the caller asked about.
    Lagging,
    /// Never built, or not in the caller's view at all.
    Unavailable,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Complete => "complete",
            State::Lagging => "lagging",
            State::Unavailable => "unavailable",
        }
    }
}

/// What an index build recorded about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub repository: String,
    /// The source frontier: the tree this index was built to.
    pub frontier: String,
    pub compiler: String,
    /// The store revision the build committed at.
    pub built_at: i64,
    pub coverage: Coverage,
}

/// Where a result came from. A caller that is about to cite something is
/// entitled to know whether it came from the index or from a bounded read of
/// the repository's own objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Index,
    Canonical,
}

/// One result: where the bytes are, never the bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub repository: String,
    pub path: String,
    pub blob: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub start_line: u32,
    pub end_line: u32,
    /// BM25 from the index; `0.0` for a canonical read, which does not rank.
    pub score: f64,
    pub origin: Origin,
    /// The span was longer than the per-read cap and was cut to it. The line
    /// range still names the chunk the span was cut from, so a reader knows
    /// where the rest is.
    pub clipped: bool,
}

/// What one repository in the view contributed, and what it could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub repository: String,
    /// The frontier actually searched, when there was one.
    pub frontier: Option<String>,
    pub state: State,
    /// Why this source could not answer completely. Present whenever the
    /// state is not `Complete` and the canonical fallback produced nothing,
    /// so that an empty answer always carries its reason.
    pub reason: Option<String>,
}

/// Which cap ended the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truncation {
    Rows,
    Bytes,
}

/// A keyset position in the total order `(score, repository, path, start_byte)`.
/// Keyset rather than an offset: an offset skips or repeats rows when the
/// index changes between pages, and an index is derived and may be rebuilt
/// under a reader.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub score: f64,
    pub repository: String,
    pub path: String,
    pub start_byte: i64,
}

impl Cursor {
    pub fn encode(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.score, self.repository, self.path, self.start_byte
        )
    }

    pub fn decode(text: &str) -> Option<Cursor> {
        let mut parts = text.split('\u{1f}');
        let score = parts.next()?.parse().ok()?;
        let repository = parts.next()?.to_string();
        let path = parts.next()?.to_string();
        let start_byte = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Cursor {
            score,
            repository,
            path,
            start_byte,
        })
    }
}

/// The three caps a caller gets, separately. `rows` bounds a page, `span`
/// bounds one read, `batch` bounds the whole answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub rows: usize,
    pub cursor: Option<Cursor>,
    pub span_bytes: usize,
    pub batch_bytes: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            rows: 20,
            cursor: None,
            span_bytes: 4096,
            batch_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub found: Vec<Found>,
    /// One per repository in the caller's view, in repository order. A
    /// repository outside the view is never named here.
    pub sources: Vec<Source>,
    pub cursor: Option<Cursor>,
    pub truncated: Option<Truncation>,
}

impl Answer {
    /// The false-absence rule, as a predicate a test can assert and a caller
    /// can check: no source may be less than complete without either
    /// contributing a canonical result or stating why it could not.
    pub fn declares_its_gaps(&self) -> bool {
        self.sources.iter().all(|source| {
            source.state == State::Complete
                || source.reason.is_some()
                || self.found.iter().any(|found| {
                    found.repository == source.repository && found.origin == Origin::Canonical
                })
        })
    }
}

/// One repository the principal may read, as the caller's view names it.
/// Building this is the authorization step: a repository the principal has no
/// read grant for is simply absent, so nothing downstream can leak it.
#[derive(Debug, Clone)]
pub struct Readable<'a> {
    pub id: &'a str,
    pub checkout: &'a Path,
    /// The tree the caller is asking about.
    pub basis: &'a str,
}

pub fn migrate(connection: &Connection) -> Result<(), IndexError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS build_manifest (
             repository          TEXT PRIMARY KEY,
             frontier            TEXT NOT NULL,
             compiler            TEXT NOT NULL,
             built_at            INTEGER NOT NULL,
             blobs               INTEGER NOT NULL,
             indexed             INTEGER NOT NULL,
             anchored            INTEGER NOT NULL,
             binary              INTEGER NOT NULL,
             too_large           INTEGER NOT NULL,
             unanchored_language INTEGER NOT NULL
         );",
    )?;
    Ok(())
}

pub fn record_manifest(connection: &Connection, manifest: &Manifest) -> Result<(), IndexError> {
    connection.execute(
        "INSERT INTO build_manifest
             (repository, frontier, compiler, built_at,
              blobs, indexed, anchored, binary, too_large, unanchored_language)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (repository) DO UPDATE SET
             frontier = excluded.frontier,
             compiler = excluded.compiler,
             built_at = excluded.built_at,
             blobs = excluded.blobs,
             indexed = excluded.indexed,
             anchored = excluded.anchored,
             binary = excluded.binary,
             too_large = excluded.too_large,
             unanchored_language = excluded.unanchored_language",
        params![
            manifest.repository,
            manifest.frontier,
            manifest.compiler,
            manifest.built_at,
            manifest.coverage.blobs as i64,
            manifest.coverage.indexed as i64,
            manifest.coverage.anchored as i64,
            manifest.coverage.binary as i64,
            manifest.coverage.too_large as i64,
            manifest.coverage.unanchored_language as i64,
        ],
    )?;
    Ok(())
}

pub fn manifest(connection: &Connection, repository: &str) -> Result<Option<Manifest>, IndexError> {
    let row = connection
        .query_row(
            "SELECT frontier, compiler, built_at,
                    blobs, indexed, anchored, binary, too_large, unanchored_language
             FROM build_manifest WHERE repository = ?1",
            params![repository],
            |row| {
                Ok(Manifest {
                    repository: repository.to_string(),
                    frontier: row.get(0)?,
                    compiler: row.get(1)?,
                    built_at: row.get(2)?,
                    coverage: Coverage {
                        blobs: row.get::<_, i64>(3)? as usize,
                        indexed: row.get::<_, i64>(4)? as usize,
                        anchored: row.get::<_, i64>(5)? as usize,
                        binary: row.get::<_, i64>(6)? as usize,
                        too_large: row.get::<_, i64>(7)? as usize,
                        unanchored_language: row.get::<_, i64>(8)? as usize,
                    },
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Index `repository` at `tree` and record the manifest for it, in the
/// caller's transaction. Indexing without recording what was indexed is the
/// state this function exists to make unreachable.
pub fn build(
    connection: &Connection,
    repository: &str,
    checkout: &Path,
    tree: &str,
    revision: i64,
) -> Result<Manifest, IndexError> {
    let coverage = index::index_tree(connection, checkout, tree)?;
    let manifest = Manifest {
        repository: repository.to_string(),
        frontier: tree.to_string(),
        compiler: COMPILER.to_string(),
        built_at: revision,
        coverage,
    };
    record_manifest(connection, &manifest)?;
    Ok(manifest)
}

/// A digest over everything the index holds for one tree, so that an index
/// rebuilt from the canonical records can be compared with one maintained
/// incrementally. The row ids are deliberately not in it: they are insertion
/// order, which is not part of what the index means.
pub fn index_digest(connection: &Connection, tree: &str) -> Result<String, IndexError> {
    let mut lines: Vec<String> = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT path, blob, start_byte, end_byte, start_line, end_line
             FROM lexical_chunk WHERE tree = ?1
             ORDER BY path, start_byte, end_byte",
        )?;
        let rows = statement.query_map(params![tree], |row| {
            Ok(format!(
                "chunk\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        for row in rows {
            lines.push(row?);
        }
    }
    {
        let mut statement = connection.prepare(
            "SELECT path, blob, kind, name, start_byte, end_byte, start_line, end_line,
                    is_definition
             FROM anchor WHERE tree = ?1
             ORDER BY path, start_byte, name, kind, is_definition",
        )?;
        let rows = statement.query_map(params![tree], |row| {
            Ok(format!(
                "anchor\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })?;
        for row in rows {
            lines.push(row?);
        }
    }
    Ok(cbr_encoding::sha256_hex(lines.join("\n").as_bytes()))
}

/// Search the caller's view.
pub fn search(
    connection: &Connection,
    view: &[Readable<'_>],
    query: &str,
    bounds: &Bounds,
) -> Result<Answer, IndexError> {
    let mut readable: Vec<&Readable<'_>> = view.iter().collect();
    readable.sort_by(|left, right| left.id.cmp(right.id));

    let terms = lexical::query_terms(query);
    let mut sources: Vec<Source> = Vec::new();
    let mut candidates: Vec<Found> = Vec::new();

    for repository in readable {
        let manifest = manifest(connection, repository.id)?;
        let state = match &manifest {
            Some(manifest) if manifest.frontier == repository.basis => State::Complete,
            Some(_) => State::Lagging,
            None => State::Unavailable,
        };
        let frontier = match state {
            // Lagging and unavailable projections are answered from the tree
            // the caller asked about, so that is the frontier the answer
            // names, not the one the index happens to hold.
            State::Complete => manifest.as_ref().map(|manifest| manifest.frontier.clone()),
            _ => Some(repository.basis.to_string()),
        };
        let mut reason = None;

        if terms.is_empty() {
            // An empty query matches nothing anywhere, which is not a gap in
            // any projection.
            sources.push(Source {
                repository: repository.id.to_string(),
                frontier,
                state: State::Complete,
                reason: None,
            });
            continue;
        }

        match state {
            State::Complete => {
                candidates.extend(from_index(
                    connection,
                    repository,
                    &manifest.expect("complete implies a manifest").frontier,
                    query,
                    bounds,
                    after(bounds, repository.id).as_ref(),
                )?);
            }
            State::Lagging | State::Unavailable => {
                match from_canonical(repository, &terms, bounds, after(bounds, repository.id)) {
                    Ok((found, budget_reached)) => {
                        if budget_reached {
                            reason = Some(format!(
                                "the {} projection's canonical fallback stopped at its \
                                 {CANONICAL_READ_BYTES}-byte read budget",
                                state.as_str()
                            ));
                        } else if found.is_empty() {
                            reason = Some(format!(
                                "the projection is {} and the canonical fallback found nothing \
                                 at this basis",
                                state.as_str()
                            ));
                        }
                        candidates.extend(found);
                    }
                    Err(error) => {
                        reason = Some(format!(
                            "the projection is {} and its canonical records could not be read: \
                             {error}",
                            state.as_str()
                        ));
                    }
                }
            }
        }

        sources.push(Source {
            repository: repository.id.to_string(),
            frontier,
            state,
            reason,
        });
    }

    candidates.sort_by(|left, right| {
        order(left)
            .partial_cmp(&order(right))
            .expect("no NaN score")
    });

    let mut found: Vec<Found> = Vec::new();
    let mut bytes: usize = 0;
    let mut truncated = None;
    for mut candidate in candidates.iter().cloned() {
        if found.len() >= bounds.rows {
            truncated = Some(Truncation::Rows);
            break;
        }
        let span = (candidate.end_byte - candidate.start_byte).max(0) as usize;
        if span > bounds.span_bytes {
            candidate.end_byte = candidate.start_byte + bounds.span_bytes as i64;
            candidate.clipped = true;
        }
        let span = (candidate.end_byte - candidate.start_byte).max(0) as usize;
        if !found.is_empty() && bytes.saturating_add(span) > bounds.batch_bytes {
            truncated = Some(Truncation::Bytes);
            break;
        }
        bytes += span;
        found.push(candidate);
    }

    let cursor = (candidates.len() > found.len())
        .then(|| found.last().map(cursor_of))
        .flatten();
    if cursor.is_none() {
        truncated = None;
    }

    Ok(Answer {
        found,
        sources,
        cursor,
        truncated,
    })
}

/// The keyset one source should continue from, given the single cursor over
/// the merged order `(score, repository, path, start_byte)`. A source whose
/// id sorts after the cursor's repository takes every row at the cursor's
/// score; one that sorts before it takes none of them.
fn after(bounds: &Bounds, repository: &str) -> Option<lexical::After> {
    let cursor = bounds.cursor.as_ref()?;
    Some(match repository.cmp(cursor.repository.as_str()) {
        std::cmp::Ordering::Equal => lexical::After {
            score: cursor.score,
            path: cursor.path.clone(),
            start_byte: cursor.start_byte,
            include_equal_score: true,
        },
        std::cmp::Ordering::Greater => lexical::After {
            score: cursor.score,
            path: String::new(),
            start_byte: -1,
            include_equal_score: true,
        },
        std::cmp::Ordering::Less => lexical::After {
            score: cursor.score,
            path: String::new(),
            start_byte: -1,
            include_equal_score: false,
        },
    })
}

fn passes(after: Option<&lexical::After>, score: f64, path: &str, start_byte: i64) -> bool {
    let Some(after) = after else {
        return true;
    };
    score > after.score
        || (after.include_equal_score
            && score == after.score
            && (path > after.path.as_str()
                || (path == after.path && start_byte > after.start_byte)))
}

fn order(found: &Found) -> (f64, String, String, i64) {
    (
        found.score,
        found.repository.clone(),
        found.path.clone(),
        found.start_byte,
    )
}

fn cursor_of(found: &Found) -> Cursor {
    Cursor {
        score: found.score,
        repository: found.repository.clone(),
        path: found.path.clone(),
        start_byte: found.start_byte,
    }
}

/// One complete projection's contribution, taken from the index. One more row
/// than the page needs, so the caller can tell whether a cursor is owed.
fn from_index(
    connection: &Connection,
    repository: &Readable<'_>,
    frontier: &str,
    query: &str,
    bounds: &Bounds,
    after: Option<&lexical::After>,
) -> Result<Vec<Found>, IndexError> {
    let hits = lexical::search_after(
        connection,
        frontier,
        query,
        after,
        bounds.rows.saturating_add(1),
    )?;
    Ok(hits
        .into_iter()
        .map(|hit| Found {
            repository: repository.id.to_string(),
            path: hit.path,
            blob: hit.blob,
            start_byte: hit.start_byte,
            end_byte: hit.end_byte,
            start_line: hit.start_line,
            end_line: hit.end_line,
            score: hit.score,
            origin: Origin::Index,
            clipped: false,
        })
        .collect())
}

/// The bounded canonical fallback: read the tree the caller asked about,
/// chunked exactly as the index would chunk it, and keep the chunks whose
/// text holds every term. It does not rank — `score` is `0.0` for all of
/// them — because ranking is what an index is for.
///
/// Returns the results and whether the read budget was reached, which is
/// itself a gap the answer has to declare.
fn from_canonical(
    repository: &Readable<'_>,
    terms: &[String],
    bounds: &Bounds,
    after: Option<lexical::After>,
) -> Result<(Vec<Found>, bool), cbr_identity::IdentityError> {
    let mut found = Vec::new();
    let mut read: usize = 0;
    let wanted = bounds.rows.saturating_add(1);
    for entry in cbr_identity::tree_entries(repository.checkout, repository.basis)? {
        if found.len() >= wanted {
            break;
        }
        if entry.mode != "100644" && entry.mode != "100755" {
            continue;
        }
        if read >= CANONICAL_READ_BYTES {
            return Ok((found, true));
        }
        let bytes = cbr_identity::read_blob(repository.checkout, &entry.blob)?;
        read += bytes.len();
        if bytes.len() > MAX_BLOB_BYTES {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        for chunk in lexical::chunks(&text, CHUNK_LINES) {
            if found.len() >= wanted {
                break;
            }
            let tokens: BTreeSet<String> = lexical::normalise(chunk.text)
                .split(' ')
                .filter(|token| !token.is_empty())
                .map(str::to_string)
                .collect();
            if !passes(after.as_ref(), 0.0, &entry.path, chunk.start_byte) {
                continue;
            }
            if terms.iter().all(|term| tokens.contains(term)) {
                found.push(Found {
                    repository: repository.id.to_string(),
                    path: entry.path.clone(),
                    blob: entry.blob.clone(),
                    start_byte: chunk.start_byte,
                    end_byte: chunk.end_byte,
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    score: 0.0,
                    origin: Origin::Canonical,
                    clipped: false,
                });
            }
        }
    }
    Ok((found, false))
}

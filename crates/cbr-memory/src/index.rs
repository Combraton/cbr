//! Indexing one tree of one repository (INTERNALS sections 4 and 5).
//!
//! An index is derived and replaceable, so this is deliberately dull: read a
//! tree's blobs, put their text in the lexical index and their definitions in
//! the anchor table, and report what was **not** covered. The report is the
//! part that matters. A packet that says "searched everything" when a tenth
//! of the tree was binary, too large or in a language CBR does not anchor is
//! worse than one that states the gap, because a caller cannot tell the
//! difference between "no result" and "never looked" (INTERNALS section 5:
//! empty search results are not proof of absence).
//!
//! Everything commits in the caller's transaction, so a half-indexed tree is
//! not a state the store can be left in.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;

use crate::{anchors, lexical};

/// Blobs larger than this are not indexed. A minified bundle or a vendored
/// dump would otherwise dominate the index while being the least useful text
/// in the tree; it is reported as skipped rather than silently included.
pub const MAX_BLOB_BYTES: usize = 1 << 20;

/// How many lines one indexed chunk covers. Smaller chunks mean tighter
/// citations and more rows.
pub const CHUNK_LINES: usize = 20;

/// What one tree's indexing covered, and what it did not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Blobs the tree names, excluding submodules and symlinks.
    pub blobs: usize,
    pub indexed: usize,
    pub anchored: usize,
    /// Not valid UTF-8: not text, so not searchable text.
    pub binary: usize,
    pub too_large: usize,
    /// Text, indexed lexically, but in a language CBR does not anchor.
    pub unanchored_language: usize,
    /// The same total, broken down by file extension, because "991 blobs in
    /// a language with no anchors" does not tell a reader whether the gap is
    /// documentation or the Cython half of a scientific library.
    pub unanchored_by_kind: BTreeMap<String, usize>,
}

#[derive(Debug)]
pub enum IndexError {
    Identity(cbr_identity::IdentityError),
    Anchor(anchors::AnchorError),
    Storage(rusqlite::Error),
}

impl From<cbr_identity::IdentityError> for IndexError {
    fn from(error: cbr_identity::IdentityError) -> Self {
        IndexError::Identity(error)
    }
}

impl From<anchors::AnchorError> for IndexError {
    fn from(error: anchors::AnchorError) -> Self {
        IndexError::Anchor(error)
    }
}

impl From<rusqlite::Error> for IndexError {
    fn from(error: rusqlite::Error) -> Self {
        IndexError::Storage(error)
    }
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::Identity(error) => write!(formatter, "identity: {error}"),
            IndexError::Anchor(error) => write!(formatter, "anchors: {error}"),
            IndexError::Storage(error) => write!(formatter, "storage: {error}"),
        }
    }
}

pub fn migrate(connection: &Connection) -> Result<(), IndexError> {
    lexical::migrate(connection)?;
    anchors::migrate(connection)?;
    Ok(())
}

/// Index every text blob of `tree`, replacing whatever was indexed for that
/// tree before. Re-indexing the same tree is therefore idempotent, and
/// indexing a second tree leaves the first alone.
pub fn index_tree(
    connection: &Connection,
    repository: &Path,
    tree: &str,
) -> Result<Coverage, IndexError> {
    lexical::remove_tree(connection, tree)?;
    anchors::remove_tree(connection, tree)?;

    let mut coverage = Coverage::default();
    for entry in cbr_identity::tree_entries(repository, tree)? {
        // A submodule is its own repository's identity, and a symlink's
        // target is a path rather than text.
        if entry.mode != "100644" && entry.mode != "100755" {
            continue;
        }
        coverage.blobs += 1;
        // The size comes from the object header, so an oversized blob is
        // refused without being allocated.
        let Some(bytes) =
            cbr_identity::read_blob_bounded(repository, &entry.blob, MAX_BLOB_BYTES as u64)?
        else {
            coverage.too_large += 1;
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            coverage.binary += 1;
            continue;
        };
        for chunk in lexical::chunks(&text, CHUNK_LINES) {
            lexical::index_chunk(
                connection,
                &lexical::Document {
                    tree,
                    path: &entry.path,
                    blob: &entry.blob,
                    chunk: &chunk,
                },
            )?;
        }
        coverage.indexed += 1;

        match anchors::language_for_path(&entry.path) {
            Some(language) => {
                let found = anchors::anchors(language, text.as_bytes())?;
                anchors::record(connection, tree, &entry.path, &entry.blob, &found)?;
                coverage.anchored += 1;
            }
            None => {
                coverage.unanchored_language += 1;
                let kind = crate::lexical::file_kind(&entry.path);
                *coverage.unanchored_by_kind.entry(kind).or_default() += 1;
            }
        }
    }
    Ok(coverage)
}

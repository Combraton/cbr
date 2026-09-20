//! Code anchors from tree-sitter tags (STACK section 6).
//!
//! For a blob, running the grammar's shipped `tags.scm` yields, per
//! definition or reference, a `(kind, name, byte range, line range)`. That is
//! the whole claim: **where a name is defined**, not which definition a call
//! refers to. Two `impl` blocks with the same method name, generics, macros,
//! re-exports and shadowing all collapse to several candidates, and precise
//! resolution has no good option that fits one local process.
//!
//! So the honest mitigation is built in rather than added later: every
//! candidate is kept and ambiguity is marked, never resolved by picking one
//! (INTERNALS section 2, "ambiguous resolution remains ambiguous"). Each
//! anchor records the tree and blob it was taken at, so an anchor that has
//! gone stale is detectable instead of merely wrong.
//!
//! Only the languages CBR pins are anchored. A file in any other language has
//! no anchors at all, which is a gap a caller can see, rather than a guess.

use rusqlite::{Connection, params};
use tree_sitter_tags::{TagsConfiguration, TagsContext};

/// The languages CBR claims. Each grammar is pinned in `Cargo.toml`, and a
/// tree-sitter core bump is a compatibility check against every one of them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
}

#[derive(Debug)]
pub enum AnchorError {
    /// The grammar and the core disagree, or the query does not compile: a
    /// build-time fault, surfaced rather than papered over.
    Grammar(String),
    Storage(rusqlite::Error),
}

impl From<rusqlite::Error> for AnchorError {
    fn from(error: rusqlite::Error) -> Self {
        AnchorError::Storage(error)
    }
}

impl std::fmt::Display for AnchorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnchorError::Grammar(reason) => write!(formatter, "grammar: {reason}"),
            AnchorError::Storage(error) => write!(formatter, "storage: {error}"),
        }
    }
}

/// One tagged name in one blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The grammar's own syntax type: `function`, `method`, `class`, `call`…
    pub kind: String,
    pub name: String,
    pub start_byte: i64,
    pub end_byte: i64,
    /// 1-based, inclusive.
    pub start_line: u32,
    pub end_line: u32,
    /// A definition, rather than a reference to one.
    pub is_definition: bool,
}

/// One recorded anchor, with where it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: String,
    pub blob: String,
    pub kind: String,
    pub name: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub start_line: u32,
    pub end_line: u32,
}

/// What a name resolves to. Several candidates are not narrowed: a caller
/// that needs one must decide, with the ambiguity in front of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub candidates: Vec<Candidate>,
    pub ambiguous: bool,
}

pub fn language_for_path(path: &str) -> Option<Language> {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension)?;
    match extension {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "js" | "mjs" | "cjs" | "jsx" => Some(Language::JavaScript),
        "ts" | "mts" | "cts" => Some(Language::TypeScript),
        "tsx" => Some(Language::Tsx),
        _ => None,
    }
}

fn configuration(language: Language) -> Result<TagsConfiguration, AnchorError> {
    // TypeScript's own tags query holds only what TypeScript adds; the
    // grammar is a superset of JavaScript's and its definitions are tagged by
    // JavaScript's query. Used alone it yields nothing for a plain function,
    // which is how this was found.
    let typescript = format!(
        "{}\n{}",
        tree_sitter_javascript::TAGS_QUERY,
        tree_sitter_typescript::TAGS_QUERY
    );
    let (grammar, tags, locals) = match language {
        Language::Rust => (
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::TAGS_QUERY.to_string(),
            String::new(),
        ),
        Language::Python => (
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::TAGS_QUERY.to_string(),
            String::new(),
        ),
        Language::JavaScript => (
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::TAGS_QUERY.to_string(),
            tree_sitter_javascript::LOCALS_QUERY.to_string(),
        ),
        Language::TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            typescript,
            tree_sitter_typescript::LOCALS_QUERY.to_string(),
        ),
        Language::Tsx => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            typescript,
            tree_sitter_typescript::LOCALS_QUERY.to_string(),
        ),
    };
    TagsConfiguration::new(grammar, &tags, &locals)
        .map_err(|error| AnchorError::Grammar(format!("{language:?}: {error}")))
}

/// Every tagged name in `source`. Source that does not parse yields the tags
/// the grammar could still find: an unparsable working tree is normal, and
/// refusing to anchor it would lose the rest of the file.
pub fn anchors(language: Language, source: &[u8]) -> Result<Vec<Anchor>, AnchorError> {
    let configuration = configuration(language)?;
    let mut context = TagsContext::new();
    let (tags, _failed) = context
        .generate_tags(&configuration, source, None)
        .map_err(|error| AnchorError::Grammar(format!("{language:?}: {error}")))?;
    let mut anchors = Vec::new();
    for tag in tags {
        let tag = tag.map_err(|error| AnchorError::Grammar(format!("{language:?}: {error}")))?;
        let name = String::from_utf8_lossy(&source[tag.name_range.clone()]).into_owned();
        anchors.push(Anchor {
            kind: configuration
                .syntax_type_name(tag.syntax_type_id)
                .to_string(),
            name,
            start_byte: tag.range.start as i64,
            end_byte: tag.range.end as i64,
            start_line: tag.span.start.row as u32 + 1,
            end_line: tag.span.end.row as u32 + 1,
            is_definition: tag.is_definition,
        });
    }
    Ok(anchors)
}

pub fn migrate(connection: &Connection) -> Result<(), AnchorError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS anchor (
             id            INTEGER PRIMARY KEY,
             tree          TEXT NOT NULL,
             path          TEXT NOT NULL,
             blob          TEXT NOT NULL,
             kind          TEXT NOT NULL,
             name          TEXT NOT NULL,
             start_byte    INTEGER NOT NULL,
             end_byte      INTEGER NOT NULL,
             start_line    INTEGER NOT NULL,
             end_line      INTEGER NOT NULL,
             is_definition INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS anchor_name ON anchor (tree, name, is_definition);
         CREATE INDEX IF NOT EXISTS anchor_tree ON anchor (tree, path);",
    )?;
    Ok(())
}

/// Record a blob's anchors at a tree.
pub fn record(
    connection: &Connection,
    tree: &str,
    path: &str,
    blob: &str,
    anchors: &[Anchor],
) -> Result<(), AnchorError> {
    for anchor in anchors {
        connection.execute(
            "INSERT INTO anchor (tree, path, blob, kind, name, start_byte, end_byte,
                                 start_line, end_line, is_definition)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                tree,
                path,
                blob,
                anchor.kind,
                anchor.name,
                anchor.start_byte,
                anchor.end_byte,
                anchor.start_line,
                anchor.end_line,
                i64::from(anchor.is_definition)
            ],
        )?;
    }
    Ok(())
}

pub fn remove_tree(connection: &Connection, tree: &str) -> Result<(), AnchorError> {
    connection.execute("DELETE FROM anchor WHERE tree = ?1", params![tree])?;
    Ok(())
}

/// Every definition of `name` at `tree`. Two candidates stay two candidates.
pub fn resolve(connection: &Connection, tree: &str, name: &str) -> Result<Resolution, AnchorError> {
    anchored(connection, tree, name, true)
}

/// Every *reference* to `name` at `tree`: where it is used rather than where
/// it is defined. Tags say a name appears here, not which definition it
/// means, so this is a list of places to look, never a call graph.
pub fn references(
    connection: &Connection,
    tree: &str,
    name: &str,
) -> Result<Vec<Candidate>, AnchorError> {
    Ok(anchored(connection, tree, name, false)?.candidates)
}

fn anchored(
    connection: &Connection,
    tree: &str,
    name: &str,
    definitions: bool,
) -> Result<Resolution, AnchorError> {
    let mut statement = connection.prepare(
        "SELECT path, blob, kind, name, start_byte, end_byte, start_line, end_line
         FROM anchor
         WHERE tree = ?1 AND name = ?2 AND is_definition = ?3
         ORDER BY path, start_byte",
    )?;
    let rows = statement.query_map(params![tree, name, i64::from(definitions)], |row| {
        Ok(Candidate {
            path: row.get(0)?,
            blob: row.get(1)?,
            kind: row.get(2)?,
            name: row.get(3)?,
            start_byte: row.get(4)?,
            end_byte: row.get(5)?,
            start_line: row.get(6)?,
            end_line: row.get(7)?,
        })
    })?;
    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(Resolution {
        ambiguous: candidates.len() > 1,
        candidates,
    })
}

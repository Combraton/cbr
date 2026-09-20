//! The lexical index: SQLite FTS5 with a pre-tokeniser written in Rust
//! (STACK section 5).
//!
//! FTS5 was chosen over a dedicated search engine for **atomicity, not
//! features**. An FTS5 table is more rows in the same database file, so
//! indexing joins the caller's transaction: a command's state change and the
//! index rows it implies commit or roll back together, with no second writer,
//! no second crash window and no "indexed up to here" watermark to chase.
//!
//! The price is that FTS5 cannot tokenise source code. `unicode61` splits on
//! punctuation, and there is **no camelCase splitting at all**, so
//! `getUserName` is one token and a search for `user` never finds it. Stemming
//! would be worse than nothing on identifiers, where `flush` and `flushed` are
//! different symbols.
//!
//! So text is normalised here before it is indexed: every identifier is stored
//! as itself *and* as its parts, and a query is expanded the same way. What
//! that costs is index size; what it buys is that searching for `user name`
//! finds `getUserName` without an FFI tokeniser or a second engine.
//!
//! An index is derived from one tree and is always replaceable: it records the
//! tree and blob it was built from, so a hit can be checked rather than
//! trusted, and dropping a tree's rows leaves every other tree alone.

use rusqlite::{Connection, params};

/// How many bytes one indexed chunk may cover, whatever its line count.
///
/// Matched to `compiler::EXCERPT_BYTES`: a chunk a packet cannot show in
/// full is a chunk whose ranking a reader cannot check.
pub const CHUNK_BYTES: usize = 2048;

/// What kind of file a path names, for reporting a coverage gap by kind.
///
/// The name after the last dot, unless the dot begins the file name: a
/// `.gitignore` is a dotfile, not twelve files of kind `gitignore`.
pub fn file_kind(path: &str) -> String {
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!(".{extension}"),
        _ => "no extension".to_string(),
    }
}

/// A span of a blob's text, in bytes and in lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub text: &'a str,
    pub start_byte: i64,
    pub end_byte: i64,
    /// 1-based, inclusive.
    pub start_line: u32,
    pub end_line: u32,
}

/// One chunk of one blob, as it is indexed.
pub struct Document<'a> {
    pub tree: &'a str,
    pub path: &'a str,
    pub blob: &'a str,
    pub chunk: &'a Chunk<'a>,
}

/// A search result: where the text is, never the text itself. The bytes come
/// from the blob, so a citation resolves to the same bytes the index was
/// built from or not at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub path: String,
    pub blob: String,
    pub start_byte: i64,
    pub end_byte: i64,
    pub start_line: u32,
    pub end_line: u32,
    /// BM25, as SQLite reports it: smaller is a better match.
    pub score: f64,
}

pub fn migrate(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS lexical_chunk (
             id         INTEGER PRIMARY KEY,
             tree       TEXT NOT NULL,
             path       TEXT NOT NULL,
             blob       TEXT NOT NULL,
             start_byte INTEGER NOT NULL,
             end_byte   INTEGER NOT NULL,
             start_line INTEGER NOT NULL,
             end_line   INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS lexical_chunk_tree ON lexical_chunk (tree, path);
         -- No stemmer: `flush` and `flushed` are different symbols. The
         -- splitting FTS5 cannot do happens in `normalise` before this sees
         -- the text.
         CREATE VIRTUAL TABLE IF NOT EXISTS lexical_search USING fts5 (
             search,
             tokenize = 'unicode61 remove_diacritics 2'
         );",
    )
}

/// Split `text` into chunks of at most `lines` lines, tiling it exactly:
/// every byte is in one chunk, in order, with the line numbers to match.
pub fn chunks(text: &str, lines: usize) -> Vec<Chunk<'_>> {
    chunks_bounded(text, lines, CHUNK_BYTES)
}

/// Split `text` into chunks of at most `lines` lines **and** at most `bytes`
/// bytes, tiling it exactly.
///
/// The byte bound is not a detail. A chunk is the unit retrieval ranks and
/// the unit a packet excerpts, and twenty lines of a Markdown table is
/// nearly nine kilobytes: one chunk held a whole decision table, so ranking
/// could not tell one decision from another and an excerpt of it could not
/// show the row that answered a question. Bounding by bytes makes the chunk
/// the thing a reader actually reads.
pub fn chunks_bounded(text: &str, lines: usize, bytes: usize) -> Vec<Chunk<'_>> {
    let lines = lines.max(1);
    let bytes = bytes.max(1);
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut start_line = 1u32;
    let mut counted = 0usize;
    let mut line_end = 0usize;
    for (offset, byte) in text.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        counted += 1;
        line_end = offset + 1;
        if counted == lines || line_end - start >= bytes {
            chunks.push(Chunk {
                text: &text[start..line_end],
                start_byte: start as i64,
                end_byte: line_end as i64,
                start_line,
                end_line: start_line + counted as u32 - 1,
            });
            start = line_end;
            start_line += counted as u32;
            counted = 0;
        }
    }
    let _ = line_end;
    if start < text.len() {
        // A final line with no newline of its own.
        let remaining = text[start..].matches('\n').count() as u32;
        chunks.push(Chunk {
            text: &text[start..],
            start_byte: start as i64,
            end_byte: text.len() as i64,
            start_line,
            end_line: start_line + remaining,
        });
    }
    chunks
}

/// The pre-tokeniser. Each run of letters and digits is emitted as itself and,
/// when it is made of several parts, as those parts: `getUserName` becomes
/// `getusername get user name`. Everything is lowercased; nothing is stemmed.
pub fn normalise(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for token in text.split(|c: char| !c.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        let parts = split_identifier(token);
        if parts.len() > 1 {
            out.push(token.to_lowercase());
            out.extend(parts.into_iter().map(|part| part.to_lowercase()));
        } else {
            out.push(token.to_lowercase());
        }
    }
    out.join(" ")
}

/// The parts of one identifier: `camelCase`, `PascalCase`, `HTTPServer` and
/// letter/digit boundaries. Separators never reach this: the caller has split
/// on them already.
fn split_identifier(token: &str) -> Vec<String> {
    let characters: Vec<char> = token.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    for (index, character) in characters.iter().enumerate() {
        let previous = index.checked_sub(1).map(|before| characters[before]);
        let next = characters.get(index + 1).copied();
        let boundary = match previous {
            None => false,
            Some(previous) => {
                // getUser | HTTPServer (the last capital starts the next word)
                (previous.is_lowercase() && character.is_uppercase())
                    || (previous.is_uppercase()
                        && character.is_uppercase()
                        && next.is_some_and(char::is_lowercase))
                    // sha | 256 | digest
                    || (previous.is_alphabetic() && character.is_numeric())
                    || (previous.is_numeric() && character.is_alphabetic())
            }
        };
        if boundary && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        current.push(*character);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Index one chunk. Runs on whatever connection or transaction the caller
/// gives it, which is the point of using FTS5 at all.
pub fn index_chunk(
    connection: &Connection,
    document: &Document<'_>,
) -> Result<i64, rusqlite::Error> {
    let chunk = document.chunk;
    connection.execute(
        "INSERT INTO lexical_chunk (tree, path, blob, start_byte, end_byte, start_line, end_line)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            document.tree,
            document.path,
            document.blob,
            chunk.start_byte,
            chunk.end_byte,
            chunk.start_line,
            chunk.end_line
        ],
    )?;
    let id = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO lexical_search (rowid, search) VALUES (?1, ?2)",
        params![id, normalise(chunk.text)],
    )?;
    Ok(id)
}

/// Drop one tree's rows. An index is derived: removing it changes no meaning,
/// and leaves every other tree indexed.
pub fn remove_tree(connection: &Connection, tree: &str) -> Result<(), rusqlite::Error> {
    connection.execute(
        "DELETE FROM lexical_search WHERE rowid IN (SELECT id FROM lexical_chunk WHERE tree = ?1)",
        params![tree],
    )?;
    connection.execute("DELETE FROM lexical_chunk WHERE tree = ?1", params![tree])?;
    Ok(())
}

/// A keyset position inside one tree's results, in the order
/// `(score, path, start_byte)`. `include_equal_score` is how a caller
/// merging several trees says whether rows tying the cursor's score belong
/// before or after it in the wider order it is imposing.
#[derive(Debug, Clone, PartialEq)]
pub struct After {
    pub score: f64,
    pub path: String,
    pub start_byte: i64,
    pub include_equal_score: bool,
}

/// What a multi-term query means.
///
/// The ladder is recorded rather than implied, because a selector of several
/// words is the common case and the two readings differ sharply: `All` finds
/// only chunks holding every term, which is precise and often empty, while
/// `Any` finds the best partial match, which is what a caller wants when the
/// precise reading returned nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terms {
    /// Every part of every token must be in the chunk.
    All,
    /// Any part may match; BM25 ranks the chunk with the most, and the
    /// rarest, of them first.
    Any,
}

/// Search one tree. Every query token must match, as itself or as its parts;
/// an empty query matches nothing rather than everything.
pub fn search(
    connection: &Connection,
    tree: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<Hit>, rusqlite::Error> {
    search_after(connection, tree, query, None, None, Terms::All, limit)
}

/// Search one tree, continuing after a keyset position. The position is
/// applied in SQL rather than by the caller, because a caller that filters
/// after the fact is filtering a page that was already truncated — it sees
/// the same top rows on every page and the walk stops early.
pub fn search_after(
    connection: &Connection,
    tree: &str,
    query: &str,
    within: Option<&str>,
    after: Option<&After>,
    terms: Terms,
    limit: usize,
) -> Result<Vec<Hit>, rusqlite::Error> {
    let Some(expression) = match_expression(query, terms) else {
        return Ok(Vec::new());
    };
    let (score, path, start_byte, equal) = match after {
        Some(after) => (
            after.score,
            after.path.clone(),
            after.start_byte,
            after.include_equal_score,
        ),
        None => (0.0, String::new(), 0, false),
    };
    let mut statement = connection.prepare(
        "SELECT path, blob, start_byte, end_byte, start_line, end_line, score FROM (
             SELECT c.path AS path, c.blob AS blob, c.start_byte AS start_byte,
                    c.end_byte AS end_byte, c.start_line AS start_line,
                    c.end_line AS end_line, bm25(lexical_search) AS score
             FROM lexical_search
             JOIN lexical_chunk c ON c.id = lexical_search.rowid
             WHERE lexical_search MATCH ?1 AND c.tree = ?2
               AND (?9 IS NULL OR c.path = ?9)
         )
         WHERE ?4 = 0
            OR score > ?5
            OR (?7 = 1 AND score = ?5
                AND (path > ?6 OR (path = ?6 AND start_byte > ?8)))
         ORDER BY score, path, start_byte
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![
            expression,
            tree,
            limit as i64,
            i64::from(after.is_some()),
            score,
            path,
            i64::from(equal),
            start_byte,
            within
        ],
        |row| {
            Ok(Hit {
                path: row.get(0)?,
                blob: row.get(1)?,
                start_byte: row.get(2)?,
                end_byte: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                score: row.get(6)?,
            })
        },
    )?;
    let mut hits = Vec::new();
    for row in rows {
        hits.push(row?);
    }
    Ok(hits)
}

/// Turn a query into FTS5's syntax: every part of every token must match.
/// A document stores each identifier as its parts as well as whole, so the
/// parts alone are enough, and asking for them means a query for
/// `getUserName` also finds prose that says "get user name".
fn match_expression(query: &str, mode: Terms) -> Option<String> {
    let terms: Vec<String> = query_terms(query)
        .into_iter()
        .map(|term| format!("\"{term}\""))
        .collect();
    let joiner = match mode {
        Terms::All => " AND ",
        Terms::Any => " OR ",
    };
    (!terms.is_empty()).then(|| terms.join(joiner))
}

/// The terms a query asks for: every part of every token, lowercased. The
/// same rule a canonical read has to apply when the index cannot answer, so
/// the two agree about what "matches" means.
pub fn query_terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for token in query.split(|c: char| !c.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        terms.extend(
            split_identifier(token)
                .into_iter()
                .map(|part| part.to_lowercase()),
        );
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_on_case_and_digit_boundaries() {
        assert_eq!(split_identifier("getUserName"), ["get", "User", "Name"]);
        assert_eq!(split_identifier("HTTPServer"), ["HTTP", "Server"]);
        assert_eq!(split_identifier("sha256"), ["sha", "256"]);
        assert_eq!(split_identifier("plain"), ["plain"]);
        assert_eq!(split_identifier("UPPER"), ["UPPER"]);
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(match_expression("   ...  ", Terms::All), None);
        assert_eq!(match_expression("   ...  ", Terms::Any), None);
    }

    #[test]
    fn the_two_readings_of_a_multi_term_query_are_different_expressions() {
        assert_eq!(
            match_expression("drainQueue now", Terms::All).expect("an expression"),
            "\"drain\" AND \"queue\" AND \"now\""
        );
        assert_eq!(
            match_expression("drainQueue now", Terms::Any).expect("an expression"),
            "\"drain\" OR \"queue\" OR \"now\""
        );
    }
}

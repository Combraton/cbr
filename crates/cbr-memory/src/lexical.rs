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
    let lines = lines.max(1);
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
        if counted == lines {
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

/// Search one tree. Every query token must match, as itself or as its parts;
/// an empty query matches nothing rather than everything.
pub fn search(
    connection: &Connection,
    tree: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<Hit>, rusqlite::Error> {
    let Some(expression) = match_expression(query) else {
        return Ok(Vec::new());
    };
    let mut statement = connection.prepare(
        "SELECT c.path, c.blob, c.start_byte, c.end_byte, c.start_line, c.end_line,
                bm25(lexical_search) AS score
         FROM lexical_search
         JOIN lexical_chunk c ON c.id = lexical_search.rowid
         WHERE lexical_search MATCH ?1 AND c.tree = ?2
         ORDER BY score, c.path, c.start_byte
         LIMIT ?3",
    )?;
    let rows = statement.query_map(params![expression, tree, limit as i64], |row| {
        Ok(Hit {
            path: row.get(0)?,
            blob: row.get(1)?,
            start_byte: row.get(2)?,
            end_byte: row.get(3)?,
            start_line: row.get(4)?,
            end_line: row.get(5)?,
            score: row.get(6)?,
        })
    })?;
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
fn match_expression(query: &str) -> Option<String> {
    let mut terms: Vec<String> = Vec::new();
    for token in query.split(|c: char| !c.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        terms.extend(
            split_identifier(token)
                .into_iter()
                .map(|part| format!("\"{}\"", part.to_lowercase())),
        );
    }
    (!terms.is_empty()).then(|| terms.join(" AND "))
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
        assert_eq!(match_expression("   ...  "), None);
    }
}

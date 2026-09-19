//! CBR's derived memory: what is computed from the journal rather than
//! recorded in it (INTERNALS sections 1 and 4).
//!
//! Three pieces, and one rule they share: **a derived thing is replaceable and
//! never authority.** An index can be rebuilt, an anchor can go stale, an
//! evaluation can be invalidated; none of them changes what the journal says.
//! So each records enough to be checked rather than trusted — the revision an
//! index was built from, the tree and blob an anchor was taken at, and the
//! exact inputs an evaluation read.
//!
//! - [`evaluator`]: the persistent dependency evaluator (STACK section 7).
//! - [`lexical`]: the FTS5 index and its pre-tokeniser (STACK section 5).
//! - [`anchors`]: code anchors from tree-sitter tags (STACK section 6).
//! - [`retrieval`]: the rules a caller of those gets — the build manifest, the
//!   false-absence rule, the view a search answers inside, and the bounds.
//!
//! Everything here works on a caller-supplied `rusqlite` connection or
//! transaction, so index rows and memo rows commit in the same transaction as
//! the state change that caused them. That is the reason these live in
//! SQLite at all.

pub mod anchors;
pub mod evaluator;
pub mod index;
pub mod lexical;
pub mod retrieval;

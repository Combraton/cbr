//! The ids a packet names: its sections, its citations, its omissions and
//! the artifact it is sealed as.
//!
//! **Every one of them is a protocol identifier.** `core/1`'s `identifier`
//! is `^[A-Za-z0-9][A-Za-z0-9._:~-]{0,127}$`, and the `context` schemas
//! require it of a section id, a citation id, an omission's section id and
//! an artifact id. The compiler used to build a discovered span's ids out
//! of its repository path, as in
//! `dc-span-crates/cbr-cli/tests/journey_two.rs-67989`: a `/` is outside
//! the grammar and a long path passes 128 bytes, so a published packet
//! carried schema-invalid ids and this provider's own `context.expand`
//! refused them. Only a citation of a file at a repository's root could be
//! followed.
//!
//! Two rules make any input an identifier.
//!
//! - **[`encode`]** keeps `[A-Za-z0-9._-]`, writes `/` as `:`, and writes
//!   every other byte as `~XX` in uppercase hex: `:` and `~` themselves, a
//!   space, a control byte, and each byte of a character outside ASCII. It
//!   is injective, and reads back with one rule ([`decode`]), so a path a
//!   packet names is still the path, only spelled inside the grammar. A
//!   path is bytes, and two spellings of one word in Unicode's two normal
//!   forms are two paths and two ids, as they are two files to git.
//! - **[`bounded`]** returns `prefix + rest` when that is an identifier,
//!   which is every id that already was one, byte for byte — J2's
//!   `s-log.o1` and `c-log` are unchanged. Past 128 bytes it keeps the
//!   prefix, then the marker `~~`, then 32 hex digits of a SHA-256 of the
//!   rest, then as much of the rest's tail as fits. `~~` never comes out
//!   of [`encode`], where every `~` begins an escape, and an unshortened id
//!   never has `~` right after its prefix, so a shortened id is never equal
//!   to an unshortened one.
//!
//! **What a shortened id costs.** Its readable part is only the tail, and
//! two inputs that differ only before the tail are told apart by the digest
//! alone: 128 bits, where a collision is not a practical concern. The
//! locator line a section carries names the whole path either way, so a
//! reader never needs to decode an id to know what a section is.
//!
//! **Where an id says what it is.** A discovered span is
//! `span-<repository>:<encoded path>-<start byte>` and an anchor is
//! `anchor-<repository>:<encoded name>`. The repository is in both because
//! the same path at the same byte in two repositories of one basis is two
//! spans, which the old id gave one name; a repository id holds no `:`
//! (`repositories.rs`), so the first `:` ends it.

/// The longest identifier the protocol admits.
pub const MAX: usize = 128;

/// What follows the prefix of a shortened id, and never an unshortened one.
const MARK: &str = "~~";

/// How many hex digits of the digest a shortened id keeps: 128 bits.
const DIGEST_DIGITS: usize = 32;

/// Mixed into the digest so it is this rule's and no other hash of the
/// same bytes. The tag is not load-bearing for uniqueness — every id
/// digests with the same one — and a mutant that changes it survives,
/// which is expected.
const DOMAIN: &[u8] = b"cbr-packet-id/1\n";

/// Whether a byte is kept as itself by [`encode`].
fn kept(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

/// Whether a byte may appear after the first in an identifier.
fn in_grammar(byte: u8) -> bool {
    kept(byte) || matches!(byte, b':' | b'~')
}

/// Spell `raw` inside the identifier grammar: `[A-Za-z0-9._-]` as itself,
/// `/` as `:`, and every other byte as `~XX`.
pub fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for &byte in raw.as_bytes() {
        if kept(byte) {
            out.push(char::from(byte));
        } else if byte == b'/' {
            out.push(':');
        } else {
            out.push_str(&format!("~{byte:02X}"));
        }
    }
    out
}

/// The bytes [`encode`] was given, or `None` for text it could not have
/// produced. Only the tests read ids back: a section's locator names its
/// path in full, so nothing in the provider needs to.
#[cfg(test)]
pub fn decode(encoded: &str) -> Option<Vec<u8>> {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b':' => out.push(b'/'),
            b'~' => {
                let hex = encoded.get(at + 1..at + 3)?;
                if !hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
                {
                    return None;
                }
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 2;
            }
            byte if kept(byte) => out.push(byte),
            _ => return None,
        }
        at += 1;
    }
    Some(out)
}

/// `prefix + rest` when that is an identifier; otherwise the prefix, the
/// marker, a digest of `rest` and the longest tail of `rest` that fits.
///
/// **The tail** starts at a `:` where one leaves room — so a shortened span
/// id ends in the path's last components — and otherwise wherever the
/// longest suffix starts, never inside a `~XX` escape and never before a
/// byte outside the grammar. Every prefix a packet uses starts with a
/// letter, so the shortened form is an identifier too.
pub fn bounded(prefix: &str, rest: &str) -> String {
    let whole = format!("{prefix}{rest}");
    if !rest.starts_with('~') && crate::envelope::is_identifier(&whole) {
        return whole;
    }
    let mut digested = DOMAIN.to_vec();
    digested.extend_from_slice(rest.as_bytes());
    let digest = cbr_encoding::sha256_hex(&digested);
    let room = MAX.saturating_sub(prefix.len() + MARK.len() + DIGEST_DIGITS);
    let shortened = format!(
        "{prefix}{MARK}{}{}",
        &digest[..DIGEST_DIGITS],
        tail(rest, room)
    );
    debug_assert!(crate::envelope::is_identifier(&shortened), "{shortened}");
    shortened
}

/// The longest suffix of `rest`, at most `room` bytes, that is all inside
/// the grammar and does not start inside an escape — starting at a `:` if
/// any suffix that fits does.
fn tail(rest: &str, room: usize) -> &str {
    let bytes = rest.as_bytes();
    // Which positions sit inside a `~XX`, read left to right the way
    // `decode` reads it.
    let mut inside = vec![false; bytes.len() + 1];
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'~'
            && at + 2 < bytes.len()
            && bytes[at + 1].is_ascii_hexdigit()
            && bytes[at + 2].is_ascii_hexdigit()
        {
            inside[at + 1] = true;
            inside[at + 2] = true;
            at += 3;
        } else {
            at += 1;
        }
    }
    let after_outside = bytes
        .iter()
        .rposition(|&byte| !in_grammar(byte))
        .map_or(0, |at| at + 1);
    let lowest = bytes.len().saturating_sub(room).max(after_outside);
    let start = (lowest..bytes.len())
        .find(|&at| bytes[at] == b':' && !inside[at])
        .or_else(|| (lowest..=bytes.len()).find(|&at| !inside[at]))
        .unwrap_or(bytes.len());
    &rest[start..]
}

/// The readable id of a discovered span, before a prefix bounds it.
pub fn span(repository: &str, path: &str, start_byte: i64) -> String {
    format!("span-{repository}:{}-{start_byte}", encode(path))
}

/// The readable id of an anchor section, before a prefix bounds it.
pub fn anchor(repository: &str, name: &str) -> String {
    format!("anchor-{repository}:{}", encode(name))
}

/// The readable id of a discovered claim, before a prefix bounds it. A
/// claim id is an identifier already, so it is not encoded.
pub fn claim(claim: &str) -> String {
    format!("claim-{claim}")
}

/// A discovered section's id, from its readable id.
pub fn discovered_section(id: &str) -> String {
    bounded("d-", id)
}

/// The citation id of a discovered section, from its readable id. It
/// digests the same `rest` as [`discovered_section`], so when both are
/// shortened they share their digest and read as a pair.
pub fn discovered_citation(id: &str) -> String {
    bounded("dc-", id)
}

/// A claim's section id, and the id it is omitted under. One function for
/// both, so a reader matching an omitted claim with the section it has in
/// another packet can.
pub fn claim_section(claim_id: &str) -> String {
    discovered_section(&claim(claim_id))
}

/// The section id of an item's own section.
pub fn item_section(item: &str) -> String {
    bounded("s-", item)
}

/// The citation id of an item's own section.
pub fn item_citation(item: &str) -> String {
    bounded("c-", item)
}

/// The section id of a projection's `n`th omission of an item's artifact.
///
/// Unchanged whenever it fits, which is the frozen J2 rubric's
/// `s-<item>.o<n>`. Shortened, it keeps `.o<n>`, because the tail is a
/// suffix and is never shorter than the 92 bytes an `s-` prefix leaves.
/// The omission pairs with its section through its `item_id`, not by
/// sharing a prefix of the section's id.
///
/// **A collision this does not fix**, recorded as a follow-up: the
/// omission `s-x.o1` of item `x` is also the section id of an item named
/// `x.o1`. J2's rubric freezes the shape, so changing it is its own change.
pub fn omission(item: &str, number: usize) -> String {
    bounded("s-", &format!("{item}.o{number}"))
}

/// The artifact a packet revision is sealed as: the convention
/// `packet.<request>.<revision>` while it fits, which a grant's
/// `id_prefix` of `packet.` names, and a shortened id under the same
/// prefix past that. A request id can be 128 characters by itself.
pub fn packet_artifact(request: &str, revision: i64) -> String {
    bounded("packet.", &format!("{request}.{revision}"))
}

#[cfg(test)]
mod tests;
